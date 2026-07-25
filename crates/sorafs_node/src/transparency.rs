//! Local SFM-4c transparency aggregate worker helpers.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::moderation::ModerationEvidenceViewerAuditReport;
use crate::reserve::{
    ReserveAppealRecord, ReserveAppealStatus, ReserveLifecycleEvent, ReserveLifecyclePolicyRecord,
    ReserveMovementCustodyStatus, ReserveMovementKind, ReserveMovementRecord,
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use iroha_crypto::sorafs::proof_token::{
    ModerationAction as ProofTokenModerationAction, ProofToken,
};
use iroha_data_model::sorafs::{
    gar::{GarEnforcementActionV1, GarEnforcementReceiptV1},
    reserve::ReserveLifecycleStage,
    transparency::{
        MODERATION_LEDGER_ENTRY_VERSION_V1, MODERATION_PRIVACY_AGGREGATE_VERSION_V1,
        ModerationLedgerEntryKindV1, ModerationLedgerEntryV1, ModerationLedgerMetadataV1,
        ModerationPrivacyAggregateMetricV1, ModerationPrivacyAggregateV1, ModerationPrivacyModeV1,
        ModerationPrivacyParametersV1, PROOF_TOKEN_ISSUANCE_VERSION_V1, ProofTokenIssuanceV1,
    },
};
use norito::codec::Encode as NoritoEncode;
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use sorafs_manifest::{
    MODERATION_LEDGER_MAX_PUBLIC_TEXT_BYTES_V1, MODERATION_PRIVACY_MAX_METRICS_V1,
    MODERATION_PRIVACY_RANDOMNESS_COMMITMENT_METADATA_KEY_V1, ModerationPrivacyNoiseSourceV1,
    ModerationPrivacyThresholdPrfCommitmentV1, SoraFsAppealFinanceReportV1,
    SoraFsAppealFinanceSettlementReceiptV1, SoraFsModerationBallotGovernanceEventV1,
};
use thiserror::Error;

const SOURCE_ENTRY_SUBJECT_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.source_entry.subject.v1";
const SOURCE_ENTRY_SUMMARY_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.source_entry.summary.v1";
const DISCRETE_LAPLACE_NOISE_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_aggregate.discrete_laplace.v1";
const NOISE_RANDOMNESS_COMMITMENT_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_aggregate.randomness_commitment.v1";
const PRIVACY_BUDGET_ENTRY_DOMAIN_V1: &[u8] = b"sorafs.node.transparency.privacy_budget.entry.v1";
const PRIVACY_BUDGET_LEDGER_VERSION_V1: u8 = 1;
const PRIVACY_BUDGET_MAX_POLICIES_V1: usize = 64;
const PRIVACY_BUDGET_MAX_CHARGES_V1: usize = 4_096;
const PRIVACY_RELEASE_LEDGER_VERSION_V1: u8 = 1;
const PRIVACY_RELEASE_MAX_RECORDS_V1: usize = 4_096;
const PRIVACY_RELEASE_RECORD_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_release.record.v1";
const PRIVACY_RELEASE_ANCHOR_GENESIS_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_release.anchor_genesis.v1";
const MAX_DISCRETE_LAPLACE_EXPECTED_CONTINUATIONS_V1: u128 = 4_096;
const MAX_DISCRETE_LAPLACE_RELEASE_EXPECTED_DRAWS_V1: u128 = 262_144;
/// Maximum source events accepted by one V1 privacy aggregation call.
pub const PRIVACY_AGGREGATE_MAX_SOURCE_EVENTS_V1: usize = 4_096;
/// Maximum governed population buckets in one V1 privacy query.
pub const PRIVACY_AGGREGATE_MAX_POPULATIONS_V1: usize = 256;
const CYCLE_ID_DOMAIN_V1: &[u8] = b"sorafs.node.transparency.privacy_aggregate.cycle_id.v1";
const CYCLE_PRF_REQUEST_BINDING_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_aggregate.cycle_prf_request.v1";
const POPULATION_INVENTORY_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_aggregate.population_inventory.v1";
const METRIC_SCHEMA_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_aggregate.metric_schema.v1";
const PRIVATE_SOURCE_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_aggregate.private_source.v1";
const SOURCE_EVENT_RECEIPT_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_aggregate.source_event_receipt.v1";
const TRANSPARENCY_LEDGER_ENTRY_ID_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.ledger.source_entry_id.v1";
const RESERVE_SOURCE_PAYLOAD_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.reserve.source_payload.v1";
const RESERVE_PRIVATE_FIELD_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.reserve.private_field.v1";

/// One privacy-safe source entry admitted into the local transparency ledger worker.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct TransparencyLedgerSourceEntry {
    /// Stable source event id used for duplicate suppression.
    pub event_id: String,
    /// Unix timestamp in seconds when the source event occurred.
    pub occurred_at_unix: u64,
    /// Public transparency ledger kind.
    pub kind: ModerationLedgerEntryKindV1,
    /// Privacy-safe subject label, such as a case id, GAR receipt id, or hold id.
    pub subject: String,
    /// BLAKE3 digest of the canonical private subject identifier.
    pub subject_digest: [u8; 32],
    /// BLAKE3 digest of the canonical source payload represented by this entry.
    pub payload_digest: [u8; 32],
    /// BLAKE3 digest of the public dashboard/explorer summary.
    pub summary_digest: [u8; 32],
    /// Optional digest of the policy/configuration that governed this entry.
    pub policy_digest: Option<[u8; 32]>,
    /// Optional public evidence URIs safe to disclose.
    pub evidence_uris: Vec<String>,
    /// Public key/value metadata. Keys must be unique and sorted.
    pub metadata: Vec<ModerationLedgerMetadataV1>,
}

impl TransparencyLedgerSourceEntry {
    pub(crate) fn validate(&self) -> Result<(), TransparencyLedgerIngestError> {
        require_transparency_public_text("event_id", &self.event_id)?;
        if self.occurred_at_unix == 0 {
            return Err(TransparencyLedgerIngestError::InvalidTimestamp {
                field: "occurred_at_unix",
            });
        }
        validate_transparency_kind(&self.kind)?;
        require_transparency_public_text("subject", &self.subject)?;
        require_transparency_nonzero32("subject_digest", &self.subject_digest)?;
        require_transparency_nonzero32("payload_digest", &self.payload_digest)?;
        require_transparency_nonzero32("summary_digest", &self.summary_digest)?;
        if let Some(policy_digest) = &self.policy_digest {
            require_transparency_nonzero32("policy_digest", policy_digest)?;
        }
        for uri in &self.evidence_uris {
            require_transparency_public_text("evidence_uris", uri)?;
        }
        validate_transparency_metadata(&self.metadata)
    }

    fn to_ledger_entry(
        &self,
        cycle_id: [u8; 16],
        entry_id: [u8; 16],
        sequence: u64,
    ) -> Result<ModerationLedgerEntryV1, TransparencyLedgerIngestError> {
        self.validate()?;
        require_transparency_nonzero16("cycle_id", &cycle_id)?;
        require_transparency_nonzero16("entry_id", &entry_id)?;
        let entry = ModerationLedgerEntryV1 {
            version: MODERATION_LEDGER_ENTRY_VERSION_V1,
            cycle_id,
            entry_id,
            sequence,
            occurred_at_unix: self.occurred_at_unix,
            kind: self.kind.clone(),
            subject: self.subject.clone(),
            subject_digest: self.subject_digest,
            payload_digest: self.payload_digest,
            summary_digest: self.summary_digest,
            policy_digest: self.policy_digest,
            evidence_uris: self.evidence_uris.clone(),
            metadata: self.metadata.clone(),
        };
        entry
            .validate()
            .map_err(|err| TransparencyLedgerIngestError::InvalidLedgerEntry {
                message: err.to_string(),
            })?;
        Ok(entry)
    }
}

/// Errors raised while deriving transparency source entries from typed SoraFS payloads.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum TransparencySourceEntryAdapterError {
    /// GAR receipt source data is malformed.
    #[error("invalid GAR enforcement receipt source: {message}")]
    InvalidGarReceipt {
        /// Validation detail.
        message: String,
    },
    /// Moderation governance event source data is malformed.
    #[error("invalid moderation governance event source: {message}")]
    InvalidModerationEvent {
        /// Validation detail.
        message: String,
    },
    /// Appeal finance report source data is malformed.
    #[error("invalid appeal finance report source: {message}")]
    InvalidAppealFinanceReport {
        /// Validation detail.
        message: String,
    },
    /// Appeal finance settlement receipt source data is malformed.
    #[error("invalid appeal finance settlement receipt source: {message}")]
    InvalidAppealFinanceSettlementReceipt {
        /// Validation detail.
        message: String,
    },
    /// Reserve lifecycle event source data is malformed.
    #[error("invalid reserve lifecycle event source: {message}")]
    InvalidReserveLifecycleEvent {
        /// Validation detail.
        message: String,
    },
    /// Reserve movement source data is malformed.
    #[error("invalid reserve movement source: {message}")]
    InvalidReserveMovement {
        /// Validation detail.
        message: String,
    },
    /// Reserve appeal source data is malformed.
    #[error("invalid reserve appeal source: {message}")]
    InvalidReserveAppeal {
        /// Validation detail.
        message: String,
    },
    /// Reserve lifecycle policy source data is malformed.
    #[error("invalid reserve lifecycle policy source: {message}")]
    InvalidReserveLifecyclePolicy {
        /// Validation detail.
        message: String,
    },
    /// Evidence-viewer audit report source data is malformed.
    #[error("invalid evidence viewer audit report source: {message}")]
    InvalidEvidenceViewerAuditReport {
        /// Validation detail.
        message: String,
    },
    /// Canonical Norito encoding failed while deriving a payload digest.
    #[error("failed to encode {payload_kind} source payload: {message}")]
    CanonicalEncode {
        /// Payload kind label.
        payload_kind: &'static str,
        /// Encode failure detail.
        message: String,
    },
    /// Derived source entry failed local source-entry validation.
    #[error("derived transparency source entry is invalid: {message}")]
    InvalidSourceEntry {
        /// Validation detail.
        message: String,
    },
}

/// Derive a transparency source entry from a GAR enforcement receipt.
///
/// # Errors
///
/// Returns [`TransparencySourceEntryAdapterError`] when the receipt is
/// malformed, canonical encoding fails, or the derived source entry is invalid.
pub fn gar_enforcement_receipt_source_entry(
    receipt: &GarEnforcementReceiptV1,
) -> Result<TransparencyLedgerSourceEntry, TransparencySourceEntryAdapterError> {
    validate_gar_enforcement_receipt(receipt)?;
    let payload_digest = canonical_payload_digest("gar_enforcement_receipt", receipt)?;
    let action = gar_enforcement_action_label(&receipt.action);
    let kind = match receipt.action {
        GarEnforcementActionV1::LegalHold => ModerationLedgerEntryKindV1::LegalHold,
        _ => ModerationLedgerEntryKindV1::GarEnforcementReceipt,
    };
    let mut metadata = BTreeMap::new();
    metadata.insert("action".to_string(), action.to_string());
    if let GarEnforcementActionV1::Custom(slug) = &receipt.action {
        metadata.insert("custom_action".to_string(), slug.clone());
    }
    metadata.insert("gar_name".to_string(), receipt.gar_name.clone());
    metadata.insert("operator".to_string(), receipt.operator.to_string());
    if let Some(expires_at_unix) = receipt.expires_at_unix {
        metadata.insert("expires_at_unix".to_string(), expires_at_unix.to_string());
    }
    if let Some(policy_version) = &receipt.policy_version {
        metadata.insert("policy_version".to_string(), policy_version.clone());
    }
    metadata.insert("reason".to_string(), receipt.reason.clone());
    for (index, label) in receipt.labels.iter().enumerate() {
        metadata.insert(format!("label_{index}"), label.clone());
    }
    let metadata = metadata_vec(metadata);
    let entry = TransparencyLedgerSourceEntry {
        event_id: format!("gar:{}", hex::encode(receipt.receipt_id)),
        occurred_at_unix: receipt.triggered_at_unix,
        kind,
        subject: format!("{}@{}", receipt.gar_name, receipt.canonical_host),
        subject_digest: source_subject_digest(
            "gar_enforcement_receipt",
            &receipt.canonical_host,
            &payload_digest,
        ),
        payload_digest,
        summary_digest: source_summary_digest("gar_enforcement_receipt", &metadata),
        policy_digest: receipt.policy_digest,
        evidence_uris: receipt.evidence_uris.clone(),
        metadata,
    };
    validate_adapter_source_entry(entry)
}

/// Derive a transparency source entry from a moderation ballot governance event.
///
/// # Errors
///
/// Returns [`TransparencySourceEntryAdapterError`] when validation, canonical
/// encoding, or source-entry derivation fails.
pub fn moderation_ballot_governance_event_source_entry(
    event: &SoraFsModerationBallotGovernanceEventV1,
) -> Result<TransparencyLedgerSourceEntry, TransparencySourceEntryAdapterError> {
    event.validate().map_err(|err| {
        TransparencySourceEntryAdapterError::InvalidModerationEvent {
            message: err.to_string(),
        }
    })?;
    let payload_digest = canonical_payload_digest("moderation_ballot_governance_event", event)?;
    let mut metadata = BTreeMap::new();
    metadata.insert("event_kind".to_string(), event.kind.as_str().to_string());
    metadata.insert("round_id".to_string(), event.round_id.clone());
    metadata.insert(
        "committed_count".to_string(),
        event.committed_count.to_string(),
    );
    metadata.insert(
        "revealed_count".to_string(),
        event.revealed_count.to_string(),
    );
    if let Some(juror_id) = &event.juror_id {
        metadata.insert("juror_id".to_string(), juror_id.clone());
    }
    if let Some(tally) = &event.tally {
        metadata.insert("contested".to_string(), tally.contested.to_string());
        metadata.insert("quorum".to_string(), tally.quorum.to_string());
        metadata.insert("votes_total".to_string(), tally.votes_total.to_string());
        if let Some(winner) = tally.winning_choice {
            metadata.insert("winning_choice".to_string(), winner.as_str().to_string());
        }
    }
    let metadata = metadata_vec(metadata);
    let entry = TransparencyLedgerSourceEntry {
        event_id: format!(
            "moderation-ballot:{}:{}:{}:{}",
            event.sequence,
            event.case_id,
            event.round_id,
            event.kind.as_str()
        ),
        occurred_at_unix: unix_ms_to_secs(event.generated_at_unix_ms).map_err(|message| {
            TransparencySourceEntryAdapterError::InvalidModerationEvent { message }
        })?,
        kind: ModerationLedgerEntryKindV1::ModerationAction,
        subject: format!("{}:{}", event.case_id, event.round_id),
        subject_digest: source_subject_digest(
            "moderation_ballot_governance_event",
            &event.case_id,
            &payload_digest,
        ),
        payload_digest,
        summary_digest: source_summary_digest("moderation_ballot_governance_event", &metadata),
        policy_digest: None,
        evidence_uris: Vec::new(),
        metadata,
    };
    validate_adapter_source_entry(entry)
}

/// Derive a transparency source entry from an appeal finance report.
///
/// # Errors
///
/// Returns [`TransparencySourceEntryAdapterError`] when validation, canonical
/// encoding, or source-entry derivation fails.
pub fn appeal_finance_report_source_entry(
    report: &SoraFsAppealFinanceReportV1,
) -> Result<TransparencyLedgerSourceEntry, TransparencySourceEntryAdapterError> {
    report.validate().map_err(|err| {
        TransparencySourceEntryAdapterError::InvalidAppealFinanceReport {
            message: err.to_string(),
        }
    })?;
    let payload_digest = canonical_payload_digest("appeal_finance_report", report)?;
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "appeal_finance_config_version".to_string(),
        report.appeal_finance_config_version.clone(),
    );
    metadata.insert("deposit_xor".to_string(), report.deposit_xor.to_string());
    metadata.insert("held_xor".to_string(), report.held.amount_xor.to_string());
    metadata.insert("outcome".to_string(), report.outcome.as_str().to_string());
    metadata.insert("panel_size".to_string(), report.panel_size.to_string());
    metadata.insert(
        "refund_xor".to_string(),
        report.refund.amount_xor.to_string(),
    );
    metadata.insert(
        "rewards_forfeited_treasury_xor".to_string(),
        report.rewards_forfeited_treasury_xor.to_string(),
    );
    metadata.insert(
        "rewards_paid_total_xor".to_string(),
        report.rewards_paid_total_xor.to_string(),
    );
    metadata.insert(
        "treasury_xor".to_string(),
        report.treasury.amount_xor.to_string(),
    );
    if let Some(round_id) = &report.round_id {
        metadata.insert("round_id".to_string(), round_id.clone());
    }
    let metadata = metadata_vec(metadata);
    let entry = TransparencyLedgerSourceEntry {
        event_id: format!("appeal-finance-report:{}", hex::encode(report.report_id)),
        occurred_at_unix: unix_ms_to_secs(report.generated_at_unix_ms).map_err(|message| {
            TransparencySourceEntryAdapterError::InvalidAppealFinanceReport { message }
        })?,
        kind: ModerationLedgerEntryKindV1::AppealOutcome,
        subject: report.case_id.clone(),
        subject_digest: source_subject_digest(
            "appeal_finance_report",
            &report.case_id,
            &payload_digest,
        ),
        payload_digest,
        summary_digest: source_summary_digest("appeal_finance_report", &metadata),
        policy_digest: report.evidence_bundle_digest,
        evidence_uris: Vec::new(),
        metadata,
    };
    validate_adapter_source_entry(entry)
}

/// Derive a transparency source entry from an appeal finance settlement receipt.
///
/// # Errors
///
/// Returns [`TransparencySourceEntryAdapterError`] when validation, canonical
/// encoding, or source-entry derivation fails.
pub fn appeal_finance_settlement_receipt_source_entry(
    receipt: &SoraFsAppealFinanceSettlementReceiptV1,
) -> Result<TransparencyLedgerSourceEntry, TransparencySourceEntryAdapterError> {
    receipt.validate().map_err(|err| {
        TransparencySourceEntryAdapterError::InvalidAppealFinanceSettlementReceipt {
            message: err.to_string(),
        }
    })?;
    let payload_digest = canonical_payload_digest("appeal_finance_settlement_receipt", receipt)?;
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "appeal_finance_config_version".to_string(),
        receipt.appeal_finance_config_version.clone(),
    );
    metadata.insert("amount_xor".to_string(), receipt.amount_xor.to_string());
    metadata.insert(
        "configured_signer_count".to_string(),
        receipt.configured_signer_count.to_string(),
    );
    metadata.insert("held_xor".to_string(), receipt.held_xor.to_string());
    metadata.insert("outcome".to_string(), receipt.outcome.as_str().to_string());
    metadata.insert("refund_xor".to_string(), receipt.refund_xor.to_string());
    metadata.insert(
        "required_authority".to_string(),
        receipt.required_authority.clone(),
    );
    metadata.insert(
        "settlement_step".to_string(),
        receipt.submitted_step.clone(),
    );
    metadata.insert("treasury_xor".to_string(), receipt.treasury_xor.to_string());
    if let Some(round_id) = &receipt.round_id {
        metadata.insert("round_id".to_string(), round_id.clone());
    }
    let metadata = metadata_vec(metadata);
    let policy_digest = hex_32_to_digest(
        &receipt.reconciliation_digest_hex,
        "appeal finance settlement reconciliation digest",
    )?;
    let entry = TransparencyLedgerSourceEntry {
        event_id: format!(
            "appeal-finance-settlement:{}",
            hex::encode(receipt.receipt_id)
        ),
        occurred_at_unix: unix_ms_to_secs(receipt.generated_at_unix_ms).map_err(|message| {
            TransparencySourceEntryAdapterError::InvalidAppealFinanceSettlementReceipt { message }
        })?,
        kind: ModerationLedgerEntryKindV1::AppealOutcome,
        subject: format!("{}:{}", receipt.case_id, receipt.submitted_step),
        subject_digest: source_subject_digest(
            "appeal_finance_settlement_receipt",
            &receipt.case_id,
            &payload_digest,
        ),
        payload_digest,
        summary_digest: source_summary_digest("appeal_finance_settlement_receipt", &metadata),
        policy_digest: Some(policy_digest),
        evidence_uris: Vec::new(),
        metadata,
    };
    validate_adapter_source_entry(entry)
}

/// Derive a transparency source entry from a payload-free evidence-viewer audit report.
///
/// # Errors
///
/// Returns [`TransparencySourceEntryAdapterError`] when report validation,
/// canonical encoding, or source-entry derivation fails.
pub fn moderation_evidence_viewer_audit_report_source_entry(
    report: &ModerationEvidenceViewerAuditReport,
) -> Result<TransparencyLedgerSourceEntry, TransparencySourceEntryAdapterError> {
    report.validate().map_err(|err| {
        TransparencySourceEntryAdapterError::InvalidEvidenceViewerAuditReport { message: err }
    })?;
    let payload_digest = canonical_payload_digest("evidence_viewer_audit_report", report)?;
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "access_event_count".to_string(),
        report.access_event_count.to_string(),
    );
    for kind_count in &report.access_kind_counts {
        metadata.insert(
            format!("access_kind_{}_count", kind_count.kind),
            kind_count.count.to_string(),
        );
    }
    metadata.insert(
        "access_event_digest_set_digest_hex".to_string(),
        hex::encode(report.access_event_digest_set_digest),
    );
    metadata.insert(
        "attestation_digest_set_digest_hex".to_string(),
        hex::encode(report.attestation_digest_set_digest),
    );
    metadata.insert(
        "attested_session_count".to_string(),
        report.attested_session_count.to_string(),
    );
    metadata.insert(
        "evidence_digest_set_digest_hex".to_string(),
        hex::encode(report.evidence_digest_set_digest),
    );
    if let Some(first_event_at_unix_ms) = report.first_event_at_unix_ms {
        metadata.insert(
            "first_event_at_unix_ms".to_string(),
            first_event_at_unix_ms.to_string(),
        );
    }
    metadata.insert(
        "generated_at_unix".to_string(),
        report.generated_at_unix.to_string(),
    );
    if let Some(last_event_at_unix_ms) = report.last_event_at_unix_ms {
        metadata.insert(
            "last_event_at_unix_ms".to_string(),
            last_event_at_unix_ms.to_string(),
        );
    }
    metadata.insert(
        "legal_hold_bound_session_count".to_string(),
        report.legal_hold_bound_session_count.to_string(),
    );
    metadata.insert(
        "logged_session_count".to_string(),
        report.logged_session_count.to_string(),
    );
    metadata.insert("payloads_included".to_string(), "false".to_string());
    metadata.insert(
        "report_digest_hex".to_string(),
        hex::encode(report.report_digest),
    );
    metadata.insert("report_scope".to_string(), report.report_scope.clone());
    metadata.insert(
        "request_digest_set_digest_hex".to_string(),
        hex::encode(report.request_digest_set_digest),
    );
    metadata.insert("response_bodies_included".to_string(), "false".to_string());
    metadata.insert(
        "session_count".to_string(),
        report.session_count.to_string(),
    );
    metadata.insert(
        "session_manifest_digest_set_digest_hex".to_string(),
        hex::encode(report.session_manifest_digest_set_digest),
    );
    metadata.insert("session_tokens_included".to_string(), "false".to_string());
    metadata.insert("signed_urls_included".to_string(), "false".to_string());
    metadata.insert(
        "unique_viewer_role_count".to_string(),
        report.unique_viewer_role_count.to_string(),
    );
    metadata.insert("viewer_accounts_included".to_string(), "false".to_string());
    metadata.insert(
        "watermark_metadata_digest_set_digest_hex".to_string(),
        hex::encode(report.watermark_metadata_digest_set_digest),
    );
    metadata.insert(
        "watermarked_session_count".to_string(),
        report.watermarked_session_count.to_string(),
    );
    metadata.insert(
        "window_end_unix".to_string(),
        report.window_end_unix.to_string(),
    );
    metadata.insert(
        "window_start_unix".to_string(),
        report.window_start_unix.to_string(),
    );
    let metadata = metadata_vec(metadata);
    let subject = format!(
        "evidence-viewer-audit:{}:{}-{}",
        report.report_scope, report.window_start_unix, report.window_end_unix
    );
    let occurred_at_unix = report.window_end_unix.saturating_sub(1);
    let entry = TransparencyLedgerSourceEntry {
        event_id: format!("evidence-viewer-audit:{}", hex::encode(report.report_id)),
        occurred_at_unix,
        kind: ModerationLedgerEntryKindV1::EvidenceAccess,
        subject: subject.clone(),
        subject_digest: source_subject_digest(
            "evidence_viewer_audit_report",
            &subject,
            &payload_digest,
        ),
        payload_digest,
        summary_digest: source_summary_digest("evidence_viewer_audit_report", &metadata),
        policy_digest: report.policy_digest,
        evidence_uris: Vec::new(),
        metadata,
    };
    validate_adapter_source_entry(entry)
}

/// Derive a transparency source entry from a local reserve lifecycle event.
///
/// # Errors
///
/// Returns [`TransparencySourceEntryAdapterError`] when the local event is not
/// suitable for the public governance log or the derived source entry is
/// invalid.
pub fn reserve_lifecycle_event_source_entry(
    event: &ReserveLifecycleEvent,
) -> Result<TransparencyLedgerSourceEntry, TransparencySourceEntryAdapterError> {
    validate_reserve_source_id(
        "provider_id",
        &event.provider_id,
        reserve_lifecycle_source_error,
    )?;
    if event.observed_at_unix == 0 {
        return Err(reserve_lifecycle_source_error(
            "observed_at_unix must be non-zero",
        ));
    }
    let current_stage = reserve_lifecycle_stage_label(event.current_stage);
    let previous_stage = event.previous_stage.map(reserve_lifecycle_stage_label);
    let provider_id_hex = hex::encode(event.provider_id);
    let mut metadata = BTreeMap::new();
    metadata.insert("event_family".to_string(), "reserve_lifecycle".to_string());
    metadata.insert("provider_id_hex".to_string(), provider_id_hex.clone());
    metadata.insert("sequence".to_string(), event.sequence.to_string());
    metadata.insert("current_stage".to_string(), current_stage.to_string());
    if let Some(previous_stage) = previous_stage {
        metadata.insert("previous_stage".to_string(), previous_stage.to_string());
    }
    metadata.insert("rent_due".to_string(), event.ledger.rent_due.to_string());
    metadata.insert(
        "reserve_shortfall".to_string(),
        event.ledger.reserve_shortfall.to_string(),
    );
    metadata.insert(
        "top_up_shortfall".to_string(),
        event.ledger.top_up_shortfall.to_string(),
    );
    metadata.insert(
        "grace_period_days".to_string(),
        event.grace_period_days.to_string(),
    );
    metadata.insert(
        "default_after_days".to_string(),
        event.default_after_days.to_string(),
    );
    if let Some(policy_id) = event.applied_policy_id {
        metadata.insert("applied_policy_id_hex".to_string(), hex::encode(policy_id));
    }
    if let Some(appeal_id) = event.applied_appeal_id {
        metadata.insert("applied_appeal_id_hex".to_string(), hex::encode(appeal_id));
    }
    metadata.insert(
        "credit_draw".to_string(),
        event.lifecycle.credit_draw.to_string(),
    );
    if let Some(available) = &event.lifecycle.credit_available_after_draw {
        metadata.insert(
            "credit_available_after_draw".to_string(),
            available.to_string(),
        );
    }
    metadata.insert(
        "credit_shortfall".to_string(),
        event.lifecycle.credit_shortfall.to_string(),
    );
    metadata.insert(
        "accrued_interest".to_string(),
        event.lifecycle.accrued_interest.to_string(),
    );
    metadata.insert(
        "total_due_after_credit".to_string(),
        event.lifecycle.total_due_after_credit.to_string(),
    );
    metadata.insert(
        "requires_governance_notification".to_string(),
        event.lifecycle.requires_governance_notification.to_string(),
    );
    metadata.insert(
        "requires_manual_credit_approval".to_string(),
        event.lifecycle.requires_manual_credit_approval.to_string(),
    );
    let metadata = metadata_vec(metadata);
    let encoded_event = event.encode();
    let payload_digest = reserve_source_payload_digest(
        "reserve_lifecycle_event",
        &[("canonical_event", encoded_event.as_slice())],
    );
    let entry = TransparencyLedgerSourceEntry {
        event_id: format!("reserve-lifecycle:{}:{provider_id_hex}", event.sequence),
        occurred_at_unix: event.observed_at_unix,
        kind: ModerationLedgerEntryKindV1::Custom("sorafs_reserve_lifecycle".to_string()),
        subject: format!("reserve-provider:{provider_id_hex}"),
        subject_digest: source_subject_digest(
            "reserve_lifecycle_event",
            &provider_id_hex,
            &payload_digest,
        ),
        payload_digest,
        summary_digest: source_summary_digest("reserve_lifecycle_event", &metadata),
        policy_digest: None,
        evidence_uris: Vec::new(),
        metadata,
    };
    validate_adapter_source_entry(entry)
}

/// Derive a transparency source entry from a local reserve movement record.
///
/// # Errors
///
/// Returns [`TransparencySourceEntryAdapterError`] when the local movement is
/// not suitable for the public governance log or the derived source entry is
/// invalid.
pub fn reserve_movement_source_entry(
    record: &ReserveMovementRecord,
) -> Result<TransparencyLedgerSourceEntry, TransparencySourceEntryAdapterError> {
    validate_reserve_source_id(
        "movement_id",
        &record.movement_id,
        reserve_movement_source_error,
    )?;
    validate_reserve_source_id(
        "provider_id",
        &record.provider_id,
        reserve_movement_source_error,
    )?;
    if record.observed_at_unix == 0 {
        return Err(reserve_movement_source_error(
            "observed_at_unix must be non-zero",
        ));
    }
    let movement_id_hex = hex::encode(record.movement_id);
    let provider_id_hex = hex::encode(record.provider_id);
    let kind = reserve_movement_kind_label(record.kind);
    let custody_status = reserve_movement_custody_status_label(record.custody_status);
    let custody_observed_at = record
        .custody_updated_at_unix
        .unwrap_or(record.observed_at_unix);
    let mut metadata = BTreeMap::new();
    metadata.insert("event_family".to_string(), "reserve_movement".to_string());
    metadata.insert("movement_id_hex".to_string(), movement_id_hex.clone());
    metadata.insert("provider_id_hex".to_string(), provider_id_hex.clone());
    metadata.insert("sequence".to_string(), record.sequence.to_string());
    metadata.insert("movement_kind".to_string(), kind.to_string());
    metadata.insert("amount".to_string(), record.amount.to_string());
    metadata.insert(
        "balance_after".to_string(),
        record.balance_after.to_string(),
    );
    metadata.insert(
        "confirmed_balance_after".to_string(),
        record.confirmed_balance_after.to_string(),
    );
    metadata.insert("custody_status".to_string(), custody_status.to_string());
    metadata.insert(
        "provider_account_digest_hex".to_string(),
        reserve_private_field_digest_hex("provider_account", &record.provider_account),
    );
    metadata.insert(
        "reserve_account_digest_hex".to_string(),
        reserve_private_field_digest_hex("reserve_account", &record.reserve_account),
    );
    metadata.insert(
        "asset_definition_digest_hex".to_string(),
        reserve_private_field_digest_hex("asset_definition_id", &record.asset_definition_id),
    );
    metadata.insert(
        "idempotency_key_digest_hex".to_string(),
        reserve_private_field_digest_hex("idempotency_key", record.idempotency_key.as_bytes()),
    );
    if let Some(tx_hash_hex) = &record.custody_tx_hash_hex {
        metadata.insert("custody_tx_hash_hex".to_string(), tx_hash_hex.clone());
    }
    let metadata = metadata_vec(metadata);
    let encoded_record = record.encode();
    let payload_digest = reserve_source_payload_digest(
        "reserve_movement",
        &[("canonical_record", encoded_record.as_slice())],
    );
    let entry = TransparencyLedgerSourceEntry {
        event_id: format!(
            "reserve-movement:{movement_id_hex}:{custody_status}:{custody_observed_at}"
        ),
        occurred_at_unix: custody_observed_at,
        kind: ModerationLedgerEntryKindV1::Custom("sorafs_reserve_movement".to_string()),
        subject: format!("reserve-provider:{provider_id_hex}"),
        subject_digest: source_subject_digest(
            "reserve_movement",
            &provider_id_hex,
            &payload_digest,
        ),
        payload_digest,
        summary_digest: source_summary_digest("reserve_movement", &metadata),
        policy_digest: None,
        evidence_uris: Vec::new(),
        metadata,
    };
    validate_adapter_source_entry(entry)
}

/// Derive a transparency source entry from a local reserve appeal record.
///
/// # Errors
///
/// Returns [`TransparencySourceEntryAdapterError`] when the local appeal is not
/// suitable for the public governance log or the derived source entry is
/// invalid.
pub fn reserve_appeal_source_entry(
    record: &ReserveAppealRecord,
) -> Result<TransparencyLedgerSourceEntry, TransparencySourceEntryAdapterError> {
    validate_reserve_source_id("appeal_id", &record.appeal_id, reserve_appeal_source_error)?;
    validate_reserve_source_id(
        "provider_id",
        &record.provider_id,
        reserve_appeal_source_error,
    )?;
    if record.opened_at_unix == 0 {
        return Err(reserve_appeal_source_error(
            "opened_at_unix must be non-zero",
        ));
    }
    if record.reason.trim().is_empty() {
        return Err(reserve_appeal_source_error("reason must not be empty"));
    }
    let appeal_id_hex = hex::encode(record.appeal_id);
    let provider_id_hex = hex::encode(record.provider_id);
    let status = reserve_appeal_status_label(record.status);
    let occurred_at_unix = record.decided_at_unix.unwrap_or(record.opened_at_unix);
    let mut metadata = BTreeMap::new();
    metadata.insert("event_family".to_string(), "reserve_appeal".to_string());
    metadata.insert("appeal_id_hex".to_string(), appeal_id_hex.clone());
    metadata.insert("provider_id_hex".to_string(), provider_id_hex.clone());
    metadata.insert("sequence".to_string(), record.sequence.to_string());
    metadata.insert("status".to_string(), status.to_string());
    if let Some(stage) = record.requested_stage {
        metadata.insert(
            "requested_stage".to_string(),
            reserve_lifecycle_stage_label(stage).to_string(),
        );
    }
    metadata.insert(
        "provider_account_digest_hex".to_string(),
        reserve_private_field_digest_hex("provider_account", &record.provider_account),
    );
    metadata.insert(
        "reason_digest_hex".to_string(),
        reserve_private_field_digest_hex("reason", record.reason.as_bytes()),
    );
    metadata.insert(
        "idempotency_key_digest_hex".to_string(),
        reserve_private_field_digest_hex("idempotency_key", record.idempotency_key.as_bytes()),
    );
    if let Some(evidence_digest_hex) = &record.evidence_digest_hex {
        metadata.insert(
            "evidence_digest_hex".to_string(),
            evidence_digest_hex.clone(),
        );
    }
    if let Some(decision_account) = &record.decision_account {
        metadata.insert(
            "decision_account_digest_hex".to_string(),
            reserve_private_field_digest_hex("decision_account", decision_account),
        );
    }
    if let Some(decision_rationale) = &record.decision_rationale {
        metadata.insert(
            "decision_rationale_digest_hex".to_string(),
            reserve_private_field_digest_hex("decision_rationale", decision_rationale.as_bytes()),
        );
    }
    let metadata = metadata_vec(metadata);
    let payload_digest = reserve_source_payload_digest(
        "reserve_appeal",
        &[
            ("sequence", record.sequence.to_le_bytes().as_ref()),
            ("appeal_id", record.appeal_id.as_ref()),
            ("provider_id", record.provider_id.as_ref()),
            ("provider_account", record.provider_account.as_ref()),
            (
                "requested_stage",
                record
                    .requested_stage
                    .map(reserve_lifecycle_stage_label)
                    .unwrap_or("")
                    .as_bytes(),
            ),
            ("reason", record.reason.as_bytes()),
            (
                "evidence_digest_hex",
                record
                    .evidence_digest_hex
                    .as_deref()
                    .unwrap_or("")
                    .as_bytes(),
            ),
            ("idempotency_key", record.idempotency_key.as_bytes()),
            ("status", status.as_bytes()),
            (
                "decision_account",
                record.decision_account.as_deref().unwrap_or(&[]),
            ),
            (
                "decision_rationale",
                record
                    .decision_rationale
                    .as_deref()
                    .unwrap_or("")
                    .as_bytes(),
            ),
        ],
    );
    let entry = TransparencyLedgerSourceEntry {
        event_id: format!("reserve-appeal:{appeal_id_hex}:{status}:{occurred_at_unix}"),
        occurred_at_unix,
        kind: if record.status == ReserveAppealStatus::Open {
            ModerationLedgerEntryKindV1::Custom("sorafs_reserve_appeal".to_string())
        } else {
            ModerationLedgerEntryKindV1::AppealOutcome
        },
        subject: format!("reserve-provider:{provider_id_hex}"),
        subject_digest: source_subject_digest("reserve_appeal", &provider_id_hex, &payload_digest),
        payload_digest,
        summary_digest: source_summary_digest("reserve_appeal", &metadata),
        policy_digest: None,
        evidence_uris: Vec::new(),
        metadata,
    };
    validate_adapter_source_entry(entry)
}

/// Derive a transparency source entry from a local reserve lifecycle-policy record.
///
/// # Errors
///
/// Returns [`TransparencySourceEntryAdapterError`] when the local policy record
/// is not suitable for the public governance log or the derived source entry is
/// invalid.
pub fn reserve_lifecycle_policy_source_entry(
    record: &ReserveLifecyclePolicyRecord,
) -> Result<TransparencyLedgerSourceEntry, TransparencySourceEntryAdapterError> {
    validate_reserve_source_id(
        "policy_id",
        &record.policy_id,
        reserve_lifecycle_policy_source_error,
    )?;
    if record.observed_at_unix == 0 {
        return Err(reserve_lifecycle_policy_source_error(
            "observed_at_unix must be non-zero",
        ));
    }
    if record.grace_period_days >= record.default_after_days {
        return Err(reserve_lifecycle_policy_source_error(
            "grace_period_days must be lower than default_after_days",
        ));
    }
    let policy_id_hex = hex::encode(record.policy_id);
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "event_family".to_string(),
        "reserve_lifecycle_policy".to_string(),
    );
    metadata.insert("policy_id_hex".to_string(), policy_id_hex.clone());
    metadata.insert("sequence".to_string(), record.sequence.to_string());
    metadata.insert(
        "authority_account_digest_hex".to_string(),
        reserve_private_field_digest_hex("authority_account", &record.authority_account),
    );
    metadata.insert(
        "grace_period_days".to_string(),
        record.grace_period_days.to_string(),
    );
    metadata.insert(
        "default_after_days".to_string(),
        record.default_after_days.to_string(),
    );
    metadata.insert(
        "effective_at_unix".to_string(),
        record.effective_at_unix.to_string(),
    );
    metadata.insert(
        "reason_digest_hex".to_string(),
        reserve_private_field_digest_hex("reason", record.reason.as_bytes()),
    );
    metadata.insert(
        "idempotency_key_digest_hex".to_string(),
        reserve_private_field_digest_hex("idempotency_key", record.idempotency_key.as_bytes()),
    );
    let metadata = metadata_vec(metadata);
    let payload_digest = reserve_source_payload_digest(
        "reserve_lifecycle_policy",
        &[
            ("sequence", record.sequence.to_le_bytes().as_ref()),
            ("policy_id", record.policy_id.as_ref()),
            ("authority_account", record.authority_account.as_ref()),
            (
                "grace_period_days",
                record.grace_period_days.to_le_bytes().as_ref(),
            ),
            (
                "default_after_days",
                record.default_after_days.to_le_bytes().as_ref(),
            ),
            (
                "effective_at_unix",
                record.effective_at_unix.to_le_bytes().as_ref(),
            ),
            ("reason", record.reason.as_bytes()),
            ("idempotency_key", record.idempotency_key.as_bytes()),
        ],
    );
    let entry = TransparencyLedgerSourceEntry {
        event_id: format!(
            "reserve-lifecycle-policy:{policy_id_hex}:{}",
            record.sequence
        ),
        occurred_at_unix: record.observed_at_unix,
        kind: ModerationLedgerEntryKindV1::Custom("sorafs_reserve_lifecycle_policy".to_string()),
        subject: format!("reserve-policy:{policy_id_hex}"),
        subject_digest: source_subject_digest(
            "reserve_lifecycle_policy",
            &policy_id_hex,
            &payload_digest,
        ),
        payload_digest,
        summary_digest: source_summary_digest("reserve_lifecycle_policy", &metadata),
        policy_digest: Some(record.policy_id),
        evidence_uris: Vec::new(),
        metadata,
    };
    validate_adapter_source_entry(entry)
}

/// One source metric observed for a privacy aggregate event.
#[derive(Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PrivacyAggregateSourceMetric {
    /// Stable metric key.
    pub key: String,
    /// Raw event contribution before privacy processing.
    pub value: u64,
    /// Unit label for the metric.
    pub unit: String,
}

impl std::fmt::Debug for PrivacyAggregateSourceMetric {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PrivacyAggregateSourceMetric(<redacted>)")
    }
}

/// One source event admitted into the local SFM-4c aggregate worker.
#[derive(Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PrivacyAggregateSourceEvent {
    /// Stable event id used for duplicate suppression.
    pub event_id: String,
    /// Unix timestamp in seconds when the source event occurred.
    pub occurred_at_unix: u64,
    /// Public population label for the aggregate bucket.
    pub population_label: String,
    /// Governed population selector digest.
    pub population_digest: [u8; 32],
    /// Digest of the canonical private subject identifier used for clipping.
    pub subject_digest: [u8; 32],
    /// Raw source metrics for this event, sorted by key.
    pub metrics: Vec<PrivacyAggregateSourceMetric>,
    /// Governed policy digest associated with this event.
    pub policy_digest: [u8; 32],
}

impl std::fmt::Debug for PrivacyAggregateSourceEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PrivacyAggregateSourceEvent(<redacted>)")
    }
}

impl PrivacyAggregateSourceEvent {
    pub(crate) fn validate(&self) -> Result<(), PrivacyAggregateWorkerError> {
        require_public_text("event_id", &self.event_id)?;
        if self.occurred_at_unix == 0 {
            return Err(PrivacyAggregateWorkerError::InvalidTimestamp {
                field: "occurred_at_unix",
            });
        }
        require_public_text("population_label", &self.population_label)?;
        require_nonzero32("population_digest", &self.population_digest)?;
        require_nonzero32("subject_digest", &self.subject_digest)?;
        require_nonzero32("policy_digest", &self.policy_digest)?;
        validate_source_metrics(&self.metrics)
    }

    pub(crate) fn canonical_digest(&self) -> Result<[u8; 32], PrivacyAggregateWorkerError> {
        self.validate()?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(SOURCE_EVENT_RECEIPT_DIGEST_DOMAIN_V1);
        hash_text(&mut hasher, &self.event_id);
        hasher.update(&self.occurred_at_unix.to_le_bytes());
        hash_text(&mut hasher, &self.population_label);
        hasher.update(&self.population_digest);
        hasher.update(&self.subject_digest);
        hasher.update(&self.policy_digest);
        hasher.update(&(self.metrics.len() as u64).to_le_bytes());
        for metric in &self.metrics {
            hash_text(&mut hasher, &metric.key);
            hash_text(&mut hasher, &metric.unit);
            hasher.update(&metric.value.to_le_bytes());
        }
        Ok(*hasher.finalize().as_bytes())
    }
}

/// Durable canonical identity retained for every admitted source event.
#[derive(Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct PrivacySourceEventReceiptV1 {
    pub(crate) event_id: String,
    pub(crate) canonical_digest: [u8; 32],
}

impl std::fmt::Debug for PrivacySourceEventReceiptV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PrivacySourceEventReceiptV1(<redacted>)")
    }
}

/// Idempotent outcome of recording one privacy source event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivacySourceEventRecordOutcomeV1 {
    /// The canonical event was durably admitted and queued.
    Recorded,
    /// The exact canonical event was already admitted or processed.
    AlreadyRecorded,
}

/// One governed public population bucket in a fixed privacy query.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PrivacyAggregatePopulationV1 {
    /// Stable public population label.
    pub label: String,
    /// Nonzero digest of the exact governed population selector.
    pub digest: [u8; 32],
}

/// One governed public metric coordinate in a fixed privacy query.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PrivacyAggregateMetricSchemaV1 {
    /// Stable public metric key.
    pub key: String,
    /// Stable public unit label.
    pub unit: String,
}

/// Configuration used to build one aggregate publication cycle from source events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyAggregateCycleConfig {
    /// Stable governed query identity, unchanged across policy rotations.
    pub query_id: [u8; 32],
    /// Governed inclusive start of the first releasable cycle.
    pub first_cycle_start_unix: u64,
    /// Governed immutable cycle width for this query lineage.
    pub cycle_seconds: u64,
    /// Prefix used when deriving public aggregate identifiers.
    pub aggregate_id_prefix: String,
    /// Fixed public population universe, sorted by `(label, digest)`.
    pub populations: Vec<PrivacyAggregatePopulationV1>,
    /// Fixed public metric universe, sorted by key.
    pub metrics: Vec<PrivacyAggregateMetricSchemaV1>,
    /// Explicit privacy parameters applied to every generated aggregate.
    pub privacy: ModerationPrivacyParametersV1,
    /// Aggregate policy/configuration digest.
    pub policy_digest: [u8; 32],
    /// Public metadata copied into every generated aggregate.
    pub metadata: Vec<ModerationLedgerMetadataV1>,
}

/// Governed pure-DP composition budget for one policy lineage.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, NoritoSerialize, NoritoDeserialize,
)]
pub struct PrivacyCompositionBudgetPolicyV1 {
    /// Nonzero digest of the governed budget policy.
    pub budget_id: [u8; 32],
    /// Reduced numerator of the maximum composed epsilon.
    pub epsilon_limit_numerator: u64,
    /// Reduced denominator of the maximum composed epsilon.
    pub epsilon_limit_denominator: u64,
    /// Maximum publication charges retained under this policy.
    pub max_publications: u64,
}

impl PrivacyCompositionBudgetPolicyV1 {
    pub(crate) fn validate(&self) -> Result<(), PrivacyCompositionBudgetError> {
        if self.budget_id.iter().all(|byte| *byte == 0) {
            return Err(PrivacyCompositionBudgetError::MissingBudgetId);
        }
        require_reduced_positive_rational(
            self.epsilon_limit_numerator,
            self.epsilon_limit_denominator,
        )
        .map_err(|_| PrivacyCompositionBudgetError::InvalidBudgetLimit)?;
        if self.max_publications == 0
            || self.max_publications > PRIVACY_BUDGET_MAX_CHARGES_V1 as u64
        {
            return Err(PrivacyCompositionBudgetError::InvalidPublicationLimit);
        }
        Ok(())
    }
}

/// One hash-chained durable composition-budget charge.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PrivacyCompositionBudgetChargeV1 {
    /// Monotonic sequence within the governed policy lineage.
    pub sequence: u64,
    /// Published cycle charged by this record.
    pub cycle_id: [u8; 16],
    /// Timestamp at which the publication was prepared.
    pub charged_at_unix: u64,
    /// Reduced epsilon numerator charged for this cycle.
    pub epsilon_numerator: u64,
    /// Reduced epsilon denominator charged for this cycle.
    pub epsilon_denominator: u64,
    /// Reduced cumulative epsilon numerator after this charge.
    pub cumulative_epsilon_numerator: u64,
    /// Reduced cumulative epsilon denominator after this charge.
    pub cumulative_epsilon_denominator: u64,
    /// Digest of the preceding charge in this policy lineage.
    pub previous_charge_digest: Option<[u8; 32]>,
    /// Domain-separated digest of this exact charge.
    pub charge_digest: [u8; 32],
}

/// One governed policy lineage in the durable composition-budget ledger.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PrivacyCompositionBudgetChainV1 {
    /// Exact governed policy applied to every charge in this lineage.
    pub policy: PrivacyCompositionBudgetPolicyV1,
    /// Ordered, hash-chained publication charges.
    pub charges: Vec<PrivacyCompositionBudgetChargeV1>,
}

/// Durable multi-policy composition-budget ledger.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PrivacyCompositionBudgetLedgerV1 {
    /// Schema version.
    pub version: u8,
    /// Policy lineages sorted by budget id.
    pub chains: Vec<PrivacyCompositionBudgetChainV1>,
}

/// Finalized external head of one stable privacy-query release chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivacyReleaseAnchorHeadV1 {
    query_id: [u8; 32],
    sequence: u64,
    release_id: [u8; 16],
    record_digest: [u8; 32],
    latest_publication_block_hash: Option<[u8; 32]>,
}

impl PrivacyReleaseAnchorHeadV1 {
    /// Construct the mandatory nonzero genesis head for a governed query.
    ///
    /// # Panics
    ///
    /// Panics when `query_id` is all zeroes. Configuration validation rejects
    /// that state before runtime dependency construction.
    #[must_use]
    pub fn genesis(query_id: [u8; 32]) -> Self {
        assert_ne!(query_id, [0; 32], "privacy query id must be nonzero");
        let mut hasher = blake3::Hasher::new();
        hasher.update(PRIVACY_RELEASE_ANCHOR_GENESIS_DOMAIN_V1);
        hasher.update(&query_id);
        Self {
            query_id,
            sequence: 0,
            release_id: [0; 16],
            record_digest: *hasher.finalize().as_bytes(),
            latest_publication_block_hash: None,
        }
    }

    /// Reconstruct a checked finalized head returned by an external anchor.
    ///
    /// # Errors
    ///
    /// Returns [`PrivacyReleaseAnchorErrorV1::InvalidState`] when the fields
    /// do not encode either the query-specific genesis head or a nonzero
    /// finalized release.
    pub fn try_from_parts(
        query_id: [u8; 32],
        sequence: u64,
        release_id: [u8; 16],
        record_digest: [u8; 32],
        latest_publication_block_hash: Option<[u8; 32]>,
    ) -> Result<Self, PrivacyReleaseAnchorErrorV1> {
        let candidate = Self {
            query_id,
            sequence,
            release_id,
            record_digest,
            latest_publication_block_hash,
        };
        if candidate.validate() {
            Ok(candidate)
        } else {
            Err(PrivacyReleaseAnchorErrorV1::InvalidState)
        }
    }

    pub(crate) fn from_record(record: &PrivacyReleaseRecordV1) -> Self {
        Self {
            query_id: record.query_id,
            sequence: record.sequence,
            release_id: record.release_id,
            record_digest: record.record_digest,
            latest_publication_block_hash: record
                .publication_block_hash
                .or(record.previous_publication_block_hash),
        }
    }

    /// Return the governed stable query identity.
    #[must_use]
    pub const fn query_id(&self) -> [u8; 32] {
        self.query_id
    }

    /// Return the monotonic release sequence, or zero for genesis.
    #[must_use]
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    /// Return the stable release id, or zero for genesis.
    #[must_use]
    pub const fn release_id(&self) -> [u8; 16] {
        self.release_id
    }

    /// Return the exact release-record digest, or the query-specific genesis digest.
    #[must_use]
    pub const fn record_digest(&self) -> [u8; 32] {
        self.record_digest
    }

    /// Return the latest finalized public transparency block hash, if any.
    #[must_use]
    pub const fn latest_publication_block_hash(&self) -> Option<[u8; 32]> {
        self.latest_publication_block_hash
    }

    pub(crate) fn validate(&self) -> bool {
        self.query_id != [0; 32]
            && self
                .latest_publication_block_hash
                .is_none_or(|digest| digest != [0; 32])
            && if self.sequence == 0 {
                *self == Self::genesis(self.query_id)
            } else {
                self.release_id != [0; 16] && self.record_digest != [0; 32]
            }
    }
}

/// Durable terminal state of one stable privacy query window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub(crate) enum PrivacyReleaseStatusV1 {
    /// A canonical aggregate publication was atomically queued.
    Published,
    /// Suppression-only policy omitted every governed population.
    Suppressed,
}

/// Internal append-only record binding one privacy release to its private input.
#[derive(Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct PrivacyReleaseRecordV1 {
    pub(crate) sequence: u64,
    pub(crate) release_id: [u8; 16],
    pub(crate) query_id: [u8; 32],
    pub(crate) first_cycle_start_unix: u64,
    pub(crate) cycle_seconds: u64,
    pub(crate) publish_delay_seconds: u64,
    pub(crate) cycle_start_unix: u64,
    pub(crate) cycle_end_unix: u64,
    pub(crate) due_at_unix: u64,
    pub(crate) private_source_digest: [u8; 32],
    pub(crate) policy_digest: [u8; 32],
    pub(crate) population_inventory_digest: [u8; 32],
    pub(crate) metric_schema_digest: [u8; 32],
    pub(crate) privacy: ModerationPrivacyParametersV1,
    pub(crate) prf_request_binding: Option<[u8; 32]>,
    pub(crate) prf_commitment: Option<[u8; 32]>,
    pub(crate) budget_charge_digest: Option<[u8; 32]>,
    pub(crate) publication_payload_digest: Option<[u8; 32]>,
    pub(crate) published_aggregate_inventory_digest: Option<[u8; 32]>,
    pub(crate) previous_publication_block_hash: Option<[u8; 32]>,
    pub(crate) publication_block_hash: Option<[u8; 32]>,
    pub(crate) status: PrivacyReleaseStatusV1,
    pub(crate) previous_record_digest: Option<[u8; 32]>,
    pub(crate) record_digest: [u8; 32],
}

impl std::fmt::Debug for PrivacyReleaseRecordV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PrivacyReleaseRecordV1")
            .field("sequence", &self.sequence)
            .field("release_id", &self.release_id)
            .field("query_id", &self.query_id)
            .field("first_cycle_start_unix", &self.first_cycle_start_unix)
            .field("cycle_seconds", &self.cycle_seconds)
            .field("publish_delay_seconds", &self.publish_delay_seconds)
            .field("cycle_start_unix", &self.cycle_start_unix)
            .field("cycle_end_unix", &self.cycle_end_unix)
            .field("due_at_unix", &self.due_at_unix)
            .field("private_source_digest", &"<redacted>")
            .field("policy_digest", &self.policy_digest)
            .field(
                "population_inventory_digest",
                &self.population_inventory_digest,
            )
            .field("metric_schema_digest", &self.metric_schema_digest)
            .field("privacy", &self.privacy)
            .field("prf_request_binding", &self.prf_request_binding)
            .field("prf_commitment", &self.prf_commitment)
            .field("budget_charge_digest", &self.budget_charge_digest)
            .field(
                "publication_payload_digest",
                &self.publication_payload_digest,
            )
            .field(
                "published_aggregate_inventory_digest",
                &self.published_aggregate_inventory_digest,
            )
            .field(
                "previous_publication_block_hash",
                &self.previous_publication_block_hash,
            )
            .field("publication_block_hash", &self.publication_block_hash)
            .field("status", &self.status)
            .field("previous_record_digest", &self.previous_record_digest)
            .field("record_digest", &self.record_digest)
            .finish()
    }
}

/// Internal hash-chained privacy release ledger persisted with the outbox.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub(crate) struct PrivacyReleaseLedgerV1 {
    pub(crate) version: u8,
    pub(crate) records: Vec<PrivacyReleaseRecordV1>,
}

impl Default for PrivacyReleaseLedgerV1 {
    fn default() -> Self {
        Self {
            version: PRIVACY_RELEASE_LEDGER_VERSION_V1,
            records: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub(crate) enum PrivacyReleaseLedgerErrorV1 {
    #[error("privacy release ledger version is unsupported")]
    UnsupportedVersion,
    #[error("privacy release ledger exceeds the V1 record bound")]
    CollectionTooLarge,
    #[error("privacy release id is duplicated")]
    DuplicateRelease,
    #[error("privacy release record is invalid")]
    InvalidRecord,
}

impl PrivacyReleaseLedgerV1 {
    pub(crate) fn validate(&self) -> Result<(), PrivacyReleaseLedgerErrorV1> {
        if self.version != PRIVACY_RELEASE_LEDGER_VERSION_V1 {
            return Err(PrivacyReleaseLedgerErrorV1::UnsupportedVersion);
        }
        if self.records.len() > PRIVACY_RELEASE_MAX_RECORDS_V1 {
            return Err(PrivacyReleaseLedgerErrorV1::CollectionTooLarge);
        }
        let mut previous_digest = None;
        let mut previous_record: Option<&PrivacyReleaseRecordV1> = None;
        let mut latest_publication_block_hash = None;
        let mut release_ids = BTreeSet::new();
        for (index, record) in self.records.iter().enumerate() {
            let expected_sequence = u64::try_from(index)
                .map_err(|_| PrivacyReleaseLedgerErrorV1::CollectionTooLarge)?
                .checked_add(1)
                .ok_or(PrivacyReleaseLedgerErrorV1::CollectionTooLarge)?;
            if record.sequence != expected_sequence
                || !release_ids.insert(record.release_id)
                || record.release_id == [0; 16]
                || record.query_id == [0; 32]
                || record.first_cycle_start_unix == 0
                || record.cycle_seconds == 0
                || record.first_cycle_start_unix % record.cycle_seconds != 0
                || record.cycle_end_unix.checked_sub(record.cycle_start_unix)
                    != Some(record.cycle_seconds)
                || record
                    .cycle_end_unix
                    .checked_add(record.publish_delay_seconds)
                    != Some(record.due_at_unix)
                || previous_record.map_or(
                    record.cycle_start_unix != record.first_cycle_start_unix,
                    |previous| {
                        record.query_id != previous.query_id
                            || record.first_cycle_start_unix != previous.first_cycle_start_unix
                            || record.cycle_seconds != previous.cycle_seconds
                            || record.publish_delay_seconds != previous.publish_delay_seconds
                            || record.cycle_start_unix != previous.cycle_end_unix
                    },
                )
                || record.release_id
                    != privacy_aggregate_cycle_id(
                        record.query_id,
                        record.cycle_start_unix,
                        record.cycle_end_unix,
                    )
                || record.private_source_digest == [0; 32]
                || record.policy_digest == [0; 32]
                || record.population_inventory_digest == [0; 32]
                || record.metric_schema_digest == [0; 32]
                || record.previous_record_digest != previous_digest
                || record.previous_publication_block_hash != latest_publication_block_hash
                || privacy_release_record_digest(record) != record.record_digest
            {
                return Err(PrivacyReleaseLedgerErrorV1::InvalidRecord);
            }
            record
                .privacy
                .validate()
                .map_err(|_| PrivacyReleaseLedgerErrorV1::InvalidRecord)?;
            let uses_dp = record.privacy.per_subject_metric_cap.is_some();
            match (record.status, uses_dp) {
                (PrivacyReleaseStatusV1::Published, true)
                    if record.prf_request_binding.is_some()
                        && record.prf_commitment.is_some()
                        && record.budget_charge_digest.is_some()
                        && record.publication_payload_digest.is_some()
                        && record.published_aggregate_inventory_digest.is_some()
                        && record.publication_block_hash.is_some() => {}
                (PrivacyReleaseStatusV1::Published, false)
                    if record.prf_request_binding.is_none()
                        && record.prf_commitment.is_none()
                        && record.budget_charge_digest.is_none()
                        && record.publication_payload_digest.is_some()
                        && record.published_aggregate_inventory_digest.is_some()
                        && record.publication_block_hash.is_some() => {}
                (PrivacyReleaseStatusV1::Suppressed, false)
                    if record.prf_request_binding.is_none()
                        && record.prf_commitment.is_none()
                        && record.budget_charge_digest.is_none()
                        && record.publication_payload_digest.is_none()
                        && record.published_aggregate_inventory_digest.is_none()
                        && record.publication_block_hash.is_none() => {}
                _ => return Err(PrivacyReleaseLedgerErrorV1::InvalidRecord),
            }
            if record
                .prf_request_binding
                .is_some_and(|digest| digest == [0; 32])
                || record
                    .prf_commitment
                    .is_some_and(|digest| digest == [0; 32])
                || record
                    .budget_charge_digest
                    .is_some_and(|digest| digest == [0; 32])
                || record
                    .publication_payload_digest
                    .is_some_and(|digest| digest == [0; 32])
                || record
                    .published_aggregate_inventory_digest
                    .is_some_and(|digest| digest == [0; 32])
                || record
                    .previous_publication_block_hash
                    .is_some_and(|digest| digest == [0; 32])
                || record
                    .publication_block_hash
                    .is_some_and(|digest| digest == [0; 32])
            {
                return Err(PrivacyReleaseLedgerErrorV1::InvalidRecord);
            }
            if uses_dp {
                let request = PrivacyCyclePrfRequestV1::new(
                    record.query_id,
                    record.policy_digest,
                    record.population_inventory_digest,
                    record.metric_schema_digest,
                    PrivacyAggregateCycleWindow {
                        cycle_start_unix: record.cycle_start_unix,
                        cycle_end_unix: record.cycle_end_unix,
                        due_at_unix: record.cycle_end_unix,
                    },
                )
                .map_err(|_| PrivacyReleaseLedgerErrorV1::InvalidRecord)?;
                if record.prf_request_binding != Some(request.binding_digest()) {
                    return Err(PrivacyReleaseLedgerErrorV1::InvalidRecord);
                }
            }
            if let Some(block_hash) = record.publication_block_hash {
                latest_publication_block_hash = Some(block_hash);
            }
            previous_digest = Some(record.record_digest);
            previous_record = Some(record);
        }
        Ok(())
    }

    pub(crate) fn append(
        &mut self,
        mut record: PrivacyReleaseRecordV1,
    ) -> Result<PrivacyReleaseRecordV1, PrivacyReleaseLedgerErrorV1> {
        self.validate()?;
        if self.records.len() >= PRIVACY_RELEASE_MAX_RECORDS_V1 {
            return Err(PrivacyReleaseLedgerErrorV1::CollectionTooLarge);
        }
        if self
            .records
            .iter()
            .any(|existing| existing.release_id == record.release_id)
        {
            return Err(PrivacyReleaseLedgerErrorV1::DuplicateRelease);
        }
        record.sequence = u64::try_from(self.records.len())
            .map_err(|_| PrivacyReleaseLedgerErrorV1::CollectionTooLarge)?
            .checked_add(1)
            .ok_or(PrivacyReleaseLedgerErrorV1::CollectionTooLarge)?;
        record.previous_record_digest = self.records.last().map(|item| item.record_digest);
        record.record_digest = privacy_release_record_digest(&record);
        let mut candidate = self.clone();
        candidate.records.push(record.clone());
        candidate.validate()?;
        *self = candidate;
        Ok(record)
    }

    pub(crate) fn head(
        &self,
        query_id: [u8; 32],
    ) -> Result<PrivacyReleaseAnchorHeadV1, PrivacyReleaseLedgerErrorV1> {
        self.validate()?;
        if self
            .records
            .iter()
            .any(|record| record.query_id != query_id)
        {
            return Err(PrivacyReleaseLedgerErrorV1::InvalidRecord);
        }
        Ok(self.records.last().map_or_else(
            || PrivacyReleaseAnchorHeadV1::genesis(query_id),
            PrivacyReleaseAnchorHeadV1::from_record,
        ))
    }
}

impl Default for PrivacyCompositionBudgetLedgerV1 {
    fn default() -> Self {
        Self {
            version: PRIVACY_BUDGET_LEDGER_VERSION_V1,
            chains: Vec::new(),
        }
    }
}

/// Fail-closed composition-budget ledger errors.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PrivacyCompositionBudgetError {
    /// Budget policy digest is all zero.
    #[error("privacy composition budget id must be nonzero")]
    MissingBudgetId,
    /// Budget limit is zero, non-reduced, or has a zero denominator.
    #[error("privacy composition budget epsilon limit must be a reduced positive rational")]
    InvalidBudgetLimit,
    /// Per-policy publication bound is invalid.
    #[error("privacy composition budget publication limit is invalid")]
    InvalidPublicationLimit,
    /// Ledger schema version is unsupported.
    #[error("privacy composition budget ledger version is unsupported")]
    UnsupportedVersion,
    /// Policy chains are unsorted or duplicated.
    #[error("privacy composition budget policy chains must be unique and sorted")]
    PolicyOrder,
    /// A configured budget id was reused with different policy parameters.
    #[error("privacy composition budget policy conflicts with the retained lineage")]
    PolicyConflict,
    /// Charge epsilon is zero, non-reduced, or has a zero denominator.
    #[error("privacy composition budget charge epsilon must be a reduced positive rational")]
    InvalidChargeEpsilon,
    /// Cycle id is all zero.
    #[error("privacy composition budget cycle id must be nonzero")]
    MissingCycleId,
    /// Charge timestamp is zero.
    #[error("privacy composition budget charge timestamp must be nonzero")]
    InvalidChargeTimestamp,
    /// A cycle was already charged.
    #[error("privacy composition budget cycle was already charged")]
    DuplicateCycle,
    /// Charge sequence, predecessor, cumulative value, or digest is malformed.
    #[error("privacy composition budget charge chain is invalid")]
    InvalidChargeChain,
    /// Rational arithmetic exceeded the bounded V1 representation.
    #[error("privacy composition budget rational arithmetic overflow")]
    ArithmeticOverflow,
    /// The governed epsilon or publication-count budget is exhausted.
    #[error("privacy composition budget exhausted")]
    BudgetExhausted,
    /// The ledger exceeds its bounded number of policy lineages or charges.
    #[error("privacy composition budget ledger exceeds V1 bounds")]
    CollectionTooLarge,
}

impl PrivacyCompositionBudgetLedgerV1 {
    /// Validate every retained policy lineage and hash-chain link.
    pub(crate) fn validate(&self) -> Result<(), PrivacyCompositionBudgetError> {
        if self.version != PRIVACY_BUDGET_LEDGER_VERSION_V1 {
            return Err(PrivacyCompositionBudgetError::UnsupportedVersion);
        }
        if self.chains.len() > PRIVACY_BUDGET_MAX_POLICIES_V1 {
            return Err(PrivacyCompositionBudgetError::CollectionTooLarge);
        }
        let mut previous_budget_id = None;
        let mut total_charges = 0_usize;
        let mut all_cycles = BTreeSet::new();
        for chain in &self.chains {
            chain.policy.validate()?;
            if previous_budget_id.is_some_and(|previous| previous >= chain.policy.budget_id) {
                return Err(PrivacyCompositionBudgetError::PolicyOrder);
            }
            previous_budget_id = Some(chain.policy.budget_id);
            total_charges = total_charges
                .checked_add(chain.charges.len())
                .ok_or(PrivacyCompositionBudgetError::CollectionTooLarge)?;
            if total_charges > PRIVACY_BUDGET_MAX_CHARGES_V1
                || chain.charges.len() > chain.policy.max_publications as usize
            {
                return Err(PrivacyCompositionBudgetError::CollectionTooLarge);
            }
            validate_budget_charge_chain(chain, &mut all_cycles)?;
        }
        Ok(())
    }

    /// Append one cycle charge after proving the composed epsilon stays in budget.
    pub(crate) fn charge(
        &mut self,
        policy: PrivacyCompositionBudgetPolicyV1,
        cycle_id: [u8; 16],
        charged_at_unix: u64,
        epsilon_numerator: u64,
        epsilon_denominator: u64,
    ) -> Result<PrivacyCompositionBudgetChargeV1, PrivacyCompositionBudgetError> {
        let mut candidate = self.clone();
        let charge = candidate.charge_in_place(
            policy,
            cycle_id,
            charged_at_unix,
            epsilon_numerator,
            epsilon_denominator,
        )?;
        candidate.validate()?;
        *self = candidate;
        Ok(charge)
    }

    fn charge_in_place(
        &mut self,
        policy: PrivacyCompositionBudgetPolicyV1,
        cycle_id: [u8; 16],
        charged_at_unix: u64,
        epsilon_numerator: u64,
        epsilon_denominator: u64,
    ) -> Result<PrivacyCompositionBudgetChargeV1, PrivacyCompositionBudgetError> {
        self.validate()?;
        policy.validate()?;
        if cycle_id.iter().all(|byte| *byte == 0) {
            return Err(PrivacyCompositionBudgetError::MissingCycleId);
        }
        if charged_at_unix == 0 {
            return Err(PrivacyCompositionBudgetError::InvalidChargeTimestamp);
        }
        require_reduced_positive_rational(epsilon_numerator, epsilon_denominator)
            .map_err(|_| PrivacyCompositionBudgetError::InvalidChargeEpsilon)?;
        if self
            .chains
            .iter()
            .flat_map(|chain| chain.charges.iter())
            .any(|charge| charge.cycle_id == cycle_id)
        {
            return Err(PrivacyCompositionBudgetError::DuplicateCycle);
        }

        let chain_index = match self
            .chains
            .binary_search_by_key(&policy.budget_id, |chain| chain.policy.budget_id)
        {
            Ok(index) => {
                if self.chains[index].policy != policy {
                    return Err(PrivacyCompositionBudgetError::PolicyConflict);
                }
                index
            }
            Err(index) => {
                if self.chains.len() >= PRIVACY_BUDGET_MAX_POLICIES_V1 {
                    return Err(PrivacyCompositionBudgetError::CollectionTooLarge);
                }
                self.chains.insert(
                    index,
                    PrivacyCompositionBudgetChainV1 {
                        policy,
                        charges: Vec::new(),
                    },
                );
                index
            }
        };
        let total_charges = self
            .chains
            .iter()
            .try_fold(0_usize, |total, chain| {
                total.checked_add(chain.charges.len())
            })
            .ok_or(PrivacyCompositionBudgetError::CollectionTooLarge)?;
        let chain = &mut self.chains[chain_index];
        if chain.charges.len() >= chain.policy.max_publications as usize
            || total_charges >= PRIVACY_BUDGET_MAX_CHARGES_V1
        {
            return Err(PrivacyCompositionBudgetError::BudgetExhausted);
        }
        let (previous_numerator, previous_denominator) =
            chain.charges.last().map_or((0, 1), |charge| {
                (
                    charge.cumulative_epsilon_numerator,
                    charge.cumulative_epsilon_denominator,
                )
            });
        let (cumulative_epsilon_numerator, cumulative_epsilon_denominator) = add_rationals(
            previous_numerator,
            previous_denominator,
            epsilon_numerator,
            epsilon_denominator,
        )?;
        if rational_greater_than(
            cumulative_epsilon_numerator,
            cumulative_epsilon_denominator,
            chain.policy.epsilon_limit_numerator,
            chain.policy.epsilon_limit_denominator,
        )? {
            return Err(PrivacyCompositionBudgetError::BudgetExhausted);
        }
        let sequence = u64::try_from(chain.charges.len())
            .map_err(|_| PrivacyCompositionBudgetError::CollectionTooLarge)?
            .checked_add(1)
            .ok_or(PrivacyCompositionBudgetError::CollectionTooLarge)?;
        let previous_charge_digest = chain.charges.last().map(|charge| charge.charge_digest);
        let mut charge = PrivacyCompositionBudgetChargeV1 {
            sequence,
            cycle_id,
            charged_at_unix,
            epsilon_numerator,
            epsilon_denominator,
            cumulative_epsilon_numerator,
            cumulative_epsilon_denominator,
            previous_charge_digest,
            charge_digest: [0; 32],
        };
        charge.charge_digest = budget_charge_digest(chain.policy.budget_id, &charge);
        chain.charges.push(charge.clone());
        Ok(charge)
    }
}

fn validate_budget_charge_chain(
    chain: &PrivacyCompositionBudgetChainV1,
    all_cycles: &mut BTreeSet<[u8; 16]>,
) -> Result<(), PrivacyCompositionBudgetError> {
    let mut previous_digest = None;
    let mut previous_timestamp = 0_u64;
    let mut cumulative = (0_u64, 1_u64);
    for (index, charge) in chain.charges.iter().enumerate() {
        let expected_sequence = u64::try_from(index)
            .map_err(|_| PrivacyCompositionBudgetError::CollectionTooLarge)?
            .checked_add(1)
            .ok_or(PrivacyCompositionBudgetError::CollectionTooLarge)?;
        if charge.sequence != expected_sequence
            || charge.cycle_id.iter().all(|byte| *byte == 0)
            || charge.charged_at_unix == 0
            || charge.charged_at_unix < previous_timestamp
            || charge.previous_charge_digest != previous_digest
            || !all_cycles.insert(charge.cycle_id)
        {
            return Err(PrivacyCompositionBudgetError::InvalidChargeChain);
        }
        require_reduced_positive_rational(charge.epsilon_numerator, charge.epsilon_denominator)
            .map_err(|_| PrivacyCompositionBudgetError::InvalidChargeChain)?;
        cumulative = add_rationals(
            cumulative.0,
            cumulative.1,
            charge.epsilon_numerator,
            charge.epsilon_denominator,
        )
        .map_err(|_| PrivacyCompositionBudgetError::InvalidChargeChain)?;
        if cumulative
            != (
                charge.cumulative_epsilon_numerator,
                charge.cumulative_epsilon_denominator,
            )
            || rational_greater_than(
                cumulative.0,
                cumulative.1,
                chain.policy.epsilon_limit_numerator,
                chain.policy.epsilon_limit_denominator,
            )
            .map_err(|_| PrivacyCompositionBudgetError::InvalidChargeChain)?
            || budget_charge_digest(chain.policy.budget_id, charge) != charge.charge_digest
        {
            return Err(PrivacyCompositionBudgetError::InvalidChargeChain);
        }
        previous_digest = Some(charge.charge_digest);
        previous_timestamp = charge.charged_at_unix;
    }
    Ok(())
}

fn budget_charge_digest(
    budget_id: [u8; 32],
    charge: &PrivacyCompositionBudgetChargeV1,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PRIVACY_BUDGET_ENTRY_DOMAIN_V1);
    hasher.update(&budget_id);
    hasher.update(&charge.sequence.to_le_bytes());
    hasher.update(&charge.cycle_id);
    hasher.update(&charge.charged_at_unix.to_le_bytes());
    hasher.update(&charge.epsilon_numerator.to_le_bytes());
    hasher.update(&charge.epsilon_denominator.to_le_bytes());
    hasher.update(&charge.cumulative_epsilon_numerator.to_le_bytes());
    hasher.update(&charge.cumulative_epsilon_denominator.to_le_bytes());
    match charge.previous_charge_digest {
        Some(digest) => {
            hasher.update(&[1]);
            hasher.update(&digest);
        }
        None => {
            hasher.update(&[0]);
        }
    }
    *hasher.finalize().as_bytes()
}

fn privacy_release_record_digest(record: &PrivacyReleaseRecordV1) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PRIVACY_RELEASE_RECORD_DOMAIN_V1);
    hasher.update(&record.sequence.to_le_bytes());
    hasher.update(&record.release_id);
    hasher.update(&record.query_id);
    hasher.update(&record.first_cycle_start_unix.to_le_bytes());
    hasher.update(&record.cycle_seconds.to_le_bytes());
    hasher.update(&record.publish_delay_seconds.to_le_bytes());
    hasher.update(&record.cycle_start_unix.to_le_bytes());
    hasher.update(&record.cycle_end_unix.to_le_bytes());
    hasher.update(&record.due_at_unix.to_le_bytes());
    hasher.update(&record.private_source_digest);
    hasher.update(&record.policy_digest);
    hasher.update(&record.population_inventory_digest);
    hasher.update(&record.metric_schema_digest);
    hash_privacy_parameters(&mut hasher, record.privacy);
    hash_option_digest(&mut hasher, record.prf_request_binding);
    hash_option_digest(&mut hasher, record.prf_commitment);
    hash_option_digest(&mut hasher, record.budget_charge_digest);
    hash_option_digest(&mut hasher, record.publication_payload_digest);
    hash_option_digest(&mut hasher, record.published_aggregate_inventory_digest);
    hash_option_digest(&mut hasher, record.previous_publication_block_hash);
    hash_option_digest(&mut hasher, record.publication_block_hash);
    hasher.update(&[match record.status {
        PrivacyReleaseStatusV1::Published => 1,
        PrivacyReleaseStatusV1::Suppressed => 2,
    }]);
    hash_option_digest(&mut hasher, record.previous_record_digest);
    *hasher.finalize().as_bytes()
}

fn hash_option_digest(hasher: &mut blake3::Hasher, value: Option<[u8; 32]>) {
    match value {
        Some(digest) => {
            hasher.update(&[1]);
            hasher.update(&digest);
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

fn require_reduced_positive_rational(numerator: u64, denominator: u64) -> Result<(), ()> {
    if numerator == 0 || denominator == 0 || gcd_u64(numerator, denominator) != 1 {
        return Err(());
    }
    Ok(())
}

fn add_rationals(
    left_numerator: u64,
    left_denominator: u64,
    right_numerator: u64,
    right_denominator: u64,
) -> Result<(u64, u64), PrivacyCompositionBudgetError> {
    if left_denominator == 0 || right_denominator == 0 {
        return Err(PrivacyCompositionBudgetError::ArithmeticOverflow);
    }
    let numerator = u128::from(left_numerator)
        .checked_mul(u128::from(right_denominator))
        .and_then(|left| {
            u128::from(right_numerator)
                .checked_mul(u128::from(left_denominator))
                .and_then(|right| left.checked_add(right))
        })
        .ok_or(PrivacyCompositionBudgetError::ArithmeticOverflow)?;
    let denominator = u128::from(left_denominator)
        .checked_mul(u128::from(right_denominator))
        .ok_or(PrivacyCompositionBudgetError::ArithmeticOverflow)?;
    let divisor = gcd_u128(numerator, denominator);
    let numerator = numerator / divisor;
    let denominator = denominator / divisor;
    Ok((
        u64::try_from(numerator).map_err(|_| PrivacyCompositionBudgetError::ArithmeticOverflow)?,
        u64::try_from(denominator)
            .map_err(|_| PrivacyCompositionBudgetError::ArithmeticOverflow)?,
    ))
}

fn rational_greater_than(
    left_numerator: u64,
    left_denominator: u64,
    right_numerator: u64,
    right_denominator: u64,
) -> Result<bool, PrivacyCompositionBudgetError> {
    if left_denominator == 0 || right_denominator == 0 {
        return Err(PrivacyCompositionBudgetError::ArithmeticOverflow);
    }
    let left = u128::from(left_numerator)
        .checked_mul(u128::from(right_denominator))
        .ok_or(PrivacyCompositionBudgetError::ArithmeticOverflow)?;
    let right = u128::from(right_numerator)
        .checked_mul(u128::from(left_denominator))
        .ok_or(PrivacyCompositionBudgetError::ArithmeticOverflow)?;
    Ok(left > right)
}

const fn gcd_u64(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

const fn gcd_u128(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

impl PrivacyAggregateCycleConfig {
    pub(crate) fn validate(&self) -> Result<(), PrivacyAggregateWorkerError> {
        require_nonzero32("query_id", &self.query_id)?;
        if self.first_cycle_start_unix == 0 {
            return Err(PrivacyAggregateWorkerError::InvalidSchedule {
                field: "first_cycle_start_unix",
            });
        }
        if self.cycle_seconds == 0
            || self.first_cycle_start_unix % self.cycle_seconds != 0
            || self
                .first_cycle_start_unix
                .checked_add(self.cycle_seconds)
                .is_none()
        {
            return Err(PrivacyAggregateWorkerError::InvalidSchedule {
                field: "cycle_seconds",
            });
        }
        require_public_text("aggregate_id_prefix", &self.aggregate_id_prefix)?;
        validate_population_inventory(&self.populations)?;
        validate_metric_schema(&self.metrics)?;
        self.privacy.validate().map_err(|err| {
            PrivacyAggregateWorkerError::InvalidPrivacyParameters {
                message: err.to_string(),
            }
        })?;
        if let Some(sensitivity) = privacy_vector_sensitivity(self)? {
            let epsilon_numerator = self.privacy.epsilon_numerator.ok_or(
                PrivacyAggregateWorkerError::InvalidPrivacyParameters {
                    message: "epsilon_numerator is required for exact discrete-Laplace noise"
                        .to_string(),
                },
            )?;
            let epsilon_denominator = self.privacy.epsilon_denominator.ok_or(
                PrivacyAggregateWorkerError::InvalidPrivacyParameters {
                    message: "epsilon_denominator is required for exact discrete-Laplace noise"
                        .to_string(),
                },
            )?;
            validate_discrete_laplace_parameters(
                epsilon_numerator,
                epsilon_denominator,
                sensitivity,
            )?;
            validate_release_noise_complexity(
                self.populations.len(),
                self.metrics.len(),
                epsilon_numerator,
                epsilon_denominator,
                sensitivity,
            )?;
        }
        require_nonzero32("policy_digest", &self.policy_digest)?;
        validate_metadata(&self.metadata)?;
        if self
            .metadata
            .iter()
            .any(|item| item.key == MODERATION_PRIVACY_RANDOMNESS_COMMITMENT_METADATA_KEY_V1)
        {
            return Err(PrivacyAggregateWorkerError::ReservedMetadataKey {
                key: MODERATION_PRIVACY_RANDOMNESS_COMMITMENT_METADATA_KEY_V1,
            });
        }
        Ok(())
    }

    /// Validate one event against this already-validated governed query.
    pub(crate) fn validate_source_event(
        &self,
        event: &PrivacyAggregateSourceEvent,
    ) -> Result<(), PrivacyAggregateWorkerError> {
        event.validate()?;
        if !self.populations.iter().any(|population| {
            population.label == event.population_label
                && population.digest == event.population_digest
        }) {
            return Err(PrivacyAggregateWorkerError::PopulationOutsideInventory);
        }
        if event.policy_digest != self.policy_digest {
            return Err(PrivacyAggregateWorkerError::PolicyDigestMismatch);
        }
        if !event_metrics_match_schema(&event.metrics, &self.metrics) {
            return Err(PrivacyAggregateWorkerError::MetricSchemaMismatch);
        }
        Ok(())
    }
}

fn validate_population_inventory(
    populations: &[PrivacyAggregatePopulationV1],
) -> Result<(), PrivacyAggregateWorkerError> {
    if populations.is_empty() {
        return Err(PrivacyAggregateWorkerError::PopulationInventoryMissing);
    }
    if populations.len() > PRIVACY_AGGREGATE_MAX_POPULATIONS_V1 {
        return Err(PrivacyAggregateWorkerError::TooManyPopulations {
            count: populations.len(),
            max: PRIVACY_AGGREGATE_MAX_POPULATIONS_V1,
        });
    }
    let mut last: Option<(&str, [u8; 32])> = None;
    let mut labels = BTreeSet::new();
    let mut digests = BTreeSet::new();
    for population in populations {
        require_public_text("populations.label", &population.label)?;
        require_nonzero32("populations.digest", &population.digest)?;
        if last.is_some_and(|previous| previous >= (population.label.as_str(), population.digest)) {
            return Err(PrivacyAggregateWorkerError::PopulationInventoryOrder);
        }
        if !labels.insert(population.label.as_str()) || !digests.insert(population.digest) {
            return Err(PrivacyAggregateWorkerError::DuplicatePopulation);
        }
        last = Some((population.label.as_str(), population.digest));
    }
    Ok(())
}

fn validate_metric_schema(
    metrics: &[PrivacyAggregateMetricSchemaV1],
) -> Result<(), PrivacyAggregateWorkerError> {
    if metrics.is_empty() {
        return Err(PrivacyAggregateWorkerError::MetricSchemaMissing);
    }
    if metrics.len() > MODERATION_PRIVACY_MAX_METRICS_V1 {
        return Err(PrivacyAggregateWorkerError::TooManySourceMetrics {
            count: metrics.len(),
            max: MODERATION_PRIVACY_MAX_METRICS_V1,
        });
    }
    let mut last_key = None;
    for metric in metrics {
        require_public_text("metric_schema.key", &metric.key)?;
        require_public_text("metric_schema.unit", &metric.unit)?;
        if last_key.is_some_and(|last| last >= metric.key.as_str()) {
            return Err(PrivacyAggregateWorkerError::MetricSchemaOrder);
        }
        last_key = Some(metric.key.as_str());
    }
    Ok(())
}

fn privacy_vector_sensitivity(
    config: &PrivacyAggregateCycleConfig,
) -> Result<Option<u64>, PrivacyAggregateWorkerError> {
    let Some(cap) = config.privacy.per_subject_metric_cap else {
        return Ok(None);
    };
    let threshold_multiplier = match config.privacy.mode {
        ModerationPrivacyModeV1::DifferentialPrivacy => 1,
        // DP+k uses the deterministic preprocessing f(D)=0 for populations
        // with fewer than k subjects and the clipped sum otherwise. Under
        // add/remove-one-subject adjacency, the boundary transition from
        // k-1 to k can expose at most k clipped contributions per metric.
        // Every other transition changes at most one clipped contribution.
        // Therefore k * cap * published_metric_count is a conservative joint
        // L1 sensitivity for the complete, fixed-schema release vector.
        ModerationPrivacyModeV1::DifferentialPrivacyWithSuppression => config
            .privacy
            .suppression_threshold
            .ok_or(PrivacyAggregateWorkerError::InvalidPrivacyParameters {
                message: "suppression_threshold is required for DP+k suppression".to_string(),
            })?,
        ModerationPrivacyModeV1::Suppression => {
            return Err(PrivacyAggregateWorkerError::InvalidPrivacyParameters {
                message: "suppression-only mode must not configure a contribution cap".to_string(),
            });
        }
    };
    cap.checked_mul(threshold_multiplier)
        .and_then(|per_metric| {
            u64::try_from(config.metrics.len())
                .ok()
                .and_then(|metric_count| per_metric.checked_mul(metric_count))
        })
        .map(Some)
        .ok_or(PrivacyAggregateWorkerError::MetricArithmeticOverflow)
}

/// Digest the exact fixed public population universe for PRF/release binding.
#[must_use]
pub fn privacy_population_inventory_digest(
    populations: &[PrivacyAggregatePopulationV1],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(POPULATION_INVENTORY_DIGEST_DOMAIN_V1);
    hasher.update(&(populations.len() as u64).to_le_bytes());
    for population in populations {
        hash_text(&mut hasher, &population.label);
        hasher.update(&population.digest);
    }
    *hasher.finalize().as_bytes()
}

/// Digest the exact fixed public metric universe for PRF/release binding.
#[must_use]
pub fn privacy_metric_schema_digest(metrics: &[PrivacyAggregateMetricSchemaV1]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(METRIC_SCHEMA_DIGEST_DOMAIN_V1);
    hasher.update(&(metrics.len() as u64).to_le_bytes());
    for metric in metrics {
        hash_text(&mut hasher, &metric.key);
        hash_text(&mut hasher, &metric.unit);
    }
    *hasher.finalize().as_bytes()
}

/// Schedule used by the local aggregate worker to choose due publication windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivacyAggregateScheduleConfig {
    /// Governed inclusive start of the first releasable cycle.
    pub first_cycle_start_unix: u64,
    /// Width of each aggregation window, in seconds.
    pub cycle_seconds: u64,
    /// Delay after a cycle closes before it becomes eligible for publication.
    pub publish_delay_seconds: u64,
}

impl PrivacyAggregateScheduleConfig {
    pub(crate) fn validate(&self) -> Result<(), PrivacyAggregateWorkerError> {
        if self.first_cycle_start_unix == 0 {
            return Err(PrivacyAggregateWorkerError::InvalidSchedule {
                field: "first_cycle_start_unix",
            });
        }
        if self.cycle_seconds == 0
            || self.first_cycle_start_unix % self.cycle_seconds != 0
            || self
                .first_cycle_start_unix
                .checked_add(self.cycle_seconds)
                .is_none()
        {
            return Err(PrivacyAggregateWorkerError::InvalidSchedule {
                field: "cycle_seconds",
            });
        }
        if self
            .first_cycle_start_unix
            .checked_add(self.cycle_seconds)
            .and_then(|end| end.checked_add(self.publish_delay_seconds))
            .is_none()
        {
            return Err(PrivacyAggregateWorkerError::InvalidSchedule {
                field: "publish_delay_seconds",
            });
        }
        Ok(())
    }

    pub(crate) fn due_window(
        &self,
        now_unix: u64,
    ) -> Result<Option<PrivacyAggregateCycleWindow>, PrivacyAggregateWorkerError> {
        self.validate()?;
        let adjusted = match now_unix.checked_sub(self.publish_delay_seconds) {
            Some(adjusted) => adjusted,
            None => return Ok(None),
        };
        let elapsed = match adjusted.checked_sub(self.first_cycle_start_unix) {
            Some(elapsed) => elapsed,
            None => return Ok(None),
        };
        let completed_cycles = elapsed / self.cycle_seconds;
        if completed_cycles == 0 {
            return Ok(None);
        }
        let completed_width = completed_cycles.checked_mul(self.cycle_seconds).ok_or(
            PrivacyAggregateWorkerError::InvalidSchedule {
                field: "cycle_seconds",
            },
        )?;
        let cycle_end_unix = self
            .first_cycle_start_unix
            .checked_add(completed_width)
            .ok_or(PrivacyAggregateWorkerError::InvalidSchedule {
                field: "cycle_seconds",
            })?;
        let cycle_start_unix = cycle_end_unix - self.cycle_seconds;
        Ok(Some(PrivacyAggregateCycleWindow {
            cycle_start_unix,
            cycle_end_unix,
            due_at_unix: cycle_end_unix
                .checked_add(self.publish_delay_seconds)
                .ok_or(PrivacyAggregateWorkerError::InvalidSchedule {
                    field: "publish_delay_seconds",
                })?,
        }))
    }

    pub(crate) fn event_window(
        &self,
        occurred_at_unix: u64,
    ) -> Result<PrivacyAggregateCycleWindow, PrivacyAggregateWorkerError> {
        self.validate()?;
        let elapsed = occurred_at_unix
            .checked_sub(self.first_cycle_start_unix)
            .ok_or(PrivacyAggregateWorkerError::EventBeforeScheduleActivation)?;
        let offset = (elapsed / self.cycle_seconds)
            .checked_mul(self.cycle_seconds)
            .ok_or(PrivacyAggregateWorkerError::InvalidSchedule {
                field: "cycle_seconds",
            })?;
        let cycle_start_unix = self.first_cycle_start_unix.checked_add(offset).ok_or(
            PrivacyAggregateWorkerError::InvalidSchedule {
                field: "cycle_seconds",
            },
        )?;
        let cycle_end_unix = cycle_start_unix.checked_add(self.cycle_seconds).ok_or(
            PrivacyAggregateWorkerError::InvalidSchedule {
                field: "cycle_seconds",
            },
        )?;
        Ok(PrivacyAggregateCycleWindow {
            cycle_start_unix,
            cycle_end_unix,
            due_at_unix: cycle_end_unix
                .checked_add(self.publish_delay_seconds)
                .ok_or(PrivacyAggregateWorkerError::InvalidSchedule {
                    field: "publish_delay_seconds",
                })?,
        })
    }
}

/// One due privacy aggregate publication window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PrivacyAggregateCycleWindow {
    /// Inclusive window start timestamp, in Unix seconds.
    pub cycle_start_unix: u64,
    /// Exclusive window end timestamp, in Unix seconds.
    pub cycle_end_unix: u64,
    /// Timestamp at which the window becomes eligible for publication.
    pub due_at_unix: u64,
}

/// Canonical version of the runtime threshold-PRF request contract.
pub const PRIVACY_CYCLE_PRF_REQUEST_VERSION_V1: u16 = 1;

/// Runtime-only threshold-PRF request bound to one governed publication window.
///
/// The provider must evaluate the request as a single domain-separated input.
/// The request contains no secret seed material and is never published.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivacyCyclePrfRequestV1 {
    version: u16,
    query_id: [u8; 32],
    policy_digest: [u8; 32],
    population_inventory_digest: [u8; 32],
    metric_schema_digest: [u8; 32],
    cycle_id: [u8; 16],
    cycle_start_unix: u64,
    cycle_end_unix: u64,
    binding_digest: [u8; 32],
}

impl PrivacyCyclePrfRequestV1 {
    /// Construct the canonical request for one exact governed cycle.
    ///
    /// # Errors
    ///
    /// Returns an error when the policy digest is zero or the cycle window is
    /// not a canonical non-empty due window.
    pub fn new(
        query_id: [u8; 32],
        policy_digest: [u8; 32],
        population_inventory_digest: [u8; 32],
        metric_schema_digest: [u8; 32],
        window: PrivacyAggregateCycleWindow,
    ) -> Result<Self, PrivacyCyclePrfRequestErrorV1> {
        if query_id == [0; 32] {
            return Err(PrivacyCyclePrfRequestErrorV1::MissingQueryId);
        }
        if policy_digest == [0; 32] {
            return Err(PrivacyCyclePrfRequestErrorV1::MissingPolicyDigest);
        }
        if population_inventory_digest == [0; 32] {
            return Err(PrivacyCyclePrfRequestErrorV1::MissingPopulationInventoryDigest);
        }
        if metric_schema_digest == [0; 32] {
            return Err(PrivacyCyclePrfRequestErrorV1::MissingMetricSchemaDigest);
        }
        if window.cycle_start_unix == 0
            || window.cycle_end_unix <= window.cycle_start_unix
            || window.due_at_unix < window.cycle_end_unix
        {
            return Err(PrivacyCyclePrfRequestErrorV1::InvalidWindow);
        }
        let cycle_id =
            privacy_aggregate_cycle_id(query_id, window.cycle_start_unix, window.cycle_end_unix);
        let mut hasher = blake3::Hasher::new();
        hasher.update(CYCLE_PRF_REQUEST_BINDING_DOMAIN_V1);
        hasher.update(&PRIVACY_CYCLE_PRF_REQUEST_VERSION_V1.to_le_bytes());
        hasher.update(&query_id);
        hasher.update(&policy_digest);
        hasher.update(&population_inventory_digest);
        hasher.update(&metric_schema_digest);
        hasher.update(&cycle_id);
        hasher.update(&window.cycle_start_unix.to_le_bytes());
        hasher.update(&window.cycle_end_unix.to_le_bytes());
        Ok(Self {
            version: PRIVACY_CYCLE_PRF_REQUEST_VERSION_V1,
            query_id,
            policy_digest,
            population_inventory_digest,
            metric_schema_digest,
            cycle_id,
            cycle_start_unix: window.cycle_start_unix,
            cycle_end_unix: window.cycle_end_unix,
            binding_digest: *hasher.finalize().as_bytes(),
        })
    }

    /// Return the contract version.
    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    /// Return the stable governed query identity.
    #[must_use]
    pub const fn query_id(&self) -> [u8; 32] {
        self.query_id
    }

    /// Return the governed privacy-policy digest.
    #[must_use]
    pub const fn policy_digest(&self) -> [u8; 32] {
        self.policy_digest
    }

    /// Return the bound fixed-population inventory digest.
    #[must_use]
    pub const fn population_inventory_digest(&self) -> [u8; 32] {
        self.population_inventory_digest
    }

    /// Return the bound fixed-metric schema digest.
    #[must_use]
    pub const fn metric_schema_digest(&self) -> [u8; 32] {
        self.metric_schema_digest
    }

    /// Return the deterministic cycle identifier.
    #[must_use]
    pub const fn cycle_id(&self) -> [u8; 16] {
        self.cycle_id
    }

    /// Return the inclusive cycle start timestamp.
    #[must_use]
    pub const fn cycle_start_unix(&self) -> u64 {
        self.cycle_start_unix
    }

    /// Return the exclusive cycle end timestamp.
    #[must_use]
    pub const fn cycle_end_unix(&self) -> u64 {
        self.cycle_end_unix
    }

    /// Return the canonical domain-separated request binding.
    #[must_use]
    pub const fn binding_digest(&self) -> [u8; 32] {
        self.binding_digest
    }
}

/// Errors constructing a canonical threshold-PRF request.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum PrivacyCyclePrfRequestErrorV1 {
    /// The governed stable query identity was all zeroes.
    #[error("privacy cycle PRF request requires a non-zero query id")]
    MissingQueryId,
    /// The governed privacy-policy digest was all zeroes.
    #[error("privacy cycle PRF request requires a non-zero policy digest")]
    MissingPolicyDigest,
    /// The governed population inventory digest was all zeroes.
    #[error("privacy cycle PRF request requires a non-zero population inventory digest")]
    MissingPopulationInventoryDigest,
    /// The governed metric schema digest was all zeroes.
    #[error("privacy cycle PRF request requires a non-zero metric schema digest")]
    MissingMetricSchemaDigest,
    /// The supplied cycle window was empty, zero-based, or due before it ended.
    #[error("privacy cycle PRF request window is invalid")]
    InvalidWindow,
}

/// Stable, payload-free threshold-PRF provider failure classes.
///
/// Implementations must retain vendor diagnostics inside their own protected
/// telemetry boundary and return only one of these fixed classes.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum PrivacyCyclePrfProviderErrorV1 {
    /// The threshold service or required key share is unavailable.
    #[error("threshold PRF provider unavailable")]
    Unavailable,
    /// Runtime authentication or authorization failed.
    #[error("threshold PRF provider authentication failed")]
    AuthenticationFailed,
    /// The provider rejected the request due to a bounded service limit.
    #[error("threshold PRF provider rate limited")]
    RateLimited,
    /// The provider could not complete the request.
    #[error("threshold PRF provider internal failure")]
    Internal,
}

/// Stable, payload-free failures returned by the external finalized release anchor.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum PrivacyReleaseAnchorErrorV1 {
    /// The finalized head service or its quorum is unavailable.
    #[error("privacy release anchor unavailable")]
    Unavailable,
    /// Runtime authentication or authorization failed.
    #[error("privacy release anchor authentication failed")]
    AuthenticationFailed,
    /// Compare-and-set observed a conflicting finalized predecessor or head.
    #[error("privacy release anchor compare-and-set conflict")]
    Conflict,
    /// The service returned a malformed or equivocating finalized head.
    #[error("privacy release anchor returned invalid state")]
    InvalidState,
    /// The anchor could not complete the request.
    #[error("privacy release anchor internal failure")]
    Internal,
}

/// Runtime-only finalized-head service for the privacy release hash chain.
///
/// Production implementations are expected to read and advance a
/// quorum-finalized Governance DAG projection. The interface is deliberately
/// compare-and-set: two workers may race, but neither can replace or fork an
/// already finalized head.
pub trait PrivacyReleaseAnchorV1: Send + Sync {
    /// Read the exact finalized head for `query_id`.
    fn finalized_head(
        &self,
        query_id: [u8; 32],
    ) -> Result<PrivacyReleaseAnchorHeadV1, PrivacyReleaseAnchorErrorV1>;

    /// Atomically advance `expected` to its direct successor `next`.
    fn compare_and_set_finalized_head(
        &self,
        expected: PrivacyReleaseAnchorHeadV1,
        next: PrivacyReleaseAnchorHeadV1,
    ) -> Result<(), PrivacyReleaseAnchorErrorV1>;
}

/// Non-copying, redacted runtime wrapper for one hidden threshold-PRF output.
pub struct PrivacyCyclePrfOutputV1([u8; 32]);

impl PrivacyCyclePrfOutputV1 {
    /// Wrap a provider output after rejecting the forbidden all-zero value.
    ///
    /// # Errors
    ///
    /// Returns an error when `output` is all zeroes.
    pub fn new(output: [u8; 32]) -> Result<Self, PrivacyCyclePrfInputErrorV1> {
        if output == [0; 32] {
            return Err(PrivacyCyclePrfInputErrorV1::ZeroOutput);
        }
        Ok(Self(output))
    }

    fn expose(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for PrivacyCyclePrfOutputV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PrivacyCyclePrfOutputV1(<redacted>)")
    }
}

impl Drop for PrivacyCyclePrfOutputV1 {
    fn drop(&mut self) {
        self.0.fill(0);
        std::hint::black_box(&mut self.0);
    }
}

/// Runtime-only provider for hidden threshold-PRF cycle outputs.
///
/// Implementations must bind evaluation to [`PrivacyCyclePrfRequestV1`] and
/// must never expose raw provider diagnostics, key shares, seeds, or outputs
/// through logs or durable state.
pub trait PrivacyCyclePrfProviderV1: Send + Sync {
    /// Derive the hidden 32-byte output for one exact cycle request.
    fn derive_cycle_output(
        &self,
        request: &PrivacyCyclePrfRequestV1,
    ) -> Result<PrivacyCyclePrfOutputV1, PrivacyCyclePrfProviderErrorV1>;
}

/// Runtime-only, request-bound threshold-PRF material for one DP cycle.
///
/// The hidden output is deliberately not serializable and its `Debug`
/// implementation is redacted. Only [`Self::commitment`] may enter a public
/// aggregate.
pub struct PrivacyCyclePrfInputV1 {
    request: PrivacyCyclePrfRequestV1,
    output: PrivacyCyclePrfOutputV1,
    commitment: ModerationPrivacyThresholdPrfCommitmentV1,
}

impl PrivacyCyclePrfInputV1 {
    /// Bind a hidden threshold-PRF output to the exact authenticated request.
    ///
    pub fn new(request: PrivacyCyclePrfRequestV1, output: PrivacyCyclePrfOutputV1) -> Self {
        let commitment = ModerationPrivacyThresholdPrfCommitmentV1 {
            commitment: noise_randomness_commitment(request.binding_digest(), output.expose()),
        };
        Self {
            request,
            output,
            commitment,
        }
    }

    /// Return the opaque public commitment to this hidden cycle output.
    ///
    /// V1 treats this nonzero value as external threshold-attestation
    /// evidence. It deliberately does not expose enough material for local
    /// verification or recovery of the hidden PRF output.
    #[must_use]
    pub const fn commitment(&self) -> ModerationPrivacyThresholdPrfCommitmentV1 {
        self.commitment
    }

    pub(crate) const fn request(&self) -> PrivacyCyclePrfRequestV1 {
        self.request
    }

    fn validate_for_release(
        &self,
        config: &PrivacyAggregateCycleConfig,
        cycle_start_unix: u64,
        cycle_end_unix: u64,
    ) -> Result<(), PrivacyAggregateWorkerError> {
        if self.request.query_id() != config.query_id
            || self.request.policy_digest() != config.policy_digest
            || self.request.population_inventory_digest()
                != privacy_population_inventory_digest(&config.populations)
            || self.request.metric_schema_digest() != privacy_metric_schema_digest(&config.metrics)
            || self.request.cycle_start_unix() != cycle_start_unix
            || self.request.cycle_end_unix() != cycle_end_unix
        {
            return Err(PrivacyAggregateWorkerError::CyclePrfBindingMismatch);
        }
        let expected =
            noise_randomness_commitment(self.request.binding_digest(), self.output.expose());
        if self.commitment.commitment != expected {
            return Err(PrivacyAggregateWorkerError::CyclePrfCommitmentMismatch);
        }
        Ok(())
    }

    fn output(&self) -> &[u8; 32] {
        self.output.expose()
    }
}

impl std::fmt::Debug for PrivacyCyclePrfInputV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PrivacyCyclePrfInputV1")
            .field("request", &self.request)
            .field("output", &"<redacted>")
            .field("commitment", &self.commitment)
            .finish()
    }
}

/// Errors constructing runtime-only threshold-PRF cycle input.
#[derive(Debug, Clone, Copy, Error, PartialEq, Eq)]
pub enum PrivacyCyclePrfInputErrorV1 {
    /// The provider returned the forbidden all-zero output.
    #[error("threshold PRF provider returned an invalid output")]
    ZeroOutput,
}

/// Derive the canonical deterministic identity for one governed query window.
#[must_use]
pub fn privacy_aggregate_cycle_id(
    query_id: [u8; 32],
    cycle_start_unix: u64,
    cycle_end_unix: u64,
) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(CYCLE_ID_DOMAIN_V1);
    hasher.update(&query_id);
    hasher.update(&cycle_start_unix.to_le_bytes());
    hasher.update(&cycle_end_unix.to_le_bytes());
    let digest = hasher.finalize();
    let mut cycle_id = [0u8; 16];
    cycle_id.copy_from_slice(&digest.as_bytes()[..16]);
    cycle_id
}

/// Errors raised by local transparency ledger source-entry ingestion.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum TransparencyLedgerIngestError {
    /// Required text is blank or contains NUL.
    #[error("transparency ledger source field `{field}` must be non-empty public text")]
    MissingText {
        /// Field name.
        field: &'static str,
    },
    /// Public text exceeds the canonical V1 byte limit.
    #[error("privacy aggregate worker field `{field}` exceeds {max} UTF-8 bytes")]
    TextTooLong {
        /// Field name.
        field: &'static str,
        /// Maximum accepted UTF-8 byte length.
        max: usize,
    },
    /// Timestamp field is zero or inconsistent.
    #[error("transparency ledger source timestamp `{field}` is invalid")]
    InvalidTimestamp {
        /// Field name.
        field: &'static str,
    },
    /// Publication cycle end must be greater than start.
    #[error("transparency ledger source cycle end must be greater than cycle start")]
    InvalidCycleWindow,
    /// Publication generated_at must not precede cycle end.
    #[error("transparency ledger source generated_at timestamp must be >= cycle end")]
    InvalidGeneratedAt,
    /// Digest field is all zero.
    #[error("transparency ledger source digest `{field}` must be non-zero")]
    MissingDigest {
        /// Field name.
        field: &'static str,
    },
    /// No source entries matched the publication window.
    #[error("transparency ledger cycle has no source entries in the requested window")]
    NoSourceEntries,
    /// A source entry timestamp is outside the requested cycle window.
    #[error("transparency ledger source entry `{event_id}` is outside the requested cycle window")]
    EntryOutsideCycle {
        /// Event id.
        event_id: String,
    },
    /// The same source event id appeared twice.
    #[error("duplicate transparency ledger source entry `{event_id}`")]
    DuplicateSourceEntry {
        /// Event id.
        event_id: String,
    },
    /// Metadata keys are not sorted.
    #[error("transparency ledger source metadata keys must be sorted")]
    MetadataKeysUnsorted,
    /// Metadata key appears more than once.
    #[error("duplicate transparency ledger source metadata key `{key}`")]
    DuplicateMetadataKey {
        /// Duplicate key.
        key: String,
    },
    /// The generated ledger entry failed data-model validation.
    #[error("generated transparency ledger entry is invalid: {message}")]
    InvalidLedgerEntry {
        /// Validation detail.
        message: String,
    },
}

pub(crate) fn build_transparency_ledger_entries_from_source_events(
    cycle_id: [u8; 16],
    cycle_start_unix: u64,
    cycle_end_unix: u64,
    generated_at_unix: u64,
    events: &[TransparencyLedgerSourceEntry],
) -> Result<Vec<ModerationLedgerEntryV1>, TransparencyLedgerIngestError> {
    require_transparency_nonzero16("cycle_id", &cycle_id)?;
    if cycle_start_unix == 0 {
        return Err(TransparencyLedgerIngestError::InvalidTimestamp {
            field: "cycle_start_unix",
        });
    }
    if cycle_end_unix <= cycle_start_unix {
        return Err(TransparencyLedgerIngestError::InvalidCycleWindow);
    }
    if generated_at_unix < cycle_end_unix {
        return Err(TransparencyLedgerIngestError::InvalidGeneratedAt);
    }
    if events.is_empty() {
        return Err(TransparencyLedgerIngestError::NoSourceEntries);
    }

    let mut seen_events = BTreeSet::new();
    let mut sorted = Vec::with_capacity(events.len());
    for event in events {
        event.validate()?;
        if event.occurred_at_unix < cycle_start_unix || event.occurred_at_unix >= cycle_end_unix {
            return Err(TransparencyLedgerIngestError::EntryOutsideCycle {
                event_id: event.event_id.clone(),
            });
        }
        if !seen_events.insert(event.event_id.clone()) {
            return Err(TransparencyLedgerIngestError::DuplicateSourceEntry {
                event_id: event.event_id.clone(),
            });
        }
        sorted.push(event.clone());
    }
    sorted.sort_by(|left, right| {
        left.occurred_at_unix
            .cmp(&right.occurred_at_unix)
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.subject.cmp(&right.subject))
            .then_with(|| left.payload_digest.cmp(&right.payload_digest))
            .then_with(|| left.event_id.cmp(&right.event_id))
    });

    sorted
        .iter()
        .enumerate()
        .map(|(index, event)| {
            let sequence = u64::try_from(index)
                .map_err(|_| TransparencyLedgerIngestError::InvalidLedgerEntry {
                    message: "transparency ledger source index overflow".to_string(),
                })?
                .saturating_add(1);
            let entry_id = transparency_ledger_source_entry_id(cycle_id, event);
            event.to_ledger_entry(cycle_id, entry_id, sequence)
        })
        .collect()
}

pub(crate) fn transparency_ledger_source_entry_id(
    cycle_id: [u8; 16],
    event: &TransparencyLedgerSourceEntry,
) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(TRANSPARENCY_LEDGER_ENTRY_ID_DOMAIN_V1);
    hasher.update(&cycle_id);
    hash_text(&mut hasher, &event.event_id);
    hasher.update(&event.payload_digest);
    hasher.update(&event.summary_digest);
    let digest = hasher.finalize();
    let mut entry_id = [0u8; 16];
    entry_id.copy_from_slice(&digest.as_bytes()[..16]);
    entry_id
}

/// Errors raised while converting issued proof-token frames into transparency records.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum ProofTokenIssuanceIngestError {
    /// The base64 transport representation is malformed.
    #[error("proof-token issuance frame is not valid URL-safe base64")]
    InvalidBase64,
    /// The decoded proof-token frame is malformed.
    #[error("failed to decode proof-token issuance frame: {message}")]
    Decode {
        /// Decode failure detail.
        message: String,
    },
    /// The supplied public key does not verify the proof-token signature.
    #[error("proof-token issuance signature verification failed: {message}")]
    InvalidSignature {
        /// Verification failure detail.
        message: String,
    },
    /// A token timestamp could not be represented as UNIX seconds.
    #[error("proof-token issuance timestamp `{field}` is out of range")]
    TimestampOutOfRange {
        /// Timestamp field name.
        field: &'static str,
    },
    /// The derived issuance record failed data-model validation.
    #[error("derived proof-token issuance is invalid: {message}")]
    InvalidIssuance {
        /// Validation failure detail.
        message: String,
    },
}

/// Convert an issued `SFGT` proof-token frame into a public transparency record.
///
/// The frame signature is verified against `signer_key`, and only public token
/// material is copied into the resulting [`ProofTokenIssuanceV1`]. Digest keys
/// remain runtime-only and are not accepted by this helper.
///
/// # Errors
///
/// Returns [`ProofTokenIssuanceIngestError`] when the frame is malformed, the
/// signature does not verify, timestamps are not representable, or the derived
/// transparency payload fails validation.
pub fn proof_token_issuance_from_frame(
    encoded_token: &[u8],
    signer_key: [u8; 32],
    evidence_digest: Option<[u8; 32]>,
    policy_digest: Option<[u8; 32]>,
    metadata: Vec<ModerationLedgerMetadataV1>,
) -> Result<ProofTokenIssuanceV1, ProofTokenIssuanceIngestError> {
    let token =
        ProofToken::decode(encoded_token).map_err(|err| ProofTokenIssuanceIngestError::Decode {
            message: err.to_string(),
        })?;
    token.verify_signature_bytes(&signer_key).map_err(|err| {
        ProofTokenIssuanceIngestError::InvalidSignature {
            message: err.to_string(),
        }
    })?;

    let issued_at_unix = system_time_to_unix_secs(
        token
            .checked_issued_at()
            .ok_or(ProofTokenIssuanceIngestError::TimestampOutOfRange { field: "issued_at" })?,
        "issued_at",
    )?;
    let expires_at_unix = token
        .checked_expires_at()
        .map(|time| system_time_to_unix_secs(time, "expires_at"))
        .transpose()?;

    let issuance = ProofTokenIssuanceV1 {
        version: PROOF_TOKEN_ISSUANCE_VERSION_V1,
        token_id: token.token_id(),
        issued_at_unix,
        expires_at_unix,
        moderation_action_code: proof_token_moderation_action_code(token.moderation()),
        signer_key,
        token_blake3: *blake3::hash(encoded_token).as_bytes(),
        blinded_digest: *token.blinded_digest(),
        entry_ids: token.entry_ids().to_vec(),
        evidence_digest,
        policy_digest,
        metadata,
    };
    issuance
        .validate()
        .map_err(|err| ProofTokenIssuanceIngestError::InvalidIssuance {
            message: err.to_string(),
        })?;
    Ok(issuance)
}

/// Convert a URL-safe base64 `SFGT` proof-token frame into a transparency record.
///
/// # Errors
///
/// Returns [`ProofTokenIssuanceIngestError`] when base64 decoding, frame
/// decoding, signature verification, timestamp conversion, or data-model
/// validation fails.
pub fn proof_token_issuance_from_base64(
    token_b64: &str,
    signer_key: [u8; 32],
    evidence_digest: Option<[u8; 32]>,
    policy_digest: Option<[u8; 32]>,
    metadata: Vec<ModerationLedgerMetadataV1>,
) -> Result<ProofTokenIssuanceV1, ProofTokenIssuanceIngestError> {
    let encoded = URL_SAFE_NO_PAD
        .decode(token_b64.trim())
        .map_err(|_| ProofTokenIssuanceIngestError::InvalidBase64)?;
    proof_token_issuance_from_frame(
        &encoded,
        signer_key,
        evidence_digest,
        policy_digest,
        metadata,
    )
}

/// Errors raised by local privacy aggregate worker preparation.
#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum PrivacyAggregateWorkerError {
    /// Required text is blank or contains NUL.
    #[error("privacy aggregate worker field `{field}` must be non-empty public text")]
    MissingText {
        /// Field name.
        field: &'static str,
    },
    /// Public text exceeds the canonical V1 byte bound.
    #[error("privacy aggregate worker field `{field}` exceeds the {max}-byte limit")]
    TextTooLong {
        /// Field name.
        field: &'static str,
        /// Maximum accepted UTF-8 byte length.
        max: usize,
    },
    /// Timestamp field is zero.
    #[error("privacy aggregate worker timestamp `{field}` must be non-zero")]
    InvalidTimestamp {
        /// Field name.
        field: &'static str,
    },
    /// Digest field is all zero.
    #[error("privacy aggregate worker digest `{field}` must be non-zero")]
    MissingDigest {
        /// Field name.
        field: &'static str,
    },
    /// Source metric list is empty.
    #[error("privacy aggregate source event requires at least one metric")]
    SourceMetricsMissing,
    /// Source metric list exceeds the canonical V1 collection bound.
    #[error("privacy aggregate source event has {count} metrics; maximum is {max}")]
    TooManySourceMetrics {
        /// Submitted metric count.
        count: usize,
        /// Maximum accepted metric count.
        max: usize,
    },
    /// Governed population inventory is empty.
    #[error("privacy aggregate governed population inventory must not be empty")]
    PopulationInventoryMissing,
    /// Governed population inventory exceeds the V1 bound.
    #[error("privacy aggregate has {count} governed populations; maximum is {max}")]
    TooManyPopulations {
        /// Configured population count.
        count: usize,
        /// Maximum accepted population count.
        max: usize,
    },
    /// Governed populations are not in strict canonical order.
    #[error("privacy aggregate governed populations must be unique and sorted")]
    PopulationInventoryOrder,
    /// A governed population label or digest appears more than once.
    #[error("privacy aggregate governed population labels and digests must be unique")]
    DuplicatePopulation,
    /// Governed metric schema is empty.
    #[error("privacy aggregate governed metric schema must not be empty")]
    MetricSchemaMissing,
    /// Governed metric coordinates are not in strict canonical order.
    #[error("privacy aggregate governed metric schema must be unique and sorted")]
    MetricSchemaOrder,
    /// Metric accumulation exceeded the exact V1 integer representation.
    #[error("privacy aggregate metric arithmetic overflow")]
    MetricArithmeticOverflow,
    /// Source metric keys are not sorted.
    #[error("privacy aggregate source metric keys must be sorted")]
    SourceMetricKeysUnsorted,
    /// Source metric key appears more than once.
    #[error("privacy aggregate source metric key is duplicated")]
    DuplicateSourceMetricKey,
    /// Metadata keys are not sorted.
    #[error("privacy aggregate metadata keys must be sorted")]
    MetadataKeysUnsorted,
    /// Metadata key appears more than once.
    #[error("privacy aggregate metadata key is duplicated")]
    DuplicateMetadataKey,
    /// Privacy parameters are structurally invalid.
    #[error("invalid privacy aggregate parameters: {message}")]
    InvalidPrivacyParameters {
        /// Validation detail.
        message: String,
    },
    /// Differential privacy was configured without hidden cycle PRF output.
    #[error("privacy aggregate differential privacy requires hidden cycle PRF output")]
    MissingCyclePrfOutput,
    /// Runtime PRF material was derived for another policy or cycle window.
    #[error("privacy aggregate cycle PRF input does not match the governed release")]
    CyclePrfBindingMismatch,
    /// Runtime PRF material carries a commitment that does not match its hidden output.
    #[error("privacy aggregate cycle PRF commitment verification failed")]
    CyclePrfCommitmentMismatch,
    /// Hidden PRF output was supplied for a policy that does not use DP.
    #[error(
        "privacy aggregate cycle PRF output is forbidden when differential privacy is disabled"
    )]
    UnexpectedCyclePrfOutput,
    /// The governed epsilon/cap parameters would exceed the exact sampler expected-work policy.
    #[error(
        "privacy aggregate exact sampler parameters exceed the expected-work resource policy: epsilon={epsilon_numerator}/{epsilon_denominator}, sensitivity={sensitivity}"
    )]
    NoiseParametersExceedResourceLimit {
        /// Governed reduced epsilon numerator.
        epsilon_numerator: u64,
        /// Governed reduced epsilon denominator.
        epsilon_denominator: u64,
        /// Integer L1 sensitivity for the complete released metric vector.
        sensitivity: u64,
    },
    /// Governed release dimensions exceed the deterministic whole-cycle work budget.
    #[error(
        "privacy aggregate release expected XOF work {estimated_draws} exceeds limit {max_draws}"
    )]
    NoiseReleaseComplexityExceedsResourceLimit {
        /// Conservative expected 128-bit XOF draws for the complete release.
        estimated_draws: u128,
        /// Maximum accepted expected 128-bit XOF draws.
        max_draws: u128,
    },
    /// A worker-owned public metadata key was supplied by a caller.
    #[error("privacy aggregate metadata key `{key}` is reserved for the worker")]
    ReservedMetadataKey {
        /// Reserved key.
        key: &'static str,
    },
    /// Schedule configuration is invalid.
    #[error("privacy aggregate schedule field `{field}` is invalid")]
    InvalidSchedule {
        /// Field name.
        field: &'static str,
    },
    /// A source event predates the governed first cycle.
    #[error("privacy aggregate source event predates schedule activation")]
    EventBeforeScheduleActivation,
    /// No source events matched the requested cycle window.
    #[error("privacy aggregate cycle has no source events in the requested window")]
    NoSourceEvents,
    /// Source-event input exceeds the bounded V1 collection limit.
    #[error("privacy aggregate cycle has {count} source events; maximum is {max}")]
    TooManySourceEvents {
        /// Submitted event count.
        count: usize,
        /// Maximum accepted event count.
        max: usize,
    },
    /// A source event timestamp is outside the requested cycle window.
    #[error("privacy aggregate source event is outside the requested cycle window")]
    EventOutsideCycle,
    /// The same source event id appeared twice in the build input.
    #[error("privacy aggregate source event id is duplicated")]
    DuplicateSourceEvent,
    /// A source event references a population outside the governed universe.
    #[error("privacy aggregate source event is outside the governed population inventory")]
    PopulationOutsideInventory,
    /// A source event does not match the fixed governed metric schema.
    #[error("privacy aggregate source event does not match the governed metric schema")]
    MetricSchemaMismatch,
    /// A source event does not bind the active governed policy.
    #[error("privacy aggregate source event does not match the governed policy")]
    PolicyDigestMismatch,
    /// One subject was assigned to more than one population in a single cycle.
    #[error("privacy aggregate subject spans multiple population buckets in one cycle")]
    SubjectPopulationOverlap,
    /// All source buckets were suppressed.
    #[error("privacy aggregate cycle suppressed every source bucket")]
    AllBucketsSuppressed,
    /// Generated aggregate payload failed validation.
    #[error("generated privacy aggregate is invalid")]
    InvalidAggregate,
}

pub(crate) fn build_privacy_aggregates_from_source_events(
    cycle_start_unix: u64,
    cycle_end_unix: u64,
    config: &PrivacyAggregateCycleConfig,
    cycle_prf_input: Option<PrivacyCyclePrfInputV1>,
    events: &[PrivacyAggregateSourceEvent],
) -> Result<Vec<ModerationPrivacyAggregateV1>, PrivacyAggregateWorkerError> {
    config.validate()?;
    if config.privacy.per_subject_metric_cap.is_some() {
        let prf_input = cycle_prf_input
            .as_ref()
            .ok_or(PrivacyAggregateWorkerError::MissingCyclePrfOutput)?;
        prf_input.validate_for_release(config, cycle_start_unix, cycle_end_unix)?;
    } else if cycle_prf_input.is_some() {
        return Err(PrivacyAggregateWorkerError::UnexpectedCyclePrfOutput);
    }
    if events.is_empty() && matches!(config.privacy.mode, ModerationPrivacyModeV1::Suppression) {
        return Err(PrivacyAggregateWorkerError::NoSourceEvents);
    }
    if events.len() > PRIVACY_AGGREGATE_MAX_SOURCE_EVENTS_V1 {
        return Err(PrivacyAggregateWorkerError::TooManySourceEvents {
            count: events.len(),
            max: PRIVACY_AGGREGATE_MAX_SOURCE_EVENTS_V1,
        });
    }
    if cycle_start_unix == 0 || cycle_end_unix <= cycle_start_unix {
        return Err(PrivacyAggregateWorkerError::InvalidTimestamp {
            field: "cycle_window",
        });
    }

    let mut seen_events = BTreeSet::new();
    let mut subject_populations = BTreeMap::<[u8; 32], PopulationKey>::new();
    let mut groups = config
        .populations
        .iter()
        .map(|population| {
            (
                PopulationKey {
                    label: population.label.clone(),
                    digest: population.digest,
                },
                Vec::new(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for event in events {
        config.validate_source_event(event)?;
        if event.occurred_at_unix < cycle_start_unix || event.occurred_at_unix >= cycle_end_unix {
            return Err(PrivacyAggregateWorkerError::EventOutsideCycle);
        }
        if !seen_events.insert(event.event_id.clone()) {
            return Err(PrivacyAggregateWorkerError::DuplicateSourceEvent);
        }
        let population = PopulationKey {
            label: event.population_label.clone(),
            digest: event.population_digest,
        };
        let Some(bucket) = groups.get_mut(&population) else {
            return Err(PrivacyAggregateWorkerError::PopulationOutsideInventory);
        };
        if let Some(previous) = subject_populations.insert(event.subject_digest, population.clone())
            && previous != population
        {
            return Err(PrivacyAggregateWorkerError::SubjectPopulationOverlap);
        }
        bucket.push(event.clone());
    }

    let private_source_digest =
        canonical_private_source_digest(cycle_start_unix, cycle_end_unix, config, events)?;
    let suppression_threshold = config.privacy.suppression_threshold.unwrap_or(0);
    let vector_sensitivity = privacy_vector_sensitivity(config)?;
    let mut aggregates = Vec::new();
    for (population, mut bucket) in groups {
        bucket.sort_by(|left, right| left.event_id.cmp(&right.event_id));
        let below_threshold = distinct_subject_count(&bucket) < suppression_threshold;
        if matches!(config.privacy.mode, ModerationPrivacyModeV1::Suppression) && below_threshold {
            continue;
        }
        let aggregate = build_population_aggregate(
            cycle_start_unix,
            cycle_end_unix,
            config,
            cycle_prf_input.as_ref(),
            vector_sensitivity,
            private_source_digest,
            population,
            &bucket,
            below_threshold
                && matches!(
                    config.privacy.mode,
                    ModerationPrivacyModeV1::DifferentialPrivacyWithSuppression
                ),
        )?;
        aggregates.push(aggregate);
    }
    if aggregates.is_empty() {
        return Err(PrivacyAggregateWorkerError::AllBucketsSuppressed);
    }
    aggregates.sort_by(|left, right| left.aggregate_id.cmp(&right.aggregate_id));
    Ok(aggregates)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct PopulationKey {
    label: String,
    digest: [u8; 32],
}

fn build_population_aggregate(
    cycle_start_unix: u64,
    cycle_end_unix: u64,
    config: &PrivacyAggregateCycleConfig,
    cycle_prf_input: Option<&PrivacyCyclePrfInputV1>,
    vector_sensitivity: Option<u64>,
    private_source_digest: [u8; 32],
    population: PopulationKey,
    events: &[PrivacyAggregateSourceEvent],
    suppress_contributions: bool,
) -> Result<ModerationPrivacyAggregateV1, PrivacyAggregateWorkerError> {
    let metrics = if suppress_contributions {
        zero_metric_vector(&config.metrics)
    } else {
        clipped_population_metrics(
            events,
            &config.metrics,
            config.privacy.per_subject_metric_cap,
        )?
    };
    let aggregate_id = aggregate_id(&config.aggregate_id_prefix, &population);
    let published_metrics = metrics
        .into_iter()
        .map(|(key, (unit, value))| {
            let noised = apply_metric_noise(
                value,
                config,
                cycle_prf_input,
                vector_sensitivity,
                &aggregate_id,
                &key,
                &private_source_digest,
            )?;
            Ok(ModerationPrivacyAggregateMetricV1 {
                key,
                value: noised,
                unit,
            })
        })
        .collect::<Result<Vec<_>, PrivacyAggregateWorkerError>>()?;

    let noise_source = cycle_prf_input
        .map_or(ModerationPrivacyNoiseSourceV1::SuppressionOnly, |input| {
            ModerationPrivacyNoiseSourceV1::ThresholdPrf(input.commitment())
        });
    let aggregate = ModerationPrivacyAggregateV1 {
        version: MODERATION_PRIVACY_AGGREGATE_VERSION_V1,
        aggregate_id,
        window_start_unix: cycle_start_unix,
        window_end_unix: cycle_end_unix,
        generated_at_unix: cycle_end_unix,
        population_label: population.label,
        population_digest: population.digest,
        privacy: config.privacy,
        noise_source,
        metrics: published_metrics,
        policy_digest: config.policy_digest,
        metadata: config.metadata.clone(),
    };
    aggregate
        .validate()
        .map_err(|_| PrivacyAggregateWorkerError::InvalidAggregate)?;
    Ok(aggregate)
}

fn distinct_subject_count(events: &[PrivacyAggregateSourceEvent]) -> u64 {
    events
        .iter()
        .map(|event| event.subject_digest)
        .collect::<BTreeSet<_>>()
        .len() as u64
}

fn event_metrics_match_schema(
    metrics: &[PrivacyAggregateSourceMetric],
    schema: &[PrivacyAggregateMetricSchemaV1],
) -> bool {
    metrics.len() == schema.len()
        && metrics
            .iter()
            .zip(schema)
            .all(|(metric, expected)| metric.key == expected.key && metric.unit == expected.unit)
}

fn clipped_population_metrics(
    events: &[PrivacyAggregateSourceEvent],
    schema: &[PrivacyAggregateMetricSchemaV1],
    per_subject_metric_cap: Option<u64>,
) -> Result<BTreeMap<String, (String, u128)>, PrivacyAggregateWorkerError> {
    let mut subject_metrics = BTreeMap::<[u8; 32], BTreeMap<String, u128>>::new();
    for event in events {
        let per_subject = subject_metrics.entry(event.subject_digest).or_default();
        for metric in &event.metrics {
            let contribution = per_subject.entry(metric.key.clone()).or_default();
            *contribution = if let Some(cap) = per_subject_metric_cap {
                (*contribution)
                    .saturating_add(u128::from(metric.value))
                    .min(u128::from(cap))
            } else {
                (*contribution)
                    .checked_add(u128::from(metric.value))
                    .ok_or(PrivacyAggregateWorkerError::MetricArithmeticOverflow)?
            };
        }
    }

    let mut totals = schema
        .iter()
        .map(|metric| (metric.key.clone(), (metric.unit.clone(), 0_u128)))
        .collect::<BTreeMap<_, _>>();
    for metrics in subject_metrics.values() {
        for (key, contribution) in metrics {
            let total = totals
                .get_mut(key)
                .expect("unit inventory is built from the same metric rows");
            total.1 = total
                .1
                .checked_add(*contribution)
                .ok_or(PrivacyAggregateWorkerError::MetricArithmeticOverflow)?;
        }
    }
    Ok(totals)
}

fn zero_metric_vector(
    schema: &[PrivacyAggregateMetricSchemaV1],
) -> BTreeMap<String, (String, u128)> {
    schema
        .iter()
        .map(|metric| (metric.key.clone(), (metric.unit.clone(), 0)))
        .collect()
}

fn apply_metric_noise(
    value: u128,
    config: &PrivacyAggregateCycleConfig,
    cycle_prf_input: Option<&PrivacyCyclePrfInputV1>,
    vector_sensitivity: Option<u64>,
    aggregate_id: &str,
    metric_key: &str,
    private_source_digest: &[u8; 32],
) -> Result<u64, PrivacyAggregateWorkerError> {
    let Some(sensitivity) = vector_sensitivity else {
        return u64::try_from(value)
            .map_err(|_| PrivacyAggregateWorkerError::MetricArithmeticOverflow);
    };
    let prf_input = cycle_prf_input.ok_or(PrivacyAggregateWorkerError::MissingCyclePrfOutput)?;
    let epsilon_numerator = config.privacy.epsilon_numerator.ok_or(
        PrivacyAggregateWorkerError::InvalidPrivacyParameters {
            message: "epsilon_numerator is required for exact discrete-Laplace noise".to_string(),
        },
    )?;
    let epsilon_denominator = config.privacy.epsilon_denominator.ok_or(
        PrivacyAggregateWorkerError::InvalidPrivacyParameters {
            message: "epsilon_denominator is required for exact discrete-Laplace noise".to_string(),
        },
    )?;
    let mut hasher = blake3::Hasher::new_keyed(prf_input.output());
    hasher.update(DISCRETE_LAPLACE_NOISE_DOMAIN_V1);
    hasher.update(private_source_digest);
    hash_text(&mut hasher, aggregate_id);
    hash_text(&mut hasher, metric_key);
    let bounded_value = value.min(u128::from(u64::MAX)) as u64;
    ExactNoiseSampler::new(hasher.finalize_xof()).apply_discrete_laplace(
        bounded_value,
        epsilon_numerator,
        epsilon_denominator,
        sensitivity,
    )
}

struct ExactNoiseSampler {
    reader: blake3::OutputReader,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ExactDiscreteLaplaceLaw {
    continuation_numerator: u128,
    continuation_denominator: u128,
    zero_numerator: u128,
    zero_denominator: u128,
}

impl ExactNoiseSampler {
    fn new(reader: blake3::OutputReader) -> Self {
        Self { reader }
    }

    fn apply_discrete_laplace(
        &mut self,
        value: u64,
        epsilon_numerator: u64,
        epsilon_denominator: u64,
        sensitivity: u64,
    ) -> Result<u64, PrivacyAggregateWorkerError> {
        validate_discrete_laplace_parameters(epsilon_numerator, epsilon_denominator, sensitivity)?;
        // Let q = A/B where A = ΔD, B = ΔD + N, ε = N/D, and Δ is
        // sensitivity. The exact two-sided geometric law is
        //
        //   P(Z = z) = ((1-q)/(1+q)) q^|z|.
        //
        // Therefore P(Z = 0) = N/(2A + N). Conditional on nonzero noise, the
        // sign is uniform and |Z| is one plus a geometric variate whose
        // continuation probability is A/B. Every choice below is an exact
        // integer rejection sample; no floating-point approximation is used.
        // Its privacy loss is -ln(q) = ln(1 + N/(ΔD)) <= N/(ΔD), so the
        // governed rational ε conservatively bounds the complete vector.
        let law = exact_discrete_laplace_law(epsilon_numerator, epsilon_denominator, sensitivity)?;
        if self.uniform_below(law.zero_denominator)? < law.zero_numerator {
            return Ok(value);
        }
        let positive = self.uniform_below(2)? == 1;
        let mut adjusted = value;
        loop {
            // The public metric is a u64, so saturating post-processing maps
            // the entire still-unbounded latent tail to the reached boundary.
            // Returning at that point is exact post-processing, not tail
            // truncation: every possible continuation has the same output.
            adjusted = if positive {
                let Some(next) = adjusted.checked_add(1) else {
                    return Ok(u64::MAX);
                };
                next
            } else {
                let Some(next) = adjusted.checked_sub(1) else {
                    return Ok(0);
                };
                next
            };
            if self.uniform_below(law.continuation_denominator)? >= law.continuation_numerator {
                return Ok(adjusted);
            }
        }
    }

    fn uniform_below(
        &mut self,
        upper_exclusive: u128,
    ) -> Result<u128, PrivacyAggregateWorkerError> {
        if upper_exclusive == 0 {
            return Err(PrivacyAggregateWorkerError::InvalidPrivacyParameters {
                message: "exact sampler denominator must be non-zero".to_string(),
            });
        }
        // `wrapping_neg() % upper` equals 2^128 mod `upper`. Rejecting values
        // below that threshold leaves an exact multiple of `upper` candidates.
        let rejection_threshold = upper_exclusive.wrapping_neg() % upper_exclusive;
        loop {
            let mut bytes = [0_u8; 16];
            self.reader.fill(&mut bytes);
            let candidate = u128::from_le_bytes(bytes);
            if candidate >= rejection_threshold {
                return Ok(candidate % upper_exclusive);
            }
        }
    }
}

fn exact_discrete_laplace_law(
    epsilon_numerator: u64,
    epsilon_denominator: u64,
    sensitivity: u64,
) -> Result<ExactDiscreteLaplaceLaw, PrivacyAggregateWorkerError> {
    let continuation_numerator = u128::from(sensitivity)
        .checked_mul(u128::from(epsilon_denominator))
        .ok_or(PrivacyAggregateWorkerError::MetricArithmeticOverflow)?;
    let continuation_denominator = continuation_numerator
        .checked_add(u128::from(epsilon_numerator))
        .ok_or(PrivacyAggregateWorkerError::MetricArithmeticOverflow)?;
    let zero_denominator = continuation_numerator
        .checked_mul(2)
        .and_then(|twice| twice.checked_add(u128::from(epsilon_numerator)))
        .ok_or(PrivacyAggregateWorkerError::MetricArithmeticOverflow)?;
    Ok(ExactDiscreteLaplaceLaw {
        continuation_numerator,
        continuation_denominator,
        zero_numerator: u128::from(epsilon_numerator),
        zero_denominator,
    })
}

fn validate_discrete_laplace_parameters(
    epsilon_numerator: u64,
    epsilon_denominator: u64,
    sensitivity: u64,
) -> Result<(), PrivacyAggregateWorkerError> {
    let sensitivity_numerator = u128::from(sensitivity)
        .checked_mul(u128::from(epsilon_denominator))
        .ok_or(PrivacyAggregateWorkerError::MetricArithmeticOverflow)?;
    let maximum_numerator = u128::from(epsilon_numerator)
        .checked_mul(MAX_DISCRETE_LAPLACE_EXPECTED_CONTINUATIONS_V1)
        .ok_or(PrivacyAggregateWorkerError::MetricArithmeticOverflow)?;
    if epsilon_numerator == 0
        || epsilon_denominator == 0
        || sensitivity == 0
        || sensitivity_numerator > maximum_numerator
    {
        return Err(
            PrivacyAggregateWorkerError::NoiseParametersExceedResourceLimit {
                epsilon_numerator,
                epsilon_denominator,
                sensitivity,
            },
        );
    }
    Ok(())
}

fn validate_release_noise_complexity(
    population_count: usize,
    metric_count: usize,
    epsilon_numerator: u64,
    epsilon_denominator: u64,
    sensitivity: u64,
) -> Result<(), PrivacyAggregateWorkerError> {
    if epsilon_numerator == 0 || epsilon_denominator == 0 || sensitivity == 0 {
        return Err(PrivacyAggregateWorkerError::InvalidPrivacyParameters {
            message: "exact sampler parameters must be positive".to_string(),
        });
    }
    let continuation_numerator = u128::from(sensitivity)
        .checked_mul(u128::from(epsilon_denominator))
        .ok_or(PrivacyAggregateWorkerError::MetricArithmeticOverflow)?;
    let numerator = continuation_numerator
        .checked_add(u128::from(epsilon_numerator).saturating_sub(1))
        .ok_or(PrivacyAggregateWorkerError::MetricArithmeticOverflow)?;
    let expected_continuations_ceiling = numerator / u128::from(epsilon_numerator);
    // One zero-mass choice, then (conservatively, even when zero was chosen)
    // one sign choice and one terminating geometric choice. Exact u128
    // rejection sampling accepts more than half of all candidates, so fewer
    // than two 128-bit XOF draws are expected for every uniform choice.
    let expected_xof_draws_per_coordinate = expected_continuations_ceiling
        .checked_add(3)
        .and_then(|draws| draws.checked_mul(2))
        .ok_or(PrivacyAggregateWorkerError::MetricArithmeticOverflow)?;
    let coordinate_count = u128::try_from(population_count)
        .ok()
        .and_then(|populations| {
            u128::try_from(metric_count)
                .ok()
                .and_then(|metrics| populations.checked_mul(metrics))
        })
        .ok_or(PrivacyAggregateWorkerError::MetricArithmeticOverflow)?;
    let estimated_draws = coordinate_count
        .checked_mul(expected_xof_draws_per_coordinate)
        .ok_or(PrivacyAggregateWorkerError::MetricArithmeticOverflow)?;
    if estimated_draws > MAX_DISCRETE_LAPLACE_RELEASE_EXPECTED_DRAWS_V1 {
        return Err(
            PrivacyAggregateWorkerError::NoiseReleaseComplexityExceedsResourceLimit {
                estimated_draws,
                max_draws: MAX_DISCRETE_LAPLACE_RELEASE_EXPECTED_DRAWS_V1,
            },
        );
    }
    Ok(())
}

fn noise_randomness_commitment(request_binding: [u8; 32], prf_output: &[u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(NOISE_RANDOMNESS_COMMITMENT_DOMAIN_V1);
    hasher.update(&request_binding);
    hasher.update(prf_output);
    *hasher.finalize().as_bytes()
}

pub(crate) fn canonical_private_source_digest(
    cycle_start_unix: u64,
    cycle_end_unix: u64,
    config: &PrivacyAggregateCycleConfig,
    events: &[PrivacyAggregateSourceEvent],
) -> Result<[u8; 32], PrivacyAggregateWorkerError> {
    config.validate()?;
    if cycle_start_unix == 0 || cycle_end_unix <= cycle_start_unix {
        return Err(PrivacyAggregateWorkerError::InvalidTimestamp {
            field: "cycle_window",
        });
    }
    if events.len() > PRIVACY_AGGREGATE_MAX_SOURCE_EVENTS_V1 {
        return Err(PrivacyAggregateWorkerError::TooManySourceEvents {
            count: events.len(),
            max: PRIVACY_AGGREGATE_MAX_SOURCE_EVENTS_V1,
        });
    }
    let mut ordered = events.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| left.event_id.cmp(&right.event_id));
    let mut previous_event_id = None;
    let mut subject_populations = BTreeMap::new();
    for event in &ordered {
        config.validate_source_event(event)?;
        if event.occurred_at_unix < cycle_start_unix || event.occurred_at_unix >= cycle_end_unix {
            return Err(PrivacyAggregateWorkerError::EventOutsideCycle);
        }
        if previous_event_id.is_some_and(|previous| previous == event.event_id.as_str()) {
            return Err(PrivacyAggregateWorkerError::DuplicateSourceEvent);
        }
        previous_event_id = Some(event.event_id.as_str());
        let population = (&event.population_label, event.population_digest);
        if let Some(previous) = subject_populations.insert(event.subject_digest, population)
            && previous != population
        {
            return Err(PrivacyAggregateWorkerError::SubjectPopulationOverlap);
        }
    }

    let mut hasher = blake3::Hasher::new();
    hasher.update(PRIVATE_SOURCE_DIGEST_DOMAIN_V1);
    hasher.update(&config.query_id);
    hasher.update(&cycle_start_unix.to_le_bytes());
    hasher.update(&cycle_end_unix.to_le_bytes());
    hasher.update(&privacy_population_inventory_digest(&config.populations));
    hasher.update(&privacy_metric_schema_digest(&config.metrics));
    hasher.update(&config.policy_digest);
    hash_text(&mut hasher, &config.aggregate_id_prefix);
    hash_privacy_parameters(&mut hasher, config.privacy);
    hasher.update(&(config.metadata.len() as u64).to_le_bytes());
    for item in &config.metadata {
        hash_text(&mut hasher, &item.key);
        hash_text(&mut hasher, &item.value);
    }
    hasher.update(&(ordered.len() as u64).to_le_bytes());
    for event in ordered {
        hash_text(&mut hasher, &event.event_id);
        hasher.update(&event.occurred_at_unix.to_le_bytes());
        hash_text(&mut hasher, &event.population_label);
        hasher.update(&event.population_digest);
        hasher.update(&event.subject_digest);
        hasher.update(&event.policy_digest);
        hasher.update(&(event.metrics.len() as u64).to_le_bytes());
        for metric in &event.metrics {
            hash_text(&mut hasher, &metric.key);
            hash_text(&mut hasher, &metric.unit);
            hasher.update(&metric.value.to_le_bytes());
        }
    }
    Ok(*hasher.finalize().as_bytes())
}

fn hash_privacy_parameters(hasher: &mut blake3::Hasher, privacy: ModerationPrivacyParametersV1) {
    hasher.update(&privacy.version.to_le_bytes());
    hasher.update(privacy_mode_label(privacy.mode).as_bytes());
    hash_option_u64(hasher, privacy.epsilon_numerator);
    hash_option_u64(hasher, privacy.epsilon_denominator);
    hash_option_u64(hasher, privacy.delta_ppb);
    hash_option_u64(hasher, privacy.per_subject_metric_cap);
    hash_option_u64(hasher, privacy.suppression_threshold);
}

fn hash_option_u64(hasher: &mut blake3::Hasher, value: Option<u64>) {
    match value {
        Some(value) => {
            hasher.update(&[1]);
            hasher.update(&value.to_le_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    };
}

fn aggregate_id(prefix: &str, population: &PopulationKey) -> String {
    let digest_prefix = hex::encode(&population.digest[..8]);
    format!(
        "{}-{}-{}",
        sanitize_label(prefix),
        sanitize_label(&population.label),
        digest_prefix
    )
}

fn validate_source_metrics(
    metrics: &[PrivacyAggregateSourceMetric],
) -> Result<(), PrivacyAggregateWorkerError> {
    if metrics.is_empty() {
        return Err(PrivacyAggregateWorkerError::SourceMetricsMissing);
    }
    if metrics.len() > MODERATION_PRIVACY_MAX_METRICS_V1 {
        return Err(PrivacyAggregateWorkerError::TooManySourceMetrics {
            count: metrics.len(),
            max: MODERATION_PRIVACY_MAX_METRICS_V1,
        });
    }
    let mut last_key: Option<&str> = None;
    let mut seen = BTreeSet::new();
    for metric in metrics {
        require_public_text("metrics.key", &metric.key)?;
        require_public_text("metrics.unit", &metric.unit)?;
        if let Some(last) = last_key
            && last > metric.key.as_str()
        {
            return Err(PrivacyAggregateWorkerError::SourceMetricKeysUnsorted);
        }
        if !seen.insert(metric.key.as_str()) {
            return Err(PrivacyAggregateWorkerError::DuplicateSourceMetricKey);
        }
        last_key = Some(metric.key.as_str());
    }
    Ok(())
}

fn validate_metadata(
    metadata: &[ModerationLedgerMetadataV1],
) -> Result<(), PrivacyAggregateWorkerError> {
    let mut last_key: Option<&str> = None;
    let mut seen = BTreeSet::new();
    for item in metadata {
        require_public_text("metadata.key", &item.key)?;
        require_public_text("metadata.value", &item.value)?;
        if let Some(last) = last_key
            && last > item.key.as_str()
        {
            return Err(PrivacyAggregateWorkerError::MetadataKeysUnsorted);
        }
        if !seen.insert(item.key.as_str()) {
            return Err(PrivacyAggregateWorkerError::DuplicateMetadataKey);
        }
        last_key = Some(item.key.as_str());
    }
    Ok(())
}

fn require_nonzero32(
    field: &'static str,
    value: &[u8; 32],
) -> Result<(), PrivacyAggregateWorkerError> {
    if value.iter().all(|byte| *byte == 0) {
        return Err(PrivacyAggregateWorkerError::MissingDigest { field });
    }
    Ok(())
}

fn require_public_text(
    field: &'static str,
    value: &str,
) -> Result<(), PrivacyAggregateWorkerError> {
    if value.trim().is_empty() || value.contains('\0') {
        return Err(PrivacyAggregateWorkerError::MissingText { field });
    }
    if value.len() > MODERATION_LEDGER_MAX_PUBLIC_TEXT_BYTES_V1 {
        return Err(PrivacyAggregateWorkerError::TextTooLong {
            field,
            max: MODERATION_LEDGER_MAX_PUBLIC_TEXT_BYTES_V1,
        });
    }
    Ok(())
}

fn hash_text(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
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
    if out.is_empty() {
        "aggregate".to_string()
    } else {
        out
    }
}

fn proof_token_moderation_action_code(action: ProofTokenModerationAction) -> u8 {
    match action {
        ProofTokenModerationAction::Block => 0,
        ProofTokenModerationAction::Quarantine => 1,
        ProofTokenModerationAction::RateLimit => 2,
        ProofTokenModerationAction::Redirect => 3,
        ProofTokenModerationAction::Custom(code) => code,
    }
}

fn validate_gar_enforcement_receipt(
    receipt: &GarEnforcementReceiptV1,
) -> Result<(), TransparencySourceEntryAdapterError> {
    if receipt.receipt_id == [0; 16] {
        return Err(TransparencySourceEntryAdapterError::InvalidGarReceipt {
            message: "receipt_id must be non-zero".to_string(),
        });
    }
    if receipt.triggered_at_unix == 0 {
        return Err(TransparencySourceEntryAdapterError::InvalidGarReceipt {
            message: "triggered_at_unix must be non-zero".to_string(),
        });
    }
    for (field, value) in [
        ("gar_name", receipt.gar_name.as_str()),
        ("canonical_host", receipt.canonical_host.as_str()),
        ("reason", receipt.reason.as_str()),
    ] {
        if value.trim().is_empty() || value.contains('\0') {
            return Err(TransparencySourceEntryAdapterError::InvalidGarReceipt {
                message: format!("{field} must be non-empty public text"),
            });
        }
    }
    if let Some(policy_digest) = &receipt.policy_digest
        && policy_digest.iter().all(|byte| *byte == 0)
    {
        return Err(TransparencySourceEntryAdapterError::InvalidGarReceipt {
            message: "policy_digest must be non-zero when present".to_string(),
        });
    }
    if let Some(expires_at_unix) = receipt.expires_at_unix
        && expires_at_unix <= receipt.triggered_at_unix
    {
        return Err(TransparencySourceEntryAdapterError::InvalidGarReceipt {
            message: "expires_at_unix must be greater than triggered_at_unix".to_string(),
        });
    }
    for (field, values) in [
        ("evidence_uris", receipt.evidence_uris.as_slice()),
        ("labels", receipt.labels.as_slice()),
    ] {
        for value in values {
            if value.trim().is_empty() || value.contains('\0') {
                return Err(TransparencySourceEntryAdapterError::InvalidGarReceipt {
                    message: format!("{field} entries must be non-empty public text"),
                });
            }
        }
    }
    Ok(())
}

fn validate_adapter_source_entry(
    entry: TransparencyLedgerSourceEntry,
) -> Result<TransparencyLedgerSourceEntry, TransparencySourceEntryAdapterError> {
    entry.validate().map_err(
        |err| TransparencySourceEntryAdapterError::InvalidSourceEntry {
            message: err.to_string(),
        },
    )?;
    Ok(entry)
}

fn canonical_payload_digest<T: NoritoEncode>(
    payload_kind: &'static str,
    value: &T,
) -> Result<[u8; 32], TransparencySourceEntryAdapterError> {
    let encoded = norito::to_bytes(value).map_err(|err| {
        TransparencySourceEntryAdapterError::CanonicalEncode {
            payload_kind,
            message: err.to_string(),
        }
    })?;
    Ok(*blake3::hash(&encoded).as_bytes())
}

fn source_subject_digest(payload_kind: &str, subject: &str, payload_digest: &[u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(SOURCE_ENTRY_SUBJECT_DIGEST_DOMAIN_V1);
    hash_text(&mut hasher, payload_kind);
    hash_text(&mut hasher, subject);
    hasher.update(payload_digest);
    *hasher.finalize().as_bytes()
}

fn source_summary_digest(payload_kind: &str, metadata: &[ModerationLedgerMetadataV1]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(SOURCE_ENTRY_SUMMARY_DIGEST_DOMAIN_V1);
    hash_text(&mut hasher, payload_kind);
    hasher.update(&(metadata.len() as u64).to_le_bytes());
    for item in metadata {
        hash_text(&mut hasher, &item.key);
        hash_text(&mut hasher, &item.value);
    }
    *hasher.finalize().as_bytes()
}

fn metadata_vec(metadata: BTreeMap<String, String>) -> Vec<ModerationLedgerMetadataV1> {
    metadata
        .into_iter()
        .map(|(key, value)| ModerationLedgerMetadataV1 { key, value })
        .collect()
}

fn reserve_source_payload_digest(payload_kind: &str, parts: &[(&str, &[u8])]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(RESERVE_SOURCE_PAYLOAD_DIGEST_DOMAIN_V1);
    hash_text(&mut hasher, payload_kind);
    hasher.update(&(parts.len() as u64).to_le_bytes());
    for (label, value) in parts {
        hash_text(&mut hasher, label);
        hasher.update(&(value.len() as u64).to_le_bytes());
        hasher.update(value);
    }
    *hasher.finalize().as_bytes()
}

fn reserve_private_field_digest_hex(label: &str, value: &[u8]) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(RESERVE_PRIVATE_FIELD_DIGEST_DOMAIN_V1);
    hash_text(&mut hasher, label);
    hasher.update(&(value.len() as u64).to_le_bytes());
    hasher.update(value);
    hex::encode(hasher.finalize().as_bytes())
}

fn validate_reserve_source_id(
    field: &'static str,
    value: &[u8; 32],
    err: fn(String) -> TransparencySourceEntryAdapterError,
) -> Result<(), TransparencySourceEntryAdapterError> {
    if value.iter().all(|byte| *byte == 0) {
        return Err(err(format!("{field} must be non-zero")));
    }
    Ok(())
}

fn reserve_lifecycle_source_error(
    message: impl Into<String>,
) -> TransparencySourceEntryAdapterError {
    TransparencySourceEntryAdapterError::InvalidReserveLifecycleEvent {
        message: message.into(),
    }
}

fn reserve_movement_source_error(
    message: impl Into<String>,
) -> TransparencySourceEntryAdapterError {
    TransparencySourceEntryAdapterError::InvalidReserveMovement {
        message: message.into(),
    }
}

fn reserve_appeal_source_error(message: impl Into<String>) -> TransparencySourceEntryAdapterError {
    TransparencySourceEntryAdapterError::InvalidReserveAppeal {
        message: message.into(),
    }
}

fn reserve_lifecycle_policy_source_error(
    message: impl Into<String>,
) -> TransparencySourceEntryAdapterError {
    TransparencySourceEntryAdapterError::InvalidReserveLifecyclePolicy {
        message: message.into(),
    }
}

fn reserve_lifecycle_stage_label(stage: ReserveLifecycleStage) -> &'static str {
    match stage {
        ReserveLifecycleStage::Active => "active",
        ReserveLifecycleStage::Warning => "warning",
        ReserveLifecycleStage::Grace => "grace",
        ReserveLifecycleStage::Delinquent => "delinquent",
        ReserveLifecycleStage::Default => "default",
    }
}

fn reserve_movement_kind_label(kind: ReserveMovementKind) -> &'static str {
    match kind {
        ReserveMovementKind::TopUp => "top_up",
        ReserveMovementKind::Withdrawal => "withdrawal",
    }
}

fn reserve_movement_custody_status_label(status: ReserveMovementCustodyStatus) -> &'static str {
    status.label()
}

fn reserve_appeal_status_label(status: ReserveAppealStatus) -> &'static str {
    status.label()
}

fn unix_ms_to_secs(unix_ms: u64) -> Result<u64, String> {
    let unix = unix_ms / 1_000;
    if unix == 0 {
        return Err("generated timestamp must be at least one UNIX second".to_string());
    }
    Ok(unix)
}

fn hex_32_to_digest(
    value: &str,
    field: &'static str,
) -> Result<[u8; 32], TransparencySourceEntryAdapterError> {
    let decoded = hex::decode(value).map_err(|err| {
        TransparencySourceEntryAdapterError::InvalidAppealFinanceSettlementReceipt {
            message: format!("{field} must be lowercase 32-byte hex: {err}"),
        }
    })?;
    let digest: [u8; 32] = decoded.try_into().map_err(|_| {
        TransparencySourceEntryAdapterError::InvalidAppealFinanceSettlementReceipt {
            message: format!("{field} must be 32 bytes"),
        }
    })?;
    if digest.iter().all(|byte| *byte == 0) {
        return Err(
            TransparencySourceEntryAdapterError::InvalidAppealFinanceSettlementReceipt {
                message: format!("{field} must be non-zero"),
            },
        );
    }
    Ok(digest)
}

fn gar_enforcement_action_label(action: &GarEnforcementActionV1) -> &'static str {
    match action {
        GarEnforcementActionV1::PurgeStaticZone => "purge_static_zone",
        GarEnforcementActionV1::CacheBypass => "cache_bypass",
        GarEnforcementActionV1::TtlOverride => "ttl_override",
        GarEnforcementActionV1::RateLimitOverride => "rate_limit_override",
        GarEnforcementActionV1::GeoFence => "geo_fence",
        GarEnforcementActionV1::LegalHold => "legal_hold",
        GarEnforcementActionV1::Moderation => "moderation",
        GarEnforcementActionV1::AuditNotice => "audit_notice",
        GarEnforcementActionV1::Custom(_) => "custom",
    }
}

fn validate_transparency_kind(
    kind: &ModerationLedgerEntryKindV1,
) -> Result<(), TransparencyLedgerIngestError> {
    if let ModerationLedgerEntryKindV1::Custom(slug) = kind {
        require_transparency_public_text("kind.custom", slug)?;
    }
    Ok(())
}

fn validate_transparency_metadata(
    metadata: &[ModerationLedgerMetadataV1],
) -> Result<(), TransparencyLedgerIngestError> {
    let mut last_key: Option<&str> = None;
    let mut seen = BTreeSet::new();
    for item in metadata {
        require_transparency_public_text("metadata.key", &item.key)?;
        require_transparency_public_text("metadata.value", &item.value)?;
        if let Some(last) = last_key
            && last > item.key.as_str()
        {
            return Err(TransparencyLedgerIngestError::MetadataKeysUnsorted);
        }
        if !seen.insert(item.key.as_str()) {
            return Err(TransparencyLedgerIngestError::DuplicateMetadataKey {
                key: item.key.clone(),
            });
        }
        last_key = Some(item.key.as_str());
    }
    Ok(())
}

fn require_transparency_nonzero16(
    field: &'static str,
    value: &[u8; 16],
) -> Result<(), TransparencyLedgerIngestError> {
    if value.iter().all(|byte| *byte == 0) {
        return Err(TransparencyLedgerIngestError::MissingDigest { field });
    }
    Ok(())
}

fn require_transparency_nonzero32(
    field: &'static str,
    value: &[u8; 32],
) -> Result<(), TransparencyLedgerIngestError> {
    if value.iter().all(|byte| *byte == 0) {
        return Err(TransparencyLedgerIngestError::MissingDigest { field });
    }
    Ok(())
}

fn require_transparency_public_text(
    field: &'static str,
    value: &str,
) -> Result<(), TransparencyLedgerIngestError> {
    if value.trim().is_empty() || value.contains('\0') {
        return Err(TransparencyLedgerIngestError::MissingText { field });
    }
    Ok(())
}

fn privacy_mode_label(mode: ModerationPrivacyModeV1) -> &'static str {
    match mode {
        ModerationPrivacyModeV1::DifferentialPrivacy => "differential_privacy",
        ModerationPrivacyModeV1::Suppression => "suppression",
        ModerationPrivacyModeV1::DifferentialPrivacyWithSuppression => {
            "differential_privacy_with_suppression"
        }
    }
}

fn system_time_to_unix_secs(
    time: SystemTime,
    field: &'static str,
) -> Result<u64, ProofTokenIssuanceIngestError> {
    time.duration_since(UNIX_EPOCH)
        .map_err(|_| ProofTokenIssuanceIngestError::TimestampOutOfRange { field })
        .map(|duration| duration.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn xor(value: &str) -> sorafs_manifest::deal::XorQuantity {
        value.parse().expect("canonical XOR quantity")
    }

    const VALID_PROOF_TOKEN_SIGNER_HEX: &str =
        "f4bfda67d38a409557e4a910dbdf0a862ee5aa6cf6c2284aa38b0b82c4f16532";
    const VALID_PROOF_TOKEN_B64: &str = "U0ZHVAEBAgAAAABrSdIeAAAAAGtLI55hYWFhYWFhYWFhYWFhYWFhAAIAD2RlbnlsaXN0L2dsb2JhbAANZ2FyL3BvbGljeS80MmRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkAEDHmshANx2cvkpmh1mCkrE94PJ6hL0A0qX4vQ-T3rWyTUKZG6uGoYM2sXbL36cYTahpsgcQ35z4R9bb1owinokB";

    fn transparency_source_entry(
        event_id: &str,
        occurred_at_unix: u64,
        kind: ModerationLedgerEntryKindV1,
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

    fn valid_signer_key() -> [u8; 32] {
        hex::decode(VALID_PROOF_TOKEN_SIGNER_HEX)
            .expect("valid signer hex")
            .try_into()
            .expect("signer key length")
    }

    fn gar_operator_account() -> iroha_data_model::account::AccountId {
        iroha_data_model::account::AccountId::parse_encoded(
            "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
        )
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .expect("account id")
    }

    fn gar_receipt_fixture(action: GarEnforcementActionV1) -> GarEnforcementReceiptV1 {
        GarEnforcementReceiptV1 {
            receipt_id: *b"gar-receipt-0001",
            gar_name: "docs.sora".to_string(),
            canonical_host: "docs.gateway.sora.net".to_string(),
            action,
            triggered_at_unix: 1_800_000_010,
            expires_at_unix: Some(1_800_086_410),
            policy_version: Some("2026-q2".to_string()),
            policy_digest: Some([0xAB; 32]),
            operator: gar_operator_account(),
            reason: "Guardian freeze window".to_string(),
            notes: Some("Escalated during SFM-4c drill".to_string()),
            evidence_uris: vec!["sora://gar/receipts/docs/0001".to_string()],
            labels: vec!["guardian-freeze".to_string(), "sfm4c".to_string()],
        }
    }

    fn moderation_governance_event_fixture() -> SoraFsModerationBallotGovernanceEventV1 {
        use sorafs_manifest::{
            SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
            SoraFsModerationBallotGovernanceEventKindV1, SoraFsModerationBallotGovernanceTallyV1,
            SoraFsModerationVoteChoiceV1, SoraFsModerationVoteCountsV1,
        };

        SoraFsModerationBallotGovernanceEventV1 {
            version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
            sequence: 7,
            kind: SoraFsModerationBallotGovernanceEventKindV1::BallotTallied,
            generated_at_unix_ms: 1_800_000_030_000,
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
                tallied_at_unix_ms: 1_800_000_030_000,
            }),
            challenge: None,
        }
    }

    fn appeal_finance_report_fixture() -> SoraFsAppealFinanceReportV1 {
        use sorafs_manifest::{
            SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1, SoraFsAppealFinanceAccountFlowV1,
            SoraFsAppealFinanceJurorPayoutV1, SoraFsAppealFinanceOutcomeV1,
        };

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

    fn appeal_finance_settlement_receipt_fixture() -> SoraFsAppealFinanceSettlementReceiptV1 {
        use sorafs_manifest::{
            SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1, SoraFsAppealFinanceOutcomeV1,
        };

        SoraFsAppealFinanceSettlementReceiptV1 {
            version: SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1,
            receipt_id: [0x52; 16],
            case_id: "case-42".to_string(),
            round_id: Some("round-1".to_string()),
            generated_at_unix_ms: 1_800_000_032_000,
            appeal_finance_config_version: "baseline-v1".to_string(),
            outcome: SoraFsAppealFinanceOutcomeV1::Frivolous,
            escrow_id_hex: "11".repeat(32),
            payer_account: "payer-account".to_string(),
            destination_account: "escrow-account".to_string(),
            release_authority_account: Some("release-authority".to_string()),
            submitted_step: "treasury-release".to_string(),
            required_authority: "release-authority".to_string(),
            amount_xor: xor("25"),
            tx_hash_hex: "22".repeat(32),
            reconciliation_digest_hex: "33".repeat(32),
            reconciliation_status: "pending".to_string(),
            observed_lifecycle_status: "funded".to_string(),
            observed_remaining_xor: xor("420"),
            deposit_xor: xor("420"),
            refund_xor: xor("0"),
            treasury_xor: xor("25"),
            held_xor: xor("395"),
            panel_size: 3,
            configured_signer_count: 2,
        }
    }

    fn reserve_quote_fixture() -> iroha_data_model::sorafs::reserve::ReserveQuote {
        iroha_data_model::sorafs::reserve::ReservePolicyV1::default()
            .quote(
                iroha_data_model::sorafs::pin_registry::StorageClass::Hot,
                10,
                iroha_data_model::sorafs::reserve::ReserveDuration::Monthly,
                iroha_data_model::sorafs::reserve::ReserveTier::TierA,
                sorafs_manifest::deal::XorQuantity::zero(),
            )
            .expect("reserve quote")
    }

    fn reserve_lifecycle_event_fixture() -> ReserveLifecycleEvent {
        let quote = reserve_quote_fixture();
        let lifecycle = quote
            .lifecycle_projection(3, 7, 30)
            .expect("lifecycle projection");
        ReserveLifecycleEvent {
            sequence: 3,
            provider_id: [0x44; 32],
            previous_stage: Some(ReserveLifecycleStage::Warning),
            current_stage: lifecycle.stage,
            observed_at_unix: 1_800_000_040,
            ledger: quote.ledger_projection().expect("ledger projection"),
            lifecycle,
            grace_period_days: 7,
            default_after_days: 30,
            applied_policy_id: None,
            applied_appeal_id: None,
        }
    }

    fn reserve_movement_record_fixture() -> ReserveMovementRecord {
        ReserveMovementRecord {
            sequence: 4,
            movement_id: [0x45; 32],
            provider_id: [0x44; 32],
            provider_account: b"provider-account".to_vec(),
            reserve_account: b"reserve-account".to_vec(),
            asset_definition_id: b"xor#sora".to_vec(),
            kind: ReserveMovementKind::TopUp,
            amount: sorafs_manifest::deal::XorQuantity::try_from_micro(100)
                .expect("legacy micro-XOR value is representable"),
            balance_after: sorafs_manifest::deal::XorQuantity::try_from_micro(100)
                .expect("legacy micro-XOR value is representable"),
            confirmed_balance_after: sorafs_manifest::deal::XorQuantity::zero(),
            idempotency_key: "movement-1".to_string(),
            observed_at_unix: 1_800_000_050,
            custody_status: ReserveMovementCustodyStatus::Submitted,
            custody_tx_hash_hex: Some(hex::encode([0x55; 32])),
            custody_updated_at_unix: Some(1_800_000_060),
        }
    }

    fn reserve_appeal_record_fixture() -> ReserveAppealRecord {
        ReserveAppealRecord {
            sequence: 5,
            appeal_id: [0x46; 32],
            provider_id: [0x44; 32],
            provider_account: b"provider-account".to_vec(),
            requested_stage: Some(ReserveLifecycleStage::Grace),
            reason: "provider asks for grace while custody tx settles".to_string(),
            evidence_digest_hex: Some(hex::encode([0x56; 32])),
            idempotency_key: "appeal-1".to_string(),
            status: ReserveAppealStatus::Accepted,
            opened_at_unix: 1_800_000_070,
            decision_account: Some(b"reserve-authority".to_vec()),
            decision_rationale: Some("custody evidence confirmed".to_string()),
            decided_at_unix: Some(1_800_000_080),
        }
    }

    fn reserve_lifecycle_policy_record_fixture() -> ReserveLifecyclePolicyRecord {
        ReserveLifecyclePolicyRecord {
            sequence: 6,
            policy_id: [0x47; 32],
            authority_account: b"reserve-authority".to_vec(),
            grace_period_days: 7,
            default_after_days: 30,
            effective_at_unix: 1_800_000_090,
            reason: "mainnet rollout baseline".to_string(),
            idempotency_key: "policy-1".to_string(),
            observed_at_unix: 1_800_000_085,
        }
    }

    fn privacy_config() -> PrivacyAggregateCycleConfig {
        PrivacyAggregateCycleConfig {
            query_id: [0xB0; 32],
            first_cycle_start_unix: 100,
            cycle_seconds: 100,
            aggregate_id_prefix: "sfm4c-cycle".to_string(),
            populations: vec![PrivacyAggregatePopulationV1 {
                label: "jurisdiction-a".to_string(),
                digest: [0xA0; 32],
            }],
            metrics: vec![PrivacyAggregateMetricSchemaV1 {
                key: "moderation_actions".to_string(),
                unit: "count".to_string(),
            }],
            privacy: ModerationPrivacyParametersV1 {
                version:
                    iroha_data_model::sorafs::transparency::MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
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

    fn privacy_prf_input(output: [u8; 32]) -> PrivacyCyclePrfInputV1 {
        let config = privacy_config();
        let request = PrivacyCyclePrfRequestV1::new(
            config.query_id,
            [0xC0; 32],
            privacy_population_inventory_digest(&config.populations),
            privacy_metric_schema_digest(&config.metrics),
            PrivacyAggregateCycleWindow {
                cycle_start_unix: 100,
                cycle_end_unix: 200,
                due_at_unix: 200,
            },
        )
        .expect("canonical test PRF request");
        PrivacyCyclePrfInputV1::new(
            request,
            PrivacyCyclePrfOutputV1::new(output).expect("valid test PRF output"),
        )
    }

    fn privacy_event(event_id: &str, occurred_at_unix: u64) -> PrivacyAggregateSourceEvent {
        PrivacyAggregateSourceEvent {
            event_id: event_id.to_string(),
            occurred_at_unix,
            population_label: "jurisdiction-a".to_string(),
            population_digest: [0xA0; 32],
            subject_digest: *blake3::hash(event_id.as_bytes()).as_bytes(),
            metrics: vec![PrivacyAggregateSourceMetric {
                key: "moderation_actions".to_string(),
                value: 1,
                unit: "count".to_string(),
            }],
            policy_digest: [0xC0; 32],
        }
    }

    #[test]
    fn exact_discrete_laplace_is_deterministic_and_context_bound() {
        fn sample(context: &[u8]) -> i128 {
            let mut hasher = blake3::Hasher::new_keyed(&[0x5A; 32]);
            hasher.update(DISCRETE_LAPLACE_NOISE_DOMAIN_V1);
            hasher.update(context);
            let center = u64::MAX / 2;
            i128::from(
                ExactNoiseSampler::new(hasher.finalize_xof())
                    .apply_discrete_laplace(center, 4, 5, 1)
                    .expect("exact sample"),
            ) - i128::from(center)
        }

        let sample_a = sample(b"aggregate-a/metric-a");
        assert_eq!(sample_a, sample(b"aggregate-a/metric-a"));
        assert_ne!(
            (sample_a, sample(b"aggregate-a/metric-b")),
            (
                sample(b"aggregate-b/metric-a"),
                sample(b"aggregate-b/metric-b")
            ),
            "independent contexts must not collapse to one repeated sample pair"
        );
    }

    #[test]
    fn exact_discrete_laplace_normalizes_zero_before_sampling_sign() {
        let law = exact_discrete_laplace_law(1, 1, 1).expect("representable exact law");

        // q = 1/2 gives P(0) = (1-q)/(1+q) = 1/3, followed by a
        // conditional fair sign and P(|Z|=k | Z!=0) = (1-q)q^(k-1).
        // Sampling a sign after a geometric magnitude starting at zero would
        // instead assign P(0)=1-q=1/2 and is not this distribution.
        assert_eq!(law.continuation_numerator, 1);
        assert_eq!(law.continuation_denominator, 2);
        assert_eq!(law.zero_numerator, 1);
        assert_eq!(law.zero_denominator, 3);
        assert_ne!(
            (law.zero_numerator, law.zero_denominator,),
            (
                law.continuation_denominator - law.continuation_numerator,
                law.continuation_denominator,
            )
        );
    }

    #[test]
    fn exact_discrete_laplace_has_symmetric_geometric_tail_structure() {
        let mut zero = 0_u64;
        let mut positive = 0_u64;
        let mut negative = 0_u64;
        let mut tail_one = 0_u64;
        let mut tail_two = 0_u64;
        let mut tail_three = 0_u64;
        let mut signed_sum = 0_i128;
        const SAMPLE_COUNT: u64 = 8_192;

        for sample_index in 0..SAMPLE_COUNT {
            let mut hasher = blake3::Hasher::new_keyed(&[0xA5; 32]);
            hasher.update(DISCRETE_LAPLACE_NOISE_DOMAIN_V1);
            hasher.update(&sample_index.to_le_bytes());
            let center = u64::MAX / 2;
            let sample = i128::from(
                ExactNoiseSampler::new(hasher.finalize_xof())
                    .apply_discrete_laplace(center, 1, 1, 1)
                    .expect("exact sample"),
            ) - i128::from(center);
            signed_sum += sample;
            match sample.cmp(&0) {
                std::cmp::Ordering::Less => negative += 1,
                std::cmp::Ordering::Equal => zero += 1,
                std::cmp::Ordering::Greater => positive += 1,
            }
            let magnitude = sample.unsigned_abs();
            tail_one += u64::from(magnitude >= 1);
            tail_two += u64::from(magnitude >= 2);
            tail_three += u64::from(magnitude >= 3);
        }

        assert_eq!(zero + positive + negative, SAMPLE_COUNT);
        assert!((2_000..=3_500).contains(&zero));
        assert!((2_000..=3_500).contains(&positive));
        assert!((2_000..=3_500).contains(&negative));
        assert!(positive.abs_diff(negative) < 500);
        assert!(tail_one > tail_two && tail_two > tail_three && tail_three > 0);
        assert!(signed_sum.unsigned_abs() < u128::from(SAMPLE_COUNT / 4));
    }

    #[test]
    fn exact_discrete_laplace_folds_only_at_the_public_integer_boundary() {
        fn sample(sample_index: u64, value: u64) -> u64 {
            let mut hasher = blake3::Hasher::new_keyed(&[0x3C; 32]);
            hasher.update(DISCRETE_LAPLACE_NOISE_DOMAIN_V1);
            hasher.update(&sample_index.to_le_bytes());
            ExactNoiseSampler::new(hasher.finalize_xof())
                .apply_discrete_laplace(value, 1, 1, 1)
                .expect("exact sample")
        }

        let center = u64::MAX / 2;
        let positive_context = (0..1_024)
            .find(|index| sample(*index, center) > center)
            .expect("deterministic corpus contains positive noise");
        let negative_context = (0..1_024)
            .find(|index| sample(*index, center) < center)
            .expect("deterministic corpus contains negative noise");

        assert_eq!(sample(positive_context, u64::MAX), u64::MAX);
        assert_eq!(sample(negative_context, 0), 0);
        let law = exact_discrete_laplace_law(1, 1, 1).expect("representable exact law");
        assert!(
            law.continuation_numerator > 0
                && law.continuation_numerator < law.continuation_denominator,
            "every finite latent tail length has nonzero probability"
        );
    }

    #[test]
    fn privacy_release_noise_complexity_is_globally_bounded() {
        fn dimensioned_config(dimension: usize) -> PrivacyAggregateCycleConfig {
            let mut config = privacy_config();
            config.populations = (0..dimension)
                .map(|index| PrivacyAggregatePopulationV1 {
                    label: format!("population-{index:03}"),
                    digest: [u8::try_from(index + 1).expect("bounded test dimension"); 32],
                })
                .collect();
            config.metrics = (0..dimension)
                .map(|index| PrivacyAggregateMetricSchemaV1 {
                    key: format!("metric-{index:03}"),
                    unit: "count".to_string(),
                })
                .collect();
            config
        }

        dimensioned_config(32)
            .validate()
            .expect("bounded whole-release sampler workload");
        assert!(matches!(
            dimensioned_config(40).validate(),
            Err(PrivacyAggregateWorkerError::NoiseReleaseComplexityExceedsResourceLimit { .. })
        ));
    }

    #[test]
    fn privacy_metrics_clip_each_subject_before_population_sum() {
        let mut first = privacy_event("event-a", 110);
        first.metrics[0].value = 9;
        let mut repeated_subject = privacy_event("event-b", 120);
        repeated_subject.subject_digest = first.subject_digest;
        repeated_subject.metrics[0].value = 8;
        let mut second_subject = privacy_event("event-c", 130);
        second_subject.metrics[0].value = 7;

        let metrics = clipped_population_metrics(
            &[first, repeated_subject, second_subject],
            &privacy_config().metrics,
            Some(10),
        )
        .expect("clip contributions");
        assert_eq!(
            metrics.get("moderation_actions"),
            Some(&("count".to_string(), 17))
        );
    }

    #[test]
    fn dp_k_emits_fixed_bucket_when_distinct_subjects_are_below_threshold() {
        let first = privacy_event("event-a", 110);
        let mut replayed_subject = privacy_event("event-b", 120);
        replayed_subject.subject_digest = first.subject_digest;
        let aggregates = build_privacy_aggregates_from_source_events(
            100,
            200,
            &privacy_config(),
            Some(privacy_prf_input([0x5A; 32])),
            &[first, replayed_subject],
        )
        .expect("DP+k must publish a noised zero vector below k");
        assert_eq!(aggregates.len(), 1);
        assert_eq!(aggregates[0].population_label, "jurisdiction-a");
        assert_eq!(aggregates[0].metrics.len(), 1);
    }

    #[test]
    fn dp_k_emits_the_same_fixed_bucket_schema_for_an_empty_cycle() {
        let config = privacy_config();
        let aggregates = build_privacy_aggregates_from_source_events(
            100,
            200,
            &config,
            Some(privacy_prf_input([0x5A; 32])),
            &[],
        )
        .expect("DP+k must publish every governed bucket for an empty cycle");

        assert_eq!(aggregates.len(), config.populations.len());
        assert_eq!(aggregates[0].population_label, "jurisdiction-a");
        assert_eq!(
            aggregates[0]
                .metrics
                .iter()
                .map(|metric| (metric.key.as_str(), metric.unit.as_str()))
                .collect::<Vec<_>>(),
            vec![("moderation_actions", "count")]
        );
    }

    #[test]
    fn privacy_cycle_rejects_replay_overlap_and_metric_schema_differencing() {
        let event = privacy_event("event-a", 110);
        assert!(matches!(
            build_privacy_aggregates_from_source_events(
                100,
                200,
                &privacy_config(),
                Some(privacy_prf_input([0x5A; 32])),
                &[event.clone(), event.clone()],
            ),
            Err(PrivacyAggregateWorkerError::DuplicateSourceEvent)
        ));

        let mut overlap_config = privacy_config();
        overlap_config
            .populations
            .push(PrivacyAggregatePopulationV1 {
                label: "jurisdiction-b".to_string(),
                digest: [0xB0; 32],
            });
        let mut other_population = privacy_event("event-b", 120);
        other_population.subject_digest = event.subject_digest;
        other_population.population_label = "jurisdiction-b".to_string();
        other_population.population_digest = [0xB0; 32];
        assert!(matches!(
            build_privacy_aggregates_from_source_events(
                100,
                200,
                &overlap_config,
                Some({
                    let request = PrivacyCyclePrfRequestV1::new(
                        overlap_config.query_id,
                        overlap_config.policy_digest,
                        privacy_population_inventory_digest(&overlap_config.populations),
                        privacy_metric_schema_digest(&overlap_config.metrics),
                        PrivacyAggregateCycleWindow {
                            cycle_start_unix: 100,
                            cycle_end_unix: 200,
                            due_at_unix: 200,
                        },
                    )
                    .expect("overlap config request");
                    PrivacyCyclePrfInputV1::new(
                        request,
                        PrivacyCyclePrfOutputV1::new([0x5A; 32]).expect("valid overlap PRF output"),
                    )
                }),
                &[event.clone(), other_population],
            ),
            Err(PrivacyAggregateWorkerError::SubjectPopulationOverlap)
        ));

        let mut mismatched_schema = privacy_event("event-b", 120);
        mismatched_schema
            .metrics
            .push(PrivacyAggregateSourceMetric {
                key: "proof_failures".to_string(),
                value: 1,
                unit: "count".to_string(),
            });
        assert!(matches!(
            build_privacy_aggregates_from_source_events(
                100,
                200,
                &privacy_config(),
                Some(privacy_prf_input([0x5A; 32])),
                &[event, mismatched_schema],
            ),
            Err(PrivacyAggregateWorkerError::MetricSchemaMismatch)
        ));
    }

    #[test]
    fn privacy_cycle_bounds_source_events_before_grouping() {
        let events = (0..=PRIVACY_AGGREGATE_MAX_SOURCE_EVENTS_V1)
            .map(|index| privacy_event(&format!("event-{index:04}"), 110))
            .collect::<Vec<_>>();
        assert_eq!(
            build_privacy_aggregates_from_source_events(
                100,
                200,
                &privacy_config(),
                Some(privacy_prf_input([0x5A; 32])),
                &events,
            ),
            Err(PrivacyAggregateWorkerError::TooManySourceEvents {
                count: PRIVACY_AGGREGATE_MAX_SOURCE_EVENTS_V1 + 1,
                max: PRIVACY_AGGREGATE_MAX_SOURCE_EVENTS_V1,
            })
        );
    }

    #[test]
    fn privacy_source_event_requires_subject_digest() {
        let mut event = privacy_event("event-a", 110);
        event.subject_digest = [0; 32];
        assert_eq!(
            event.validate(),
            Err(PrivacyAggregateWorkerError::MissingDigest {
                field: "subject_digest",
            })
        );
    }

    #[test]
    fn privacy_source_event_enforces_text_and_metric_bounds() {
        let mut event = privacy_event("event-a", 110);
        event.event_id = "x".repeat(MODERATION_LEDGER_MAX_PUBLIC_TEXT_BYTES_V1 + 1);
        assert_eq!(
            event.validate(),
            Err(PrivacyAggregateWorkerError::TextTooLong {
                field: "event_id",
                max: MODERATION_LEDGER_MAX_PUBLIC_TEXT_BYTES_V1,
            })
        );

        let mut event = privacy_event("event-a", 110);
        event.metrics = (0..=MODERATION_PRIVACY_MAX_METRICS_V1)
            .map(|index| PrivacyAggregateSourceMetric {
                key: format!("metric-{index:04}"),
                value: 1,
                unit: "count".to_string(),
            })
            .collect();
        assert_eq!(
            event.validate(),
            Err(PrivacyAggregateWorkerError::TooManySourceMetrics {
                count: MODERATION_PRIVACY_MAX_METRICS_V1 + 1,
                max: MODERATION_PRIVACY_MAX_METRICS_V1,
            })
        );
    }

    #[test]
    fn privacy_metric_overflow_fails_closed() {
        let mut config = privacy_config();
        config.privacy = ModerationPrivacyParametersV1 {
            version:
                iroha_data_model::sorafs::transparency::MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
            mode: ModerationPrivacyModeV1::Suppression,
            epsilon_numerator: None,
            epsilon_denominator: None,
            delta_ppb: None,
            per_subject_metric_cap: None,
            suppression_threshold: Some(1),
        };
        let mut first = privacy_event("event-a", 110);
        first.metrics[0].value = u64::MAX;
        let mut second = privacy_event("event-b", 120);
        second.metrics[0].value = u64::MAX;

        assert_eq!(
            build_privacy_aggregates_from_source_events(100, 200, &config, None, &[first, second],),
            Err(PrivacyAggregateWorkerError::MetricArithmeticOverflow)
        );
    }

    #[test]
    fn privacy_noise_uses_joint_metric_vector_sensitivity() {
        let mut config = privacy_config();
        config.privacy.epsilon_numerator = Some(1);
        config.privacy.epsilon_denominator = Some(300);
        config.privacy.per_subject_metric_cap = Some(10);
        config.metrics.insert(
            0,
            PrivacyAggregateMetricSchemaV1 {
                key: "appeals".to_string(),
                unit: "count".to_string(),
            },
        );
        let mut first = privacy_event("event-a", 110);
        first.metrics.insert(
            0,
            PrivacyAggregateSourceMetric {
                key: "appeals".to_string(),
                value: 1,
                unit: "count".to_string(),
            },
        );
        let mut second = privacy_event("event-b", 120);
        second.metrics.insert(
            0,
            PrivacyAggregateSourceMetric {
                key: "appeals".to_string(),
                value: 1,
                unit: "count".to_string(),
            },
        );

        assert_eq!(
            build_privacy_aggregates_from_source_events(
                100,
                200,
                &config,
                Some(privacy_prf_input([0x5A; 32])),
                &[first, second],
            ),
            Err(
                PrivacyAggregateWorkerError::NoiseParametersExceedResourceLimit {
                    epsilon_numerator: 1,
                    epsilon_denominator: 300,
                    sensitivity: 40,
                }
            )
        );
    }

    #[test]
    fn privacy_noise_clamps_wide_internal_sums_before_integer_post_processing() {
        let config = privacy_config();
        let sensitivity = privacy_vector_sensitivity(&config).expect("valid sensitivity");
        let private_source_digest = [0x44; 32];
        let at_u64_max = apply_metric_noise(
            u128::from(u64::MAX),
            &config,
            Some(&privacy_prf_input([0x5A; 32])),
            sensitivity,
            "aggregate-a",
            "moderation_actions",
            &private_source_digest,
        )
        .expect("noise at the public integer bound");
        let above_u64_max = apply_metric_noise(
            u128::MAX,
            &config,
            Some(&privacy_prf_input([0x5A; 32])),
            sensitivity,
            "aggregate-a",
            "moderation_actions",
            &private_source_digest,
        )
        .expect("wide sum is deterministically clamped");

        assert_eq!(above_u64_max, at_u64_max);
    }

    fn privacy_budget_policy() -> PrivacyCompositionBudgetPolicyV1 {
        PrivacyCompositionBudgetPolicyV1 {
            budget_id: [0xD0; 32],
            epsilon_limit_numerator: 1,
            epsilon_limit_denominator: 1,
            max_publications: 4,
        }
    }

    #[test]
    fn privacy_composition_budget_is_durable_hash_chained_and_fail_closed() {
        let mut ledger = PrivacyCompositionBudgetLedgerV1::default();
        let first = ledger
            .charge(privacy_budget_policy(), [0x01; 16], 200, 1, 2)
            .expect("first budget charge");
        let second = ledger
            .charge(privacy_budget_policy(), [0x02; 16], 201, 1, 2)
            .expect("second budget charge");
        assert_eq!(first.sequence, 1);
        assert_eq!(second.sequence, 2);
        assert_eq!(second.previous_charge_digest, Some(first.charge_digest));
        assert_eq!(
            (
                second.cumulative_epsilon_numerator,
                second.cumulative_epsilon_denominator
            ),
            (1, 1)
        );
        ledger.validate().expect("ledger validates");

        let encoded = norito::to_bytes(&ledger).expect("budget ledger encodes");
        let decoded: PrivacyCompositionBudgetLedgerV1 =
            norito::decode_from_bytes(&encoded).expect("budget ledger decodes");
        assert_eq!(decoded, ledger);
        decoded.validate().expect("restored ledger validates");

        let before = ledger.clone();
        assert_eq!(
            ledger.charge(privacy_budget_policy(), [0x03; 16], 202, 1, 2),
            Err(PrivacyCompositionBudgetError::BudgetExhausted)
        );
        assert_eq!(ledger, before, "failed charges must be atomic");
    }

    #[test]
    fn privacy_composition_budget_rejects_replay_conflict_and_tampering() {
        let mut ledger = PrivacyCompositionBudgetLedgerV1::default();
        ledger
            .charge(privacy_budget_policy(), [0x01; 16], 200, 1, 4)
            .expect("budget charge");
        let before = ledger.clone();
        assert_eq!(
            ledger.charge(privacy_budget_policy(), [0x01; 16], 201, 1, 4),
            Err(PrivacyCompositionBudgetError::DuplicateCycle)
        );
        assert_eq!(ledger, before);

        let mut conflicting_policy = privacy_budget_policy();
        conflicting_policy.max_publications = 3;
        assert_eq!(
            ledger.charge(conflicting_policy, [0x02; 16], 201, 1, 4),
            Err(PrivacyCompositionBudgetError::PolicyConflict)
        );
        assert_eq!(ledger, before);

        let mut tampered = ledger.clone();
        tampered.chains[0].charges[0].cumulative_epsilon_numerator = 2;
        assert_eq!(
            tampered.validate(),
            Err(PrivacyCompositionBudgetError::InvalidChargeChain)
        );
        let mut tampered = ledger;
        tampered.chains[0].charges[0].charge_digest[0] ^= 1;
        assert_eq!(
            tampered.validate(),
            Err(PrivacyCompositionBudgetError::InvalidChargeChain)
        );

        let mut truncated =
            norito::to_bytes(&before).expect("encode valid privacy budget checkpoint");
        truncated.pop();
        assert!(
            norito::decode_from_bytes::<PrivacyCompositionBudgetLedgerV1>(&truncated).is_err(),
            "truncated privacy budget checkpoint must fail closed"
        );
    }

    fn privacy_release_record(
        cycle_start_unix: u64,
        cycle_end_unix: u64,
        seed: u8,
        previous_publication_block_hash: Option<[u8; 32]>,
    ) -> PrivacyReleaseRecordV1 {
        let config = privacy_config();
        let request = PrivacyCyclePrfRequestV1::new(
            config.query_id,
            config.policy_digest,
            privacy_population_inventory_digest(&config.populations),
            privacy_metric_schema_digest(&config.metrics),
            PrivacyAggregateCycleWindow {
                cycle_start_unix,
                cycle_end_unix,
                due_at_unix: cycle_end_unix,
            },
        )
        .expect("release PRF request");
        PrivacyReleaseRecordV1 {
            sequence: 0,
            release_id: privacy_aggregate_cycle_id(
                config.query_id,
                cycle_start_unix,
                cycle_end_unix,
            ),
            query_id: config.query_id,
            first_cycle_start_unix: config.first_cycle_start_unix,
            cycle_seconds: config.cycle_seconds,
            publish_delay_seconds: 0,
            cycle_start_unix,
            cycle_end_unix,
            due_at_unix: cycle_end_unix,
            private_source_digest: [seed; 32],
            policy_digest: config.policy_digest,
            population_inventory_digest: privacy_population_inventory_digest(&config.populations),
            metric_schema_digest: privacy_metric_schema_digest(&config.metrics),
            privacy: config.privacy,
            prf_request_binding: Some(request.binding_digest()),
            prf_commitment: Some([seed.wrapping_add(2); 32]),
            budget_charge_digest: Some([seed.wrapping_add(3); 32]),
            publication_payload_digest: Some([seed.wrapping_add(4); 32]),
            published_aggregate_inventory_digest: Some([seed.wrapping_add(6); 32]),
            previous_publication_block_hash,
            publication_block_hash: Some([seed.wrapping_add(5); 32]),
            status: PrivacyReleaseStatusV1::Published,
            previous_record_digest: None,
            record_digest: [0; 32],
        }
    }

    #[test]
    fn privacy_release_ledger_is_append_only_hash_chained_and_redacts_private_input() {
        let mut ledger = PrivacyReleaseLedgerV1::default();
        let first = ledger
            .append(privacy_release_record(100, 200, 0x41, None))
            .expect("append first release");
        let second = ledger
            .append(privacy_release_record(
                200,
                300,
                0x51,
                first.publication_block_hash,
            ))
            .expect("append second release");

        assert_eq!(first.sequence, 1);
        assert_eq!(second.sequence, 2);
        assert_eq!(second.previous_record_digest, Some(first.record_digest));
        assert_eq!(ledger.head([0xB0; 32]).expect("release head").sequence(), 2);
        ledger.validate().expect("release ledger validates");
        let restored: PrivacyReleaseLedgerV1 =
            norito::decode_from_bytes(&norito::to_bytes(&ledger).expect("encode release ledger"))
                .expect("decode release ledger");
        assert_eq!(restored, ledger);
        assert!(!format!("{:?}", ledger.records[0]).contains(&hex::encode([0x41; 32])));

        let mut delay_ledger = PrivacyReleaseLedgerV1::default();
        let delay_first = delay_ledger
            .append(privacy_release_record(100, 200, 0x41, None))
            .expect("append first delay-lineage release");
        let mut changed_delay =
            privacy_release_record(200, 300, 0x61, delay_first.publication_block_hash);
        changed_delay.publish_delay_seconds = 10;
        changed_delay.due_at_unix = 310;
        assert_eq!(
            delay_ledger
                .append(changed_delay)
                .expect_err("publish delay is immutable for a query lineage"),
            PrivacyReleaseLedgerErrorV1::InvalidRecord
        );

        let mut tampered = ledger;
        tampered.records[0].private_source_digest[0] ^= 1;
        assert_eq!(
            tampered.validate(),
            Err(PrivacyReleaseLedgerErrorV1::InvalidRecord)
        );
    }

    #[test]
    fn privacy_cycle_prf_request_binds_policy_and_exact_window() {
        let config = privacy_config();
        let population_inventory_digest = privacy_population_inventory_digest(&config.populations);
        let metric_schema_digest = privacy_metric_schema_digest(&config.metrics);
        let window = PrivacyAggregateCycleWindow {
            cycle_start_unix: 100,
            cycle_end_unix: 200,
            due_at_unix: 210,
        };
        let request = PrivacyCyclePrfRequestV1::new(
            config.query_id,
            [0xC0; 32],
            population_inventory_digest,
            metric_schema_digest,
            window,
        )
        .expect("canonical PRF request");

        assert_eq!(request.version(), PRIVACY_CYCLE_PRF_REQUEST_VERSION_V1);
        assert_eq!(request.query_id(), config.query_id);
        assert_eq!(request.policy_digest(), [0xC0; 32]);
        assert_eq!(
            request.cycle_id(),
            privacy_aggregate_cycle_id(config.query_id, 100, 200)
        );
        assert_eq!(request.cycle_start_unix(), 100);
        assert_eq!(request.cycle_end_unix(), 200);

        let other_policy = PrivacyCyclePrfRequestV1::new(
            config.query_id,
            [0xC1; 32],
            population_inventory_digest,
            metric_schema_digest,
            window,
        )
        .expect("other policy request");
        let other_window = PrivacyCyclePrfRequestV1::new(
            config.query_id,
            [0xC0; 32],
            population_inventory_digest,
            metric_schema_digest,
            PrivacyAggregateCycleWindow {
                cycle_start_unix: 200,
                cycle_end_unix: 300,
                due_at_unix: 310,
            },
        )
        .expect("other window request");
        assert_ne!(request.binding_digest(), other_policy.binding_digest());
        assert_eq!(request.cycle_id(), other_policy.cycle_id());
        assert_ne!(request.cycle_id(), other_window.cycle_id());
        assert_ne!(request.binding_digest(), other_window.binding_digest());
        let changed_due = PrivacyCyclePrfRequestV1::new(
            config.query_id,
            [0xC0; 32],
            population_inventory_digest,
            metric_schema_digest,
            PrivacyAggregateCycleWindow {
                due_at_unix: 999,
                ..window
            },
        )
        .expect("changed operational due time");
        assert_eq!(request, changed_due);
        assert_eq!(
            PrivacyCyclePrfRequestV1::new(
                [0; 32],
                [0xC0; 32],
                population_inventory_digest,
                metric_schema_digest,
                window,
            ),
            Err(PrivacyCyclePrfRequestErrorV1::MissingQueryId)
        );
    }

    #[test]
    fn privacy_aggregate_publishes_commitment_not_runtime_noise_material() {
        let events = vec![privacy_event("event-a", 110), privacy_event("event-b", 120)];
        let config = privacy_config();

        let first = build_privacy_aggregates_from_source_events(
            100,
            200,
            &config,
            Some(privacy_prf_input([0x5A; 32])),
            &events,
        )
        .expect("build aggregate");
        let second = build_privacy_aggregates_from_source_events(
            100,
            200,
            &config,
            Some(privacy_prf_input([0x5A; 32])),
            &events,
        )
        .expect("rebuild aggregate");

        assert_eq!(first, second);
        assert_eq!(first.len(), 1);
        let aggregate = &first[0];
        let expected_commitment = privacy_prf_input([0x5A; 32]).commitment();
        assert_eq!(
            aggregate.noise_source,
            ModerationPrivacyNoiseSourceV1::ThresholdPrf(expected_commitment)
        );
        assert!(
            aggregate
                .metadata
                .iter()
                .all(|item| item.value != hex::encode([0x5A; 32]))
        );
        let encoded = norito::to_bytes(aggregate).expect("encode aggregate");
        assert!(
            !encoded
                .windows(32)
                .any(|window| window == [0x5A; 32].as_slice()),
            "runtime threshold-PRF output must not enter the public aggregate"
        );

        let changed = build_privacy_aggregates_from_source_events(
            100,
            200,
            &privacy_config(),
            Some(privacy_prf_input([0x5B; 32])),
            &events,
        )
        .expect("build with changed runtime output");
        assert_ne!(
            changed[0].noise_source, aggregate.noise_source,
            "the opaque public commitment must bind the runtime PRF output"
        );
    }

    #[test]
    fn privacy_cycle_rejects_wrong_prf_binding_and_commitment_without_logging_output() {
        let events = vec![privacy_event("event-a", 110), privacy_event("event-b", 120)];
        let config = privacy_config();
        let wrong_request = PrivacyCyclePrfRequestV1::new(
            config.query_id,
            [0xC1; 32],
            privacy_population_inventory_digest(&config.populations),
            privacy_metric_schema_digest(&config.metrics),
            PrivacyAggregateCycleWindow {
                cycle_start_unix: 100,
                cycle_end_unix: 200,
                due_at_unix: 200,
            },
        )
        .expect("canonical wrong-policy request");
        let wrong_binding = PrivacyCyclePrfInputV1::new(
            wrong_request,
            PrivacyCyclePrfOutputV1::new([0x5A; 32]).expect("valid provider output"),
        );
        assert_eq!(
            build_privacy_aggregates_from_source_events(
                100,
                200,
                &privacy_config(),
                Some(wrong_binding),
                &events,
            ),
            Err(PrivacyAggregateWorkerError::CyclePrfBindingMismatch)
        );

        let mut wrong_commitment = privacy_prf_input([0x5A; 32]);
        wrong_commitment.commitment.commitment[0] ^= 1;
        assert_eq!(
            build_privacy_aggregates_from_source_events(
                100,
                200,
                &privacy_config(),
                Some(wrong_commitment),
                &events,
            ),
            Err(PrivacyAggregateWorkerError::CyclePrfCommitmentMismatch)
        );

        let input = privacy_prf_input([0x5A; 32]);
        let debug = format!("{input:?}");
        assert!(debug.contains("output: \"<redacted>\""));
        assert!(
            !debug.contains(&format!("{:?}", [0x5A; 32])),
            "runtime threshold-PRF output must not enter Debug/log output"
        );
    }

    #[test]
    fn privacy_aggregate_is_byte_identical_across_input_orderings() {
        let events = vec![
            privacy_event("event-a", 110),
            privacy_event("event-b", 120),
            privacy_event("event-c", 130),
        ];
        let mut reversed = events.clone();
        reversed.reverse();
        let first = build_privacy_aggregates_from_source_events(
            100,
            200,
            &privacy_config(),
            Some(privacy_prf_input([0x5A; 32])),
            &events,
        )
        .expect("build first replica output");
        let second = build_privacy_aggregates_from_source_events(
            100,
            200,
            &privacy_config(),
            Some(privacy_prf_input([0x5A; 32])),
            &reversed,
        )
        .expect("build second replica output");

        assert_eq!(first, second);
        assert_eq!(
            norito::to_bytes(&first).expect("encode first replica output"),
            norito::to_bytes(&second).expect("encode second replica output")
        );
    }

    #[test]
    fn suppression_only_release_has_no_randomness_commitment() {
        let mut config = privacy_config();
        config.privacy = ModerationPrivacyParametersV1 {
            version:
                iroha_data_model::sorafs::transparency::MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
            mode: ModerationPrivacyModeV1::Suppression,
            epsilon_numerator: None,
            epsilon_denominator: None,
            delta_ppb: None,
            per_subject_metric_cap: None,
            suppression_threshold: Some(2),
        };
        let aggregates = build_privacy_aggregates_from_source_events(
            100,
            200,
            &config,
            None,
            &[privacy_event("event-a", 110), privacy_event("event-b", 120)],
        )
        .expect("build suppression-only aggregate");
        assert_eq!(aggregates.len(), 1);
        assert_eq!(
            aggregates[0].noise_source,
            ModerationPrivacyNoiseSourceV1::SuppressionOnly
        );
    }

    #[test]
    fn privacy_aggregate_rejects_sampler_resource_exhaustion_policy() {
        let mut config = privacy_config();
        config.privacy.epsilon_numerator = Some(1);
        config.privacy.epsilon_denominator = Some(10_000);

        let error = build_privacy_aggregates_from_source_events(
            100,
            200,
            &config,
            Some(privacy_prf_input([0x5A; 32])),
            &[privacy_event("event-a", 110), privacy_event("event-b", 120)],
        )
        .expect_err("unbounded expected sampler work must fail");

        assert_eq!(
            error,
            PrivacyAggregateWorkerError::NoiseParametersExceedResourceLimit {
                epsilon_numerator: 1,
                epsilon_denominator: 10_000,
                sensitivity: 2,
            }
        );
    }

    #[test]
    fn privacy_aggregate_rejects_caller_supplied_randomness_commitment() {
        let mut config = privacy_config();
        config.metadata = vec![ModerationLedgerMetadataV1 {
            key: MODERATION_PRIVACY_RANDOMNESS_COMMITMENT_METADATA_KEY_V1.to_string(),
            value: hex::encode([0x11; 32]),
        }];

        let error = config
            .validate()
            .expect_err("worker-owned commitment key must be reserved");
        assert_eq!(
            error,
            PrivacyAggregateWorkerError::ReservedMetadataKey {
                key: MODERATION_PRIVACY_RANDOMNESS_COMMITMENT_METADATA_KEY_V1,
            }
        );

        let mut missing_policy = privacy_config();
        missing_policy.policy_digest = [0; 32];
        assert_eq!(
            missing_policy.validate(),
            Err(PrivacyAggregateWorkerError::MissingDigest {
                field: "policy_digest",
            })
        );
    }

    #[test]
    fn concrete_source_entry_adapters_derive_valid_entries() {
        let gar_receipt = gar_receipt_fixture(GarEnforcementActionV1::LegalHold);
        let gar_entry =
            gar_enforcement_receipt_source_entry(&gar_receipt).expect("gar source entry");
        assert_eq!(gar_entry.kind, ModerationLedgerEntryKindV1::LegalHold);
        assert_eq!(gar_entry.event_id, "gar:6761722d726563656970742d30303031");
        assert_eq!(gar_entry.occurred_at_unix, 1_800_000_010);
        assert_eq!(gar_entry.policy_digest, Some([0xAB; 32]));
        assert_eq!(
            gar_entry.payload_digest,
            *blake3::hash(&norito::to_bytes(&gar_receipt).expect("encode gar")).as_bytes()
        );
        gar_entry.validate().expect("gar entry validates");

        let moderation_event = moderation_governance_event_fixture();
        let moderation_entry = moderation_ballot_governance_event_source_entry(&moderation_event)
            .expect("moderation source entry");
        assert_eq!(
            moderation_entry.kind,
            ModerationLedgerEntryKindV1::ModerationAction
        );
        assert_eq!(moderation_entry.occurred_at_unix, 1_800_000_030);
        assert!(
            moderation_entry
                .metadata
                .iter()
                .any(|item| item.key == "winning_choice" && item.value == "overturn")
        );
        moderation_entry
            .validate()
            .expect("moderation entry validates");

        let report = appeal_finance_report_fixture();
        let report_entry =
            appeal_finance_report_source_entry(&report).expect("appeal report source entry");
        assert_eq!(
            report_entry.kind,
            ModerationLedgerEntryKindV1::AppealOutcome
        );
        assert_eq!(report_entry.policy_digest, Some([0xA7; 32]));
        assert_eq!(report_entry.subject, "case-42");
        report_entry
            .validate()
            .expect("appeal report entry validates");

        let receipt = appeal_finance_settlement_receipt_fixture();
        let receipt_entry = appeal_finance_settlement_receipt_source_entry(&receipt)
            .expect("appeal settlement source entry");
        assert_eq!(
            receipt_entry.kind,
            ModerationLedgerEntryKindV1::AppealOutcome
        );
        assert_eq!(receipt_entry.policy_digest, Some([0x33; 32]));
        assert_eq!(receipt_entry.subject, "case-42:treasury-release");
        receipt_entry
            .validate()
            .expect("appeal settlement entry validates");

        let reserve_lifecycle = reserve_lifecycle_event_fixture();
        let reserve_lifecycle_entry = reserve_lifecycle_event_source_entry(&reserve_lifecycle)
            .expect("reserve lifecycle source entry");
        assert_eq!(
            reserve_lifecycle_entry.kind,
            ModerationLedgerEntryKindV1::Custom("sorafs_reserve_lifecycle".to_string())
        );
        assert_eq!(
            reserve_lifecycle_entry.event_id,
            format!(
                "reserve-lifecycle:{}:{}",
                reserve_lifecycle.sequence,
                hex::encode(reserve_lifecycle.provider_id)
            )
        );
        assert!(
            reserve_lifecycle_entry
                .metadata
                .iter()
                .any(|item| item.key == "current_stage" && item.value == "grace")
        );
        reserve_lifecycle_entry
            .validate()
            .expect("reserve lifecycle entry validates");

        let reserve_movement = reserve_movement_record_fixture();
        let reserve_movement_entry =
            reserve_movement_source_entry(&reserve_movement).expect("reserve movement entry");
        assert_eq!(
            reserve_movement_entry.kind,
            ModerationLedgerEntryKindV1::Custom("sorafs_reserve_movement".to_string())
        );
        assert_eq!(reserve_movement_entry.occurred_at_unix, 1_800_000_060);
        assert!(
            reserve_movement_entry
                .metadata
                .iter()
                .any(|item| item.key == "provider_account_digest_hex")
        );
        assert!(
            !reserve_movement_entry
                .metadata
                .iter()
                .any(|item| item.value == "provider-account")
        );
        reserve_movement_entry
            .validate()
            .expect("reserve movement entry validates");

        let reserve_appeal = reserve_appeal_record_fixture();
        let reserve_appeal_entry =
            reserve_appeal_source_entry(&reserve_appeal).expect("reserve appeal entry");
        assert_eq!(
            reserve_appeal_entry.kind,
            ModerationLedgerEntryKindV1::AppealOutcome
        );
        assert!(
            reserve_appeal_entry
                .metadata
                .iter()
                .any(|item| item.key == "reason_digest_hex")
        );
        assert!(
            !reserve_appeal_entry
                .metadata
                .iter()
                .any(|item| item.value == reserve_appeal.reason)
        );
        reserve_appeal_entry
            .validate()
            .expect("reserve appeal entry validates");

        let reserve_policy = reserve_lifecycle_policy_record_fixture();
        let reserve_policy_entry =
            reserve_lifecycle_policy_source_entry(&reserve_policy).expect("reserve policy entry");
        assert_eq!(
            reserve_policy_entry.kind,
            ModerationLedgerEntryKindV1::Custom("sorafs_reserve_lifecycle_policy".to_string())
        );
        assert_eq!(
            reserve_policy_entry.policy_digest,
            Some(reserve_policy.policy_id)
        );
        assert!(
            reserve_policy_entry
                .metadata
                .iter()
                .any(|item| item.key == "authority_account_digest_hex")
        );
        reserve_policy_entry
            .validate()
            .expect("reserve policy entry validates");
    }

    #[test]
    fn reserve_source_entries_preserve_exact_quantities_and_hash_canonical_records() {
        let sub_micro: sorafs_manifest::deal::XorQuantity =
            "0.0000001".parse().expect("sub-micro quantity");
        let wide: sorafs_manifest::deal::XorQuantity = "340282366920938463463374607431768211456"
            .parse()
            .expect("quantity wider than u128");
        let high_precision: sorafs_manifest::deal::XorQuantity =
            "1.000000001".parse().expect("high precision quantity");

        let mut lifecycle = reserve_lifecycle_event_fixture();
        lifecycle.ledger.rent_due = sub_micro.clone();
        lifecycle.ledger.reserve_shortfall = wide.clone();
        lifecycle.ledger.top_up_shortfall = high_precision.clone();
        lifecycle.lifecycle.credit_draw = wide.clone();
        lifecycle.lifecycle.credit_available_after_draw = Some(high_precision.clone());
        lifecycle.lifecycle.credit_shortfall = sub_micro.clone();
        lifecycle.lifecycle.accrued_interest = high_precision.clone();
        lifecycle.lifecycle.total_due_after_credit = wide.clone();
        let lifecycle_entry =
            reserve_lifecycle_event_source_entry(&lifecycle).expect("exact lifecycle entry");
        for (key, expected) in [
            ("rent_due", "0.0000001"),
            (
                "reserve_shortfall",
                "340282366920938463463374607431768211456",
            ),
            ("top_up_shortfall", "1.000000001"),
            ("credit_draw", "340282366920938463463374607431768211456"),
            ("credit_available_after_draw", "1.000000001"),
            ("credit_shortfall", "0.0000001"),
            ("accrued_interest", "1.000000001"),
            (
                "total_due_after_credit",
                "340282366920938463463374607431768211456",
            ),
        ] {
            assert!(
                lifecycle_entry
                    .metadata
                    .iter()
                    .any(|item| item.key == key && item.value == expected),
                "missing exact lifecycle metadata {key}"
            );
        }
        assert!(
            lifecycle_entry
                .metadata
                .iter()
                .all(|item| !item.key.contains("micro_xor")),
            "legacy micro-XOR metadata must not be emitted"
        );
        let lifecycle_bytes = lifecycle.encode();
        assert_eq!(
            lifecycle_entry.payload_digest,
            reserve_source_payload_digest(
                "reserve_lifecycle_event",
                &[("canonical_event", lifecycle_bytes.as_slice())],
            )
        );
        let mut changed_lifecycle = lifecycle.clone();
        changed_lifecycle.lifecycle.total_due_after_credit = high_precision.clone();
        assert_ne!(
            reserve_lifecycle_event_source_entry(&changed_lifecycle)
                .expect("changed lifecycle entry")
                .payload_digest,
            lifecycle_entry.payload_digest,
            "every canonical lifecycle field must be authenticated"
        );

        let mut movement = reserve_movement_record_fixture();
        movement.amount = sub_micro;
        movement.balance_after = wide;
        movement.confirmed_balance_after = high_precision;
        let movement_entry =
            reserve_movement_source_entry(&movement).expect("exact movement entry");
        for (key, expected) in [
            ("amount", "0.0000001"),
            ("balance_after", "340282366920938463463374607431768211456"),
            ("confirmed_balance_after", "1.000000001"),
        ] {
            assert!(
                movement_entry
                    .metadata
                    .iter()
                    .any(|item| item.key == key && item.value == expected),
                "missing exact movement metadata {key}"
            );
        }
        let movement_bytes = movement.encode();
        assert_eq!(
            movement_entry.payload_digest,
            reserve_source_payload_digest(
                "reserve_movement",
                &[("canonical_record", movement_bytes.as_slice())],
            )
        );
        let mut changed_movement = movement.clone();
        changed_movement.custody_updated_at_unix = Some(
            changed_movement
                .custody_updated_at_unix
                .expect("custody timestamp")
                .saturating_add(1),
        );
        assert_ne!(
            reserve_movement_source_entry(&changed_movement)
                .expect("changed movement entry")
                .payload_digest,
            movement_entry.payload_digest,
            "every canonical movement field must be authenticated"
        );
    }

    #[test]
    fn concrete_source_entry_adapters_reject_invalid_sources() {
        let mut receipt = gar_receipt_fixture(GarEnforcementActionV1::GeoFence);
        receipt.receipt_id = [0; 16];
        let err = gar_enforcement_receipt_source_entry(&receipt)
            .expect_err("zero GAR receipt id rejected");
        assert!(matches!(
            err,
            TransparencySourceEntryAdapterError::InvalidGarReceipt { .. }
        ));

        let mut event = moderation_governance_event_fixture();
        event.tally = None;
        let err = moderation_ballot_governance_event_source_entry(&event)
            .expect_err("invalid moderation event rejected");
        assert!(matches!(
            err,
            TransparencySourceEntryAdapterError::InvalidModerationEvent { .. }
        ));

        let mut report = appeal_finance_report_fixture();
        report.report_id = [0; 16];
        let err = appeal_finance_report_source_entry(&report)
            .expect_err("invalid appeal report rejected");
        assert!(matches!(
            err,
            TransparencySourceEntryAdapterError::InvalidAppealFinanceReport { .. }
        ));

        let mut reserve_lifecycle = reserve_lifecycle_event_fixture();
        reserve_lifecycle.provider_id = [0; 32];
        let err = reserve_lifecycle_event_source_entry(&reserve_lifecycle)
            .expect_err("invalid reserve lifecycle rejected");
        assert!(matches!(
            err,
            TransparencySourceEntryAdapterError::InvalidReserveLifecycleEvent { .. }
        ));

        let mut reserve_policy = reserve_lifecycle_policy_record_fixture();
        reserve_policy.default_after_days = reserve_policy.grace_period_days;
        let err = reserve_lifecycle_policy_source_entry(&reserve_policy)
            .expect_err("invalid reserve policy rejected");
        assert!(matches!(
            err,
            TransparencySourceEntryAdapterError::InvalidReserveLifecyclePolicy { .. }
        ));
    }

    #[test]
    fn transparency_ledger_source_entries_build_sorted_ledger_entries() {
        let cycle_id = *b"cycle-src-test01";
        let entries = build_transparency_ledger_entries_from_source_events(
            cycle_id,
            100,
            200,
            201,
            &[
                transparency_source_entry(
                    "redaction-1",
                    130,
                    ModerationLedgerEntryKindV1::Redaction,
                    "redaction-case-1",
                    0x70,
                ),
                transparency_source_entry(
                    "gar-1",
                    120,
                    ModerationLedgerEntryKindV1::GarEnforcementReceipt,
                    "gar-receipt-1",
                    0x50,
                ),
                transparency_source_entry(
                    "hold-1",
                    130,
                    ModerationLedgerEntryKindV1::LegalHold,
                    "hold-case-1",
                    0x60,
                ),
                transparency_source_entry(
                    "appeal-1",
                    110,
                    ModerationLedgerEntryKindV1::AppealOutcome,
                    "appeal-case-1",
                    0x40,
                ),
            ],
        )
        .expect("build source ledger entries");

        assert_eq!(entries.len(), 4);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.subject.as_str())
                .collect::<Vec<_>>(),
            vec![
                "appeal-case-1",
                "gar-receipt-1",
                "hold-case-1",
                "redaction-case-1"
            ]
        );
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.sequence)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        assert!(
            entries
                .iter()
                .all(|entry| entry.version == MODERATION_LEDGER_ENTRY_VERSION_V1)
        );
        assert!(entries.iter().all(|entry| entry.cycle_id == cycle_id));
        assert!(entries.iter().all(|entry| entry.validate().is_ok()));
        let ids = entries
            .iter()
            .map(|entry| entry.entry_id)
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), entries.len());
        assert!(!ids.contains(&[0; 16]));
    }

    #[test]
    fn transparency_ledger_source_entries_reject_duplicate_and_out_of_window() {
        let cycle_id = *b"cycle-src-test01";
        let duplicate = transparency_source_entry(
            "gar-1",
            120,
            ModerationLedgerEntryKindV1::GarEnforcementReceipt,
            "gar-receipt-1",
            0x50,
        );

        let err = build_transparency_ledger_entries_from_source_events(
            cycle_id,
            100,
            200,
            201,
            &[duplicate.clone(), duplicate],
        )
        .expect_err("duplicate event rejected");
        assert!(matches!(
            err,
            TransparencyLedgerIngestError::DuplicateSourceEntry { .. }
        ));

        let err = build_transparency_ledger_entries_from_source_events(
            cycle_id,
            100,
            200,
            201,
            &[transparency_source_entry(
                "future-1",
                200,
                ModerationLedgerEntryKindV1::EvidenceAccess,
                "evidence-view-1",
                0x80,
            )],
        )
        .expect_err("out-of-window event rejected");
        assert!(matches!(
            err,
            TransparencyLedgerIngestError::EntryOutsideCycle { .. }
        ));
    }

    #[test]
    fn proof_token_issuance_from_base64_verifies_and_derives_public_record() {
        let issuance = proof_token_issuance_from_base64(
            VALID_PROOF_TOKEN_B64,
            valid_signer_key(),
            Some([0x65; 32]),
            Some([0x66; 32]),
            vec![ModerationLedgerMetadataV1 {
                key: "issuer".to_string(),
                value: "gateway-a".to_string(),
            }],
        )
        .expect("valid proof-token issuance");

        assert_eq!(issuance.version, PROOF_TOKEN_ISSUANCE_VERSION_V1);
        assert_eq!(issuance.token_id, [0x61; 16]);
        assert_eq!(issuance.issued_at_unix, 1_800_000_030);
        assert_eq!(issuance.expires_at_unix, Some(1_800_086_430));
        assert_eq!(issuance.moderation_action_code, 2);
        assert_eq!(issuance.signer_key, valid_signer_key());
        assert_ne!(issuance.token_blake3, [0; 32]);
        assert_eq!(issuance.blinded_digest, [0x64; 32]);
        assert_eq!(
            issuance.entry_ids,
            vec!["denylist/global".to_string(), "gar/policy/42".to_string()]
        );
        assert_eq!(issuance.evidence_digest, Some([0x65; 32]));
        assert_eq!(issuance.policy_digest, Some([0x66; 32]));
        assert_eq!(issuance.metadata[0].key, "issuer");
    }

    #[test]
    fn proof_token_issuance_from_base64_rejects_bad_signature_key() {
        let mut signer_key = valid_signer_key();
        signer_key[0] ^= 0x01;

        let err = proof_token_issuance_from_base64(
            VALID_PROOF_TOKEN_B64,
            signer_key,
            Some([0x65; 32]),
            None,
            Vec::new(),
        )
        .expect_err("wrong signer key must fail");

        assert!(matches!(
            err,
            ProofTokenIssuanceIngestError::InvalidSignature { .. }
        ));
    }
}
