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
    SoraFsAppealFinanceReportV1, SoraFsAppealFinanceSettlementReceiptV1,
    SoraFsModerationBallotGovernanceEventV1,
};
use thiserror::Error;

const SOURCE_ENTRY_SUBJECT_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.source_entry.subject.v1";
const SOURCE_ENTRY_SUMMARY_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.source_entry.summary.v1";
const SOURCE_PAYLOAD_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_aggregate.source_payload.v1";
const POPULATION_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_aggregate.population.v1";
const DISCRETE_LAPLACE_NOISE_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_aggregate.discrete_laplace.v1";
const NOISE_RANDOMNESS_COMMITMENT_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_aggregate.randomness_commitment.v1";
const PRIVACY_BUDGET_ENTRY_DOMAIN_V1: &[u8] = b"sorafs.node.transparency.privacy_budget.entry.v1";
const NOISE_RANDOMNESS_COMMITMENT_METADATA_KEY_V1: &str = "cycle_randomness_commitment_blake3";
const PRIVACY_BUDGET_LEDGER_VERSION_V1: u8 = 1;
const PRIVACY_BUDGET_MAX_POLICIES_V1: usize = 64;
const PRIVACY_BUDGET_MAX_CHARGES_V1: usize = 4_096;
const MAX_DISCRETE_LAPLACE_MEAN_SUCCESSES_V1: u128 = 4_096;
const MAX_DISCRETE_LAPLACE_RANDOM_DRAWS_V1: u64 = 1_048_576;
const CYCLE_ID_DOMAIN_V1: &[u8] = b"sorafs.node.transparency.privacy_aggregate.cycle_id.v1";
const CYCLE_PRF_REQUEST_BINDING_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_aggregate.cycle_prf_request.v1";
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
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PrivacyAggregateSourceMetric {
    /// Stable metric key.
    pub key: String,
    /// Raw event contribution before privacy processing.
    pub value: u64,
    /// Unit label for the metric.
    pub unit: String,
}

/// One source event admitted into the local SFM-4c aggregate worker.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct PrivacyAggregateSourceEvent {
    /// Stable event id used for duplicate suppression.
    pub event_id: String,
    /// Unix timestamp in seconds when the source event occurred.
    pub occurred_at_unix: u64,
    /// Public population label for the aggregate bucket.
    pub population_label: String,
    /// Optional private selector digest; when absent the label is hashed.
    pub population_digest: Option<[u8; 32]>,
    /// Digest of the canonical private subject identifier used for clipping.
    pub subject_digest: [u8; 32],
    /// Raw source metrics for this event, sorted by key.
    pub metrics: Vec<PrivacyAggregateSourceMetric>,
    /// Optional policy digest associated with this event.
    pub policy_digest: Option<[u8; 32]>,
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
        if let Some(digest) = &self.population_digest {
            require_nonzero32("population_digest", digest)?;
        }
        require_nonzero32("subject_digest", &self.subject_digest)?;
        if let Some(digest) = &self.policy_digest {
            require_nonzero32("policy_digest", digest)?;
        }
        validate_source_metrics(&self.metrics)
    }
}

/// Configuration used to build one aggregate publication cycle from source events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyAggregateCycleConfig {
    /// Prefix used when deriving public aggregate identifiers.
    pub aggregate_id_prefix: String,
    /// Explicit privacy parameters applied to every generated aggregate.
    pub privacy: ModerationPrivacyParametersV1,
    /// Optional aggregate policy/configuration digest.
    pub policy_digest: Option<[u8; 32]>,
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
        require_public_text("aggregate_id_prefix", &self.aggregate_id_prefix)?;
        let mut privacy = self.privacy;
        privacy.suppressed_count = 0;
        privacy.validate().map_err(|err| {
            PrivacyAggregateWorkerError::InvalidPrivacyParameters {
                message: err.to_string(),
            }
        })?;
        if privacy.per_subject_metric_cap.is_some() {
            validate_discrete_laplace_resource_policy(privacy)?;
        }
        if let Some(digest) = &self.policy_digest {
            require_nonzero32("policy_digest", digest)?;
        }
        validate_metadata(&self.metadata)?;
        if self
            .metadata
            .iter()
            .any(|item| item.key == NOISE_RANDOMNESS_COMMITMENT_METADATA_KEY_V1)
        {
            return Err(PrivacyAggregateWorkerError::ReservedMetadataKey {
                key: NOISE_RANDOMNESS_COMMITMENT_METADATA_KEY_V1,
            });
        }
        Ok(())
    }
}

/// Schedule used by the local aggregate worker to choose due publication windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivacyAggregateScheduleConfig {
    /// Width of each aggregation window, in seconds.
    pub cycle_seconds: u64,
    /// Delay after a cycle closes before it becomes eligible for publication.
    pub publish_delay_seconds: u64,
}

impl PrivacyAggregateScheduleConfig {
    pub(crate) fn validate(&self) -> Result<(), PrivacyAggregateWorkerError> {
        if self.cycle_seconds == 0 {
            return Err(PrivacyAggregateWorkerError::InvalidSchedule {
                field: "cycle_seconds",
            });
        }
        Ok(())
    }

    pub(crate) fn due_window(
        &self,
        now_unix: u64,
    ) -> Result<Option<PrivacyAggregateCycleWindow>, PrivacyAggregateWorkerError> {
        self.validate()?;
        if now_unix <= self.publish_delay_seconds {
            return Ok(None);
        }
        let adjusted = now_unix.saturating_sub(self.publish_delay_seconds);
        let cycle_end_unix = (adjusted / self.cycle_seconds) * self.cycle_seconds;
        if cycle_end_unix == 0 {
            return Ok(None);
        }
        let cycle_start_unix = cycle_end_unix.saturating_sub(self.cycle_seconds);
        Ok(Some(PrivacyAggregateCycleWindow {
            cycle_start_unix,
            cycle_end_unix,
            due_at_unix: cycle_end_unix.saturating_add(self.publish_delay_seconds),
        }))
    }

    pub(crate) fn event_window(
        &self,
        occurred_at_unix: u64,
    ) -> Result<Option<PrivacyAggregateCycleWindow>, PrivacyAggregateWorkerError> {
        self.validate()?;
        let cycle_start_unix = (occurred_at_unix / self.cycle_seconds) * self.cycle_seconds;
        if cycle_start_unix == 0 {
            return Ok(None);
        }
        let cycle_end_unix = cycle_start_unix.saturating_add(self.cycle_seconds);
        Ok(Some(PrivacyAggregateCycleWindow {
            cycle_start_unix,
            cycle_end_unix,
            due_at_unix: cycle_end_unix.saturating_add(self.publish_delay_seconds),
        }))
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
    policy_digest: [u8; 32],
    cycle_id: [u8; 16],
    cycle_start_unix: u64,
    cycle_end_unix: u64,
    due_at_unix: u64,
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
        policy_digest: [u8; 32],
        window: PrivacyAggregateCycleWindow,
    ) -> Result<Self, PrivacyCyclePrfRequestErrorV1> {
        if policy_digest == [0; 32] {
            return Err(PrivacyCyclePrfRequestErrorV1::MissingPolicyDigest);
        }
        if window.cycle_start_unix == 0
            || window.cycle_end_unix <= window.cycle_start_unix
            || window.due_at_unix < window.cycle_end_unix
        {
            return Err(PrivacyCyclePrfRequestErrorV1::InvalidWindow);
        }
        let cycle_id = privacy_aggregate_cycle_id(window);
        let mut hasher = blake3::Hasher::new();
        hasher.update(CYCLE_PRF_REQUEST_BINDING_DOMAIN_V1);
        hasher.update(&PRIVACY_CYCLE_PRF_REQUEST_VERSION_V1.to_le_bytes());
        hasher.update(&policy_digest);
        hasher.update(&cycle_id);
        hasher.update(&window.cycle_start_unix.to_le_bytes());
        hasher.update(&window.cycle_end_unix.to_le_bytes());
        hasher.update(&window.due_at_unix.to_le_bytes());
        Ok(Self {
            version: PRIVACY_CYCLE_PRF_REQUEST_VERSION_V1,
            policy_digest,
            cycle_id,
            cycle_start_unix: window.cycle_start_unix,
            cycle_end_unix: window.cycle_end_unix,
            due_at_unix: window.due_at_unix,
            binding_digest: *hasher.finalize().as_bytes(),
        })
    }

    /// Return the contract version.
    #[must_use]
    pub const fn version(&self) -> u16 {
        self.version
    }

    /// Return the governed privacy-policy digest.
    #[must_use]
    pub const fn policy_digest(&self) -> [u8; 32] {
        self.policy_digest
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

    /// Return the timestamp at which this exact cycle became due.
    #[must_use]
    pub const fn due_at_unix(&self) -> u64 {
        self.due_at_unix
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
    /// The governed privacy-policy digest was all zeroes.
    #[error("privacy cycle PRF request requires a non-zero policy digest")]
    MissingPolicyDigest,
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
    ) -> Result<[u8; 32], PrivacyCyclePrfProviderErrorV1>;
}

pub(crate) fn privacy_aggregate_cycle_id(window: PrivacyAggregateCycleWindow) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(CYCLE_ID_DOMAIN_V1);
    hasher.update(&window.cycle_start_unix.to_le_bytes());
    hasher.update(&window.cycle_end_unix.to_le_bytes());
    hasher.update(&window.due_at_unix.to_le_bytes());
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
    /// Metric accumulation exceeded the exact V1 integer representation.
    #[error("privacy aggregate metric arithmetic overflow")]
    MetricArithmeticOverflow,
    /// Source metric keys are not sorted.
    #[error("privacy aggregate source metric keys must be sorted")]
    SourceMetricKeysUnsorted,
    /// Source metric key appears more than once.
    #[error("duplicate privacy aggregate source metric key `{key}`")]
    DuplicateSourceMetricKey {
        /// Duplicate key.
        key: String,
    },
    /// Metadata keys are not sorted.
    #[error("privacy aggregate metadata keys must be sorted")]
    MetadataKeysUnsorted,
    /// Metadata key appears more than once.
    #[error("duplicate privacy aggregate metadata key `{key}`")]
    DuplicateMetadataKey {
        /// Duplicate key.
        key: String,
    },
    /// Privacy parameters are structurally invalid.
    #[error("invalid privacy aggregate parameters: {message}")]
    InvalidPrivacyParameters {
        /// Validation detail.
        message: String,
    },
    /// Differential privacy was configured without hidden cycle PRF output.
    #[error("privacy aggregate differential privacy requires hidden cycle PRF output")]
    MissingCyclePrfOutput,
    /// Hidden PRF output was supplied for a policy that does not use DP.
    #[error(
        "privacy aggregate cycle PRF output is forbidden when differential privacy is disabled"
    )]
    UnexpectedCyclePrfOutput,
    /// The governed epsilon/cap parameters would exceed the bounded exact sampler policy.
    #[error(
        "privacy aggregate exact sampler parameters exceed the bounded resource policy: epsilon={epsilon_numerator}/{epsilon_denominator}, sensitivity={sensitivity}"
    )]
    NoiseParametersExceedResourceLimit {
        /// Governed reduced epsilon numerator.
        epsilon_numerator: u64,
        /// Governed reduced epsilon denominator.
        epsilon_denominator: u64,
        /// Integer sensitivity equal to the per-subject metric cap.
        sensitivity: u64,
    },
    /// The exact sampler exhausted its fail-closed random-draw budget.
    #[error("privacy aggregate exact discrete-Laplace sampler exhausted its random-draw budget")]
    NoiseSamplingLimitExceeded,
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
    /// No source events matched the requested cycle window.
    #[error("privacy aggregate cycle has no source events in the requested window")]
    NoSourceEvents,
    /// A source event timestamp is outside the requested cycle window.
    #[error("privacy aggregate source event `{event_id}` is outside the requested cycle window")]
    EventOutsideCycle {
        /// Event id.
        event_id: String,
    },
    /// The same source event id appeared twice in the build input.
    #[error("duplicate privacy aggregate source event `{event_id}`")]
    DuplicateSourceEvent {
        /// Event id.
        event_id: String,
    },
    /// Source events for one aggregate carry conflicting metric units.
    #[error("privacy aggregate metric `{key}` has conflicting units")]
    ConflictingMetricUnit {
        /// Metric key.
        key: String,
    },
    /// Source events for one aggregate carry conflicting policy digests.
    #[error("privacy aggregate source events carry conflicting policy digests")]
    ConflictingPolicyDigest,
    /// All source buckets were suppressed.
    #[error("privacy aggregate cycle suppressed every source bucket")]
    AllBucketsSuppressed,
    /// Generated aggregate payload failed validation.
    #[error("generated privacy aggregate is invalid: {message}")]
    InvalidAggregate {
        /// Validation detail.
        message: String,
    },
}

pub(crate) fn build_privacy_aggregates_from_source_events(
    cycle_start_unix: u64,
    cycle_end_unix: u64,
    generated_at_unix: u64,
    config: &PrivacyAggregateCycleConfig,
    cycle_prf_output: Option<[u8; 32]>,
    events: &[PrivacyAggregateSourceEvent],
) -> Result<Vec<ModerationPrivacyAggregateV1>, PrivacyAggregateWorkerError> {
    config.validate()?;
    if config.privacy.per_subject_metric_cap.is_some() {
        let prf_output = cycle_prf_output
            .as_ref()
            .ok_or(PrivacyAggregateWorkerError::MissingCyclePrfOutput)?;
        require_nonzero32("cycle_prf_output", prf_output)?;
    } else if cycle_prf_output.is_some() {
        return Err(PrivacyAggregateWorkerError::UnexpectedCyclePrfOutput);
    }
    if events.is_empty() {
        return Err(PrivacyAggregateWorkerError::NoSourceEvents);
    }
    if cycle_start_unix == 0 || cycle_end_unix <= cycle_start_unix {
        return Err(PrivacyAggregateWorkerError::InvalidTimestamp {
            field: "cycle_window",
        });
    }
    if generated_at_unix < cycle_end_unix {
        return Err(PrivacyAggregateWorkerError::InvalidTimestamp {
            field: "generated_at_unix",
        });
    }

    let mut seen_events = BTreeSet::new();
    let mut groups = BTreeMap::<PopulationKey, Vec<PrivacyAggregateSourceEvent>>::new();
    for event in events {
        event.validate()?;
        if event.occurred_at_unix < cycle_start_unix || event.occurred_at_unix >= cycle_end_unix {
            return Err(PrivacyAggregateWorkerError::EventOutsideCycle {
                event_id: event.event_id.clone(),
            });
        }
        if !seen_events.insert(event.event_id.clone()) {
            return Err(PrivacyAggregateWorkerError::DuplicateSourceEvent {
                event_id: event.event_id.clone(),
            });
        }
        let population_digest = event
            .population_digest
            .unwrap_or_else(|| population_digest_from_label(&event.population_label));
        groups
            .entry(PopulationKey {
                label: event.population_label.clone(),
                digest: population_digest,
            })
            .or_default()
            .push(event.clone());
    }

    let suppression_threshold = config.privacy.suppression_threshold.unwrap_or(0);
    let suppressed_count = groups
        .values()
        .filter(|bucket| distinct_subject_count(bucket) < suppression_threshold)
        .count() as u64;
    let build_context = PopulationAggregateBuildContext {
        cycle_start_unix,
        cycle_end_unix,
        generated_at_unix,
        config,
        cycle_prf_output: cycle_prf_output.as_ref(),
        suppressed_count,
    };
    let mut aggregates = Vec::new();
    for (population, mut bucket) in groups {
        bucket.sort_by(|left, right| left.event_id.cmp(&right.event_id));
        if distinct_subject_count(&bucket) < suppression_threshold {
            continue;
        }
        let aggregate = build_population_aggregate(&build_context, population, &bucket)?;
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

struct PopulationAggregateBuildContext<'a> {
    cycle_start_unix: u64,
    cycle_end_unix: u64,
    generated_at_unix: u64,
    config: &'a PrivacyAggregateCycleConfig,
    cycle_prf_output: Option<&'a [u8; 32]>,
    suppressed_count: u64,
}

fn build_population_aggregate(
    context: &PopulationAggregateBuildContext<'_>,
    population: PopulationKey,
    events: &[PrivacyAggregateSourceEvent],
) -> Result<ModerationPrivacyAggregateV1, PrivacyAggregateWorkerError> {
    let config = context.config;
    let cycle_prf_output = context.cycle_prf_output;
    let policy_digest = resolve_policy_digest(config.policy_digest, events)?;
    let metrics = clipped_population_metrics(events, config.privacy.per_subject_metric_cap)?;
    let source_subject_count = distinct_subject_count(events);

    let aggregate_id = aggregate_id(&config.aggregate_id_prefix, &population);
    let source_payload_digest = source_payload_digest(
        config,
        cycle_prf_output,
        &population,
        events,
        context.suppressed_count,
        policy_digest,
    );
    let published_metrics = metrics
        .into_iter()
        .map(|(key, (unit, value))| {
            let noised = apply_metric_noise(
                value,
                config,
                cycle_prf_output,
                &aggregate_id,
                &key,
                &source_payload_digest,
            )?;
            Ok(ModerationPrivacyAggregateMetricV1 {
                key,
                value: noised,
                unit,
            })
        })
        .collect::<Result<Vec<_>, PrivacyAggregateWorkerError>>()?;

    let mut privacy = config.privacy;
    privacy.suppressed_count = context.suppressed_count;
    let aggregate = ModerationPrivacyAggregateV1 {
        version: MODERATION_PRIVACY_AGGREGATE_VERSION_V1,
        aggregate_id,
        window_start_unix: context.cycle_start_unix,
        window_end_unix: context.cycle_end_unix,
        generated_at_unix: context.generated_at_unix,
        population_label: population.label,
        population_digest: population.digest,
        privacy,
        source_event_count: events.len() as u64,
        source_subject_count,
        source_payload_digest,
        metrics: published_metrics,
        policy_digest,
        metadata: publication_metadata(config, cycle_prf_output),
    };
    aggregate
        .validate()
        .map_err(|err| PrivacyAggregateWorkerError::InvalidAggregate {
            message: err.to_string(),
        })?;
    Ok(aggregate)
}

fn distinct_subject_count(events: &[PrivacyAggregateSourceEvent]) -> u64 {
    events
        .iter()
        .map(|event| event.subject_digest)
        .collect::<BTreeSet<_>>()
        .len() as u64
}

fn clipped_population_metrics(
    events: &[PrivacyAggregateSourceEvent],
    per_subject_metric_cap: Option<u64>,
) -> Result<BTreeMap<String, (String, u128)>, PrivacyAggregateWorkerError> {
    let mut units = BTreeMap::<String, String>::new();
    let mut subject_metrics = BTreeMap::<[u8; 32], BTreeMap<String, u128>>::new();
    for event in events {
        let per_subject = subject_metrics.entry(event.subject_digest).or_default();
        for metric in &event.metrics {
            match units.entry(metric.key.clone()) {
                std::collections::btree_map::Entry::Occupied(occupied) => {
                    if occupied.get() != &metric.unit {
                        return Err(PrivacyAggregateWorkerError::ConflictingMetricUnit {
                            key: metric.key.clone(),
                        });
                    }
                }
                std::collections::btree_map::Entry::Vacant(vacant) => {
                    vacant.insert(metric.unit.clone());
                }
            }
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

    let mut totals = units
        .into_iter()
        .map(|(key, unit)| (key, (unit, 0_u128)))
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

fn resolve_policy_digest(
    configured: Option<[u8; 32]>,
    events: &[PrivacyAggregateSourceEvent],
) -> Result<Option<[u8; 32]>, PrivacyAggregateWorkerError> {
    let mut resolved = configured;
    for event in events {
        if let Some(event_digest) = event.policy_digest {
            match resolved {
                Some(current) if current != event_digest => {
                    return Err(PrivacyAggregateWorkerError::ConflictingPolicyDigest);
                }
                Some(_) => {}
                None => resolved = Some(event_digest),
            }
        }
    }
    Ok(resolved)
}

fn apply_metric_noise(
    value: u128,
    config: &PrivacyAggregateCycleConfig,
    cycle_prf_output: Option<&[u8; 32]>,
    aggregate_id: &str,
    metric_key: &str,
    source_payload_digest: &[u8; 32],
) -> Result<u64, PrivacyAggregateWorkerError> {
    let Some(sensitivity) = config.privacy.per_subject_metric_cap else {
        return u64::try_from(value)
            .map_err(|_| PrivacyAggregateWorkerError::MetricArithmeticOverflow);
    };
    let prf_output = cycle_prf_output.ok_or(PrivacyAggregateWorkerError::MissingCyclePrfOutput)?;
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
    let mut hasher = blake3::Hasher::new_keyed(prf_output);
    hasher.update(DISCRETE_LAPLACE_NOISE_DOMAIN_V1);
    hasher.update(source_payload_digest);
    hash_text(&mut hasher, aggregate_id);
    hash_text(&mut hasher, metric_key);
    let mut sampler = ExactNoiseSampler::new(hasher.finalize_xof());
    let noise =
        sampler.sample_discrete_laplace(epsilon_numerator, epsilon_denominator, sensitivity)?;
    let adjusted = if noise.is_negative() {
        value.saturating_sub(noise.unsigned_abs())
    } else {
        value
            .checked_add(noise as u128)
            .ok_or(PrivacyAggregateWorkerError::MetricArithmeticOverflow)?
    };
    u64::try_from(adjusted).map_err(|_| PrivacyAggregateWorkerError::MetricArithmeticOverflow)
}

struct ExactNoiseSampler {
    reader: blake3::OutputReader,
    remaining_draws: u64,
}

impl ExactNoiseSampler {
    fn new(reader: blake3::OutputReader) -> Self {
        Self {
            reader,
            remaining_draws: MAX_DISCRETE_LAPLACE_RANDOM_DRAWS_V1,
        }
    }

    fn sample_discrete_laplace(
        &mut self,
        epsilon_numerator: u64,
        epsilon_denominator: u64,
        sensitivity: u64,
    ) -> Result<i128, PrivacyAggregateWorkerError> {
        validate_discrete_laplace_parameters(epsilon_numerator, epsilon_denominator, sensitivity)?;
        // Let q = ΔD / (ΔD + N), where ε = N/D and Δ is sensitivity.
        // The difference of two independent geometric(q) variates is an exact
        // two-sided geometric (discrete-Laplace) variate. Its privacy loss is
        // -ln(q) = ln(1 + N/(ΔD)) <= N/(ΔD), so it is conservatively bounded
        // by the governed rational ε without floating-point approximation.
        let continuation_numerator =
            u128::from(sensitivity).saturating_mul(u128::from(epsilon_denominator));
        let geometric_denominator =
            continuation_numerator.saturating_add(u128::from(epsilon_numerator));
        let positive = self.sample_geometric(continuation_numerator, geometric_denominator)?;
        let negative = self.sample_geometric(continuation_numerator, geometric_denominator)?;
        Ok(i128::from(positive) - i128::from(negative))
    }

    fn sample_geometric(
        &mut self,
        continuation_numerator: u128,
        denominator: u128,
    ) -> Result<u64, PrivacyAggregateWorkerError> {
        let mut successes = 0_u64;
        loop {
            if self.uniform_below(denominator)? >= continuation_numerator {
                return Ok(successes);
            }
            successes = successes
                .checked_add(1)
                .ok_or(PrivacyAggregateWorkerError::NoiseSamplingLimitExceeded)?;
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
            if self.remaining_draws == 0 {
                return Err(PrivacyAggregateWorkerError::NoiseSamplingLimitExceeded);
            }
            self.remaining_draws -= 1;
            let mut bytes = [0_u8; 16];
            self.reader.fill(&mut bytes);
            let candidate = u128::from_le_bytes(bytes);
            if candidate >= rejection_threshold {
                return Ok(candidate % upper_exclusive);
            }
        }
    }
}

fn validate_discrete_laplace_resource_policy(
    privacy: ModerationPrivacyParametersV1,
) -> Result<(), PrivacyAggregateWorkerError> {
    let epsilon_numerator =
        privacy
            .epsilon_numerator
            .ok_or(PrivacyAggregateWorkerError::InvalidPrivacyParameters {
                message: "epsilon_numerator is required for exact discrete-Laplace noise"
                    .to_string(),
            })?;
    let epsilon_denominator = privacy.epsilon_denominator.ok_or(
        PrivacyAggregateWorkerError::InvalidPrivacyParameters {
            message: "epsilon_denominator is required for exact discrete-Laplace noise".to_string(),
        },
    )?;
    let sensitivity = privacy.per_subject_metric_cap.ok_or(
        PrivacyAggregateWorkerError::InvalidPrivacyParameters {
            message: "per_subject_metric_cap is required for exact discrete-Laplace noise"
                .to_string(),
        },
    )?;
    validate_discrete_laplace_parameters(epsilon_numerator, epsilon_denominator, sensitivity)
}

fn validate_discrete_laplace_parameters(
    epsilon_numerator: u64,
    epsilon_denominator: u64,
    sensitivity: u64,
) -> Result<(), PrivacyAggregateWorkerError> {
    let sensitivity_numerator =
        u128::from(sensitivity).saturating_mul(u128::from(epsilon_denominator));
    let maximum_numerator =
        u128::from(epsilon_numerator).saturating_mul(MAX_DISCRETE_LAPLACE_MEAN_SUCCESSES_V1);
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

fn noise_randomness_commitment(prf_output: &[u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(NOISE_RANDOMNESS_COMMITMENT_DOMAIN_V1);
    hasher.update(prf_output);
    *hasher.finalize().as_bytes()
}

fn publication_metadata(
    config: &PrivacyAggregateCycleConfig,
    cycle_prf_output: Option<&[u8; 32]>,
) -> Vec<ModerationLedgerMetadataV1> {
    let mut metadata = config.metadata.clone();
    if let Some(prf_output) = cycle_prf_output {
        let commitment = ModerationLedgerMetadataV1 {
            key: NOISE_RANDOMNESS_COMMITMENT_METADATA_KEY_V1.to_string(),
            value: hex::encode(noise_randomness_commitment(prf_output)),
        };
        let index = metadata
            .binary_search_by(|item| {
                item.key
                    .as_str()
                    .cmp(NOISE_RANDOMNESS_COMMITMENT_METADATA_KEY_V1)
            })
            .unwrap_or_else(|index| index);
        metadata.insert(index, commitment);
    }
    metadata
}

fn source_payload_digest(
    config: &PrivacyAggregateCycleConfig,
    cycle_prf_output: Option<&[u8; 32]>,
    population: &PopulationKey,
    events: &[PrivacyAggregateSourceEvent],
    suppressed_count: u64,
    policy_digest: Option<[u8; 32]>,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(SOURCE_PAYLOAD_DIGEST_DOMAIN_V1);
    hash_text(&mut hasher, &config.aggregate_id_prefix);
    hash_privacy_parameters(&mut hasher, config.privacy, suppressed_count);
    if let Some(prf_output) = cycle_prf_output {
        hasher.update(&noise_randomness_commitment(prf_output));
    }
    if let Some(digest) = &policy_digest {
        hasher.update(digest);
    }
    hash_text(&mut hasher, &population.label);
    hasher.update(&population.digest);
    hasher.update(&(events.len() as u64).to_le_bytes());
    for event in events {
        hash_text(&mut hasher, &event.event_id);
        hasher.update(&event.occurred_at_unix.to_le_bytes());
        hasher.update(&event.subject_digest);
        if let Some(digest) = &event.policy_digest {
            hasher.update(digest);
        }
        hasher.update(&(event.metrics.len() as u64).to_le_bytes());
        for metric in &event.metrics {
            hash_text(&mut hasher, &metric.key);
            hash_text(&mut hasher, &metric.unit);
            hasher.update(&metric.value.to_le_bytes());
        }
    }
    *hasher.finalize().as_bytes()
}

fn hash_privacy_parameters(
    hasher: &mut blake3::Hasher,
    mut privacy: ModerationPrivacyParametersV1,
    suppressed_count: u64,
) {
    privacy.suppressed_count = suppressed_count;
    hasher.update(&privacy.version.to_le_bytes());
    hasher.update(privacy_mode_label(privacy.mode).as_bytes());
    hash_option_u64(hasher, privacy.epsilon_numerator);
    hash_option_u64(hasher, privacy.epsilon_denominator);
    hash_option_u64(hasher, privacy.delta_ppb);
    hash_option_u64(hasher, privacy.per_subject_metric_cap);
    hash_option_u64(hasher, privacy.suppression_threshold);
    hasher.update(&privacy.suppressed_count.to_le_bytes());
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

fn population_digest_from_label(label: &str) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(POPULATION_DIGEST_DOMAIN_V1);
    hasher.update(label.as_bytes());
    *hasher.finalize().as_bytes()
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
            return Err(PrivacyAggregateWorkerError::DuplicateSourceMetricKey {
                key: metric.key.clone(),
            });
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
            return Err(PrivacyAggregateWorkerError::DuplicateMetadataKey {
                key: item.key.clone(),
            });
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
            aggregate_id_prefix: "sfm4c-cycle".to_string(),
            privacy: ModerationPrivacyParametersV1 {
                version:
                    iroha_data_model::sorafs::transparency::MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
                mode: ModerationPrivacyModeV1::DifferentialPrivacyWithSuppression,
                epsilon_numerator: Some(4),
                epsilon_denominator: Some(5),
                delta_ppb: Some(0),
                per_subject_metric_cap: Some(1),
                suppression_threshold: Some(2),
                suppressed_count: 0,
            },
            policy_digest: Some([0xC0; 32]),
            metadata: vec![ModerationLedgerMetadataV1 {
                key: "publisher".to_string(),
                value: "sfm4c-worker".to_string(),
            }],
        }
    }

    fn privacy_event(event_id: &str, occurred_at_unix: u64) -> PrivacyAggregateSourceEvent {
        PrivacyAggregateSourceEvent {
            event_id: event_id.to_string(),
            occurred_at_unix,
            population_label: "jurisdiction-a".to_string(),
            population_digest: Some([0xA0; 32]),
            subject_digest: *blake3::hash(event_id.as_bytes()).as_bytes(),
            metrics: vec![PrivacyAggregateSourceMetric {
                key: "moderation_actions".to_string(),
                value: 1,
                unit: "count".to_string(),
            }],
            policy_digest: Some([0xC0; 32]),
        }
    }

    #[test]
    fn exact_discrete_laplace_is_deterministic_and_context_bound() {
        fn sample(context: &[u8]) -> i128 {
            let mut hasher = blake3::Hasher::new_keyed(&[0x5A; 32]);
            hasher.update(DISCRETE_LAPLACE_NOISE_DOMAIN_V1);
            hasher.update(context);
            ExactNoiseSampler::new(hasher.finalize_xof())
                .sample_discrete_laplace(4, 5, 1)
                .expect("bounded exact sample")
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
    fn privacy_metrics_clip_each_subject_before_population_sum() {
        let mut first = privacy_event("event-a", 110);
        first.metrics[0].value = 9;
        let mut repeated_subject = privacy_event("event-b", 120);
        repeated_subject.subject_digest = first.subject_digest;
        repeated_subject.metrics[0].value = 8;
        let mut second_subject = privacy_event("event-c", 130);
        second_subject.metrics[0].value = 7;

        let metrics =
            clipped_population_metrics(&[first, repeated_subject, second_subject], Some(10))
                .expect("clip contributions");
        assert_eq!(
            metrics.get("moderation_actions"),
            Some(&("count".to_string(), 17))
        );
    }

    #[test]
    fn suppression_counts_distinct_subjects_not_repeated_events() {
        let first = privacy_event("event-a", 110);
        let mut replayed_subject = privacy_event("event-b", 120);
        replayed_subject.subject_digest = first.subject_digest;
        let error = build_privacy_aggregates_from_source_events(
            100,
            200,
            201,
            &privacy_config(),
            Some([0x5A; 32]),
            &[first, replayed_subject],
        )
        .expect_err("one subject cannot satisfy k=2");
        assert_eq!(error, PrivacyAggregateWorkerError::AllBucketsSuppressed);
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
            suppressed_count: 0,
        };
        let mut first = privacy_event("event-a", 110);
        first.metrics[0].value = u64::MAX;
        let mut second = privacy_event("event-b", 120);
        second.metrics[0].value = u64::MAX;

        assert_eq!(
            build_privacy_aggregates_from_source_events(
                100,
                200,
                201,
                &config,
                None,
                &[first, second],
            ),
            Err(PrivacyAggregateWorkerError::MetricArithmeticOverflow)
        );
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
    }

    #[test]
    fn privacy_cycle_prf_request_binds_policy_and_exact_window() {
        let window = PrivacyAggregateCycleWindow {
            cycle_start_unix: 100,
            cycle_end_unix: 200,
            due_at_unix: 210,
        };
        let request =
            PrivacyCyclePrfRequestV1::new([0xC0; 32], window).expect("canonical PRF request");

        assert_eq!(request.version(), PRIVACY_CYCLE_PRF_REQUEST_VERSION_V1);
        assert_eq!(request.policy_digest(), [0xC0; 32]);
        assert_eq!(request.cycle_id(), privacy_aggregate_cycle_id(window));
        assert_eq!(request.cycle_start_unix(), 100);
        assert_eq!(request.cycle_end_unix(), 200);
        assert_eq!(request.due_at_unix(), 210);

        let other_policy =
            PrivacyCyclePrfRequestV1::new([0xC1; 32], window).expect("other policy request");
        let other_window = PrivacyCyclePrfRequestV1::new(
            [0xC0; 32],
            PrivacyAggregateCycleWindow {
                cycle_start_unix: 200,
                cycle_end_unix: 300,
                due_at_unix: 310,
            },
        )
        .expect("other window request");
        assert_ne!(request.binding_digest(), other_policy.binding_digest());
        assert_ne!(request.cycle_id(), other_window.cycle_id());
        assert_ne!(request.binding_digest(), other_window.binding_digest());
        assert_eq!(
            PrivacyCyclePrfRequestV1::new([0; 32], window),
            Err(PrivacyCyclePrfRequestErrorV1::MissingPolicyDigest)
        );
    }

    #[test]
    fn privacy_aggregate_publishes_commitment_not_runtime_noise_material() {
        let events = vec![privacy_event("event-a", 110), privacy_event("event-b", 120)];
        let config = privacy_config();

        let first = build_privacy_aggregates_from_source_events(
            100,
            200,
            201,
            &config,
            Some([0x5A; 32]),
            &events,
        )
        .expect("build aggregate");
        let second = build_privacy_aggregates_from_source_events(
            100,
            200,
            201,
            &config,
            Some([0x5A; 32]),
            &events,
        )
        .expect("rebuild aggregate");

        assert_eq!(first, second);
        assert_eq!(first.len(), 1);
        let aggregate = &first[0];
        let commitment_hex = hex::encode(noise_randomness_commitment(&[0x5A; 32]));
        assert!(aggregate.metadata.iter().any(|item| {
            item.key == NOISE_RANDOMNESS_COMMITMENT_METADATA_KEY_V1 && item.value == commitment_hex
        }));
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
            201,
            &privacy_config(),
            Some([0x5B; 32]),
            &events,
        )
        .expect("build with changed runtime output");
        assert_ne!(
            changed[0].source_payload_digest, aggregate.source_payload_digest,
            "the public source digest must bind the cycle randomness commitment"
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
            201,
            &config,
            Some([0x5A; 32]),
            &[privacy_event("event-a", 110), privacy_event("event-b", 120)],
        )
        .expect_err("unbounded expected sampler work must fail");

        assert_eq!(
            error,
            PrivacyAggregateWorkerError::NoiseParametersExceedResourceLimit {
                epsilon_numerator: 1,
                epsilon_denominator: 10_000,
                sensitivity: 1,
            }
        );
    }

    #[test]
    fn privacy_aggregate_rejects_caller_supplied_randomness_commitment() {
        let mut config = privacy_config();
        config.metadata = vec![ModerationLedgerMetadataV1 {
            key: NOISE_RANDOMNESS_COMMITMENT_METADATA_KEY_V1.to_string(),
            value: hex::encode([0x11; 32]),
        }];

        let error = config
            .validate()
            .expect_err("worker-owned commitment key must be reserved");
        assert_eq!(
            error,
            PrivacyAggregateWorkerError::ReservedMetadataKey {
                key: NOISE_RANDOMNESS_COMMITMENT_METADATA_KEY_V1,
            }
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
