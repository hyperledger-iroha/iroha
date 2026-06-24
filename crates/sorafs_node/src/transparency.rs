//! Local SFM-4c transparency aggregate worker helpers.

use std::{
    collections::{BTreeMap, BTreeSet},
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use iroha_crypto::sorafs::proof_token::{
    ModerationAction as ProofTokenModerationAction, ProofToken,
};
use iroha_data_model::sorafs::{
    gar::{GarEnforcementActionV1, GarEnforcementReceiptV1},
    transparency::{
        MODERATION_LEDGER_ENTRY_VERSION_V1, MODERATION_PRIVACY_AGGREGATE_VERSION_V1,
        ModerationLedgerEntryKindV1, ModerationLedgerEntryV1, ModerationLedgerMetadataV1,
        ModerationPrivacyAggregateMetricV1, ModerationPrivacyAggregateV1, ModerationPrivacyModeV1,
        ModerationPrivacyParametersV1, PROOF_TOKEN_ISSUANCE_VERSION_V1, ProofTokenIssuanceV1,
    },
};
use norito::codec::Encode as NoritoEncode;
use sorafs_manifest::{
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
const DETERMINISTIC_NOISE_DOMAIN_V1: &[u8] = b"sorafs.node.transparency.privacy_aggregate.noise.v1";
const CYCLE_ID_DOMAIN_V1: &[u8] = b"sorafs.node.transparency.privacy_aggregate.cycle_id.v1";
const TRANSPARENCY_LEDGER_ENTRY_ID_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.ledger.source_entry_id.v1";

/// One privacy-safe source entry admitted into the local transparency ledger worker.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    metadata.insert("deposit_xor".to_string(), report.deposit_xor.clone());
    metadata.insert("held_xor".to_string(), report.held.amount_xor.clone());
    metadata.insert("outcome".to_string(), report.outcome.as_str().to_string());
    metadata.insert("panel_size".to_string(), report.panel_size.to_string());
    metadata.insert("refund_xor".to_string(), report.refund.amount_xor.clone());
    metadata.insert(
        "rewards_forfeited_treasury_xor".to_string(),
        report.rewards_forfeited_treasury_xor.clone(),
    );
    metadata.insert(
        "rewards_paid_total_xor".to_string(),
        report.rewards_paid_total_xor.clone(),
    );
    metadata.insert(
        "treasury_xor".to_string(),
        report.treasury.amount_xor.clone(),
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
    metadata.insert("amount_xor".to_string(), receipt.amount_xor.clone());
    metadata.insert(
        "configured_signer_count".to_string(),
        receipt.configured_signer_count.to_string(),
    );
    metadata.insert("held_xor".to_string(), receipt.held_xor.clone());
    metadata.insert("outcome".to_string(), receipt.outcome.as_str().to_string());
    metadata.insert("refund_xor".to_string(), receipt.refund_xor.clone());
    metadata.insert(
        "required_authority".to_string(),
        receipt.required_authority.clone(),
    );
    metadata.insert(
        "settlement_step".to_string(),
        receipt.submitted_step.clone(),
    );
    metadata.insert("treasury_xor".to_string(), receipt.treasury_xor.clone());
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

/// One source metric observed for a privacy aggregate event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyAggregateSourceMetric {
    /// Stable metric key.
    pub key: String,
    /// Raw event contribution before privacy processing.
    pub value: u64,
    /// Unit label for the metric.
    pub unit: String,
}

/// One source event admitted into the local SFM-4c aggregate worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyAggregateSourceEvent {
    /// Stable event id used for duplicate suppression.
    pub event_id: String,
    /// Unix timestamp in seconds when the source event occurred.
    pub occurred_at_unix: u64,
    /// Public population label for the aggregate bucket.
    pub population_label: String,
    /// Optional private selector digest; when absent the label is hashed.
    pub population_digest: Option<[u8; 32]>,
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
    /// Optional deterministic noise seed. Required when `noise_scale_micros` is present.
    pub noise_seed: Option<[u8; 32]>,
    /// Optional aggregate policy/configuration digest.
    pub policy_digest: Option<[u8; 32]>,
    /// Public metadata copied into every generated aggregate.
    pub metadata: Vec<ModerationLedgerMetadataV1>,
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
        if privacy.noise_scale_micros.is_some() {
            let seed = self
                .noise_seed
                .as_ref()
                .ok_or(PrivacyAggregateWorkerError::MissingNoiseSeed)?;
            require_nonzero32("noise_seed", seed)?;
        }
        if let Some(digest) = &self.policy_digest {
            require_nonzero32("policy_digest", digest)?;
        }
        validate_metadata(&self.metadata)?;
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
    /// A noise scale was configured without runtime seed material.
    #[error("privacy aggregate noise scale requires a runtime noise seed")]
    MissingNoiseSeed,
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
    events: &[PrivacyAggregateSourceEvent],
) -> Result<Vec<ModerationPrivacyAggregateV1>, PrivacyAggregateWorkerError> {
    config.validate()?;
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
        .filter(|bucket| (bucket.len() as u64) < suppression_threshold)
        .count() as u64;
    let mut aggregates = Vec::new();
    for (population, mut bucket) in groups {
        bucket.sort_by(|left, right| left.event_id.cmp(&right.event_id));
        if (bucket.len() as u64) < suppression_threshold {
            continue;
        }
        let aggregate = build_population_aggregate(
            cycle_start_unix,
            cycle_end_unix,
            generated_at_unix,
            config,
            suppressed_count,
            population,
            &bucket,
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
    generated_at_unix: u64,
    config: &PrivacyAggregateCycleConfig,
    suppressed_count: u64,
    population: PopulationKey,
    events: &[PrivacyAggregateSourceEvent],
) -> Result<ModerationPrivacyAggregateV1, PrivacyAggregateWorkerError> {
    let mut metrics = BTreeMap::<String, (String, u128)>::new();
    let policy_digest = resolve_policy_digest(config.policy_digest, events)?;
    for event in events {
        for metric in &event.metrics {
            match metrics.entry(metric.key.clone()) {
                std::collections::btree_map::Entry::Occupied(mut occupied) => {
                    if occupied.get().0 != metric.unit {
                        return Err(PrivacyAggregateWorkerError::ConflictingMetricUnit {
                            key: metric.key.clone(),
                        });
                    }
                    occupied.get_mut().1 =
                        occupied.get().1.saturating_add(u128::from(metric.value));
                }
                std::collections::btree_map::Entry::Vacant(vacant) => {
                    vacant.insert((metric.unit.clone(), u128::from(metric.value)));
                }
            }
        }
    }

    let aggregate_id = aggregate_id(&config.aggregate_id_prefix, &population);
    let source_payload_digest =
        source_payload_digest(config, &population, events, suppressed_count, policy_digest);
    let published_metrics = metrics
        .into_iter()
        .map(|(key, (unit, value))| {
            let noised =
                apply_metric_noise(value, config, &aggregate_id, &key, &source_payload_digest);
            ModerationPrivacyAggregateMetricV1 {
                key,
                value: noised,
                unit,
            }
        })
        .collect::<Vec<_>>();

    let mut privacy = config.privacy;
    privacy.suppressed_count = suppressed_count;
    let aggregate = ModerationPrivacyAggregateV1 {
        version: MODERATION_PRIVACY_AGGREGATE_VERSION_V1,
        aggregate_id,
        window_start_unix: cycle_start_unix,
        window_end_unix: cycle_end_unix,
        generated_at_unix,
        population_label: population.label,
        population_digest: population.digest,
        privacy,
        source_event_count: events.len() as u64,
        source_payload_digest,
        metrics: published_metrics,
        policy_digest,
        metadata: config.metadata.clone(),
    };
    aggregate
        .validate()
        .map_err(|err| PrivacyAggregateWorkerError::InvalidAggregate {
            message: err.to_string(),
        })?;
    Ok(aggregate)
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
    aggregate_id: &str,
    metric_key: &str,
    source_payload_digest: &[u8; 32],
) -> u64 {
    let Some(scale_micros) = config.privacy.noise_scale_micros else {
        return value.min(u128::from(u64::MAX)) as u64;
    };
    let Some(seed) = &config.noise_seed else {
        return value.min(u128::from(u64::MAX)) as u64;
    };
    let bound = scale_micros.div_ceil(1_000_000).max(1);
    let range = u128::from(bound).saturating_mul(2).saturating_add(1);
    let mut hasher = blake3::Hasher::new();
    hasher.update(DETERMINISTIC_NOISE_DOMAIN_V1);
    hasher.update(seed);
    hasher.update(source_payload_digest);
    hasher.update(aggregate_id.as_bytes());
    hasher.update(metric_key.as_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest.as_bytes()[..16]);
    let sample = u128::from_le_bytes(bytes) % range;
    let negative = sample < u128::from(bound);
    let magnitude = if negative {
        u128::from(bound) - sample
    } else {
        sample - u128::from(bound)
    };
    let adjusted = if negative {
        value.saturating_sub(magnitude)
    } else {
        value.saturating_add(magnitude)
    };
    adjusted.min(u128::from(u64::MAX)) as u64
}

fn source_payload_digest(
    config: &PrivacyAggregateCycleConfig,
    population: &PopulationKey,
    events: &[PrivacyAggregateSourceEvent],
    suppressed_count: u64,
    policy_digest: Option<[u8; 32]>,
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(SOURCE_PAYLOAD_DIGEST_DOMAIN_V1);
    hash_text(&mut hasher, &config.aggregate_id_prefix);
    hash_privacy_parameters(&mut hasher, config.privacy, suppressed_count);
    if let Some(seed) = &config.noise_seed {
        hasher.update(seed);
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
    hash_option_u64(hasher, privacy.epsilon_micros);
    hash_option_u64(hasher, privacy.delta_ppb);
    hash_option_u64(hasher, privacy.noise_scale_micros);
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
            deposit_xor: "420".to_string(),
            refund: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "refund-account".to_string(),
                amount_xor: "420".to_string(),
            },
            treasury: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "treasury-account".to_string(),
                amount_xor: "50".to_string(),
            },
            held: SoraFsAppealFinanceAccountFlowV1 {
                account_id: "escrow-account".to_string(),
                amount_xor: "0".to_string(),
            },
            panel_size: 3,
            panel_reward_total_xor: "85".to_string(),
            rewards_paid_total_xor: "60".to_string(),
            rewards_forfeited_treasury_xor: "25".to_string(),
            juror_payouts: vec![
                SoraFsAppealFinanceJurorPayoutV1 {
                    juror_id: "juror-a".to_string(),
                    stipend_xor: "25".to_string(),
                    bonus_xor: "5".to_string(),
                    total_xor: "30".to_string(),
                },
                SoraFsAppealFinanceJurorPayoutV1 {
                    juror_id: "juror-b".to_string(),
                    stipend_xor: "25".to_string(),
                    bonus_xor: "5".to_string(),
                    total_xor: "30".to_string(),
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
            amount_xor: "25".to_string(),
            tx_hash_hex: "22".repeat(32),
            reconciliation_digest_hex: "33".repeat(32),
            reconciliation_status: "pending".to_string(),
            observed_lifecycle_status: "funded".to_string(),
            observed_remaining_xor: "420".to_string(),
            deposit_xor: "420".to_string(),
            refund_xor: "0".to_string(),
            treasury_xor: "25".to_string(),
            held_xor: "395".to_string(),
            panel_size: 3,
            configured_signer_count: 2,
        }
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
