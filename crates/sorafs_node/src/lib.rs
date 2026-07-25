//! SoraFS provider-node services and durable protocol implementations.

#![deny(missing_docs)]
#![allow(
    clippy::clone_on_copy,
    clippy::collapsible_if,
    clippy::field_reassign_with_default
)]

pub mod capacity;
pub mod config;
pub mod deal;
mod durable_transaction_forwarder;
mod economics;
mod governance;
pub mod metering;
mod moderation;
pub mod moderation_orchestrator;
mod native_repair_singleflight;
pub mod native_repair_worker;
pub mod orderbook_transaction_forwarder;
pub mod pdp_provider;
pub mod pop_credentials;
pub mod por;
pub mod potr;
pub mod proof_outcome_forwarder;
mod reconciliation;
pub mod repair_ledger_projection;
pub mod repair_transaction_forwarder;
pub mod reserve_transaction_forwarder;
pub mod scheduler;
pub mod store;
pub mod telemetry;
mod transparency;

pub use deal::{
    ClientSnapshot, DealEngine, DealEngineError, DealSettlementOutcome, DealSnapshot,
    ProviderSnapshot, UsageOutcome, derive_micropayment_ticket_id,
};
pub use economics::{
    EconomicsRuntimeError, GovernedPricingAdmissionOutcome, SignedHedgingFeedAdmissionOutcome,
};
pub use governance::FilesystemGovernancePublisher;
pub use moderation::{
    MODERATION_SCREENING_ADMISSION_RECEIPT_VERSION_V1,
    MODERATION_SCREENING_AUTHORITY_BUNDLE_VERSION_V1,
    ModerationAuthenticatedScreeningAdmissionError, ModerationAuthenticatedScreeningEvidenceV1,
    ModerationAuthenticatedScreeningOutcomeV1, ModerationAuthenticatedScreeningRequestV1,
    ModerationCorpusRegistryRecord, ModerationEvidenceViewerAccessEventRecord,
    ModerationEvidenceViewerAccessInput, ModerationEvidenceViewerAccessKind,
    ModerationEvidenceViewerAuditKindCount, ModerationEvidenceViewerAuditReport,
    ModerationEvidenceViewerAuditReportInput, ModerationEvidenceViewerError,
    ModerationEvidenceViewerSessionInput, ModerationEvidenceViewerSessionRecord,
    ModerationEvidenceViewerSnapshot, ModerationModelRegistryError,
    ModerationModelRegistrySnapshot, ModerationQuarantineKeyWrapper,
    ModerationQuarantineObjectError, ModerationQuarantineObjectInput,
    ModerationQuarantineObjectPayload, ModerationQuarantineObjectRangePayload,
    ModerationQuarantineObjectRecord, ModerationQuarantineObjectSnapshot,
    ModerationQuarantineRecord, ModerationQuarantineReleaseInput, ModerationQuarantineReviewInput,
    ModerationQuarantineState, ModerationReproRegistryRecord,
    ModerationScreeningAdmissionReceiptV1, ModerationScreeningAuthenticationError,
    ModerationScreeningAuthorityBundleV1, ModerationScreeningAuthorityV1, ModerationScreeningError,
    ModerationScreeningInput, ModerationScreeningOutcome, ModerationScreeningRecord,
    ModerationScreeningSnapshot, ModerationScreeningVerdict,
    verify_authenticated_moderation_screening_v1,
};
pub use pdp_provider::{
    PDP_STATUS_EXPORT_MAX_RECORDS_V1, PdpChallengeEnqueueOutcome, PdpChallengeLifecycleV1,
    PdpChallengeStatusV1, PdpGovernanceArchiveV1, PdpNextChallengeV1, PdpProofBuildError,
    PdpProviderProtocol, PdpProviderProtocolError, PdpProviderTelemetrySnapshot,
    PdpRejectionReasonV1, PdpTerminalDecisionV1, PdpTerminalHandoff, PdpTerminalOutcomeV1,
    build_signed_pdp_proof_v1,
};
pub use por::{
    ManifestVrfBundle, ManifestVrfKey, PlannedChallenge, PorChallengePlannerError,
    PorFailedRepairIntentV1, PorProtocolMetricsSnapshot, PorRandomness, PorRepairHandoff,
    PorRepairHandoffError, PorTracker, PorTrackerError, PorVerdictStats,
    build_por_challenge_for_manifest, canonical_por_failure_repair_report_v1,
    por_repair_source_identity_v1,
};
pub use potr::{
    POTR_EXPORT_MAX_RECORDS_V1, POTR_RECEIPT_MAX_CANONICAL_BYTES_V1,
    POTR_TRACKER_CHECKPOINT_FILE_NAME_V1, PotrReceiptStatusV1, PotrRecordOutcome, PotrTrackerError,
};
pub use proof_outcome_forwarder::{
    PROOF_OUTCOME_OUTBOX_DEFAULT_MAX_ATTEMPTS_V1, PROOF_OUTCOME_OUTBOX_MAX_SCAN_ITEMS_V1,
    ProofOutcomeDeadLetterReasonV1, ProofOutcomeDeadLetterV1, ProofOutcomeDeliveryStateV1,
    ProofOutcomeEnqueueResultV1, ProofOutcomeOutbox, ProofOutcomeOutboxError,
    ProofOutcomeOutboxPolicyV1, ProofOutcomePendingDeliveryV1,
};
use repair_ledger_projection::RepairLedgerTaskProjectionV1;
/// Outcome returned when recording a PoR verdict.
#[derive(Debug, Clone)]
pub struct PorVerdictOutcome {
    /// Statistics extracted from the verdict.
    pub stats: PorVerdictStats,
    /// Chain-authoritative native repair task identifier (failed verdicts only).
    pub repair_task_id: Option<[u8; 32]>,
    /// Current consecutive failure streak for this provider/manifest.
    pub consecutive_failures: u64,
    /// Recommended slash derived from the configured policy and provider bond state.
    pub slash: Option<SlashRecommendation>,
}

/// Slash recommendation produced after PoR verification failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlashRecommendation {
    /// Provider identifier targeted by the slash.
    pub provider_id: ProviderId,
    /// Manifest digest associated with the failed proof.
    pub manifest_digest: [u8; 32],
    /// Exact proposed penalty to be debited from the provider bond.
    pub penalty: XorQuantity,
    /// Failure streak length that triggered the slash.
    pub strikes: u32,
    /// Reason recorded for the recommendation.
    pub reason: String,
}

fn checked_por_penalty(
    bond_available: &XorQuantity,
    bond_locked: &XorQuantity,
    penalty_bond_bps: u16,
) -> Result<XorQuantity, sorafs_manifest::deal::DealAmountError> {
    bond_available
        .checked_add(bond_locked)?
        .checked_mul_basis_points(penalty_bond_bps)
}

/// Aggregated PoR ingestion status for a manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorIngestionStatus {
    /// Manifest digest covered by the snapshot.
    pub manifest_digest: [u8; 32],
    /// Provider-specific status entries.
    pub providers: Vec<PorIngestionProviderStatus>,
}

/// Provider-level PoR ingestion state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PorIngestionProviderStatus {
    /// Manifest digest served by the provider.
    pub manifest_digest: [u8; 32],
    /// Provider identifier.
    pub provider_id: [u8; 32],
    /// Outstanding challenge count.
    pub pending_challenges: u64,
    /// Oldest epoch identifier recorded in the backlog.
    pub oldest_epoch_id: Option<u64>,
    /// Earliest pending response deadline.
    pub oldest_response_deadline_unix: Option<u64>,
    /// Unix timestamp for the most recent success verdict.
    pub last_success_unix: Option<u64>,
    /// Unix timestamp for the most recent failure verdict.
    pub last_failure_unix: Option<u64>,
    /// Total failure count recorded for the manifest/provider pair.
    pub failures_total: u64,
    /// Consecutive failure streak length.
    pub consecutive_failures: u64,
}

/// Summary of an eviction action performed during GC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcEviction {
    /// Manifest identifier evicted by the sweep.
    pub manifest_id: String,
    /// Manifest digest evicted.
    pub manifest_digest: [u8; 32],
    /// Retention epoch recorded on the manifest.
    pub retention_epoch: u64,
    /// Bytes freed by the eviction.
    pub freed_bytes: u64,
    /// Reason label associated with the eviction.
    pub reason: String,
}

/// Summary of a manifest skipped during a GC sweep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GcSkip {
    /// Manifest identifier that was skipped.
    pub manifest_id: String,
    /// Reason label describing why the manifest was skipped.
    pub reason: String,
}

/// Summary of a GC sweep execution.
#[derive(Debug, Clone, Default)]
pub struct GcSweepReport {
    /// Evictions performed during the sweep.
    pub evictions: Vec<GcEviction>,
    /// Manifests skipped during the sweep.
    pub skipped: Vec<GcSkip>,
    /// Total bytes freed by evictions.
    pub freed_bytes: u64,
    /// Number of errors encountered during the sweep.
    pub errors: u32,
}

/// Result of deriving and recording a payload-free evidence-viewer audit report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModerationEvidenceViewerAuditReportOutcome {
    /// Canonical payload-free report derived from local evidence-viewer audit records.
    pub report: ModerationEvidenceViewerAuditReport,
    /// Transparency source entry recorded for later ledger/Governance DAG publication.
    pub source_entry: TransparencyLedgerSourceEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GcIntentDisposition {
    DiscardedPreDomain,
    FinalizedCommitted,
}

#[derive(Debug)]
struct GcEvictionTransactionOutcome {
    freed_bytes: u64,
    publish_error: Option<String>,
}

const GOVERNANCE_PUBLISH_INDEX_FILE: &str = "publish-index.json";
const GOVERNANCE_PUBLISH_INDEX_SCHEMA: &str = "sorafs.governance_dag.local_publish_index.v1";
const APPEAL_FINANCE_WEEKLY_ROLLUP_KIND: &str = "appeal_finance_weekly_rollup";
const ECONOMICS_RUNTIME_STATE_DIR: &str = "economics";
const PRICING_RUNTIME_SNAPSHOT_FILE: &str = "governed-pricing.to";
const HEDGING_RUNTIME_SNAPSHOT_FILE: &str = "signed-hedging-feeds.to";
const MODERATION_MODEL_REGISTRY_DIR: &str = "moderation-model-registry";
const MODERATION_MODEL_REGISTRY_SNAPSHOT_FILE: &str = "registry-snapshot.to";
const MODERATION_SCREENING_DIR: &str = "moderation-screening";
const MODERATION_SCREENING_SNAPSHOT_FILE: &str = "screening-snapshot.to";
const MODERATION_QUARANTINE_OBJECT_STORE_DIR: &str = "moderation-quarantine-objects";
const MODERATION_QUARANTINE_OBJECT_INDEX_FILE: &str = "object-index.to";
const MODERATION_QUARANTINE_OBJECT_STORE_MAX_DEPTH: usize = 4;
const MODERATION_EVIDENCE_VIEWER_DIR: &str = "moderation-evidence-viewer";
const MODERATION_EVIDENCE_VIEWER_SNAPSHOT_FILE: &str = "evidence-viewer-snapshot.to";
const AUX_RUNTIME_STATE_DIR: &str = "runtime-state";
const AUX_RUNTIME_STATE_SNAPSHOT_FILE: &str = "auxiliary-snapshot-v2.to";
const RETIRED_AUX_RUNTIME_STATE_SNAPSHOT_FILE_V1: &str = "auxiliary-snapshot.to";
const RUNTIME_STATE_INITIALIZATION_FILE: &str = "initialized-v2";
const RETIRED_RUNTIME_STATE_INITIALIZATION_FILE_V1: &str = "initialized-v1";
const RUNTIME_STATE_INITIALIZATION_BYTES: &[u8] = b"sorafs.node.runtime-state.initialized.v2\n";
const AUX_RUNTIME_STATE_VERSION_V2: u8 = 2;
const ADMITTED_REPUTATION_SNAPSHOT_VERSION_V1: u8 = 1;
const GOVERNANCE_OUTBOX_VERSION_V2: u8 = 2;
const GOVERNANCE_OUTBOX_BINDING_DOMAIN_V2: &[u8] = b"sorafs.node.governance_outbox.binding.v2";
const GC_EVICTION_INTENT_VERSION_V1: u8 = 1;
const GC_EVICTION_AUDIT_LINK_VERSION_V1: u8 = 1;
const GC_EVICTION_RESERVED_OUTBOX_SLOTS: u8 = 1;
const GC_EVICTION_INTENT_BINDING_DOMAIN_V1: &[u8] = b"sorafs.node.gc.eviction_intent.binding.v1";
const GC_EVICTION_AUDIT_LINK_BINDING_DOMAIN_V1: &[u8] =
    b"sorafs.node.gc.eviction_audit_link.binding.v1";
const GC_STORAGE_MANIFEST_IDENTITY_DOMAIN_V1: &[u8] =
    b"sorafs.node.gc.storage_manifest.identity.v1";
const GC_STORAGE_MANIFEST_SET_DOMAIN_V1: &[u8] = b"sorafs.node.gc.manifest_set.identity.v1";
const GC_STORAGE_CHUNK_REFCOUNTS_DOMAIN_V1: &[u8] = b"sorafs.node.gc.chunk_refcounts.identity.v1";
const LOCAL_RUNTIME_SNAPSHOT_TMP_EXT: &str = "tmp";
static LOCAL_CHECKPOINT_WRITE_LOCK: Mutex<()> = Mutex::new(());
const EVIDENCE_VIEWER_AUDIT_CYCLE_ID_DOMAIN_V1: &[u8] =
    b"sorafs.node.moderation.evidence_viewer_audit.cycle_id.v1";
const PRIVACY_AGGREGATE_ENTRY_ID_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_aggregate.entry_id.v1";

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    fs,
    io::{self, ErrorKind, Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use capacity::{
    CapacityError, CapacityManager, CapacityUsageSnapshot, DeclarationWindow, ReplicationPlan,
    ReplicationRelease,
};
use config::{GcConfig, RepairConfig, StorageConfig};
use iroha_crypto::numeric::{Numeric, Quantity, RoundingMode};
#[cfg(test)]
use iroha_data_model::sorafs::repair::GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1;
use iroha_data_model::{
    account::AccountId,
    da::ingest::DaStripeLayout,
    sorafs::{
        capacity::{CapacityDeclarationRecord, ProviderId},
        deal::{ClientId, DealId, DealProposal, DealRecord, DealUsageReport},
        gar::GarEnforcementReceiptV1,
        moderation::{AdversarialCorpusManifestV1, ModerationReproManifestV1},
        moderation_ledger::{
            REPAIR_LEDGER_MAX_LEASE_MS_V1, REPAIR_LEDGER_MIN_LEASE_MS_V1, RepairFinalizedCursorV1,
        },
        orderbook::OrderbookFinalizedCursorV1,
        reserve::ReserveFinalizedCursorV1,
        transparency::{
            MODERATION_LEDGER_MAX_PUBLIC_TEXT_BYTES_V1, ModerationLedgerCyclePublicationV1,
            ModerationLedgerEntryKindV1, ModerationLedgerEntryV1, ModerationLedgerMetadataV1,
            ModerationPrivacyAggregateV1, ProofTokenIssuanceV1,
        },
    },
};
use iroha_telemetry::metrics::{
    MicropaymentCreditSnapshot, MicropaymentTicketCounters, global_or_default,
    global_sorafs_gc_otel, global_sorafs_node_otel, global_sorafs_reconciliation_otel,
};
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use norito::json::Value as JsonValue;
use orderbook_transaction_forwarder::{
    ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1, OrderbookOperationV1,
    OrderbookTransactionContextV1, OrderbookTransactionDeadLetterV1,
    OrderbookTransactionEnqueueResultV1, OrderbookTransactionForwarder,
    OrderbookTransactionForwarderError, OrderbookTransactionForwarderPolicyV1,
    OrderbookTransactionPendingV1, OrderbookTransactionSigningRequestV1,
};
use rand::{rand_core::TryRngCore as _, rngs::OsRng};
use repair_transaction_forwarder::{
    REPAIR_TRANSACTION_MAX_CANONICAL_BYTES_V1, RepairOperationV1, RepairTransactionContextV1,
    RepairTransactionDeadLetterV1, RepairTransactionEnqueueResultV1, RepairTransactionForwarder,
    RepairTransactionForwarderError, RepairTransactionForwarderPolicyV1,
    RepairTransactionPendingV1, RepairTransactionSigningRequestV1,
};
use reserve_transaction_forwarder::{
    RESERVE_TRANSACTION_MAX_CANONICAL_BYTES_V1, ReserveOperationV1, ReserveTransactionContextV1,
    ReserveTransactionDeadLetterV1, ReserveTransactionEnqueueResultV1, ReserveTransactionForwarder,
    ReserveTransactionForwarderError, ReserveTransactionForwarderPolicyV1,
    ReserveTransactionPendingV1, ReserveTransactionReconciliationV1,
    ReserveTransactionSigningRequestV1,
};
use sorafs_car::{CarBuildPlan, PorProof};
use sorafs_manifest::reputation::signed::{
    MAX_REPUTATION_TRUST_POLICY_ENCODED_BYTES, decode_reputation_trust_policy,
    decode_signed_reputation_snapshot,
};
use sorafs_manifest::{
    AdmissionRecord, AppealFinanceReconciliationSummaryV1, ManifestV1,
    ReconciliationValidationError, ReputationScoringEvidenceV1, ReputationSnapshotEventV1,
    ReputationSnapshotTrustPolicyV1, ReputationSnapshotV1, SORAFS_RECONCILIATION_REPORT_VERSION_V1,
    SignedReputationSnapshotV1, SoraFsAppealFinanceReportV1,
    SoraFsAppealFinanceSettlementReceiptV1, SoraFsAppealFinanceWeeklyRollupV1,
    SoraFsModerationBallotGovernanceEventV1, SorafsReconciliationReportV1,
    capacity::{CapacityTelemetryV1, ReplicationOrderV1},
    deal::{DealSettlementStatusV1, DealSettlementV1, XorQuantity},
    por::{AuditOutcomeV1, AuditVerdictV1, PorChallengeV1, PorProofV1},
    potr::PotrReceiptV1,
    proof_stream::ProofStreamTier,
    repair::{
        GC_AUDIT_BLOCKED_DEAL_ACTIVE_V1, GC_AUDIT_BLOCKED_REPAIR_ACTIVE_V1,
        GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1, GC_AUDIT_EVENT_VERSION_V1, GC_AUDIT_PAYLOAD_VERSION_V1,
        GC_AUDIT_REASON_RETENTION_EXPIRED_V1, GC_AUDIT_SIGNER_V1, GcAuditEventV1, GcAuditPayloadV1,
        RepairReportV1, SorafsAuditHeaderV1, gc_audit_payload_digest_v1,
    },
    validate_reputation_snapshot_transition,
};
use sorafs_manifest::{
    hedging::signed::{
        GovernedHedgingReferencePriceDecisionV1, HedgingFeedTrustPolicyV1,
        MAX_HEDGING_TRUST_POLICY_BYTES, SignedHedgingFeedLedgerV1, SignedHedgingPriceFeedV1,
        decode_hedging_feed_trust_policy, decode_signed_hedging_price_feed,
    },
    pricing::signed::{
        GovernedPricingManifestV1, GovernedPricingSeriesV1, MAX_PRICING_TRUST_POLICY_BYTES,
        PricingTrustPolicyV1, decode_governed_pricing_manifest, decode_pricing_trust_policy,
    },
};
use thiserror::Error;
use tokio::sync::broadcast;
pub use transparency::moderation_ballot_governance_event_source_entry;
pub use transparency::{
    PRIVACY_AGGREGATE_MAX_POPULATIONS_V1, PRIVACY_AGGREGATE_MAX_SOURCE_EVENTS_V1,
    PrivacyAggregateCycleConfig, PrivacyAggregateCycleWindow, PrivacyAggregateMetricSchemaV1,
    PrivacyAggregatePopulationV1, PrivacyAggregateScheduleConfig, PrivacyAggregateSourceEvent,
    PrivacyAggregateSourceMetric, PrivacyCompositionBudgetChainV1,
    PrivacyCompositionBudgetChargeV1, PrivacyCompositionBudgetError,
    PrivacyCompositionBudgetLedgerV1, PrivacyCompositionBudgetPolicyV1,
    PrivacyCyclePrfInputErrorV1, PrivacyCyclePrfInputV1, PrivacyCyclePrfOutputV1,
    PrivacyCyclePrfProviderErrorV1, PrivacyCyclePrfProviderV1, PrivacyCyclePrfRequestErrorV1,
    PrivacyCyclePrfRequestV1, PrivacyReleaseAnchorErrorV1, PrivacyReleaseAnchorHeadV1,
    PrivacyReleaseAnchorV1, PrivacySourceEventRecordOutcomeV1, ProofTokenIssuanceIngestError,
    TransparencyLedgerIngestError, TransparencyLedgerSourceEntry,
    TransparencySourceEntryAdapterError, appeal_finance_report_source_entry,
    appeal_finance_settlement_receipt_source_entry, gar_enforcement_receipt_source_entry,
    moderation_evidence_viewer_audit_report_source_entry, privacy_aggregate_cycle_id,
    privacy_metric_schema_digest, privacy_population_inventory_digest,
    proof_token_issuance_from_base64, proof_token_issuance_from_frame,
    reserve_finalized_event_source_entry,
};

use crate::{
    capacity::CapacityRuntimeCheckpointV1,
    deal::DealRuntimeCheckpointV1,
    economics::{
        MAX_HEDGING_RUNTIME_CHECKPOINT_BYTES, MAX_PRICING_RUNTIME_CHECKPOINT_BYTES,
        decode_hedging_checkpoint, decode_pricing_checkpoint, encode_hedging_checkpoint,
        encode_pricing_checkpoint,
    },
    metering::{CapacityMeter, MeteringSnapshot, ReplicationUsageSample},
    moderation::{
        MODERATION_SCREENING_AUTHORITY_BUNDLE_MAX_BYTES_V1, ModerationEvidenceViewerRuntime,
        ModerationModelRegistry, ModerationQuarantineObjectEnvelopeV1,
        ModerationQuarantineObjectRuntime, ModerationScreeningRuntime,
        moderation_quarantine_object_relative_path, normalize_moderation_quarantine_object_input,
        open_moderation_quarantine_object, open_moderation_quarantine_object_range,
        rewrap_moderation_quarantine_object, seal_moderation_quarantine_object,
        validate_moderation_quarantine_key_wrapper, validate_quarantine_object_envelope,
        validate_relative_object_path,
    },
    potr::PotrTracker,
    scheduler::{SchedulerAdmissionError, StorageSchedulerConfig, StorageSchedulersRuntime},
    store::{
        ChunkFileRecord, ChunkRefcountEntry, ChunkRoleMetadata, StorageBackend, StorageError,
        StoredManifest,
    },
    telemetry::{TelemetryAccumulator, TelemetryError},
};
fn privacy_aggregate_entry_id(
    cycle_id: [u8; 16],
    aggregate_hash: [u8; 32],
    aggregate_id: &str,
) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PRIVACY_AGGREGATE_ENTRY_ID_DOMAIN_V1);
    hasher.update(&cycle_id);
    hasher.update(&aggregate_hash);
    hasher.update(aggregate_id.as_bytes());
    let digest = hasher.finalize();
    let mut entry_id = [0u8; 16];
    entry_id.copy_from_slice(&digest.as_bytes()[..16]);
    entry_id
}

fn unix_now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn collect_due_evidence_viewer_audit_window(
    schedule: &PrivacyAggregateScheduleConfig,
    latest_window: PrivacyAggregateCycleWindow,
    published_cycles: &BTreeSet<[u8; 16]>,
    timestamp_unix_ms: u64,
    due_windows: &mut BTreeSet<PrivacyAggregateCycleWindow>,
) -> Result<(), GovernancePublishError> {
    let timestamp_unix = timestamp_unix_ms / 1_000;
    let window = schedule.event_window(timestamp_unix).map_err(|err| {
        GovernancePublishError::other(format!("evidence viewer audit schedule: {err}"))
    })?;
    if window.cycle_end_unix > latest_window.cycle_end_unix
        || window.due_at_unix > latest_window.due_at_unix
    {
        return Ok(());
    }
    if published_cycles.contains(&evidence_viewer_audit_cycle_id(window)) {
        return Ok(());
    }
    due_windows.insert(window);
    Ok(())
}

fn evidence_viewer_audit_cycle_id(window: PrivacyAggregateCycleWindow) -> [u8; 16] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(EVIDENCE_VIEWER_AUDIT_CYCLE_ID_DOMAIN_V1);
    hasher.update(&window.cycle_start_unix.to_le_bytes());
    hasher.update(&window.cycle_end_unix.to_le_bytes());
    hasher.update(&window.due_at_unix.to_le_bytes());
    let digest = hasher.finalize();
    let mut cycle_id = [0u8; 16];
    cycle_id.copy_from_slice(&digest.as_bytes()[..16]);
    cycle_id
}

fn pricing_runtime_snapshot_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(ECONOMICS_RUNTIME_STATE_DIR)
        .join(PRICING_RUNTIME_SNAPSHOT_FILE)
}

fn hedging_runtime_snapshot_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(ECONOMICS_RUNTIME_STATE_DIR)
        .join(HEDGING_RUNTIME_SNAPSHOT_FILE)
}

fn moderation_model_registry_checkpoint_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(MODERATION_MODEL_REGISTRY_DIR)
        .join(MODERATION_MODEL_REGISTRY_SNAPSHOT_FILE)
}

fn moderation_screening_checkpoint_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(MODERATION_SCREENING_DIR)
        .join(MODERATION_SCREENING_SNAPSHOT_FILE)
}

fn moderation_quarantine_object_store_root(data_dir: &Path) -> PathBuf {
    data_dir.join(MODERATION_QUARANTINE_OBJECT_STORE_DIR)
}

fn moderation_quarantine_object_index_path(data_dir: &Path) -> PathBuf {
    moderation_quarantine_object_store_root(data_dir).join(MODERATION_QUARANTINE_OBJECT_INDEX_FILE)
}

fn moderation_evidence_viewer_checkpoint_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(MODERATION_EVIDENCE_VIEWER_DIR)
        .join(MODERATION_EVIDENCE_VIEWER_SNAPSHOT_FILE)
}

fn auxiliary_runtime_checkpoint_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(AUX_RUNTIME_STATE_DIR)
        .join(AUX_RUNTIME_STATE_SNAPSHOT_FILE)
}

fn runtime_state_initialization_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(AUX_RUNTIME_STATE_DIR)
        .join(RUNTIME_STATE_INITIALIZATION_FILE)
}

fn retired_runtime_state_initialization_path_v1(data_dir: &Path) -> PathBuf {
    data_dir
        .join(AUX_RUNTIME_STATE_DIR)
        .join(RETIRED_RUNTIME_STATE_INITIALIZATION_FILE_V1)
}

fn retired_auxiliary_runtime_checkpoint_path_v1(data_dir: &Path) -> PathBuf {
    data_dir
        .join(AUX_RUNTIME_STATE_DIR)
        .join(RETIRED_AUX_RUNTIME_STATE_SNAPSHOT_FILE_V1)
}

fn required_runtime_checkpoint_paths(data_dir: &Path) -> [(&'static str, PathBuf); 7] {
    [
        (
            "governed pricing runtime",
            pricing_runtime_snapshot_path(data_dir),
        ),
        (
            "signed hedging-feed runtime",
            hedging_runtime_snapshot_path(data_dir),
        ),
        (
            "moderation model registry",
            moderation_model_registry_checkpoint_path(data_dir),
        ),
        (
            "moderation screening",
            moderation_screening_checkpoint_path(data_dir),
        ),
        (
            "moderation quarantine object index",
            moderation_quarantine_object_index_path(data_dir),
        ),
        (
            "moderation evidence viewer",
            moderation_evidence_viewer_checkpoint_path(data_dir),
        ),
        (
            "auxiliary runtime",
            auxiliary_runtime_checkpoint_path(data_dir),
        ),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeCheckpointInitialization {
    Fresh,
    Initialized,
}

fn inspect_runtime_checkpoint_initialization(
    data_dir: &Path,
    checkpoint_max_bytes: u64,
) -> Result<RuntimeCheckpointInitialization, NodeInitError> {
    let marker_path = runtime_state_initialization_path(data_dir);
    let retired_marker_path = retired_runtime_state_initialization_path_v1(data_dir);
    let retired_checkpoint_path = retired_auxiliary_runtime_checkpoint_path_v1(data_dir);
    if read_local_checkpoint_bounded(
        &retired_marker_path,
        u64::try_from(b"sorafs.node.runtime-state.initialized.v1\n".len()).unwrap_or(u64::MAX),
    )
    .map_err(|err| {
        NodeInitError::checkpoint(
            "retired runtime initialization marker",
            &retired_marker_path,
            err,
        )
    })?
    .is_some()
    {
        return Err(NodeInitError::checkpoint(
            "retired runtime initialization marker",
            &retired_marker_path,
            "SoraFS development runtime-state v1 is not supported; discard and reseed the local state directory",
        ));
    }
    if read_local_checkpoint_bounded(&retired_checkpoint_path, checkpoint_max_bytes)
        .map_err(|err| {
            NodeInitError::checkpoint(
                "retired auxiliary runtime checkpoint",
                &retired_checkpoint_path,
                err,
            )
        })?
        .is_some()
    {
        return Err(NodeInitError::checkpoint(
            "retired auxiliary runtime checkpoint",
            &retired_checkpoint_path,
            "SoraFS development runtime-state v1 is not supported; discard and reseed the local state directory",
        ));
    }
    let marker = read_local_checkpoint_bounded(
        &marker_path,
        u64::try_from(RUNTIME_STATE_INITIALIZATION_BYTES.len()).unwrap_or(u64::MAX),
    )
    .map_err(|err| NodeInitError::checkpoint("runtime initialization marker", &marker_path, err))?;
    match marker {
        Some(bytes) => {
            if bytes != RUNTIME_STATE_INITIALIZATION_BYTES {
                return Err(NodeInitError::checkpoint(
                    "runtime initialization marker",
                    &marker_path,
                    "marker contents are not canonical for runtime-state v2",
                ));
            }
            for (component, path) in required_runtime_checkpoint_paths(data_dir) {
                if read_local_checkpoint_bounded(&path, checkpoint_max_bytes)
                    .map_err(|err| NodeInitError::checkpoint(component, &path, err))?
                    .is_none()
                {
                    return Err(NodeInitError::checkpoint(
                        component,
                        &path,
                        "checkpoint is missing after runtime initialization",
                    ));
                }
            }
            Ok(RuntimeCheckpointInitialization::Initialized)
        }
        None => {
            for (component, path) in required_runtime_checkpoint_paths(data_dir) {
                if read_local_checkpoint_bounded(&path, checkpoint_max_bytes)
                    .map_err(|err| NodeInitError::checkpoint(component, &path, err))?
                    .is_some()
                {
                    return Err(NodeInitError::checkpoint(
                        "runtime initialization marker",
                        &marker_path,
                        format!(
                            "marker is missing while {component} checkpoint `{}` exists",
                            path.display()
                        ),
                    ));
                }
            }
            Ok(RuntimeCheckpointInitialization::Fresh)
        }
    }
}

#[derive(Debug)]
struct LocalCheckpointWriteError {
    error: io::Error,
    committed: bool,
}

impl LocalCheckpointWriteError {
    fn precommit(error: io::Error) -> Self {
        Self {
            error,
            committed: false,
        }
    }

    fn committed(error: io::Error) -> Self {
        Self {
            error,
            committed: true,
        }
    }
}

impl std::fmt::Display for LocalCheckpointWriteError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.committed {
            write!(
                formatter,
                "checkpoint rename committed but durability is uncertain: {}",
                self.error
            )
        } else {
            self.error.fmt(formatter)
        }
    }
}

impl std::error::Error for LocalCheckpointWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

impl From<io::Error> for LocalCheckpointWriteError {
    fn from(error: io::Error) -> Self {
        Self::precommit(error)
    }
}

#[derive(Debug, Error)]
#[error("{message}")]
struct RuntimeCheckpointPersistError {
    message: String,
    committed: bool,
}

impl RuntimeCheckpointPersistError {
    fn precommit(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            committed: false,
        }
    }
}

fn write_local_checkpoint_atomic(
    path: &Path,
    bytes: &[u8],
) -> Result<(), LocalCheckpointWriteError> {
    write_local_checkpoint_atomic_with_mode(path, bytes, false)
}

fn write_local_checkpoint_atomic_bounded(
    path: &Path,
    bytes: &[u8],
    max_bytes: u64,
) -> Result<(), LocalCheckpointWriteError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(LocalCheckpointWriteError::precommit(io::Error::other(
            format!(
                "checkpoint `{}` is {} bytes, exceeding limit {max_bytes}",
                path.display(),
                bytes.len()
            ),
        )));
    }
    write_local_checkpoint_atomic(path, bytes)
}

fn write_local_private_checkpoint_atomic(
    path: &Path,
    bytes: &[u8],
) -> Result<(), LocalCheckpointWriteError> {
    write_local_checkpoint_atomic_with_mode(path, bytes, true)
}

fn write_local_checkpoint_atomic_with_mode(
    path: &Path,
    bytes: &[u8],
    _private: bool,
) -> Result<(), LocalCheckpointWriteError> {
    write_local_checkpoint_atomic_with_mode_and_parent_sync(
        path,
        bytes,
        _private,
        sync_local_checkpoint_parent,
    )
}

fn sync_local_checkpoint_parent(parent: &Path) -> io::Result<()> {
    let directory = fs::File::open(parent)?;
    directory.sync_all()
}

fn remove_local_checkpoint_file_durably(path: &Path) -> io::Result<()> {
    let path = absolute_local_checkpoint_path(path)?;
    reject_unsafe_checkpoint_ancestors(&path)?;
    let metadata = fs::symlink_metadata(&path)?;
    validate_local_checkpoint_file_metadata(&path, &metadata)?;
    fs::remove_file(&path)?;
    if let Some(parent) = path.parent() {
        sync_local_checkpoint_parent(parent)?;
    }
    Ok(())
}

fn write_local_checkpoint_atomic_with_mode_and_parent_sync(
    path: &Path,
    bytes: &[u8],
    _private: bool,
    parent_sync: fn(&Path) -> io::Result<()>,
) -> Result<(), LocalCheckpointWriteError> {
    let _writer_guard = LOCAL_CHECKPOINT_WRITE_LOCK.lock().map_err(|_| {
        LocalCheckpointWriteError::precommit(io::Error::other(
            "local checkpoint writer lock is poisoned",
        ))
    })?;
    let path = absolute_local_checkpoint_path(path)?;
    let path = path.as_path();
    reject_unsafe_checkpoint_ancestors(path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        reject_unsafe_checkpoint_ancestors(path)?;
    }
    reject_unsafe_checkpoint_target(path)?;
    let tmp_path = local_checkpoint_tmp_path(path)?;
    let result: Result<(), LocalCheckpointWriteError> = (|| {
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        set_local_no_follow_flag(&mut options);
        #[cfg(unix)]
        options.mode(0o600);
        let mut file = options.open(&tmp_path)?;
        file.write_all(bytes)?;
        set_local_private_file_permissions(&tmp_path)?;
        file.sync_all()?;
        reject_unsafe_checkpoint_ancestors(path)?;
        reject_unsafe_checkpoint_target(path)?;
        fs::rename(&tmp_path, path)?;
        if let Some(parent) = path.parent() {
            parent_sync(parent).map_err(LocalCheckpointWriteError::committed)?;
        }
        Ok(())
    })();
    if result.as_ref().is_err_and(|err| !err.committed) {
        let _ = fs::remove_file(&tmp_path);
    }
    result
}

fn reject_unsafe_checkpoint_target(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => validate_local_checkpoint_file_metadata(path, &metadata),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

fn absolute_local_checkpoint_path(path: &Path) -> io::Result<PathBuf> {
    if path.file_name().is_none() {
        return Err(io::Error::other("checkpoint path must name a file"));
    }
    let mut normalized = if path.is_absolute() {
        PathBuf::new()
    } else {
        std::env::current_dir().map_err(|err| {
            io::Error::new(
                err.kind(),
                format!("failed to resolve checkpoint working directory: {err}"),
            )
        })?
    };
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(io::Error::other(format!(
                    "checkpoint path `{}` must not contain parent-directory components",
                    path.display()
                )));
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    if !normalized.is_absolute() {
        return Err(io::Error::other(format!(
            "checkpoint path `{}` could not be resolved absolutely",
            path.display()
        )));
    }
    Ok(normalized)
}

fn validate_local_checkpoint_file_metadata(path: &Path, metadata: &fs::Metadata) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::other(format!(
            "checkpoint target `{}` must be a regular file",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        if metadata.nlink() != 1 {
            return Err(io::Error::other(format!(
                "checkpoint target `{}` must have exactly one hard link",
                path.display()
            )));
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::other(format!(
                "checkpoint target `{}` must not be accessible by group or other users",
                path.display()
            )));
        }
    }
    Ok(())
}

fn reject_unsafe_checkpoint_ancestors(path: &Path) -> io::Result<()> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(io::Error::other(format!(
            "checkpoint path `{}` must not contain parent-directory components",
            path.display()
        )));
    }
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    for ancestor in parent.ancestors() {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        match fs::symlink_metadata(ancestor) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err(io::Error::other(format!(
                    "checkpoint ancestor `{}` must be a real directory",
                    ancestor.display()
                )));
            }
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
    }
    #[cfg(unix)]
    {
        match fs::symlink_metadata(parent) {
            Ok(metadata) if metadata.permissions().mode() & 0o022 != 0 => {
                return Err(io::Error::other(format!(
                    "checkpoint parent `{}` must not be group- or world-writable",
                    parent.display()
                )));
            }
            Ok(_) => {}
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

fn read_local_checkpoint_bounded(path: &Path, max_bytes: u64) -> io::Result<Option<Vec<u8>>> {
    let path = absolute_local_checkpoint_path(path)?;
    let path = path.as_path();
    reject_unsafe_checkpoint_ancestors(path)?;
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    validate_local_checkpoint_file_metadata(path, &metadata)?;
    if metadata.len() > max_bytes {
        return Err(io::Error::other(format!(
            "checkpoint `{}` is {} bytes, exceeding limit {max_bytes}",
            path.display(),
            metadata.len()
        )));
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    set_local_no_follow_flag(&mut options);
    let file = options.open(path)?;
    let opened = file.metadata()?;
    reject_unsafe_checkpoint_ancestors(path)?;
    validate_local_checkpoint_file_metadata(path, &opened)?;
    if opened.len() > max_bytes || !same_local_file_identity(&metadata, &opened) {
        return Err(io::Error::other(format!(
            "checkpoint `{}` changed identity or exceeded its size limit while opening",
            path.display()
        )));
    }
    let capacity = usize::try_from(opened.len()).map_err(|_| {
        io::Error::other(format!(
            "checkpoint `{}` length does not fit memory address space",
            path.display()
        ))
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(io::Error::other(format!(
            "checkpoint `{}` grew beyond limit {max_bytes} while reading",
            path.display()
        )));
    }
    Ok(Some(bytes))
}

pub(crate) fn decode_local_checkpoint_canonical<T>(
    bytes: &[u8],
    max_bytes: u64,
    max_sequence_elements: usize,
) -> Result<T, String>
where
    for<'de> T: norito::core::NoritoDeserialize<'de> + norito::core::NoritoSerialize,
{
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(format!(
            "checkpoint is {} bytes, exceeding limit {max_bytes}",
            bytes.len()
        ));
    }
    let maximum_bytes = usize::try_from(max_bytes)
        .map_err(|_| "checkpoint byte limit does not fit memory address space".to_owned())?;
    let limits = norito::DecodeLimits::new(
        max_sequence_elements.max(1),
        maximum_bytes,
        maximum_bytes.saturating_mul(2),
        maximum_bytes.saturating_mul(4),
        64,
    );
    let checkpoint: T = norito::decode_from_bytes_with_limits(bytes, limits)
        .map_err(|err| format!("bounded checkpoint decode failed: {err}"))?;
    let canonical = norito::to_bytes(&checkpoint)
        .map_err(|err| format!("canonical checkpoint encoding failed: {err}"))?;
    if canonical != bytes {
        return Err("checkpoint is not the exact canonical Norito encoding".to_owned());
    }
    Ok(checkpoint)
}

fn read_reputation_trust_policy_file(path: &Path) -> io::Result<Vec<u8>> {
    read_trust_policy_file(
        path,
        MAX_REPUTATION_TRUST_POLICY_ENCODED_BYTES,
        "reputation trust policy",
    )
}

fn read_trust_policy_file(
    path: &Path,
    max_bytes: usize,
    label: &'static str,
) -> io::Result<Vec<u8>> {
    let path = absolute_local_checkpoint_path(path)?;
    let path = path.as_path();
    reject_unsafe_checkpoint_ancestors(path)?;
    let before_open = fs::symlink_metadata(path)?;
    validate_trust_policy_file_metadata(path, &before_open, label)?;
    let max_bytes = u64::try_from(max_bytes)
        .map_err(|_| io::Error::other(format!("{label} size cap does not fit u64")))?;
    if before_open.len() > max_bytes {
        return Err(io::Error::other(format!(
            "{label} `{}` is {} bytes, exceeding limit {max_bytes}",
            path.display(),
            before_open.len()
        )));
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    set_local_no_follow_flag(&mut options);
    let file = options.open(path)?;
    let opened = file.metadata()?;
    validate_trust_policy_file_metadata(path, &opened, label)?;
    if opened.len() > max_bytes || !reputation_policy_metadata_stable(&before_open, &opened) {
        return Err(io::Error::other(format!(
            "{label} `{}` changed identity or exceeded its size limit while opening",
            path.display()
        )));
    }
    let capacity = usize::try_from(opened.len()).map_err(|_| {
        io::Error::other(format!(
            "{label} `{}` length does not fit memory address space",
            path.display()
        ))
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity).map_err(|_| {
        io::Error::other(format!(
            "failed to reserve memory for {label} `{}`",
            path.display()
        ))
    })?;
    let mut limited = file.take(max_bytes.saturating_add(1));
    limited.read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(io::Error::other(format!(
            "{label} `{}` grew beyond limit {max_bytes} while reading",
            path.display()
        )));
    }
    let after_read_file = limited.get_ref().metadata()?;
    let after_read = fs::symlink_metadata(path)?;
    validate_trust_policy_file_metadata(path, &after_read, label)?;
    if !reputation_policy_metadata_stable(&opened, &after_read_file)
        || !reputation_policy_metadata_stable(&after_read_file, &after_read)
    {
        return Err(io::Error::other(format!(
            "{label} `{}` changed while being read",
            path.display()
        )));
    }
    Ok(bytes)
}

#[cfg(unix)]
fn reputation_policy_metadata_stable(expected: &fs::Metadata, observed: &fs::Metadata) -> bool {
    expected.dev() == observed.dev()
        && expected.ino() == observed.ino()
        && expected.len() == observed.len()
        && expected.mtime() == observed.mtime()
        && expected.mtime_nsec() == observed.mtime_nsec()
        && expected.ctime() == observed.ctime()
        && expected.ctime_nsec() == observed.ctime_nsec()
}

#[cfg(not(unix))]
fn reputation_policy_metadata_stable(expected: &fs::Metadata, observed: &fs::Metadata) -> bool {
    expected.len() == observed.len()
        && expected.modified().ok() == observed.modified().ok()
        && same_local_file_identity(expected, observed)
}

fn validate_trust_policy_file_metadata(
    path: &Path,
    metadata: &fs::Metadata,
    label: &'static str,
) -> io::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(io::Error::other(format!(
            "{label} `{}` must be a regular non-symlink file",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        if metadata.nlink() != 1 {
            return Err(io::Error::other(format!(
                "{label} `{}` must have exactly one hard link",
                path.display()
            )));
        }
        if metadata.permissions().mode() & 0o022 != 0 {
            return Err(io::Error::other(format!(
                "{label} `{}` must not be writable by group or other users",
                path.display()
            )));
        }
    }
    Ok(())
}

fn collect_secure_object_store_files(
    root: &Path,
    current: &Path,
    depth: usize,
    file_limit: usize,
    files: &mut BTreeSet<PathBuf>,
) -> io::Result<()> {
    if depth > MODERATION_QUARANTINE_OBJECT_STORE_MAX_DEPTH {
        return Err(io::Error::other(format!(
            "quarantine object store `{}` exceeds maximum directory depth {}",
            root.display(),
            MODERATION_QUARANTINE_OBJECT_STORE_MAX_DEPTH
        )));
    }
    let entries = match fs::read_dir(current) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound && current == root => return Ok(()),
        Err(err) => return Err(err),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(io::Error::other(format!(
                "quarantine object store entry `{}` must not be a symlink",
                path.display()
            )));
        }
        if metadata.is_dir() {
            #[cfg(unix)]
            if metadata.permissions().mode() & 0o022 != 0 {
                return Err(io::Error::other(format!(
                    "quarantine object directory `{}` must not be group- or world-writable",
                    path.display()
                )));
            }
            collect_secure_object_store_files(root, &path, depth + 1, file_limit, files)?;
        } else if metadata.is_file() {
            validate_local_checkpoint_file_metadata(&path, &metadata)?;
            if !files.insert(path.clone()) {
                return Err(io::Error::other(format!(
                    "duplicate quarantine object store path `{}`",
                    path.display()
                )));
            }
            if files.len() > file_limit {
                return Err(io::Error::other(format!(
                    "quarantine object store exceeds file limit {file_limit}"
                )));
            }
        } else {
            return Err(io::Error::other(format!(
                "quarantine object store entry `{}` must be a regular file or directory",
                path.display()
            )));
        }
    }
    Ok(())
}

#[cfg(unix)]
fn same_local_file_identity(expected: &fs::Metadata, opened: &fs::Metadata) -> bool {
    expected.dev() == opened.dev() && expected.ino() == opened.ino()
}

#[cfg(not(unix))]
fn same_local_file_identity(expected: &fs::Metadata, opened: &fs::Metadata) -> bool {
    expected.len() == opened.len()
        && expected.modified().ok() == opened.modified().ok()
        && expected.created().ok() == opened.created().ok()
}

#[cfg(unix)]
fn set_local_no_follow_flag(options: &mut fs::OpenOptions) {
    options.custom_flags(local_no_follow_flag());
}

#[cfg(not(unix))]
fn set_local_no_follow_flag(_options: &mut fs::OpenOptions) {}

#[cfg(any(target_os = "linux", target_os = "android"))]
const fn local_no_follow_flag() -> i32 {
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
const fn local_no_follow_flag() -> i32 {
    0x0000_0100
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
const fn local_no_follow_flag() -> i32 {
    0
}

#[cfg(unix)]
fn set_local_private_file_permissions(path: &Path) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn set_local_private_file_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

fn local_checkpoint_tmp_path(path: &Path) -> io::Result<PathBuf> {
    let mut nonce = [0_u8; 16];
    OsRng.try_fill_bytes(&mut nonce).map_err(|err| {
        io::Error::other(format!(
            "failed to generate checkpoint temporary-file nonce: {err}"
        ))
    })?;
    let suffix = format!(
        "{LOCAL_RUNTIME_SNAPSHOT_TMP_EXT}-{}-{}",
        std::process::id(),
        hex::encode(nonce)
    );
    let candidate = path.with_added_extension(&suffix);
    Ok(match candidate.file_name().and_then(|name| name.to_str()) {
        Some(name) => candidate.with_file_name(format!(".{name}")),
        None => candidate,
    })
}

const REPUTATION_EVENT_CHANNEL_CAPACITY: usize = 128;

fn reconciliation_divergence_count(storage: &StorageBackend, manifests: &[StoredManifest]) -> u32 {
    let mut divergence_count = 0_u32;
    let manifest_count = storage.manifest_count();
    let index_count = storage.index_manifest_count();
    if manifest_count != index_count {
        divergence_count = divergence_count.saturating_add(1);
        iroha_logger::warn!(
            manifest_count,
            index_count,
            "reconciliation mismatch: manifest count diverges from index"
        );
    }

    for manifest in manifests {
        if let Some(source) = manifest.retention_source() {
            if source.effective_epoch() != manifest.retention_epoch() {
                divergence_count = divergence_count.saturating_add(1);
                iroha_logger::warn!(
                    manifest_id = %manifest.manifest_id(),
                    retention_epoch = manifest.retention_epoch(),
                    source_epoch = source.effective_epoch(),
                    "reconciliation mismatch: retention epoch differs from source"
                );
            }
        } else if manifest.retention_epoch() != 0 {
            divergence_count = divergence_count.saturating_add(1);
            iroha_logger::warn!(
                manifest_id = %manifest.manifest_id(),
                retention_epoch = manifest.retention_epoch(),
                "reconciliation mismatch: missing retention source for retained manifest"
            );
        }
    }

    divergence_count
}

fn empty_appeal_finance_reconciliation_summary()
-> Result<AppealFinanceReconciliationSummaryV1, ReconciliationError> {
    let snapshot = reconciliation::AppealFinanceRollupReconciliationSnapshot {
        version: reconciliation::RECONCILIATION_SNAPSHOT_VERSION_V1,
        rollups: Vec::new(),
    };
    Ok(AppealFinanceReconciliationSummaryV1 {
        rollup_snapshot_hash: reconciliation::hash_snapshot(&snapshot)?,
        rollup_count: 0,
        source_report_count: 0,
        case_count: 0,
        total_treasury_xor: XorQuantity::zero(),
        total_rewards_forfeited_treasury_xor: XorQuantity::zero(),
    })
}

fn appeal_finance_rollup_reconciliation_entry(
    governance_dir: &Path,
    index_entry: &JsonValue,
) -> Result<reconciliation::AppealFinanceRollupReconciliationEntry, ReconciliationError> {
    let json_path = required_json_string(index_entry, "json_path")?;
    let sidecar_path = governance_index_relative_path(governance_dir, &json_path)?;
    let sidecar_bytes = fs::read(&sidecar_path).map_err(|err| {
        ReconciliationError::AppealFinance(format!(
            "failed to read appeal finance rollup sidecar `{}`: {err}",
            sidecar_path.display()
        ))
    })?;
    let sidecar = norito::json::from_slice::<JsonValue>(&sidecar_bytes).map_err(|err| {
        ReconciliationError::AppealFinance(format!(
            "failed to decode appeal finance rollup sidecar `{}`: {err}",
            sidecar_path.display()
        ))
    })?;
    let metadata = sidecar.get("metadata").ok_or_else(|| {
        ReconciliationError::AppealFinance(format!(
            "appeal finance rollup sidecar `{}` is missing metadata",
            sidecar_path.display()
        ))
    })?;
    let total_rewards_forfeited_treasury_xor = metadata
        .get("total_rewards_forfeited_treasury_xor")
        .and_then(JsonValue::as_str)
        .unwrap_or("0")
        .parse::<XorQuantity>()
        .map_err(|error| {
            ReconciliationError::AppealFinance(format!(
                "invalid `total_rewards_forfeited_treasury_xor`: {error}"
            ))
        })?;

    Ok(reconciliation::AppealFinanceRollupReconciliationEntry {
        cycle: required_json_string(metadata, "cycle")?,
        encoded_blake3: required_json_string(index_entry, "encoded_blake3")?,
        report_count: required_json_u64(metadata, "report_count")?,
        case_count: required_json_u64(metadata, "case_count")?,
        total_treasury_xor: required_json_xor_quantity(metadata, "total_treasury_xor")?,
        total_rewards_forfeited_treasury_xor,
        published_at_unix: required_json_u64(index_entry, "published_at_unix")?,
    })
}

fn governance_index_relative_path(
    governance_dir: &Path,
    raw_path: &str,
) -> Result<std::path::PathBuf, ReconciliationError> {
    let relative = Path::new(raw_path);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(ReconciliationError::AppealFinance(format!(
            "governance publish index path `{raw_path}` must be relative to the governance root"
        )));
    }
    Ok(governance_dir.join(relative))
}

fn required_json_string(
    value: &JsonValue,
    field: &'static str,
) -> Result<String, ReconciliationError> {
    value
        .get(field)
        .and_then(JsonValue::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            ReconciliationError::AppealFinance(format!("appeal finance rollup missing `{field}`"))
        })
}

fn required_json_u64(value: &JsonValue, field: &'static str) -> Result<u64, ReconciliationError> {
    value.get(field).and_then(JsonValue::as_u64).ok_or_else(|| {
        ReconciliationError::AppealFinance(format!("appeal finance rollup missing `{field}`"))
    })
}

fn required_json_xor_quantity(
    value: &JsonValue,
    field: &'static str,
) -> Result<XorQuantity, ReconciliationError> {
    required_json_string(value, field)?
        .parse()
        .map_err(|error| {
            ReconciliationError::AppealFinance(format!(
                "appeal finance rollup has invalid `{field}`: {error}"
            ))
        })
}

/// Interface for emitting settlement artefacts to the governance DAG.
///
/// Implementations must be idempotent for identical canonical payload bytes.
/// The durable node outbox intentionally retries after crashes where external
/// publication succeeded but the local acknowledgement was not yet durable.
pub trait GovernancePublisher: Send + Sync + std::fmt::Debug {
    /// Persist the supplied settlement NORITO payload to the governance pipeline.
    fn publish_deal_settlement(
        &self,
        settlement: &DealSettlementV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist an admission-bound PDP terminal archive to the governance pipeline.
    fn publish_pdp_archive(
        &self,
        archive: &PdpGovernanceArchiveV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist a GC audit event to the governance pipeline.
    fn publish_gc_audit_event(
        &self,
        event: &sorafs_manifest::repair::GcAuditEventV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist a reconciliation report to the governance pipeline.
    fn publish_reconciliation_report(
        &self,
        report: &SorafsReconciliationReportV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist an externally authorized reputation snapshot to the governance pipeline.
    fn publish_reputation_snapshot(
        &self,
        snapshot: &SignedReputationSnapshotV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist a moderation ballot lifecycle event to the governance pipeline.
    fn publish_moderation_ballot_event(
        &self,
        event: &SoraFsModerationBallotGovernanceEventV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist a moderation transparency ledger cycle publication to the governance pipeline.
    fn publish_transparency_ledger_publication(
        &self,
        publication: &ModerationLedgerCyclePublicationV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist a proof-token issuance summary to the governance pipeline.
    fn publish_proof_token_issuance(
        &self,
        issuance: &ProofTokenIssuanceV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist an appeal finance report to the governance pipeline.
    fn publish_appeal_finance_report(
        &self,
        report: &SoraFsAppealFinanceReportV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist a weekly appeal finance rollup to the governance pipeline.
    fn publish_appeal_finance_weekly_rollup(
        &self,
        rollup: &SoraFsAppealFinanceWeeklyRollupV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist an appeal finance settlement receipt to the governance pipeline.
    fn publish_appeal_finance_settlement_receipt(
        &self,
        receipt: &SoraFsAppealFinanceSettlementReceiptV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
}

/// Errors surfaced when publishing governance artefacts fails.
#[derive(Debug, Error)]
pub enum GovernancePublishError {
    /// Underlying IO error while writing the artefact.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Generic publish failure with human-readable context.
    #[error("{0}")]
    Other(String),
}

impl GovernancePublishError {
    /// Construct a generic publish failure.
    #[must_use]
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }
}

/// Result of one scheduled privacy aggregate publication attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrivacyAggregateScheduleOutcome {
    /// Config-backed scheduling is disabled.
    Disabled,
    /// No cycle is old enough to publish at the supplied timestamp.
    NotDue,
    /// The due cycle was already published by this node runtime.
    AlreadyPublished {
        /// Due window that was skipped.
        window: PrivacyAggregateCycleWindow,
        /// Deterministic cycle id.
        cycle_id: [u8; 16],
    },
    /// The due cycle had no source events.
    NoSourceEvents {
        /// Due window that was skipped.
        window: PrivacyAggregateCycleWindow,
        /// Deterministic cycle id.
        cycle_id: [u8; 16],
    },
    /// The due cycle had source events, but every bucket was suppressed.
    AllBucketsSuppressed {
        /// Due window that was skipped.
        window: PrivacyAggregateCycleWindow,
        /// Deterministic cycle id.
        cycle_id: [u8; 16],
    },
    /// The due cycle was published.
    Published {
        /// Due window that was published.
        window: PrivacyAggregateCycleWindow,
        /// Published transparency ledger cycle.
        publication: ModerationLedgerCyclePublicationV1,
    },
}

/// Complete input for publishing one test-built privacy aggregate source cycle.
#[cfg(test)]
struct PrivacyAggregateSourceCycleInput {
    /// Stable identifier assigned to the published cycle.
    cycle_id: [u8; 16],
    /// Inclusive source-event window start, in Unix seconds.
    cycle_start_unix: u64,
    /// Exclusive source-event window end, in Unix seconds.
    cycle_end_unix: u64,
    /// Optional hash linking the cycle to its predecessor.
    previous_block_hash: Option<[u8; 32]>,
    /// Governed aggregation and privacy policy.
    config: PrivacyAggregateCycleConfig,
    /// Optional request-bound threshold-PRF input for this cycle.
    cycle_prf_input: Option<PrivacyCyclePrfInputV1>,
}

const PRIVACY_PUBLISH_REQUEST_DIGEST_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_publish_request.v1";

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum PrivacyPublishRequestOutcomeV1 {
    Published { publication_bytes: Vec<u8> },
    AllBucketsSuppressed,
}

#[derive(Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct PrivacyPublishRequestReceiptV1 {
    idempotency_key: String,
    request_digest: [u8; 32],
    query_id: [u8; 32],
    requested_now_unix: u64,
    cycle_id: [u8; 16],
    cycle_start_unix: u64,
    cycle_end_unix: u64,
    publish_delay_seconds: u64,
    due_at_unix: u64,
    outcome: PrivacyPublishRequestOutcomeV1,
}

impl std::fmt::Debug for PrivacyPublishRequestReceiptV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PrivacyPublishRequestReceiptV1")
            .field("idempotency_key", &"<redacted>")
            .field("request_digest", &self.request_digest)
            .field("query_id", &self.query_id)
            .field("requested_now_unix", &self.requested_now_unix)
            .field("cycle_id", &self.cycle_id)
            .field("cycle_start_unix", &self.cycle_start_unix)
            .field("cycle_end_unix", &self.cycle_end_unix)
            .field("publish_delay_seconds", &self.publish_delay_seconds)
            .field("due_at_unix", &self.due_at_unix)
            .field(
                "outcome",
                &match &self.outcome {
                    PrivacyPublishRequestOutcomeV1::Published { .. } => "published",
                    PrivacyPublishRequestOutcomeV1::AllBucketsSuppressed => {
                        "all_buckets_suppressed"
                    }
                },
            )
            .finish()
    }
}

impl PrivacyPublishRequestReceiptV1 {
    fn validate(&self) -> Result<(), GovernancePublishError> {
        validate_privacy_publish_idempotency_key(&self.idempotency_key)?;
        if self.query_id == [0; 32]
            || self.requested_now_unix == 0
            || self.request_digest
                != privacy_publish_request_digest(
                    self.query_id,
                    &self.idempotency_key,
                    self.cycle_id,
                )
            || self.cycle_start_unix == 0
            || self.cycle_end_unix <= self.cycle_start_unix
            || self.cycle_end_unix.checked_add(self.publish_delay_seconds) != Some(self.due_at_unix)
            || self.requested_now_unix < self.due_at_unix
            || self.cycle_id
                != transparency::privacy_aggregate_cycle_id(
                    self.query_id,
                    self.cycle_start_unix,
                    self.cycle_end_unix,
                )
        {
            return Err(GovernancePublishError::other(
                "privacy publish-request receipt is invalid",
            ));
        }
        if let PrivacyPublishRequestOutcomeV1::Published { publication_bytes } = &self.outcome {
            let publication = decode_canonical_governance_payload::<
                ModerationLedgerCyclePublicationV1,
            >(publication_bytes)?;
            if publication.block.cycle_id != self.cycle_id
                || publication.block.cycle_start_unix != self.cycle_start_unix
                || publication.block.cycle_end_unix != self.cycle_end_unix
            {
                return Err(GovernancePublishError::other(
                    "privacy publish-request receipt publication mismatch",
                ));
            }
        }
        Ok(())
    }

    fn outcome(&self) -> Result<PrivacyAggregateScheduleOutcome, GovernancePublishError> {
        self.validate()?;
        let window = PrivacyAggregateCycleWindow {
            cycle_start_unix: self.cycle_start_unix,
            cycle_end_unix: self.cycle_end_unix,
            due_at_unix: self.due_at_unix,
        };
        match &self.outcome {
            PrivacyPublishRequestOutcomeV1::Published { publication_bytes } => {
                let publication = decode_canonical_governance_payload::<
                    ModerationLedgerCyclePublicationV1,
                >(publication_bytes)?;
                Ok(PrivacyAggregateScheduleOutcome::Published {
                    window,
                    publication,
                })
            }
            PrivacyPublishRequestOutcomeV1::AllBucketsSuppressed => {
                Ok(PrivacyAggregateScheduleOutcome::AllBucketsSuppressed {
                    window,
                    cycle_id: self.cycle_id,
                })
            }
        }
    }
}

fn validate_privacy_publish_idempotency_key(
    idempotency_key: &str,
) -> Result<(), GovernancePublishError> {
    if idempotency_key.is_empty()
        || idempotency_key.len() > 256
        || idempotency_key.trim() != idempotency_key
        || idempotency_key.chars().any(char::is_control)
    {
        return Err(GovernancePublishError::other(
            "privacy publish idempotency key is invalid",
        ));
    }
    Ok(())
}

fn privacy_publish_request_digest(
    query_id: [u8; 32],
    idempotency_key: &str,
    expected_cycle_id: [u8; 16],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(PRIVACY_PUBLISH_REQUEST_DIGEST_DOMAIN_V1);
    hasher.update(&query_id);
    hasher.update(&expected_cycle_id);
    hasher.update(&(idempotency_key.len() as u64).to_le_bytes());
    hasher.update(idempotency_key.as_bytes());
    *hasher.finalize().as_bytes()
}

fn validate_privacy_release_lineage(
    release_ledger: &transparency::PrivacyReleaseLedgerV1,
    schedule: PrivacyAggregateScheduleConfig,
    config: &PrivacyAggregateCycleConfig,
) -> Result<(), GovernancePublishError> {
    if let Some(record) = release_ledger.records.last()
        && (record.query_id != config.query_id
            || record.first_cycle_start_unix != config.first_cycle_start_unix
            || record.cycle_seconds != config.cycle_seconds
            || record.first_cycle_start_unix != schedule.first_cycle_start_unix
            || record.cycle_seconds != schedule.cycle_seconds
            || record.publish_delay_seconds != schedule.publish_delay_seconds)
    {
        return Err(GovernancePublishError::other(
            "durable privacy release cadence does not match the configured query lineage",
        ));
    }
    Ok(())
}

fn validate_privacy_publish_receipt_release(
    receipt: &PrivacyPublishRequestReceiptV1,
    record: &transparency::PrivacyReleaseRecordV1,
) -> Result<(), GovernancePublishError> {
    receipt.validate()?;
    if record.release_id != receipt.cycle_id
        || record.query_id != receipt.query_id
        || record.cycle_start_unix != receipt.cycle_start_unix
        || record.cycle_end_unix != receipt.cycle_end_unix
        || record.publish_delay_seconds != receipt.publish_delay_seconds
        || record.due_at_unix != receipt.due_at_unix
    {
        return Err(GovernancePublishError::other(
            "privacy publish-request receipt conflicts with its release record",
        ));
    }
    match &receipt.outcome {
        PrivacyPublishRequestOutcomeV1::Published { publication_bytes } => {
            let publication = decode_canonical_governance_payload::<
                ModerationLedgerCyclePublicationV1,
            >(publication_bytes)?;
            let payload_digest = *blake3::hash(publication_bytes).as_bytes();
            let block_hash = publication.block.block_hash().map_err(|error| {
                GovernancePublishError::other(format!("hash privacy receipt publication: {error}"))
            })?;
            let aggregate_inventory_digest =
                privacy_published_aggregate_inventory_digest(&publication.privacy_aggregates)?;
            if record.status != transparency::PrivacyReleaseStatusV1::Published
                || record.publication_payload_digest != Some(payload_digest)
                || record.published_aggregate_inventory_digest != Some(aggregate_inventory_digest)
                || record.publication_block_hash != Some(block_hash)
                || record.previous_publication_block_hash != publication.block.previous_block_hash
                || publication.block.cycle_id != record.release_id
                || publication.block.cycle_start_unix != record.cycle_start_unix
                || publication.block.cycle_end_unix != record.cycle_end_unix
                || publication.block.generated_at_unix != record.cycle_end_unix
            {
                return Err(GovernancePublishError::other(
                    "published privacy request receipt conflicts with its exact release bytes",
                ));
            }
        }
        PrivacyPublishRequestOutcomeV1::AllBucketsSuppressed => {
            if record.status != transparency::PrivacyReleaseStatusV1::Suppressed
                || record.publication_payload_digest.is_some()
                || record.published_aggregate_inventory_digest.is_some()
                || record.publication_block_hash.is_some()
            {
                return Err(GovernancePublishError::other(
                    "suppressed privacy request receipt conflicts with its exact release",
                ));
            }
        }
    }
    Ok(())
}

/// Result of one scheduled evidence-viewer audit-report publication attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModerationEvidenceViewerAuditScheduleOutcome {
    /// Config-backed scheduling is disabled.
    Disabled,
    /// No cycle is old enough to publish at the supplied timestamp.
    NotDue,
    /// The due cycle was already published by this node runtime.
    AlreadyPublished {
        /// Due window that was skipped.
        window: PrivacyAggregateCycleWindow,
        /// Deterministic evidence-viewer audit cycle id.
        cycle_id: [u8; 16],
    },
    /// The due cycle had no local evidence-viewer session or access records.
    NoSourceEvents {
        /// Due window that was skipped.
        window: PrivacyAggregateCycleWindow,
        /// Deterministic evidence-viewer audit cycle id.
        cycle_id: [u8; 16],
    },
    /// The due cycle was reported and published.
    Published {
        /// Due window that was published.
        window: PrivacyAggregateCycleWindow,
        /// Payload-free audit report recorded for the window.
        report: Box<ModerationEvidenceViewerAuditReport>,
        /// Transparency source entry recorded for the report.
        source_entry: Box<TransparencyLedgerSourceEntry>,
        /// Published transparency ledger cycle.
        publication: ModerationLedgerCyclePublicationV1,
    },
}

/// Payload returned by a repair orchestrator for a missing chunk.
#[derive(Debug, Clone)]
pub struct RepairChunkPayload {
    /// Expected BLAKE3-256 digest of the chunk.
    pub digest: [u8; 32],
    /// Raw chunk bytes.
    pub bytes: Vec<u8>,
    /// Optional source label (provider id, URL, or orchestrator hint).
    pub source: Option<String>,
}

/// Bounded replay result for a monotonic local event stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventReplay<T> {
    /// Oldest sequence still retained by the node, when the stream is non-empty.
    pub oldest_available_sequence: Option<u64>,
    /// Latest sequence ever appended to this runtime stream.
    pub latest_sequence: Option<u64>,
    /// Whether the requested cursor predates retained history.
    pub gap: bool,
    /// Retained events after the requested cursor.
    pub events: Vec<T>,
}

#[derive(Debug, Clone)]
struct BoundedEventHistory<T> {
    events: VecDeque<T>,
    latest_sequence: u64,
    limit: usize,
}

impl<T> BoundedEventHistory<T> {
    fn new(limit: usize) -> Self {
        Self {
            events: VecDeque::new(),
            latest_sequence: 0,
            limit: limit.max(1),
        }
    }

    fn append(&mut self, build: impl FnOnce(u64) -> T) -> Result<T, GovernancePublishError>
    where
        T: Clone,
    {
        let sequence = self
            .latest_sequence
            .checked_add(1)
            .ok_or_else(|| GovernancePublishError::other("event sequence exhausted"))?;
        let event = build(sequence);
        self.events.push_back(event.clone());
        self.latest_sequence = sequence;
        while self.events.len() > self.limit {
            self.events.pop_front();
        }
        Ok(event)
    }

    fn restore(
        &mut self,
        events: Vec<T>,
        sequence_of: impl Fn(&T) -> u64,
    ) -> Result<(), GovernancePublishError> {
        let mut previous = None;
        for event in &events {
            let sequence = sequence_of(event);
            if sequence == 0
                || previous.is_some_and(|previous: u64| previous.checked_add(1) != Some(sequence))
            {
                return Err(GovernancePublishError::other(
                    "event checkpoint sequences must be non-zero and strictly consecutive",
                ));
            }
            previous = Some(sequence);
        }
        self.latest_sequence = previous.unwrap_or(0);
        self.events = events.into_iter().collect();
        while self.events.len() > self.limit {
            self.events.pop_front();
        }
        Ok(())
    }

    fn replay(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
        sequence_of: impl Fn(&T) -> u64,
    ) -> EventReplay<T>
    where
        T: Clone,
    {
        let oldest_available_sequence = self.events.front().map(&sequence_of);
        let latest_sequence = (self.latest_sequence != 0).then_some(self.latest_sequence);
        let since = since_sequence.unwrap_or(0);
        let gap = since_sequence.is_some_and(|cursor| {
            oldest_available_sequence.is_some_and(|oldest| cursor.saturating_add(1) < oldest)
        });
        let events = self
            .events
            .iter()
            .filter(|event| sequence_of(event) > since)
            .take(limit.max(1))
            .cloned()
            .collect();
        EventReplay {
            oldest_available_sequence,
            latest_sequence,
            gap,
            events,
        }
    }

    fn retained(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.events.iter().cloned().collect()
    }
}

/// Errors surfaced when the repair orchestrator cannot fetch missing chunks.
#[derive(Debug, Error)]
pub enum RepairOrchestratorError {
    /// Generic failure with human-readable context.
    #[error("{0}")]
    Other(String),
}

impl RepairOrchestratorError {
    /// Construct a generic orchestrator failure.
    #[must_use]
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }
}

/// Interface for orchestrator-backed repair rehydration.
pub trait RepairOrchestrator: Send + Sync + std::fmt::Debug {
    /// Fetch invalid chunks from remote sources for the exact native lease.
    ///
    /// Implementations must return no more payloads than `invalid_chunks` and
    /// each payload must match one requested digest and length.
    fn rehydrate_missing_chunks(
        &self,
        context: &native_repair_worker::NativeRepairExecutionContextV1,
        manifest: &StoredManifest,
        invalid_chunks: &[ChunkFileRecord],
    ) -> Result<Vec<RepairChunkPayload>, RepairOrchestratorError>;
}

/// Runtime-only dependencies supplied by the embedding daemon.
///
/// Secret-bearing providers are deliberately absent from [`StorageConfig`].
/// This container records only opaque service handles and never formats their
/// implementation state.
#[derive(Clone, Default)]
pub struct NodeRuntimeDeps {
    moderation_quarantine_key_wrapper: Option<Arc<dyn ModerationQuarantineKeyWrapper>>,
    privacy_cycle_prf_provider: Option<Arc<dyn PrivacyCyclePrfProviderV1>>,
    privacy_release_anchor: Option<Arc<dyn PrivacyReleaseAnchorV1>>,
}

impl std::fmt::Debug for NodeRuntimeDeps {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NodeRuntimeDeps")
            .field(
                "moderation_quarantine_key_wrapper",
                &self.moderation_quarantine_key_wrapper.is_some(),
            )
            .field(
                "privacy_cycle_prf_provider",
                &self.privacy_cycle_prf_provider.is_some(),
            )
            .field(
                "privacy_release_anchor",
                &self.privacy_release_anchor.is_some(),
            )
            .finish()
    }
}

impl NodeRuntimeDeps {
    /// Attach the runtime-only PKCS#11/KMS quarantine-key wrapper.
    #[must_use]
    pub fn with_moderation_quarantine_key_wrapper(
        mut self,
        key_wrapper: Arc<dyn ModerationQuarantineKeyWrapper>,
    ) -> Self {
        self.moderation_quarantine_key_wrapper = Some(key_wrapper);
        self
    }

    /// Attach the runtime-only threshold-PRF provider used by DP aggregates.
    #[must_use]
    pub fn with_privacy_cycle_prf_provider(
        mut self,
        provider: Arc<dyn PrivacyCyclePrfProviderV1>,
    ) -> Self {
        self.privacy_cycle_prf_provider = Some(provider);
        self
    }

    /// Attach the independently administered finalized privacy-release head.
    #[must_use]
    pub fn with_privacy_release_anchor(mut self, anchor: Arc<dyn PrivacyReleaseAnchorV1>) -> Self {
        self.privacy_release_anchor = Some(anchor);
        self
    }
}

#[derive(Clone)]
struct OpaqueModerationQuarantineKeyWrapper(Arc<dyn ModerationQuarantineKeyWrapper>);

impl std::fmt::Debug for OpaqueModerationQuarantineKeyWrapper {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ModerationQuarantineKeyWrapper(<runtime-only>)")
    }
}

impl std::ops::Deref for OpaqueModerationQuarantineKeyWrapper {
    type Target = dyn ModerationQuarantineKeyWrapper;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

#[derive(Clone)]
struct OpaquePrivacyCyclePrfProvider(Arc<dyn PrivacyCyclePrfProviderV1>);

impl std::fmt::Debug for OpaquePrivacyCyclePrfProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PrivacyCyclePrfProviderV1(<runtime-only>)")
    }
}

impl std::ops::Deref for OpaquePrivacyCyclePrfProvider {
    type Target = dyn PrivacyCyclePrfProviderV1;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

#[derive(Clone)]
struct OpaquePrivacyReleaseAnchor(Arc<dyn PrivacyReleaseAnchorV1>);

impl std::fmt::Debug for OpaquePrivacyReleaseAnchor {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("PrivacyReleaseAnchorV1(<runtime-only>)")
    }
}

impl std::ops::Deref for OpaquePrivacyReleaseAnchor {
    type Target = dyn PrivacyReleaseAnchorV1;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

/// Lightweight handle representing the embedded SoraFS storage worker.
#[derive(Debug, Clone)]
pub struct NodeHandle {
    config: StorageConfig,
    repair_config: RepairConfig,
    gc_config: GcConfig,
    capacity: Arc<CapacityManager>,
    meter: CapacityMeter,
    telemetry: Arc<RwLock<Option<TelemetryAccumulator>>>,
    schedulers: StorageSchedulersRuntime,
    por: PorTracker,
    potr: PotrTracker,
    proof_outcome_outbox: ProofOutcomeOutbox,
    repair_transaction_forwarder: RepairTransactionForwarder,
    native_repair_singleflight: native_repair_singleflight::NativeRepairSingleflightV1,
    orderbook_transaction_forwarder: OrderbookTransactionForwarder,
    reserve_transaction_forwarder: ReserveTransactionForwarder,
    por_history: Arc<RwLock<HashMap<PorHistoryKey, PorHistoryEntry>>>,
    storage: Option<Arc<StorageBackend>>,
    pdp_provider: Option<PdpProviderProtocol>,
    deal_engine: DealEngine,
    gc_mutation_lock: Arc<Mutex<()>>,
    gc_eviction_intents: Arc<RwLock<GcEvictionIntentRuntime>>,
    gc_eviction_audit_links: Arc<RwLock<BTreeMap<u64, GcEvictionAuditLinkV1>>>,
    repair_orchestrator: Arc<RwLock<Option<Arc<dyn RepairOrchestrator>>>>,
    governance_publisher: Arc<RwLock<Option<Arc<dyn GovernancePublisher>>>>,
    governance_outbox: Arc<RwLock<GovernanceOutboxRuntime>>,
    governance_outbox_drain_lock: Arc<Mutex<()>>,
    runtime_mutation_lock: Arc<Mutex<()>>,
    auxiliary_checkpoint_lock: Arc<Mutex<()>>,
    durability_failure: Arc<Mutex<Option<String>>>,
    auxiliary_runtime_checkpoint_path: Option<PathBuf>,
    reputation_trust_policy: Option<Arc<ReputationSnapshotTrustPolicyV1>>,
    latest_reputation_snapshot: Arc<RwLock<Option<ReputationSnapshotV1>>>,
    reputation_snapshots: Arc<RwLock<BTreeMap<[u8; 16], AdmittedReputationSnapshotV1>>>,
    reputation_events: Arc<RwLock<BoundedEventHistory<ReputationSnapshotEventV1>>>,
    reputation_event_sender: broadcast::Sender<ReputationSnapshotEventV1>,
    pricing_trust_policy: Option<Arc<PricingTrustPolicyV1>>,
    governed_pricing: Arc<RwLock<Option<GovernedPricingSeriesV1>>>,
    pricing_checkpoint_path: Option<PathBuf>,
    hedging_feed_trust_policy: Option<Arc<HedgingFeedTrustPolicyV1>>,
    signed_hedging_feeds: Arc<RwLock<Option<SignedHedgingFeedLedgerV1>>>,
    hedging_checkpoint_path: Option<PathBuf>,
    moderation_model_registry_checkpoint_path: Option<PathBuf>,
    moderation_model_registry: Arc<RwLock<ModerationModelRegistry>>,
    moderation_screening_checkpoint_path: Option<PathBuf>,
    moderation_screening: Arc<RwLock<ModerationScreeningRuntime>>,
    moderation_screening_authority: Arc<RwLock<Option<ModerationScreeningAuthorityV1>>>,
    moderation_quarantine_object_root: Option<PathBuf>,
    moderation_quarantine_object_index_path: Option<PathBuf>,
    moderation_quarantine_key_wrapper: Option<OpaqueModerationQuarantineKeyWrapper>,
    moderation_quarantine_objects: Arc<RwLock<ModerationQuarantineObjectRuntime>>,
    moderation_evidence_viewer_checkpoint_path: Option<PathBuf>,
    moderation_evidence_viewer: Arc<RwLock<ModerationEvidenceViewerRuntime>>,
    transparency_ledger_source_entries:
        Arc<RwLock<BTreeMap<String, TransparencyLedgerSourceEntry>>>,
    privacy_aggregate_source_events: Arc<RwLock<BTreeMap<String, PrivacyAggregateSourceEvent>>>,
    privacy_source_event_receipts: Arc<RwLock<BTreeMap<String, [u8; 32]>>>,
    privacy_publish_request_receipts: Arc<RwLock<BTreeMap<String, PrivacyPublishRequestReceiptV1>>>,
    published_privacy_aggregate_cycles: Arc<RwLock<BTreeSet<[u8; 16]>>>,
    privacy_composition_budget: Arc<RwLock<PrivacyCompositionBudgetLedgerV1>>,
    privacy_release_ledger: Arc<RwLock<transparency::PrivacyReleaseLedgerV1>>,
    privacy_cycle_prf_provider: Option<OpaquePrivacyCyclePrfProvider>,
    privacy_release_anchor: Option<OpaquePrivacyReleaseAnchor>,
    published_evidence_viewer_audit_cycles: Arc<RwLock<BTreeSet<[u8; 16]>>>,
}

type PorHistoryKey = ([u8; 32], [u8; 32]);

#[derive(Debug, Default, Clone)]
struct PorHistoryEntry {
    last_success_unix: Option<u64>,
    last_failure_unix: Option<u64>,
    failures_total: u64,
    consecutive_failures: u64,
    last_slash_unix: Option<u64>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct PorHistoryCheckpointEntryV1 {
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    last_success_unix: Option<u64>,
    last_failure_unix: Option<u64>,
    failures_total: u64,
    consecutive_failures: u64,
    last_slash_unix: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
enum GovernanceOutboxKindV1 {
    DealSettlement,
    GcAudit,
    ReconciliationReport,
    SignedReputationSnapshot,
    TransparencyLedgerPublication,
    ProofTokenIssuance,
    AppealFinanceReport,
    AppealFinanceWeeklyRollup,
    AppealFinanceSettlementReceipt,
    PdpArchive,
}

impl GovernanceOutboxKindV1 {
    const fn tag(self) -> u8 {
        match self {
            Self::DealSettlement => 0,
            Self::GcAudit => 1,
            Self::ReconciliationReport => 2,
            Self::SignedReputationSnapshot => 3,
            Self::TransparencyLedgerPublication => 5,
            Self::ProofTokenIssuance => 6,
            Self::AppealFinanceReport => 7,
            Self::AppealFinanceWeeklyRollup => 8,
            Self::AppealFinanceSettlementReceipt => 9,
            Self::PdpArchive => 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct GovernanceOutboxEntryV1 {
    version: u8,
    sequence: u64,
    kind: GovernanceOutboxKindV1,
    payload_digest: [u8; 32],
    binding_digest: [u8; 32],
    payload_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GovernanceOutboxRuntime {
    next_sequence: u64,
    entries: BTreeMap<u64, GovernanceOutboxEntryV1>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GovernanceOutboxReservationUse {
    None,
    GcEviction,
}

impl Default for GovernanceOutboxRuntime {
    fn default() -> Self {
        Self {
            next_sequence: 1,
            entries: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct GcStorageIdentityV1 {
    total_bytes: u64,
    manifest_count: u64,
    gc_freed_bytes_total: u64,
    gc_evictions_total: u64,
    manifest_set_digest: [u8; 32],
    chunk_refcounts_digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct GcEvictionIntentV1 {
    version: u8,
    sequence: u64,
    manifest_id: String,
    manifest_digest: [u8; 32],
    manifest_identity_digest: [u8; 32],
    provider_id: [u8; 32],
    audit_timestamp_unix: u64,
    reason: String,
    expected_freed_bytes: u64,
    storage_before: GcStorageIdentityV1,
    storage_after: GcStorageIdentityV1,
    reserved_outbox_slots: u8,
    binding_digest: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GcEvictionIntentRuntime {
    next_sequence: u64,
    entries: BTreeMap<u64, GcEvictionIntentV1>,
}

impl Default for GcEvictionIntentRuntime {
    fn default() -> Self {
        Self {
            next_sequence: 1,
            entries: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct GcEvictionAuditLinkV1 {
    version: u8,
    intent_sequence: u64,
    outbox_sequence: u64,
    manifest_id: String,
    manifest_digest: [u8; 32],
    provider_id: [u8; 32],
    occurred_at_unix: u64,
    freed_bytes: u64,
    reason: String,
    storage_gc_evictions_total: u64,
    payload_digest: [u8; 32],
    outbox_payload_digest: [u8; 32],
    binding_digest: [u8; 32],
}

#[derive(Debug)]
struct GcStorageSnapshot {
    identity: GcStorageIdentityV1,
    manifests: Vec<StoredManifest>,
    chunk_refcounts: Vec<ChunkRefcountEntry>,
}

fn gc_manifest_identity_digest(manifest: &StoredManifest) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(GC_STORAGE_MANIFEST_IDENTITY_DOMAIN_V1);
    hash_length_prefixed(&mut hasher, manifest.manifest_id().as_bytes());
    hash_length_prefixed(&mut hasher, manifest.manifest_cid());
    hasher.update(manifest.manifest_digest());
    hasher.update(manifest.payload_digest());
    hasher.update(&manifest.content_length().to_le_bytes());
    hash_length_prefixed(&mut hasher, manifest.chunk_profile_handle().as_bytes());
    hasher.update(&manifest.stored_at_unix_secs().to_le_bytes());
    hasher.update(&manifest.retention_epoch().to_le_bytes());
    hasher.update(
        &u64::try_from(manifest.chunk_count())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for index in 0..manifest.chunk_count() {
        let Some(chunk) = manifest.chunk(index) else {
            hasher.update(&[0xFF]);
            continue;
        };
        hasher.update(&chunk.offset.to_le_bytes());
        hasher.update(&chunk.length.to_le_bytes());
        hasher.update(&chunk.digest);
    }
    *hasher.finalize().as_bytes()
}

fn gc_manifest_set_digest(manifests: &[StoredManifest]) -> [u8; 32] {
    let mut identities = manifests
        .iter()
        .map(|manifest| {
            (
                manifest.manifest_id().to_owned(),
                gc_manifest_identity_digest(manifest),
            )
        })
        .collect::<Vec<_>>();
    identities.sort_by(|left, right| left.0.cmp(&right.0));
    let mut hasher = blake3::Hasher::new();
    hasher.update(GC_STORAGE_MANIFEST_SET_DOMAIN_V1);
    hasher.update(
        &u64::try_from(identities.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for (manifest_id, digest) in identities {
        hash_length_prefixed(&mut hasher, manifest_id.as_bytes());
        hasher.update(&digest);
    }
    *hasher.finalize().as_bytes()
}

fn gc_chunk_refcounts_digest(refcounts: &[ChunkRefcountEntry]) -> [u8; 32] {
    let mut refcounts = refcounts.to_vec();
    refcounts.sort_by_key(|entry| entry.digest);
    let mut hasher = blake3::Hasher::new();
    hasher.update(GC_STORAGE_CHUNK_REFCOUNTS_DOMAIN_V1);
    hasher.update(
        &u64::try_from(refcounts.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );
    for entry in refcounts {
        hasher.update(&entry.digest);
        hasher.update(&entry.count.to_le_bytes());
    }
    *hasher.finalize().as_bytes()
}

fn gc_storage_identity_from_parts(
    manifests: &[StoredManifest],
    chunk_refcounts: &[ChunkRefcountEntry],
    total_bytes: u64,
    gc_counters: (u64, u64),
) -> Result<GcStorageIdentityV1, GovernancePublishError> {
    Ok(GcStorageIdentityV1 {
        total_bytes,
        manifest_count: u64::try_from(manifests.len())
            .map_err(|_| GovernancePublishError::other("storage manifest count exceeds u64"))?,
        gc_freed_bytes_total: gc_counters.0,
        gc_evictions_total: gc_counters.1,
        manifest_set_digest: gc_manifest_set_digest(manifests),
        chunk_refcounts_digest: gc_chunk_refcounts_digest(chunk_refcounts),
    })
}

fn gc_storage_snapshot_once_unchecked(
    storage: &StorageBackend,
) -> Result<GcStorageSnapshot, GovernancePublishError> {
    let manifests = storage.manifests();
    let chunk_refcounts = storage.chunk_refcount_snapshot();
    let total_bytes = storage.total_bytes();
    let gc_counters = storage.gc_counters();
    let identity =
        gc_storage_identity_from_parts(&manifests, &chunk_refcounts, total_bytes, gc_counters)?;
    Ok(GcStorageSnapshot {
        identity,
        manifests,
        chunk_refcounts,
    })
}

fn gc_storage_snapshot_once(
    storage: &StorageBackend,
) -> Result<GcStorageSnapshot, GovernancePublishError> {
    storage
        .ensure_durability_healthy()
        .map_err(|err| GovernancePublishError::other(err.to_string()))?;
    gc_storage_snapshot_once_unchecked(storage)
}

fn gc_storage_snapshot(
    storage: &StorageBackend,
) -> Result<GcStorageSnapshot, GovernancePublishError> {
    let first = gc_storage_snapshot_once(storage)?;
    let second = gc_storage_snapshot_once(storage)?;
    if first.identity != second.identity {
        return Err(GovernancePublishError::other(
            "storage generation changed while capturing a GC eviction intent",
        ));
    }
    Ok(second)
}

fn gc_storage_snapshot_unchecked(
    storage: &StorageBackend,
) -> Result<GcStorageSnapshot, GovernancePublishError> {
    let first = gc_storage_snapshot_once_unchecked(storage)?;
    let second = gc_storage_snapshot_once_unchecked(storage)?;
    if first.identity != second.identity {
        return Err(GovernancePublishError::other(
            "storage generation changed while inspecting a failed GC eviction",
        ));
    }
    Ok(second)
}

fn gc_expected_post_storage_identity(
    snapshot: &GcStorageSnapshot,
    target: &StoredManifest,
) -> Result<GcStorageIdentityV1, GovernancePublishError> {
    let manifests = snapshot
        .manifests
        .iter()
        .filter(|manifest| manifest.manifest_id() != target.manifest_id())
        .cloned()
        .collect::<Vec<_>>();
    if manifests.len().checked_add(1) != Some(snapshot.manifests.len()) {
        return Err(GovernancePublishError::other(
            "GC target is not uniquely present in the storage generation",
        ));
    }
    let mut refcounts = snapshot
        .chunk_refcounts
        .iter()
        .map(|entry| (entry.digest, entry.count))
        .collect::<BTreeMap<_, _>>();
    if refcounts.len() != snapshot.chunk_refcounts.len() {
        return Err(GovernancePublishError::other(
            "storage chunk refcount snapshot contains duplicate digests",
        ));
    }
    for index in 0..target.chunk_count() {
        let chunk = target.chunk(index).ok_or_else(|| {
            GovernancePublishError::other("GC target chunk metadata is incomplete")
        })?;
        let count = refcounts.get_mut(&chunk.digest).ok_or_else(|| {
            GovernancePublishError::other("GC target chunk lacks a storage refcount")
        })?;
        *count = count
            .checked_sub(1)
            .ok_or_else(|| GovernancePublishError::other("GC target chunk refcount underflow"))?;
        if *count == 0 {
            refcounts.remove(&chunk.digest);
        }
    }
    let refcounts = refcounts
        .into_iter()
        .map(|(digest, count)| ChunkRefcountEntry { digest, count })
        .collect::<Vec<_>>();
    let total_bytes = snapshot
        .identity
        .total_bytes
        .checked_sub(target.content_length())
        .ok_or_else(|| {
            GovernancePublishError::other("GC target exceeds accounted storage bytes")
        })?;
    let freed_bytes_total = snapshot
        .identity
        .gc_freed_bytes_total
        .checked_add(target.content_length())
        .ok_or_else(|| GovernancePublishError::other("GC freed-byte counter overflow"))?;
    let evictions_total = snapshot
        .identity
        .gc_evictions_total
        .checked_add(1)
        .ok_or_else(|| GovernancePublishError::other("GC eviction counter overflow"))?;
    gc_storage_identity_from_parts(
        &manifests,
        &refcounts,
        total_bytes,
        (freed_bytes_total, evictions_total),
    )
}

fn gc_eviction_payload(intent: &GcEvictionIntentV1) -> GcAuditPayloadV1 {
    GcAuditPayloadV1 {
        version: GC_AUDIT_PAYLOAD_VERSION_V1,
        manifest_digest: intent.manifest_digest,
        provider_id: intent.provider_id,
        evicted_at_unix: intent.audit_timestamp_unix,
        freed_bytes: intent.expected_freed_bytes,
        reason: intent.reason.clone(),
        blocked_reason: None,
    }
}

fn gc_eviction_intent_binding_digest(
    intent: &GcEvictionIntentV1,
) -> Result<[u8; 32], GovernancePublishError> {
    let storage_before = norito::to_bytes(&intent.storage_before).map_err(|err| {
        GovernancePublishError::other(format!("encode GC pre-eviction identity: {err}"))
    })?;
    let storage_after = norito::to_bytes(&intent.storage_after).map_err(|err| {
        GovernancePublishError::other(format!("encode GC post-eviction identity: {err}"))
    })?;
    let mut hasher = blake3::Hasher::new();
    hasher.update(GC_EVICTION_INTENT_BINDING_DOMAIN_V1);
    hasher.update(&[intent.version]);
    hasher.update(&intent.sequence.to_le_bytes());
    hash_length_prefixed(&mut hasher, intent.manifest_id.as_bytes());
    hasher.update(&intent.manifest_digest);
    hasher.update(&intent.manifest_identity_digest);
    hasher.update(&intent.provider_id);
    hasher.update(&intent.audit_timestamp_unix.to_le_bytes());
    hash_length_prefixed(&mut hasher, intent.reason.as_bytes());
    hasher.update(&intent.expected_freed_bytes.to_le_bytes());
    hash_length_prefixed(&mut hasher, &storage_before);
    hash_length_prefixed(&mut hasher, &storage_after);
    hasher.update(&[intent.reserved_outbox_slots]);
    Ok(*hasher.finalize().as_bytes())
}

fn validate_gc_eviction_intent(intent: &GcEvictionIntentV1) -> Result<(), GovernancePublishError> {
    if intent.version != GC_EVICTION_INTENT_VERSION_V1
        || intent.sequence == 0
        || intent.reserved_outbox_slots != GC_EVICTION_RESERVED_OUTBOX_SLOTS
        || intent.manifest_identity_digest == [0; 32]
    {
        return Err(GovernancePublishError::other(
            "GC eviction intent version, sequence, identity, or reservation is invalid",
        ));
    }
    if intent.manifest_id != hex::encode(intent.manifest_digest) {
        return Err(GovernancePublishError::other(
            "GC eviction intent manifest id is not canonical for its digest",
        ));
    }
    gc_eviction_payload(intent)
        .validate()
        .map_err(|err| GovernancePublishError::other(err.to_string()))?;
    if intent.storage_before.manifest_count
        != intent
            .storage_after
            .manifest_count
            .checked_add(1)
            .ok_or_else(|| GovernancePublishError::other("GC manifest generation count overflow"))?
        || intent.storage_before.total_bytes
            != intent
                .storage_after
                .total_bytes
                .checked_add(intent.expected_freed_bytes)
                .ok_or_else(|| GovernancePublishError::other("GC byte generation overflow"))?
        || intent.storage_after.gc_freed_bytes_total
            != intent
                .storage_before
                .gc_freed_bytes_total
                .checked_add(intent.expected_freed_bytes)
                .ok_or_else(|| GovernancePublishError::other("GC freed-byte counter overflow"))?
        || intent.storage_after.gc_evictions_total
            != intent
                .storage_before
                .gc_evictions_total
                .checked_add(1)
                .ok_or_else(|| GovernancePublishError::other("GC counter overflow"))?
        || intent.storage_before.manifest_set_digest == intent.storage_after.manifest_set_digest
    {
        return Err(GovernancePublishError::other(
            "GC eviction intent storage generations do not encode one exact eviction",
        ));
    }
    if intent.binding_digest != gc_eviction_intent_binding_digest(intent)? {
        return Err(GovernancePublishError::other(
            "GC eviction intent binding digest mismatch",
        ));
    }
    Ok(())
}

fn restore_gc_eviction_intents(
    next_sequence: u64,
    entries: Vec<GcEvictionIntentV1>,
) -> Result<GcEvictionIntentRuntime, GovernancePublishError> {
    if next_sequence == 0 || entries.len() > 1 {
        return Err(GovernancePublishError::other(
            "GC eviction intent checkpoint has an invalid sequence or count",
        ));
    }
    let mut restored = BTreeMap::new();
    for intent in entries {
        validate_gc_eviction_intent(&intent)?;
        if intent.sequence >= next_sequence || restored.insert(intent.sequence, intent).is_some() {
            return Err(GovernancePublishError::other(
                "GC eviction intent sequence must be unique and below its high-water mark",
            ));
        }
    }
    Ok(GcEvictionIntentRuntime {
        next_sequence,
        entries: restored,
    })
}

fn gc_eviction_reserved_outbox_slots(
    runtime: &GcEvictionIntentRuntime,
) -> Result<usize, GovernancePublishError> {
    runtime.entries.values().try_fold(0usize, |total, intent| {
        validate_gc_eviction_intent(intent)?;
        total
            .checked_add(usize::from(intent.reserved_outbox_slots))
            .ok_or_else(|| GovernancePublishError::other("GC outbox reservation count overflow"))
    })
}

fn gc_eviction_audit_link_binding_digest(link: &GcEvictionAuditLinkV1) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(GC_EVICTION_AUDIT_LINK_BINDING_DOMAIN_V1);
    hasher.update(&[link.version]);
    hasher.update(&link.intent_sequence.to_le_bytes());
    hasher.update(&link.outbox_sequence.to_le_bytes());
    hash_length_prefixed(&mut hasher, link.manifest_id.as_bytes());
    hasher.update(&link.manifest_digest);
    hasher.update(&link.provider_id);
    hasher.update(&link.occurred_at_unix.to_le_bytes());
    hasher.update(&link.freed_bytes.to_le_bytes());
    hash_length_prefixed(&mut hasher, link.reason.as_bytes());
    hasher.update(&link.storage_gc_evictions_total.to_le_bytes());
    hasher.update(&link.payload_digest);
    hasher.update(&link.outbox_payload_digest);
    *hasher.finalize().as_bytes()
}

fn gc_eviction_audit_event_from_link(
    link: &GcEvictionAuditLinkV1,
) -> Result<GcAuditEventV1, GovernancePublishError> {
    let payload = GcAuditPayloadV1 {
        version: GC_AUDIT_PAYLOAD_VERSION_V1,
        manifest_digest: link.manifest_digest,
        provider_id: link.provider_id,
        evicted_at_unix: link.occurred_at_unix,
        freed_bytes: link.freed_bytes,
        reason: link.reason.clone(),
        blocked_reason: None,
    };
    let event = GcAuditEventV1 {
        version: GC_AUDIT_EVENT_VERSION_V1,
        header: SorafsAuditHeaderV1 {
            sequence: link.outbox_sequence,
            occurred_at_unix: link.occurred_at_unix,
            signer: GC_AUDIT_SIGNER_V1.to_owned(),
            payload_digest: gc_audit_payload_digest_v1(&payload)
                .map_err(|err| GovernancePublishError::other(err.to_string()))?,
        },
        payload,
    };
    event
        .validate()
        .map_err(|err| GovernancePublishError::other(err.to_string()))?;
    Ok(event)
}

fn validate_gc_eviction_audit_link(
    link: &GcEvictionAuditLinkV1,
) -> Result<(), GovernancePublishError> {
    if link.version != GC_EVICTION_AUDIT_LINK_VERSION_V1
        || link.intent_sequence == 0
        || link.outbox_sequence == 0
        || link.storage_gc_evictions_total == 0
        || link.manifest_id != hex::encode(link.manifest_digest)
    {
        return Err(GovernancePublishError::other(
            "GC eviction audit linkage metadata is invalid",
        ));
    }
    let event = gc_eviction_audit_event_from_link(link)?;
    let payload_digest = gc_audit_payload_digest_v1(&event.payload)
        .map_err(|err| GovernancePublishError::other(err.to_string()))?;
    let encoded = norito::to_bytes(&event)
        .map_err(|err| GovernancePublishError::other(format!("encode GC audit link: {err}")))?;
    if link.payload_digest != payload_digest
        || link.outbox_payload_digest != *blake3::hash(&encoded).as_bytes()
        || link.binding_digest != gc_eviction_audit_link_binding_digest(link)
    {
        return Err(GovernancePublishError::other(
            "GC eviction audit linkage digest mismatch",
        ));
    }
    Ok(())
}

fn hash_length_prefixed(hasher: &mut blake3::Hasher, bytes: &[u8]) {
    hasher.update(&u64::try_from(bytes.len()).unwrap_or(u64::MAX).to_le_bytes());
    hasher.update(bytes);
}

fn governance_outbox_binding_digest(
    version: u8,
    sequence: u64,
    kind: GovernanceOutboxKindV1,
    payload_digest: [u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(GOVERNANCE_OUTBOX_BINDING_DOMAIN_V2);
    hasher.update(&[version]);
    hasher.update(&sequence.to_le_bytes());
    hasher.update(&[kind.tag()]);
    hasher.update(&payload_digest);
    *hasher.finalize().as_bytes()
}

fn decode_canonical_governance_payload<T>(bytes: &[u8]) -> Result<T, GovernancePublishError>
where
    for<'de> T: norito::NoritoDeserialize<'de>,
    T: norito::NoritoSerialize,
{
    let value = norito::decode_from_bytes::<T>(bytes).map_err(|err| {
        GovernancePublishError::other(format!("decode governance outbox payload: {err}"))
    })?;
    let canonical = norito::to_bytes(&value).map_err(|err| {
        GovernancePublishError::other(format!("re-encode governance outbox payload: {err}"))
    })?;
    if canonical != bytes {
        return Err(GovernancePublishError::other(
            "governance outbox payload is not canonically encoded",
        ));
    }
    Ok(value)
}

fn decode_canonical_signed_reputation_payload(
    bytes: &[u8],
) -> Result<SignedReputationSnapshotV1, GovernancePublishError> {
    decode_signed_reputation_snapshot(bytes).map_err(|err| {
        GovernancePublishError::other(format!(
            "decode signed reputation governance payload: {err}"
        ))
    })
}

fn validate_gc_audit_event(
    entry_sequence: u64,
    event: &GcAuditEventV1,
) -> Result<(), GovernancePublishError> {
    event
        .validate()
        .map_err(|err| GovernancePublishError::other(err.to_string()))?;
    if event.header.sequence != entry_sequence {
        return Err(GovernancePublishError::other(
            "GC audit header sequence does not match its outbox entry",
        ));
    }
    Ok(())
}

fn validate_governance_outbox_entry(
    entry: &GovernanceOutboxEntryV1,
) -> Result<(), GovernancePublishError> {
    if entry.version != GOVERNANCE_OUTBOX_VERSION_V2 {
        return Err(GovernancePublishError::other(format!(
            "unsupported governance outbox entry version {}",
            entry.version
        )));
    }
    if entry.sequence == 0 || entry.payload_bytes.is_empty() {
        return Err(GovernancePublishError::other(
            "governance outbox sequence and payload must be non-zero/non-empty",
        ));
    }
    let digest = *blake3::hash(&entry.payload_bytes).as_bytes();
    if digest != entry.payload_digest {
        return Err(GovernancePublishError::other(
            "governance outbox payload digest mismatch",
        ));
    }
    let binding_digest = governance_outbox_binding_digest(
        entry.version,
        entry.sequence,
        entry.kind,
        entry.payload_digest,
    );
    if binding_digest != entry.binding_digest {
        return Err(GovernancePublishError::other(
            "governance outbox kind/sequence binding digest mismatch",
        ));
    }
    match entry.kind {
        GovernanceOutboxKindV1::DealSettlement => {
            decode_canonical_governance_payload::<DealSettlementV1>(&entry.payload_bytes)?
                .validate()
                .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
        GovernanceOutboxKindV1::GcAudit => {
            let event =
                decode_canonical_governance_payload::<GcAuditEventV1>(&entry.payload_bytes)?;
            validate_gc_audit_event(entry.sequence, &event)?;
        }
        GovernanceOutboxKindV1::ReconciliationReport => {
            decode_canonical_governance_payload::<SorafsReconciliationReportV1>(
                &entry.payload_bytes,
            )?
            .validate()
            .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
        GovernanceOutboxKindV1::SignedReputationSnapshot => {
            decode_canonical_signed_reputation_payload(&entry.payload_bytes)?;
        }
        GovernanceOutboxKindV1::TransparencyLedgerPublication => {
            decode_canonical_governance_payload::<ModerationLedgerCyclePublicationV1>(
                &entry.payload_bytes,
            )?
            .validate()
            .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
        GovernanceOutboxKindV1::ProofTokenIssuance => {
            decode_canonical_governance_payload::<ProofTokenIssuanceV1>(&entry.payload_bytes)?
                .validate()
                .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
        GovernanceOutboxKindV1::AppealFinanceReport => {
            decode_canonical_governance_payload::<SoraFsAppealFinanceReportV1>(
                &entry.payload_bytes,
            )?
            .validate()
            .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
        GovernanceOutboxKindV1::AppealFinanceWeeklyRollup => {
            decode_canonical_governance_payload::<SoraFsAppealFinanceWeeklyRollupV1>(
                &entry.payload_bytes,
            )?
            .validate()
            .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
        GovernanceOutboxKindV1::AppealFinanceSettlementReceipt => {
            decode_canonical_governance_payload::<SoraFsAppealFinanceSettlementReceiptV1>(
                &entry.payload_bytes,
            )?
            .validate()
            .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
        GovernanceOutboxKindV1::PdpArchive => {
            decode_canonical_governance_payload::<PdpGovernanceArchiveV1>(&entry.payload_bytes)?
                .validate()
                .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        }
    }
    Ok(())
}

fn insert_governance_outbox_entry(
    outbox: &mut GovernanceOutboxRuntime,
    kind: GovernanceOutboxKindV1,
    payload_bytes: Vec<u8>,
    entry_limit: usize,
    reserved_slots: usize,
) -> Result<(u64, bool), GovernancePublishError> {
    let payload_digest = *blake3::hash(&payload_bytes).as_bytes();
    if let Some(existing) = outbox.entries.values().find(|entry| {
        entry.kind == kind
            && entry.payload_digest == payload_digest
            && entry.payload_bytes == payload_bytes
    }) {
        return Ok((existing.sequence, false));
    }
    let unreserved_limit = entry_limit.checked_sub(reserved_slots).ok_or_else(|| {
        GovernancePublishError::other(format!(
            "governance outbox reservation {reserved_slots} exceeds retention limit {entry_limit}"
        ))
    })?;
    if outbox.entries.len() >= unreserved_limit {
        return Err(GovernancePublishError::other(format!(
            "governance outbox retention exhausted: {reserved_slots} of {entry_limit} slots are reserved for durable domain transactions"
        )));
    }
    let sequence = outbox.next_sequence;
    let next_sequence = sequence
        .checked_add(1)
        .ok_or_else(|| GovernancePublishError::other("governance outbox sequence exhausted"))?;
    let entry = GovernanceOutboxEntryV1 {
        version: GOVERNANCE_OUTBOX_VERSION_V2,
        sequence,
        kind,
        payload_digest,
        binding_digest: governance_outbox_binding_digest(
            GOVERNANCE_OUTBOX_VERSION_V2,
            sequence,
            kind,
            payload_digest,
        ),
        payload_bytes,
    };
    validate_governance_outbox_entry(&entry)?;
    outbox.entries.insert(sequence, entry);
    outbox.next_sequence = next_sequence;
    Ok((sequence, true))
}

fn restore_governance_outbox(
    next_sequence: u64,
    entries: Vec<GovernanceOutboxEntryV1>,
    entry_limit: usize,
) -> Result<GovernanceOutboxRuntime, GovernancePublishError> {
    if next_sequence == 0 {
        return Err(GovernancePublishError::other(
            "governance outbox next sequence must be non-zero",
        ));
    }
    if entries.len() > entry_limit {
        return Err(GovernancePublishError::other(format!(
            "governance outbox checkpoint count {} exceeds configured limit {entry_limit}",
            entries.len()
        )));
    }
    let mut restored = BTreeMap::new();
    let mut previous_sequence = None;
    for entry in entries {
        validate_governance_outbox_entry(&entry)?;
        // Gaps are intentional: an acknowledged artifact or a rejected
        // prepared domain intent can be removed while later entries remain.
        if entry.sequence >= next_sequence
            || previous_sequence.is_some_and(|previous| entry.sequence <= previous)
        {
            return Err(GovernancePublishError::other(
                "governance outbox entries must be strictly ordered below the next sequence",
            ));
        }
        previous_sequence = Some(entry.sequence);
        if restored.insert(entry.sequence, entry).is_some() {
            return Err(GovernancePublishError::other(
                "duplicate governance outbox sequence",
            ));
        }
    }
    Ok(GovernanceOutboxRuntime {
        next_sequence,
        entries: restored,
    })
}

fn validate_privacy_release_outbox(
    release_ledger: &transparency::PrivacyReleaseLedgerV1,
    published_cycles: &BTreeSet<[u8; 16]>,
    outbox: &GovernanceOutboxRuntime,
) -> Result<(), GovernancePublishError> {
    let records = release_ledger
        .records
        .iter()
        .map(|record| (record.release_id, record))
        .collect::<BTreeMap<_, _>>();
    let mut pending_cycles = BTreeSet::new();
    for entry in outbox
        .entries
        .values()
        .filter(|entry| entry.kind == GovernanceOutboxKindV1::TransparencyLedgerPublication)
    {
        let publication = decode_canonical_governance_payload::<ModerationLedgerCyclePublicationV1>(
            &entry.payload_bytes,
        )?;
        let is_privacy = !publication.proofs.is_empty()
            && publication
                .proofs
                .iter()
                .all(|proof| proof.entry.kind == ModerationLedgerEntryKindV1::PrivacyAggregate);
        if !is_privacy {
            continue;
        }
        let cycle_id = publication.block.cycle_id;
        if !published_cycles.contains(&cycle_id) || !pending_cycles.insert(cycle_id) {
            return Err(GovernancePublishError::other(
                "pending privacy publication has no unique published-cycle guard",
            ));
        }
        let record = records.get(&cycle_id).ok_or_else(|| {
            GovernancePublishError::other(
                "pending privacy publication has no durable privacy release record",
            )
        })?;
        let payload_digest = *blake3::hash(&entry.payload_bytes).as_bytes();
        if record.status != transparency::PrivacyReleaseStatusV1::Published
            || record.publication_payload_digest != Some(payload_digest)
            || publication.block.cycle_start_unix != record.cycle_start_unix
            || publication.block.cycle_end_unix != record.cycle_end_unix
            || publication.block.generated_at_unix != record.cycle_end_unix
            || publication.block.previous_block_hash != record.previous_publication_block_hash
            || publication.block.block_hash().map_err(|error| {
                GovernancePublishError::other(format!(
                    "hash pending privacy publication block: {error}"
                ))
            })? != record.publication_block_hash.ok_or_else(|| {
                GovernancePublishError::other("published privacy release omitted its block hash")
            })?
        {
            return Err(GovernancePublishError::other(
                "pending privacy publication conflicts with its durable release record",
            ));
        }
        let mut populations = Vec::with_capacity(publication.privacy_aggregates.len());
        let mut metric_schema: Option<Vec<PrivacyAggregateMetricSchemaV1>> = None;
        for aggregate in &publication.privacy_aggregates {
            if aggregate.policy_digest != record.policy_digest
                || aggregate.privacy != record.privacy
            {
                return Err(GovernancePublishError::other(
                    "pending privacy aggregate payload conflicts with release policy",
                ));
            }
            match (aggregate.noise_source, record.prf_commitment) {
                (
                    sorafs_manifest::ModerationPrivacyNoiseSourceV1::ThresholdPrf(commitment),
                    Some(expected),
                ) if commitment.commitment == expected => {}
                (sorafs_manifest::ModerationPrivacyNoiseSourceV1::SuppressionOnly, None) => {}
                _ => {
                    return Err(GovernancePublishError::other(
                        "pending privacy aggregate payload conflicts with release randomness evidence",
                    ));
                }
            }
            populations.push(PrivacyAggregatePopulationV1 {
                label: aggregate.population_label.clone(),
                digest: aggregate.population_digest,
            });
            let aggregate_schema = aggregate
                .metrics
                .iter()
                .map(|metric| PrivacyAggregateMetricSchemaV1 {
                    key: metric.key.clone(),
                    unit: metric.unit.clone(),
                })
                .collect::<Vec<_>>();
            if metric_schema
                .as_ref()
                .is_some_and(|expected| expected != &aggregate_schema)
            {
                return Err(GovernancePublishError::other(
                    "pending privacy aggregate payloads use inconsistent metric schemas",
                ));
            }
            metric_schema.get_or_insert(aggregate_schema);
        }
        let metric_schema = metric_schema.ok_or_else(|| {
            GovernancePublishError::other(
                "pending privacy publication omitted its aggregate payload inventory",
            )
        })?;
        populations.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| left.digest.cmp(&right.digest))
        });
        if privacy_published_aggregate_inventory_digest(&publication.privacy_aggregates)?
            != record.published_aggregate_inventory_digest.ok_or_else(|| {
                GovernancePublishError::other(
                    "published privacy release omitted its aggregate inventory digest",
                )
            })?
            || privacy_metric_schema_digest(&metric_schema) != record.metric_schema_digest
            || (record.privacy.per_subject_metric_cap.is_some()
                && privacy_population_inventory_digest(&populations)
                    != record.population_inventory_digest)
        {
            return Err(GovernancePublishError::other(
                "pending privacy aggregate payload inventory conflicts with release bindings",
            ));
        }
    }
    Ok(())
}

const PRIVACY_PUBLISHED_AGGREGATE_INVENTORY_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_release.published_aggregate_inventory.v1";

fn privacy_published_aggregate_inventory_digest(
    aggregates: &[ModerationPrivacyAggregateV1],
) -> Result<[u8; 32], GovernancePublishError> {
    if aggregates.is_empty() {
        return Err(GovernancePublishError::other(
            "privacy publication aggregate inventory is empty",
        ));
    }
    let mut hasher = blake3::Hasher::new();
    hasher.update(PRIVACY_PUBLISHED_AGGREGATE_INVENTORY_DOMAIN_V1);
    hasher.update(&(aggregates.len() as u64).to_le_bytes());
    let mut previous_id: Option<&str> = None;
    for aggregate in aggregates {
        if previous_id.is_some_and(|previous| previous >= aggregate.aggregate_id.as_str()) {
            return Err(GovernancePublishError::other(
                "privacy publication aggregate inventory is not canonical",
            ));
        }
        let digest = aggregate.aggregate_hash().map_err(|error| {
            GovernancePublishError::other(format!(
                "hash privacy publication aggregate inventory: {error}"
            ))
        })?;
        hasher.update(&(aggregate.aggregate_id.len() as u64).to_le_bytes());
        hasher.update(aggregate.aggregate_id.as_bytes());
        hasher.update(&digest);
        previous_id = Some(aggregate.aggregate_id.as_str());
    }
    Ok(*hasher.finalize().as_bytes())
}

fn publish_governance_outbox_entry(
    publisher: &dyn GovernancePublisher,
    entry: &GovernanceOutboxEntryV1,
) -> Result<(), GovernancePublishError> {
    validate_governance_outbox_entry(entry)?;
    match entry.kind {
        GovernanceOutboxKindV1::DealSettlement => {
            let payload =
                decode_canonical_governance_payload::<DealSettlementV1>(&entry.payload_bytes)?;
            publisher.publish_deal_settlement(&payload, &entry.payload_bytes)
        }
        GovernanceOutboxKindV1::GcAudit => {
            let payload =
                decode_canonical_governance_payload::<GcAuditEventV1>(&entry.payload_bytes)?;
            publisher.publish_gc_audit_event(&payload, &entry.payload_bytes)
        }
        GovernanceOutboxKindV1::ReconciliationReport => {
            let payload = decode_canonical_governance_payload::<SorafsReconciliationReportV1>(
                &entry.payload_bytes,
            )?;
            publisher.publish_reconciliation_report(&payload, &entry.payload_bytes)
        }
        GovernanceOutboxKindV1::SignedReputationSnapshot => {
            let payload = decode_canonical_signed_reputation_payload(&entry.payload_bytes)?;
            publisher.publish_reputation_snapshot(&payload, &entry.payload_bytes)
        }
        GovernanceOutboxKindV1::TransparencyLedgerPublication => {
            let payload = decode_canonical_governance_payload::<ModerationLedgerCyclePublicationV1>(
                &entry.payload_bytes,
            )?;
            publisher.publish_transparency_ledger_publication(&payload, &entry.payload_bytes)
        }
        GovernanceOutboxKindV1::ProofTokenIssuance => {
            let payload =
                decode_canonical_governance_payload::<ProofTokenIssuanceV1>(&entry.payload_bytes)?;
            publisher.publish_proof_token_issuance(&payload, &entry.payload_bytes)
        }
        GovernanceOutboxKindV1::AppealFinanceReport => {
            let payload = decode_canonical_governance_payload::<SoraFsAppealFinanceReportV1>(
                &entry.payload_bytes,
            )?;
            publisher.publish_appeal_finance_report(&payload, &entry.payload_bytes)
        }
        GovernanceOutboxKindV1::AppealFinanceWeeklyRollup => {
            let payload = decode_canonical_governance_payload::<SoraFsAppealFinanceWeeklyRollupV1>(
                &entry.payload_bytes,
            )?;
            publisher.publish_appeal_finance_weekly_rollup(&payload, &entry.payload_bytes)
        }
        GovernanceOutboxKindV1::AppealFinanceSettlementReceipt => {
            let payload = decode_canonical_governance_payload::<
                SoraFsAppealFinanceSettlementReceiptV1,
            >(&entry.payload_bytes)?;
            publisher.publish_appeal_finance_settlement_receipt(&payload, &entry.payload_bytes)
        }
        GovernanceOutboxKindV1::PdpArchive => {
            let payload = decode_canonical_governance_payload::<PdpGovernanceArchiveV1>(
                &entry.payload_bytes,
            )?;
            publisher.publish_pdp_archive(&payload, &entry.payload_bytes)
        }
    }
}

/// Signed reputation envelope plus the exact admission time used for freshness checks.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
struct AdmittedReputationSnapshotV1 {
    version: u8,
    admitted_at_unix: u64,
    encoded_len: u64,
    envelope: SignedReputationSnapshotV1,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct AuxiliaryRuntimeCheckpointV2 {
    version: u8,
    capacity_runtime: CapacityRuntimeCheckpointV1,
    deal_runtime: DealRuntimeCheckpointV1,
    por_tracker: por::PorTrackerCheckpointV1,
    por_history: Vec<PorHistoryCheckpointEntryV1>,
    gc_eviction_intent_next_sequence: u64,
    gc_eviction_intents: Vec<GcEvictionIntentV1>,
    gc_eviction_audit_links: Vec<GcEvictionAuditLinkV1>,
    reputation_snapshots: Vec<AdmittedReputationSnapshotV1>,
    latest_reputation_snapshot_id: Option<[u8; 16]>,
    reputation_events: Vec<ReputationSnapshotEventV1>,
    transparency_source_entries: Vec<TransparencyLedgerSourceEntry>,
    privacy_source_events: Vec<PrivacyAggregateSourceEvent>,
    privacy_source_event_receipts: Vec<transparency::PrivacySourceEventReceiptV1>,
    privacy_publish_request_receipts: Vec<PrivacyPublishRequestReceiptV1>,
    published_privacy_aggregate_cycles: Vec<[u8; 16]>,
    privacy_composition_budget: PrivacyCompositionBudgetLedgerV1,
    privacy_release_ledger: transparency::PrivacyReleaseLedgerV1,
    published_evidence_viewer_audit_cycles: Vec<[u8; 16]>,
    governance_outbox_next_sequence: u64,
    governance_outbox_entries: Vec<GovernanceOutboxEntryV1>,
}

/// Unsigned deterministic reputation material intended for external governance signing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReputationSnapshotSigningMaterialV1 {
    /// Canonical snapshot reproduced by the scoring evidence.
    pub snapshot: ReputationSnapshotV1,
    /// Complete provider inputs and trust edges required to replay the snapshot.
    pub scoring_evidence: ReputationScoringEvidenceV1,
}

/// Error type returned by storage-related operations on [`NodeHandle`].
#[derive(Debug, Error)]
pub enum NodeStorageError {
    /// Storage subsystem is disabled in the configuration.
    #[error("SoraFS storage is disabled for this node")]
    Disabled,
    /// Underlying storage backend reported an error.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// Scheduler admission refused work without parking the caller.
    #[error(transparent)]
    Scheduler(#[from] SchedulerAdmissionError),
}

/// Errors returned by the embedded admission-bound PDP provider service.
#[derive(Debug, Error)]
pub enum NodePdpError {
    /// Storage and the durable provider protocol are disabled.
    #[error("SoraFS PDP provider service is disabled for this node")]
    Disabled,
    /// Durable challenge/proof lifecycle processing failed.
    #[error(transparent)]
    Protocol(#[from] PdpProviderProtocolError),
    /// Stored manifest or witness access failed.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// Provider proof construction or signing failed.
    #[error(transparent)]
    ProofBuild(#[from] PdpProofBuildError),
    /// The retained manifest has no PDP commitment.
    #[error("stored SoraFS manifest has no PDP commitment")]
    CommitmentUnavailable,
    /// The queued challenge does not bind the retained manifest commitment.
    #[error("PDP challenge commitment does not match retained storage")]
    CommitmentMismatch,
    /// The active council admission does not authorise the locally configured signing key.
    #[error("configured PDP provider signing key is not authorised by active admission")]
    SigningKeyNotAdmitted,
    /// The configured provider signing key could not be loaded safely.
    #[error("failed to load configured PDP provider signing key: {0}")]
    SigningKey(String),
}

/// Errors that prevent a SoraFS node handle from starting with trustworthy state.
#[derive(Debug, Error)]
pub enum NodeInitError {
    /// The configured storage backend could not be opened or validated.
    #[error("failed to initialise SoraFS storage backend: {0}")]
    Storage(#[from] StorageError),
    /// The durable admission-bound PDP provider checkpoint could not be opened or validated.
    #[error(
        "failed to initialise SoraFS PDP provider checkpoint `{path}`: {message}",
        path = path.display()
    )]
    PdpProvider {
        /// Configured storage root containing the PDP checkpoint.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// The durable final signed PoTR receipt checkpoint could not be opened or validated.
    #[error(
        "failed to initialise SoraFS PoTR receipt checkpoint `{path}`: {message}",
        path = path.display()
    )]
    Potr {
        /// Configured storage root containing the PoTR checkpoint.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// The durable proof-outcome transaction outbox could not be opened or validated.
    #[error(
        "failed to initialise SoraFS proof-outcome outbox `{path}`: {message}",
        path = path.display()
    )]
    ProofOutcomeOutbox {
        /// Configured storage root containing the delivery checkpoint.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// The durable native repair-transaction forwarder could not be opened or validated.
    #[error(
        "failed to initialise SoraFS repair transaction forwarder `{path}`: {message}",
        path = path.display()
    )]
    RepairTransactionForwarder {
        /// Configured storage root containing the repair delivery checkpoint.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// Native repair worker controls are invalid for the consensus lease contract.
    #[error("invalid SoraFS native repair configuration: {message}")]
    NativeRepairConfig {
        /// Stable configuration diagnostic.
        message: String,
    },
    /// The durable native orderbook-transaction forwarder could not be opened or validated.
    #[error(
        "failed to initialise SoraFS orderbook transaction forwarder `{path}`: {message}",
        path = path.display()
    )]
    OrderbookTransactionForwarder {
        /// Configured storage root containing the orderbook delivery checkpoint.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// Native reserve/rent worker controls are outside supported resource bounds.
    #[error("invalid SoraFS native reserve worker configuration: {message}")]
    ReserveWorkerConfig {
        /// Stable configuration diagnostic.
        message: String,
    },
    /// The durable native reserve-transaction forwarder could not be opened or validated.
    #[error(
        "failed to initialise SoraFS reserve transaction forwarder `{path}`: {message}",
        path = path.display()
    )]
    ReserveTransactionForwarder {
        /// Configured storage root containing the reserve delivery checkpoint.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// A durable runtime checkpoint could not be read, decoded, or validated.
    #[error(
        "failed to restore SoraFS {component} checkpoint `{path}`: {message}",
        path = path.display()
    )]
    Checkpoint {
        /// Logical checkpoint component.
        component: &'static str,
        /// Checkpoint path.
        path: PathBuf,
        /// Validation or I/O detail.
        message: String,
    },
    /// Governance publication was configured but its durable publisher could not start.
    #[error("failed to initialise SoraFS governance publisher: {0}")]
    GovernancePublisher(String),
    /// The configured external reputation trust policy could not be read or validated.
    #[error(
        "failed to load SoraFS reputation trust policy `{path}`: {message}",
        path = path.display()
    )]
    ReputationTrustPolicy {
        /// Configured trust-policy path.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// The configured external pricing trust policy could not be read or validated.
    #[error(
        "failed to load SoraFS pricing trust policy `{path}`: {message}",
        path = path.display()
    )]
    PricingTrustPolicy {
        /// Configured trust-policy path.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// The configured external hedging-feed trust policy could not be read or validated.
    #[error(
        "failed to load SoraFS hedging-feed trust policy `{path}`: {message}",
        path = path.display()
    )]
    HedgingFeedTrustPolicy {
        /// Configured trust-policy path.
        path: PathBuf,
        /// Validation or I/O diagnostic.
        message: String,
    },
    /// The config-pinned moderation screening authority bundle could not be
    /// read, digest-verified, canonically decoded, or validated.
    #[error(
        "failed to load SoraFS moderation screening authority bundle `{path}`: {message}",
        path = path.display()
    )]
    ModerationScreeningAuthorityBundle {
        /// Configured authority bundle path, or a configuration sentinel.
        path: PathBuf,
        /// Validation, digest, canonical-codec, or I/O diagnostic.
        message: String,
    },
    /// Authenticated screening was enabled without its runtime-only
    /// PKCS#11/KMS quarantine-key dependency.
    #[error(
        "SoraFS moderation screening requires an injected runtime-only PKCS#11/KMS quarantine key wrapper"
    )]
    ModerationQuarantineKeyWrapperUnavailable,
    /// The injected quarantine-key wrapper exposed an invalid public key handle.
    #[error("failed to validate the SoraFS moderation quarantine key wrapper: {message}")]
    ModerationQuarantineKeyWrapperInvalid {
        /// Stable, payload-free validation detail.
        message: String,
    },
    /// Differential-privacy aggregates were enabled without their runtime-only
    /// threshold-PRF dependency.
    #[error(
        "SoraFS differential-privacy aggregates require an injected runtime-only threshold PRF provider"
    )]
    PrivacyCyclePrfProviderUnavailable,
    /// Differential-privacy aggregates were enabled without an independently
    /// administered finalized release-head dependency.
    #[error(
        "SoraFS differential-privacy aggregates require an injected finalized privacy release anchor"
    )]
    PrivacyReleaseAnchorUnavailable,
}

impl NodeInitError {
    fn checkpoint(component: &'static str, path: &Path, error: impl std::fmt::Display) -> Self {
        Self::Checkpoint {
            component,
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    }
}

fn validate_native_repair_config(config: &RepairConfig) -> Result<(), NodeInitError> {
    let lease_duration_ms = config.claim_ttl_secs().checked_mul(1_000).ok_or_else(|| {
        NodeInitError::NativeRepairConfig {
            message: "claim_ttl_secs overflows milliseconds".to_owned(),
        }
    })?;
    if !(REPAIR_LEDGER_MIN_LEASE_MS_V1..=REPAIR_LEDGER_MAX_LEASE_MS_V1).contains(&lease_duration_ms)
    {
        return Err(NodeInitError::NativeRepairConfig {
            message: format!(
                "claim_ttl_secs must resolve within {REPAIR_LEDGER_MIN_LEASE_MS_V1}..={REPAIR_LEDGER_MAX_LEASE_MS_V1} milliseconds"
            ),
        });
    }
    let renewal_lead_ms = config
        .heartbeat_interval_secs()
        .checked_mul(1_000)
        .ok_or_else(|| NodeInitError::NativeRepairConfig {
            message: "heartbeat_interval_secs overflows milliseconds".to_owned(),
        })?;
    if renewal_lead_ms == 0 || renewal_lead_ms >= lease_duration_ms {
        return Err(NodeInitError::NativeRepairConfig {
            message: "heartbeat_interval_secs must be non-zero and strictly below claim_ttl_secs"
                .to_owned(),
        });
    }
    if !(1..=iroha_config::parameters::defaults::sorafs::repair::MAX_ATTEMPTS_LIMIT)
        .contains(&config.max_attempts())
    {
        return Err(NodeInitError::NativeRepairConfig {
            message: format!(
                "max_attempts must be within 1..={}",
                iroha_config::parameters::defaults::sorafs::repair::MAX_ATTEMPTS_LIMIT
            ),
        });
    }
    if !(1..=iroha_config::parameters::defaults::sorafs::repair::WORKER_CONCURRENCY_LIMIT)
        .contains(&config.worker_concurrency())
    {
        return Err(NodeInitError::NativeRepairConfig {
            message: format!(
                "worker_concurrency must be within 1..={}",
                iroha_config::parameters::defaults::sorafs::repair::WORKER_CONCURRENCY_LIMIT
            ),
        });
    }
    Ok(())
}

fn load_reputation_trust_policy(
    path: &Path,
) -> Result<ReputationSnapshotTrustPolicyV1, NodeInitError> {
    let bytes = read_reputation_trust_policy_file(path).map_err(|error| {
        NodeInitError::ReputationTrustPolicy {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    decode_reputation_trust_policy(&bytes).map_err(|error| NodeInitError::ReputationTrustPolicy {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn load_pricing_trust_policy(path: &Path) -> Result<PricingTrustPolicyV1, NodeInitError> {
    let bytes =
        read_trust_policy_file(path, MAX_PRICING_TRUST_POLICY_BYTES, "pricing trust policy")
            .map_err(|error| NodeInitError::PricingTrustPolicy {
                path: path.to_path_buf(),
                message: error.to_string(),
            })?;
    decode_pricing_trust_policy(&bytes).map_err(|error| NodeInitError::PricingTrustPolicy {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn load_hedging_feed_trust_policy(path: &Path) -> Result<HedgingFeedTrustPolicyV1, NodeInitError> {
    let bytes = read_trust_policy_file(
        path,
        MAX_HEDGING_TRUST_POLICY_BYTES,
        "hedging-feed trust policy",
    )
    .map_err(|error| NodeInitError::HedgingFeedTrustPolicy {
        path: path.to_path_buf(),
        message: error.to_string(),
    })?;
    decode_hedging_feed_trust_policy(&bytes).map_err(|error| {
        NodeInitError::HedgingFeedTrustPolicy {
            path: path.to_path_buf(),
            message: error.to_string(),
        }
    })
}

fn load_moderation_screening_authority_bundle(
    path: &Path,
    expected_digest: [u8; 32],
    now_unix: u64,
) -> Result<ModerationScreeningAuthorityV1, NodeInitError> {
    let error = |message: String| NodeInitError::ModerationScreeningAuthorityBundle {
        path: path.to_path_buf(),
        message,
    };
    if !path.is_absolute() {
        return Err(error(
            "configured authority bundle path must be absolute".to_owned(),
        ));
    }
    if expected_digest == [0; 32] {
        return Err(error(
            "configured authority bundle digest must be non-zero".to_owned(),
        ));
    }
    let bytes = read_trust_policy_file(
        path,
        MODERATION_SCREENING_AUTHORITY_BUNDLE_MAX_BYTES_V1,
        "moderation screening authority bundle",
    )
    .map_err(|read_error| error(read_error.to_string()))?;
    let actual_digest = *blake3::hash(&bytes).as_bytes();
    if actual_digest != expected_digest {
        return Err(error(format!(
            "bundle digest mismatch: expected {}, got {}",
            hex::encode(expected_digest),
            hex::encode(actual_digest)
        )));
    }
    let bundle = decode_local_checkpoint_canonical::<ModerationScreeningAuthorityBundleV1>(
        &bytes,
        u64::try_from(MODERATION_SCREENING_AUTHORITY_BUNDLE_MAX_BYTES_V1).unwrap_or(u64::MAX),
        4_096,
    )
    .map_err(error)?;
    bundle.into_authority(now_unix).map_err(|authority_error| {
        NodeInitError::ModerationScreeningAuthorityBundle {
            path: path.to_path_buf(),
            message: authority_error.to_string(),
        }
    })
}

/// Errors raised while computing reconciliation summaries.
#[derive(Debug, Error)]
pub enum ReconciliationError {
    /// Timestamp must be non-zero.
    #[error("reconciliation timestamp must be non-zero")]
    InvalidTimestamp,
    /// Storage backend is required for reconciliation.
    #[error("SoraFS storage backend is disabled")]
    StorageDisabled,
    /// Local provider identity is required to bind the published report.
    #[error("SoraFS provider identity is unavailable for reconciliation")]
    ProviderBindingUnavailable,
    /// Failed to encode reconciliation snapshot data.
    #[error(transparent)]
    Norito(#[from] norito::Error),
    /// Local appeal-finance Governance DAG data could not be reconciled.
    #[error("appeal finance reconciliation failed: {0}")]
    AppealFinance(String),
    /// Reconciliation report failed validation.
    #[error(transparent)]
    Validation(#[from] ReconciliationValidationError),
}

/// Project an exact decimal quantity onto a legacy micro-unit metrics axis.
///
/// Metrics cannot carry the bounded 512-bit decimal representation used by
/// settlement state. Fractional micro-XOR is therefore rounded toward zero and
/// values outside the metrics counter domain are saturated. This helper must
/// not be used for state, wire payloads, signatures, or digest preimages.
fn quantity_to_metric_micro_saturating(amount: &Quantity) -> u128 {
    amount
        .try_mul_decimal(&Numeric::from(1_000_000_u64))
        .and_then(|scaled| {
            scaled.as_numeric().try_decimal_div_round(
                &Numeric::from(1_u64),
                0,
                RoundingMode::TowardZero,
            )
        })
        .ok()
        .and_then(|scaled| scaled.try_mantissa_u128())
        .unwrap_or(u128::MAX)
}

fn quantity_divergence_bps_saturating(feed: &Quantity, reference: &Quantity) -> u64 {
    if reference.is_zero() {
        return u64::MAX;
    }
    let difference = if feed >= reference {
        feed.checked_sub(reference)
    } else {
        reference.checked_sub(feed)
    };
    difference
        .and_then(|difference| {
            difference.as_numeric().try_decimal_mul_div_round(
                &Numeric::from(10_000_u64),
                reference.as_numeric(),
                0,
                RoundingMode::TowardZero,
            )
        })
        .ok()
        .and_then(|value| value.try_mantissa_u128())
        .and_then(|value| u64::try_from(value).ok())
        .unwrap_or(u64::MAX)
}

impl PdpTerminalHandoff for NodeHandle {
    fn archive(
        &self,
        idempotency_key: [u8; 32],
        archive: &PdpGovernanceArchiveV1,
    ) -> Result<[u8; 32], pdp_provider::PdpExternalHandoffError> {
        self.archive_pdp_terminal_outcome(idempotency_key, archive)
    }

    fn repair(
        &self,
        idempotency_key: [u8; 32],
        report: &RepairReportV1,
    ) -> Result<[u8; 32], pdp_provider::PdpExternalHandoffError> {
        let _ = (idempotency_key, report);
        Err(pdp_provider::PdpExternalHandoffError(
            "chain-authoritative repair transaction handoff is required".to_owned(),
        ))
    }
}

impl NodeHandle {
    /// Durably archive one terminal PDP outcome and enqueue its proof-ledger projection.
    pub fn archive_pdp_terminal_outcome(
        &self,
        idempotency_key: [u8; 32],
        archive: &PdpGovernanceArchiveV1,
    ) -> Result<[u8; 32], pdp_provider::PdpExternalHandoffError> {
        archive.validate().map_err(|error| {
            pdp_provider::PdpExternalHandoffError(format!(
                "validate PDP governance archive handoff: {error}"
            ))
        })?;
        let bytes = norito::to_bytes(archive).map_err(|error| {
            pdp_provider::PdpExternalHandoffError(format!(
                "encode PDP governance archive handoff: {error}"
            ))
        })?;
        self.proof_outcome_outbox
            .enqueue_pdp(archive)
            .map_err(|error| {
                pdp_provider::PdpExternalHandoffError(format!(
                    "persist PDP proof-outcome delivery: {error}"
                ))
            })?;
        self.enqueue_governance_outbox(GovernanceOutboxKindV1::PdpArchive, bytes.clone())
            .map_err(|error| pdp_provider::PdpExternalHandoffError(error.to_string()))?;
        self.flush_governance_outbox()
            .map_err(|error| pdp_provider::PdpExternalHandoffError(error.to_string()))?;
        Ok(pdp_handoff_receipt(
            b"archive",
            idempotency_key,
            *blake3::hash(&bytes).as_bytes(),
        ))
    }
}

impl potr::PotrLatencyRepairHandoff for NodeHandle {
    fn enqueue_proof_outcome(
        &self,
        source_identity: [u8; 32],
        receipt: &PotrReceiptV1,
        admission_envelope_digest: [u8; 32],
    ) -> Result<[u8; 32], potr::PotrRepairHandoffError> {
        self.enqueue_potr_proof_outcome(source_identity, receipt, admission_envelope_digest)
    }

    fn enqueue_latency_repair(
        &self,
        source_identity: [u8; 32],
        report: &RepairReportV1,
    ) -> Result<[u8; 32], potr::PotrRepairHandoffError> {
        let _ = (source_identity, report);
        Err(potr::PotrRepairHandoffError(
            "chain-authoritative repair transaction handoff is required".to_owned(),
        ))
    }
}

impl NodeHandle {
    /// Durably enqueue one final PoTR receipt for native proof-ledger submission.
    pub fn enqueue_potr_proof_outcome(
        &self,
        source_identity: [u8; 32],
        receipt: &PotrReceiptV1,
        admission_envelope_digest: [u8; 32],
    ) -> Result<[u8; 32], potr::PotrRepairHandoffError> {
        if receipt.signed_receipt_digest().ok() != Some(source_identity) {
            return Err(potr::PotrRepairHandoffError(
                "PoTR proof-outcome source identity does not match the signed receipt".to_owned(),
            ));
        }
        self.proof_outcome_outbox
            .enqueue_potr(receipt, admission_envelope_digest)
            .map(ProofOutcomeEnqueueResultV1::operation_id)
            .map_err(|error| potr::PotrRepairHandoffError(error.to_string()))
    }
}

fn pdp_handoff_receipt(
    kind: &[u8],
    idempotency_key: [u8; 32],
    payload_digest: [u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"sorafs.node.pdp.handoff-receipt.v1\0");
    hasher.update(&(kind.len() as u64).to_le_bytes());
    hasher.update(kind);
    hasher.update(&idempotency_key);
    hasher.update(&payload_digest);
    *hasher.finalize().as_bytes()
}

impl NodeHandle {
    /// Construct a new handle for the embedded storage worker.
    ///
    /// # Panics
    ///
    /// Panics when durable storage or checkpoint state cannot be trusted. Runtime
    /// integrations should use [`Self::try_new`] and surface the startup error.
    #[must_use]
    pub fn new(config: StorageConfig) -> Self {
        Self::try_new(config)
            .unwrap_or_else(|err| panic!("failed to initialise SoraFS node: {err}"))
    }

    /// Construct a new handle and report storage/checkpoint startup failures.
    ///
    /// # Errors
    ///
    /// Returns an error when configured durable state cannot be opened, decoded,
    /// validated, or when a configured governance publisher cannot start.
    pub fn try_new(config: StorageConfig) -> Result<Self, NodeInitError> {
        Self::try_new_with_policies(config, RepairConfig::default(), GcConfig::default())
    }

    /// Construct a new handle with a runtime-only PKCS#11/KMS quarantine-key wrapper.
    ///
    /// # Errors
    ///
    /// Returns an error when the wrapper exposes a non-canonical key handle or
    /// configured durable state cannot be opened, decoded, and authenticated.
    pub fn try_new_with_quarantine_key_wrapper(
        config: StorageConfig,
        key_wrapper: Arc<dyn ModerationQuarantineKeyWrapper>,
    ) -> Result<Self, NodeInitError> {
        Self::try_new_with_runtime_deps(
            config,
            NodeRuntimeDeps::default().with_moderation_quarantine_key_wrapper(key_wrapper),
        )
    }

    /// Construct a new handle with deployment-owned runtime dependencies.
    ///
    /// # Errors
    ///
    /// Returns an error when a configured service is missing its runtime
    /// dependency or durable state cannot be trusted.
    pub fn try_new_with_runtime_deps(
        config: StorageConfig,
        runtime_deps: NodeRuntimeDeps,
    ) -> Result<Self, NodeInitError> {
        Self::try_new_with_policies_and_runtime_deps(
            config,
            RepairConfig::default(),
            GcConfig::default(),
            runtime_deps,
        )
    }

    /// Construct a new handle with explicit repair/GC policies.
    ///
    /// # Panics
    ///
    /// Panics when durable storage or checkpoint state cannot be trusted. Runtime
    /// integrations should use [`Self::try_new_with_policies`].
    #[must_use]
    pub fn new_with_policies(
        config: StorageConfig,
        repair_config: RepairConfig,
        gc_config: GcConfig,
    ) -> Self {
        Self::try_new_with_policies(config, repair_config, gc_config)
            .unwrap_or_else(|err| panic!("failed to initialise SoraFS node: {err}"))
    }

    /// Construct a new handle with explicit policies and fallible durable startup.
    ///
    /// # Errors
    ///
    /// Returns an error when configured durable state cannot be opened, decoded,
    /// validated, or when a configured governance publisher cannot start.
    pub fn try_new_with_policies(
        config: StorageConfig,
        repair_config: RepairConfig,
        gc_config: GcConfig,
    ) -> Result<Self, NodeInitError> {
        Self::try_new_with_policies_and_runtime_deps(
            config,
            repair_config,
            gc_config,
            NodeRuntimeDeps::default(),
        )
    }

    /// Construct a new handle with explicit policies and a runtime-only
    /// PKCS#11/KMS quarantine-key wrapper.
    ///
    /// # Errors
    ///
    /// Returns an error when the wrapper exposes a non-canonical key handle or
    /// configured durable state cannot be opened, decoded, and authenticated.
    pub fn try_new_with_policies_and_quarantine_key_wrapper(
        config: StorageConfig,
        repair_config: RepairConfig,
        gc_config: GcConfig,
        key_wrapper: Arc<dyn ModerationQuarantineKeyWrapper>,
    ) -> Result<Self, NodeInitError> {
        Self::try_new_with_policies_and_runtime_deps(
            config,
            repair_config,
            gc_config,
            NodeRuntimeDeps::default().with_moderation_quarantine_key_wrapper(key_wrapper),
        )
    }

    /// Construct a new handle with explicit policies and deployment-owned
    /// runtime dependencies.
    ///
    /// # Errors
    ///
    /// Returns an error when a configured service is missing its runtime
    /// dependency, a dependency exposes an invalid public handle, or durable
    /// state cannot be trusted.
    pub fn try_new_with_policies_and_runtime_deps(
        config: StorageConfig,
        repair_config: RepairConfig,
        gc_config: GcConfig,
        runtime_deps: NodeRuntimeDeps,
    ) -> Result<Self, NodeInitError> {
        validate_native_repair_config(&repair_config)?;
        let NodeRuntimeDeps {
            moderation_quarantine_key_wrapper,
            privacy_cycle_prf_provider,
            privacy_release_anchor,
        } = runtime_deps;
        let moderation_screening_authority = if config.moderation_screening_enabled() {
            if !config.enabled() {
                return Err(NodeInitError::ModerationScreeningAuthorityBundle {
                    path: PathBuf::from("<iroha_config>"),
                    message:
                        "moderation screening requires the durable SoraFS storage worker to be enabled"
                            .to_owned(),
                });
            }
            let path = config
                .moderation_screening_authority_bundle_path()
                .ok_or_else(|| NodeInitError::ModerationScreeningAuthorityBundle {
                    path: PathBuf::from("<iroha_config>"),
                    message: "enabled moderation screening requires an authority bundle path"
                        .to_owned(),
                })?;
            let expected_digest = config
                .moderation_screening_authority_bundle_digest()
                .ok_or_else(|| NodeInitError::ModerationScreeningAuthorityBundle {
                    path: path.clone(),
                    message:
                        "enabled moderation screening requires a non-zero configured bundle digest"
                            .to_owned(),
                })?;
            Some(load_moderation_screening_authority_bundle(
                path,
                expected_digest,
                unix_now_secs(),
            )?)
        } else {
            None
        };
        if config.moderation_screening_enabled() && moderation_quarantine_key_wrapper.is_none() {
            return Err(NodeInitError::ModerationQuarantineKeyWrapperUnavailable);
        }
        if let Some(key_wrapper) = moderation_quarantine_key_wrapper.as_deref() {
            validate_moderation_quarantine_key_wrapper(key_wrapper).map_err(|error| {
                NodeInitError::ModerationQuarantineKeyWrapperInvalid {
                    message: error.to_string(),
                }
            })?;
        }
        let privacy_cycle_prf_required = config.privacy_aggregate_schedule().is_some()
            && config
                .privacy_aggregate_policy()
                .is_some_and(config::PrivacyAggregatePolicyConfig::requires_cycle_prf);
        if privacy_cycle_prf_required && privacy_cycle_prf_provider.is_none() {
            return Err(NodeInitError::PrivacyCyclePrfProviderUnavailable);
        }
        let privacy_release_anchor_required = config.privacy_aggregate_schedule().is_some()
            && config.privacy_aggregate_policy().is_some();
        if privacy_release_anchor_required && privacy_release_anchor.is_none() {
            return Err(NodeInitError::PrivacyReleaseAnchorUnavailable);
        }
        let reputation_trust_policy = config
            .reputation_trust_policy_path()
            .map(|path| load_reputation_trust_policy(path))
            .transpose()?
            .map(Arc::new);
        let pricing_trust_policy = config
            .pricing_trust_policy_path()
            .map(|path| load_pricing_trust_policy(path))
            .transpose()?
            .map(Arc::new);
        let governed_pricing = match pricing_trust_policy.as_deref() {
            Some(policy) => Some(GovernedPricingSeriesV1::new(policy).map_err(|error| {
                NodeInitError::PricingTrustPolicy {
                    path: config
                        .pricing_trust_policy_path()
                        .cloned()
                        .unwrap_or_else(|| PathBuf::from("<configured-pricing-policy>")),
                    message: error.to_string(),
                }
            })?),
            None => None,
        };
        let hedging_feed_trust_policy = config
            .hedging_feed_trust_policy_path()
            .map(|path| load_hedging_feed_trust_policy(path))
            .transpose()?
            .map(Arc::new);
        let signed_hedging_feeds = match hedging_feed_trust_policy.as_deref() {
            Some(policy) => Some(SignedHedgingFeedLedgerV1::new(policy).map_err(|error| {
                NodeInitError::HedgingFeedTrustPolicy {
                    path: config
                        .hedging_feed_trust_policy_path()
                        .cloned()
                        .unwrap_or_else(|| PathBuf::from("<configured-hedging-policy>")),
                    message: error.to_string(),
                }
            })?),
            None => None,
        };
        let gc_config = gc_config.with_default_state_dir(config.data_dir());
        let scheduler_config = StorageSchedulerConfig::from_storage_config(&config);
        let schedulers = StorageSchedulersRuntime::new(scheduler_config);
        let capacity_limit = config.max_capacity_bytes().0;

        let storage = if config.enabled() {
            match StorageBackend::new(config.clone()) {
                Ok(backend) => {
                    let backend = Arc::new(backend);
                    schedulers.update_storage_bytes(backend.total_bytes(), capacity_limit);
                    Some(backend)
                }
                Err(err) => return Err(NodeInitError::Storage(err)),
            }
        } else {
            schedulers.update_storage_bytes(0, capacity_limit);
            None
        };
        let pdp_provider_state_dir = config.data_dir().join("pdp-provider");
        let pdp_provider = storage
            .as_ref()
            .map(|_| {
                PdpProviderProtocol::open(config.pdp_provider_policy(), &pdp_provider_state_dir)
                    .map_err(|error| NodeInitError::PdpProvider {
                        path: pdp_provider_state_dir.clone(),
                        message: error.to_string(),
                    })
            })
            .transpose()?;

        let smoothing = config.smoothing_config();
        let event_history_limit = config.runtime_retention().event_history_limit();
        let state_entry_limit = config.runtime_retention().state_entry_limit();
        let potr_state_dir = config.data_dir().join("potr-receipts");
        let potr = storage
            .as_ref()
            .map(|_| {
                PotrTracker::open(
                    &potr_state_dir,
                    state_entry_limit,
                    config.runtime_retention().checkpoint_max_bytes(),
                )
                .map_err(|error| NodeInitError::Potr {
                    path: potr_state_dir.clone(),
                    message: error.to_string(),
                })
            })
            .transpose()?
            .unwrap_or_default();
        let proof_outcome_outbox_state_dir = config.data_dir().join("proof-outcome-forwarder");
        let proof_outcome_outbox_policy = ProofOutcomeOutboxPolicyV1 {
            max_pending: state_entry_limit,
            max_completed: state_entry_limit,
            max_dead_letters: state_entry_limit,
            max_attempts: config.runtime_retention().proof_outcome_max_attempts(),
            checkpoint_max_bytes: config.runtime_retention().checkpoint_max_bytes(),
        };
        let proof_outcome_outbox = if storage.is_some() {
            ProofOutcomeOutbox::open(&proof_outcome_outbox_state_dir, proof_outcome_outbox_policy)
                .map_err(|error| NodeInitError::ProofOutcomeOutbox {
                path: proof_outcome_outbox_state_dir.clone(),
                message: error.to_string(),
            })?
        } else {
            ProofOutcomeOutbox::in_memory(proof_outcome_outbox_policy).map_err(|error| {
                NodeInitError::ProofOutcomeOutbox {
                    path: proof_outcome_outbox_state_dir.clone(),
                    message: error.to_string(),
                }
            })?
        };
        let repair_transaction_state_dir = config.data_dir().join("repair-transaction-forwarder");
        let repair_transaction_policy = RepairTransactionForwarderPolicyV1 {
            max_pending: state_entry_limit,
            max_completed: state_entry_limit,
            max_dead_letters: state_entry_limit,
            max_attempts: repair_config.max_attempts(),
            max_transaction_bytes: REPAIR_TRANSACTION_MAX_CANONICAL_BYTES_V1,
            checkpoint_max_bytes: config.runtime_retention().checkpoint_max_bytes(),
        };
        let repair_transaction_forwarder = if storage.is_some() {
            RepairTransactionForwarder::open(
                &repair_transaction_state_dir,
                repair_transaction_policy,
            )
            .map_err(|error| NodeInitError::RepairTransactionForwarder {
                path: repair_transaction_state_dir.clone(),
                message: error.to_string(),
            })?
        } else {
            RepairTransactionForwarder::in_memory(repair_transaction_policy).map_err(|error| {
                NodeInitError::RepairTransactionForwarder {
                    path: repair_transaction_state_dir.clone(),
                    message: error.to_string(),
                }
            })?
        };
        let orderbook_worker_policy = config.orderbook_worker_policy();
        let orderbook_transaction_state_dir =
            config.data_dir().join("orderbook-transaction-forwarder");
        let orderbook_transaction_policy = OrderbookTransactionForwarderPolicyV1 {
            max_pending: orderbook_worker_policy.max_pending(),
            max_completed: orderbook_worker_policy.max_completed(),
            max_dead_letters: orderbook_worker_policy.max_dead_letters(),
            max_attempts: orderbook_worker_policy.max_attempts(),
            max_transaction_bytes: ORDERBOOK_TRANSACTION_MAX_CANONICAL_BYTES_V1,
            checkpoint_max_bytes: orderbook_worker_policy.checkpoint_max_bytes(),
        };
        // This durability boundary is independent of both provider storage and
        // supervised scanning. `enabled` gates only the worker loop; any
        // operation admitted through this handle is always persisted.
        let orderbook_transaction_forwarder = OrderbookTransactionForwarder::open(
            &orderbook_transaction_state_dir,
            orderbook_transaction_policy,
        )
        .map_err(|error| NodeInitError::OrderbookTransactionForwarder {
            path: orderbook_transaction_state_dir.clone(),
            message: error.to_string(),
        })?;
        let reserve_worker_policy = config.reserve_worker_policy();
        reserve_worker_policy
            .validate()
            .map_err(|message| NodeInitError::ReserveWorkerConfig { message })?;
        let reserve_transaction_state_dir = config.data_dir().join("reserve-transaction-forwarder");
        let reserve_transaction_policy = ReserveTransactionForwarderPolicyV1 {
            max_pending: reserve_worker_policy.max_pending(),
            max_completed: reserve_worker_policy.max_completed(),
            max_dead_letters: reserve_worker_policy.max_dead_letters(),
            max_attempts: reserve_worker_policy.max_attempts(),
            max_transaction_bytes: RESERVE_TRANSACTION_MAX_CANONICAL_BYTES_V1,
            checkpoint_max_bytes: reserve_worker_policy.checkpoint_max_bytes(),
        };
        // This durability boundary is independent of both provider storage and
        // supervised scanning. `enabled` gates only the worker loop; any
        // operation admitted through this handle is always persisted.
        let reserve_transaction_forwarder = ReserveTransactionForwarder::open(
            &reserve_transaction_state_dir,
            reserve_transaction_policy,
        )
        .map_err(|error| NodeInitError::ReserveTransactionForwarder {
            path: reserve_transaction_state_dir.clone(),
            message: error.to_string(),
        })?;
        let runtime_checkpoint_initialization = if storage.is_some() {
            Some(inspect_runtime_checkpoint_initialization(
                config.data_dir(),
                config.runtime_retention().checkpoint_max_bytes(),
            )?)
        } else {
            None
        };
        let deal_engine = DealEngine::with_entry_limit(state_entry_limit);
        let governance_dir = config.governance_dir().cloned();
        let governance_dag_publisher_peer_id = config.governance_dag_publisher_peer_id().cloned();
        let governance_dag_signing_key_path = config.governance_dag_signing_key_path().cloned();
        let pricing_checkpoint_path = storage
            .as_ref()
            .map(|_| pricing_runtime_snapshot_path(config.data_dir()));
        let hedging_checkpoint_path = storage
            .as_ref()
            .map(|_| hedging_runtime_snapshot_path(config.data_dir()));
        let moderation_model_registry_checkpoint_path = storage
            .as_ref()
            .map(|_| moderation_model_registry_checkpoint_path(config.data_dir()));
        let moderation_screening_checkpoint_path = storage
            .as_ref()
            .map(|_| moderation_screening_checkpoint_path(config.data_dir()));
        let moderation_quarantine_object_root = storage
            .as_ref()
            .map(|_| moderation_quarantine_object_store_root(config.data_dir()));
        let moderation_quarantine_object_index_path = storage
            .as_ref()
            .map(|_| moderation_quarantine_object_index_path(config.data_dir()));
        let moderation_evidence_viewer_checkpoint_path = storage
            .as_ref()
            .map(|_| moderation_evidence_viewer_checkpoint_path(config.data_dir()));
        let auxiliary_runtime_checkpoint_path = storage
            .as_ref()
            .map(|_| auxiliary_runtime_checkpoint_path(config.data_dir()));
        let (reputation_event_sender, _) = broadcast::channel(REPUTATION_EVENT_CHANNEL_CAPACITY);
        let native_repair_singleflight =
            native_repair_singleflight::NativeRepairSingleflightV1::new(
                repair_config.worker_concurrency(),
            );
        let node = Self {
            config,
            repair_config,
            gc_config,
            capacity: Arc::new(CapacityManager::with_entry_limit(state_entry_limit)),
            meter: CapacityMeter::with_smoothing(smoothing),
            telemetry: Arc::new(RwLock::new(None)),
            schedulers,
            por: PorTracker::with_entry_limit(state_entry_limit),
            potr,
            proof_outcome_outbox,
            repair_transaction_forwarder,
            native_repair_singleflight,
            orderbook_transaction_forwarder,
            reserve_transaction_forwarder,
            por_history: Arc::new(RwLock::new(HashMap::new())),
            storage,
            pdp_provider,
            deal_engine,
            gc_mutation_lock: Arc::new(Mutex::new(())),
            gc_eviction_intents: Arc::new(RwLock::new(GcEvictionIntentRuntime::default())),
            gc_eviction_audit_links: Arc::new(RwLock::new(BTreeMap::new())),
            repair_orchestrator: Arc::new(RwLock::new(None)),
            governance_publisher: Arc::new(RwLock::new(None)),
            governance_outbox: Arc::new(RwLock::new(GovernanceOutboxRuntime::default())),
            governance_outbox_drain_lock: Arc::new(Mutex::new(())),
            runtime_mutation_lock: Arc::new(Mutex::new(())),
            auxiliary_checkpoint_lock: Arc::new(Mutex::new(())),
            durability_failure: Arc::new(Mutex::new(None)),
            auxiliary_runtime_checkpoint_path,
            reputation_trust_policy,
            latest_reputation_snapshot: Arc::new(RwLock::new(None)),
            reputation_snapshots: Arc::new(RwLock::new(BTreeMap::new())),
            reputation_events: Arc::new(RwLock::new(BoundedEventHistory::new(event_history_limit))),
            reputation_event_sender,
            pricing_trust_policy,
            governed_pricing: Arc::new(RwLock::new(governed_pricing)),
            pricing_checkpoint_path,
            hedging_feed_trust_policy,
            signed_hedging_feeds: Arc::new(RwLock::new(signed_hedging_feeds)),
            hedging_checkpoint_path,
            moderation_model_registry_checkpoint_path,
            moderation_model_registry: Arc::new(RwLock::new(
                ModerationModelRegistry::with_entry_limit(state_entry_limit),
            )),
            moderation_screening_checkpoint_path,
            moderation_screening: Arc::new(RwLock::new(
                ModerationScreeningRuntime::with_entry_limit(state_entry_limit),
            )),
            moderation_screening_authority: Arc::new(RwLock::new(moderation_screening_authority)),
            moderation_quarantine_object_root,
            moderation_quarantine_object_index_path,
            moderation_quarantine_key_wrapper: moderation_quarantine_key_wrapper
                .map(OpaqueModerationQuarantineKeyWrapper),
            moderation_quarantine_objects: Arc::new(RwLock::new(
                ModerationQuarantineObjectRuntime::with_entry_limit(state_entry_limit),
            )),
            moderation_evidence_viewer_checkpoint_path,
            moderation_evidence_viewer: Arc::new(RwLock::new(
                ModerationEvidenceViewerRuntime::with_entry_limit(state_entry_limit),
            )),
            transparency_ledger_source_entries: Arc::new(RwLock::new(BTreeMap::new())),
            privacy_aggregate_source_events: Arc::new(RwLock::new(BTreeMap::new())),
            privacy_source_event_receipts: Arc::new(RwLock::new(BTreeMap::new())),
            privacy_publish_request_receipts: Arc::new(RwLock::new(BTreeMap::new())),
            published_privacy_aggregate_cycles: Arc::new(RwLock::new(BTreeSet::new())),
            privacy_composition_budget: Arc::new(RwLock::new(
                PrivacyCompositionBudgetLedgerV1::default(),
            )),
            privacy_release_ledger: Arc::new(RwLock::new(
                transparency::PrivacyReleaseLedgerV1::default(),
            )),
            privacy_cycle_prf_provider: privacy_cycle_prf_provider
                .map(OpaquePrivacyCyclePrfProvider),
            privacy_release_anchor: privacy_release_anchor.map(OpaquePrivacyReleaseAnchor),
            published_evidence_viewer_audit_cycles: Arc::new(RwLock::new(BTreeSet::new())),
        };

        if node.storage.is_some() {
            match runtime_checkpoint_initialization {
                Some(RuntimeCheckpointInitialization::Fresh) => {
                    node.initialize_runtime_checkpoints()?;
                }
                Some(RuntimeCheckpointInitialization::Initialized) => {
                    node.load_pricing_checkpoint()?;
                    node.load_hedging_checkpoint()?;
                    node.load_moderation_model_registry_checkpoint()?;
                    node.load_moderation_screening_checkpoint()?;
                    node.load_moderation_quarantine_object_index_checkpoint()?;
                    node.audit_moderation_quarantine_object_store()?;
                    node.load_moderation_evidence_viewer_checkpoint()?;
                    node.load_auxiliary_runtime_checkpoint()?;
                }
                None => unreachable!("storage-backed node must inspect runtime checkpoints"),
            }
            node.reconcile_gc_eviction_intents_on_startup()
                .map_err(|err| {
                    NodeInitError::checkpoint(
                        "auxiliary runtime",
                        node.auxiliary_runtime_checkpoint_path
                            .as_ref()
                            .expect("storage-backed node has an auxiliary checkpoint path"),
                        err,
                    )
                })?;
            // PDP and PoTR handoffs intentionally remain durable here. Repair-required
            // records need Torii's chain-authoritative transaction adapter; NodeHandle
            // must not fabricate a local receipt or make restart depend on that adapter.
            // The supervised Torii repair forwarder resumes each protocol on its first
            // immediate scan and once per subsequent scan.
            if let Some(dir) = governance_dir.clone() {
                let publisher = FilesystemGovernancePublisher::try_new(dir.clone())
                    .map_err(|err| NodeInitError::GovernancePublisher(err.to_string()))?;
                let publisher = match (
                    governance_dag_publisher_peer_id.clone(),
                    governance_dag_signing_key_path.clone(),
                ) {
                    (Some(peer_id), Some(signing_key_path)) => publisher
                        .with_runtime_dag_signer(peer_id.into_bytes(), signing_key_path)
                        .map_err(|err| NodeInitError::GovernancePublisher(err.to_string()))?,
                    (Some(_), None) | (None, Some(_)) => {
                        return Err(NodeInitError::GovernancePublisher(
                            "runtime DAG signing requires both publisher peer id and signing key path"
                                .to_owned(),
                        ));
                    }
                    (None, None) => publisher,
                };
                iroha_logger::info!(
                    path = ?dir,
                    signed_runtime_dag = governance_dag_publisher_peer_id.is_some()
                        && governance_dag_signing_key_path.is_some(),
                    "SoraFS governance publisher initialised"
                );
                node.try_set_governance_publisher(Arc::new(publisher))
                    .map_err(|err| NodeInitError::GovernancePublisher(err.to_string()))?;
            }
        } else if governance_dir.is_some() {
            iroha_logger::warn!(
                "skipping governance publisher initialisation: storage backend disabled"
            );
        }

        node.reconcile_privacy_release_anchor()
            .map_err(|error| NodeInitError::Checkpoint {
                component: "privacy release anchor",
                path: crate::auxiliary_runtime_checkpoint_path(node.config.data_dir()),
                message: error.to_string(),
            })?;
        Ok(node)
    }

    /// Returns a reference to the storage configuration.
    #[must_use]
    pub fn config(&self) -> &StorageConfig {
        &self.config
    }

    /// Return the durable PDP provider protocol when storage is enabled.
    #[must_use]
    pub fn pdp_provider_protocol(&self) -> Option<&PdpProviderProtocol> {
        self.pdp_provider.as_ref()
    }

    /// Resume durable PDP terminal handoffs through an explicit production adapter.
    pub fn resume_pdp_terminal_handoffs(
        &self,
        handoff: &dyn PdpTerminalHandoff,
        limit: usize,
    ) -> Result<usize, PdpProviderProtocolError> {
        let Some(pdp_provider) = self.pdp_provider.as_ref() else {
            return Ok(0);
        };
        pdp_provider
            .resume_handoffs(handoff, limit)
            .map(|outcomes| outcomes.len())
    }

    /// Resume durable PoTR proof-ledger and repair handoffs through an explicit adapter.
    pub fn resume_potr_terminal_handoffs(
        &self,
        handoff: &dyn potr::PotrLatencyRepairHandoff,
    ) -> Result<usize, PotrTrackerError> {
        self.potr.resume_terminal_handoffs(handoff)
    }

    /// Return pending proof-outcome deliveries in stable sequence order.
    pub fn pending_proof_outcome_deliveries(
        &self,
        limit: usize,
    ) -> Result<Vec<ProofOutcomePendingDeliveryV1>, ProofOutcomeOutboxError> {
        self.proof_outcome_outbox.pending(limit)
    }

    /// Return a circular page of proof-outcome deliveries after a sequence cursor.
    pub fn pending_proof_outcome_deliveries_after(
        &self,
        after_sequence: Option<u64>,
        limit: usize,
    ) -> Result<Vec<ProofOutcomePendingDeliveryV1>, ProofOutcomeOutboxError> {
        self.proof_outcome_outbox
            .pending_after(after_sequence, limit)
    }

    /// Return payload-free terminal proof-outcome deliveries for operator reconciliation.
    pub fn proof_outcome_dead_letters(
        &self,
        limit: usize,
    ) -> Result<Vec<ProofOutcomeDeadLetterV1>, ProofOutcomeOutboxError> {
        self.proof_outcome_outbox.dead_letters(limit)
    }

    /// Restore one explicitly selected proof-outcome dead letter for governed replay.
    pub fn retry_proof_outcome_dead_letter(
        &self,
        operation_id: [u8; 32],
        expected_outcome_digest: [u8; 32],
    ) -> Result<(), ProofOutcomeOutboxError> {
        self.proof_outcome_outbox
            .retry_dead_letter(operation_id, expected_outcome_digest)
    }

    /// Durably claim one proof outcome for isolated runtime signing.
    pub fn claim_proof_outcome_for_signing(
        &self,
        operation_id: [u8; 32],
        baseline_finalized_cursor: iroha_data_model::sorafs::proof_ledger::ProofOutcomeFinalizedCursorV1,
    ) -> Result<ProofOutcomePendingDeliveryV1, ProofOutcomeOutboxError> {
        self.proof_outcome_outbox
            .claim_for_signing(operation_id, baseline_finalized_cursor)
    }

    /// Persist an exact signed proof-outcome transaction before queue exposure.
    pub fn store_signed_proof_outcome_transaction(
        &self,
        operation_id: [u8; 32],
        transaction: iroha_data_model::transaction::SignedTransaction,
    ) -> Result<[u8; 32], ProofOutcomeOutboxError> {
        self.proof_outcome_outbox
            .store_signed_transaction(operation_id, transaction)
    }

    /// Release an isolated proof-outcome signing claim after signer failure.
    pub fn release_proof_outcome_signing_claim(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), ProofOutcomeOutboxError> {
        self.proof_outcome_outbox
            .release_signing_claim(operation_id)
    }

    /// Mark a durable signed proof-outcome transaction ambiguous before submission.
    pub fn begin_proof_outcome_submission(
        &self,
        operation_id: [u8; 32],
    ) -> Result<iroha_data_model::transaction::SignedTransaction, ProofOutcomeOutboxError> {
        self.proof_outcome_outbox.begin_submission(operation_id)
    }

    /// Record that an exact proof-outcome transaction is pending or applied.
    pub fn mark_proof_outcome_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), ProofOutcomeOutboxError> {
        self.proof_outcome_outbox.mark_submitted(operation_id)
    }

    /// Record a proof-outcome queue failure known to precede submission.
    pub fn mark_proof_outcome_not_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), ProofOutcomeOutboxError> {
        self.proof_outcome_outbox.mark_not_submitted(operation_id)
    }

    /// Permit retry after finalized absence of the exact proof outcome and transaction.
    pub fn mark_proof_outcome_finalized_absent(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: iroha_data_model::sorafs::proof_ledger::ProofOutcomeFinalizedCursorV1,
    ) -> Result<(), ProofOutcomeOutboxError> {
        self.proof_outcome_outbox
            .mark_finalized_absent(operation_id, observed_finalized_cursor)
    }

    /// Reconcile one exact finalized proof outcome.
    pub fn mark_proof_outcome_finalized(
        &self,
        operation_id: [u8; 32],
        finalized: &iroha_data_model::sorafs::proof_ledger::ProofOutcomeFinalizedRecordV1,
    ) -> Result<(), ProofOutcomeOutboxError> {
        self.proof_outcome_outbox
            .mark_finalized(operation_id, finalized)
    }

    /// Dead-letter one exact rejected or expired proof-outcome transaction.
    pub fn mark_proof_outcome_transaction_rejected(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: iroha_data_model::sorafs::proof_ledger::ProofOutcomeFinalizedCursorV1,
    ) -> Result<(), ProofOutcomeOutboxError> {
        self.proof_outcome_outbox
            .mark_transaction_rejected(operation_id, observed_finalized_cursor)
    }

    /// Durably enqueue one authority-bound native repair operation.
    pub fn enqueue_repair_transaction(
        &self,
        authority: AccountId,
        operation: RepairOperationV1,
        context: &RepairTransactionContextV1,
    ) -> Result<RepairTransactionEnqueueResultV1, RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .enqueue_unsigned_operation(authority, operation, context)
    }

    /// Return a fair circular page of pending native repair transactions.
    pub fn pending_repair_transactions_after(
        &self,
        after_sequence: Option<u64>,
        limit: usize,
    ) -> Result<Vec<RepairTransactionPendingV1>, RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .pending_after(after_sequence, limit)
    }

    /// Return payload-free terminal repair-transaction dead letters.
    pub fn repair_transaction_dead_letters(
        &self,
        limit: usize,
    ) -> Result<Vec<RepairTransactionDeadLetterV1>, RepairTransactionForwarderError> {
        self.repair_transaction_forwarder.dead_letters(limit)
    }

    /// Claim one native repair operation for isolated runtime signing.
    pub fn claim_repair_transaction_for_signing(
        &self,
        operation_id: [u8; 32],
    ) -> Result<RepairTransactionSigningRequestV1, RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .claim_for_signing(operation_id)
    }

    /// Read exact repair semantics for finalized reconciliation without claiming a signer attempt.
    pub fn repair_transaction_operation_for_reconciliation(
        &self,
        operation_id: [u8; 32],
    ) -> Result<RepairTransactionSigningRequestV1, RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .operation_for_reconciliation(operation_id)
    }

    /// Persist exact signed native repair-transaction bytes before queue exposure.
    pub fn store_signed_repair_transaction(
        &self,
        operation_id: [u8; 32],
        signed_transaction_bytes: &[u8],
    ) -> Result<[u8; 32], RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .store_signed_transaction(operation_id, signed_transaction_bytes)
    }

    /// Release an interrupted repair-transaction signing claim.
    pub fn release_repair_transaction_signing_claim(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .release_signing_claim(operation_id)
    }

    /// Mark exact signed repair bytes ambiguous before transaction ingress.
    pub fn begin_repair_transaction_submission(
        &self,
        operation_id: [u8; 32],
    ) -> Result<Vec<u8>, RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .begin_submission(operation_id)
    }

    /// Record that an exact repair transaction is pending or applied.
    pub fn mark_repair_transaction_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .mark_submitted(operation_id)
    }

    /// Reconcile exact finalized repair-transaction bytes.
    pub fn mark_repair_transaction_finalized(
        &self,
        operation_id: [u8; 32],
        expected_transaction_digest: [u8; 32],
        finalized_cursor: RepairFinalizedCursorV1,
    ) -> Result<(), RepairTransactionForwarderError> {
        self.repair_transaction_forwarder.mark_finalized(
            operation_id,
            expected_transaction_digest,
            finalized_cursor,
        )
    }

    /// Record a repair queue failure proven to precede submission.
    pub fn mark_repair_transaction_not_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .mark_not_submitted(operation_id)
    }

    /// Permit retry after finalized absence of the exact repair operation.
    pub fn mark_repair_transaction_finalized_absent(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: RepairFinalizedCursorV1,
    ) -> Result<(), RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .mark_finalized_absent(operation_id, observed_finalized_cursor)
    }

    /// Reconcile an exact semantic repair operation committed by any ingress.
    pub fn mark_repair_transaction_semantic_finalized(
        &self,
        operation_id: [u8; 32],
        finalized_cursor: RepairFinalizedCursorV1,
    ) -> Result<(), RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .mark_semantic_finalized(operation_id, finalized_cursor)
    }

    /// Dead-letter a repair operation that conflicts with finalized state.
    pub fn mark_repair_transaction_finalized_conflict(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: RepairFinalizedCursorV1,
    ) -> Result<(), RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .mark_finalized_conflict(operation_id, observed_finalized_cursor)
    }

    /// Clear a rejected repair envelope for bounded replacement signing.
    pub fn mark_repair_transaction_rejected(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: RepairFinalizedCursorV1,
    ) -> Result<(), RepairTransactionForwarderError> {
        self.repair_transaction_forwarder
            .mark_transaction_rejected(operation_id, observed_finalized_cursor)
    }

    /// Durably enqueue one native orderbook operation bound to finalized governance state.
    pub fn enqueue_orderbook_transaction(
        &self,
        operation: OrderbookOperationV1,
        context: &OrderbookTransactionContextV1,
    ) -> Result<OrderbookTransactionEnqueueResultV1, OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .enqueue_unsigned_operation(operation, context)
    }

    /// Durably enqueue one canonical signed native orderbook transaction.
    ///
    /// The forwarder binds both the exact governed authority and active chain
    /// identity from the finalized context before persisting any bytes.
    pub fn enqueue_signed_orderbook_transaction(
        &self,
        signed_transaction_bytes: &[u8],
        context: &OrderbookTransactionContextV1,
    ) -> Result<OrderbookTransactionEnqueueResultV1, OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .enqueue_signed_transaction(signed_transaction_bytes, context)
    }

    /// Return the oldest bounded page of pending native orderbook transactions.
    pub fn pending_orderbook_transactions(
        &self,
        limit: usize,
    ) -> Result<Vec<OrderbookTransactionPendingV1>, OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder.pending(limit)
    }

    /// Return a fair circular page of pending native orderbook transactions.
    pub fn pending_orderbook_transactions_after(
        &self,
        after_sequence: Option<u64>,
        limit: usize,
    ) -> Result<Vec<OrderbookTransactionPendingV1>, OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .pending_after(after_sequence, limit)
    }

    /// Read exact orderbook semantics for finalized reconciliation without claiming an attempt.
    pub fn orderbook_transaction_operation_for_reconciliation(
        &self,
        operation_id: [u8; 32],
    ) -> Result<OrderbookTransactionSigningRequestV1, OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .operation_for_reconciliation(operation_id)
    }

    /// Return payload-free terminal orderbook-transaction dead letters.
    pub fn orderbook_transaction_dead_letters(
        &self,
        limit: usize,
    ) -> Result<Vec<OrderbookTransactionDeadLetterV1>, OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder.dead_letters(limit)
    }

    /// Claim one native orderbook operation for isolated runtime signing.
    pub fn claim_orderbook_transaction_for_signing(
        &self,
        operation_id: [u8; 32],
    ) -> Result<OrderbookTransactionSigningRequestV1, OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .claim_for_signing(operation_id)
    }

    /// Persist exact signed orderbook transaction bytes before submitter exposure.
    pub fn store_signed_orderbook_transaction(
        &self,
        operation_id: [u8; 32],
        signed_transaction_bytes: &[u8],
    ) -> Result<[u8; 32], OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .store_signed_transaction(operation_id, signed_transaction_bytes)
    }

    /// Release an interrupted orderbook-transaction signing claim.
    pub fn release_orderbook_transaction_signing_claim(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .release_signing_claim(operation_id)
    }

    /// Mark exact signed orderbook bytes ambiguous before transaction ingress.
    pub fn begin_orderbook_transaction_submission(
        &self,
        operation_id: [u8; 32],
    ) -> Result<Vec<u8>, OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .begin_submission(operation_id)
    }

    /// Record that an exact orderbook transaction is pending or applied.
    pub fn mark_orderbook_transaction_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .mark_submitted(operation_id)
    }

    /// Record an orderbook queue failure proven to precede submission.
    pub fn mark_orderbook_transaction_not_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .mark_not_submitted(operation_id)
    }

    /// Permit exact-byte retry after finalized absence of an orderbook transaction.
    pub fn mark_orderbook_transaction_finalized_absent(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .mark_finalized_absent(operation_id, observed_finalized_cursor)
    }

    /// Reconcile exact finalized orderbook success by retained transaction digest.
    pub fn mark_orderbook_transaction_finalized(
        &self,
        operation_id: [u8; 32],
        expected_transaction_digest: [u8; 32],
        finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder.mark_finalized(
            operation_id,
            expected_transaction_digest,
            finalized_cursor,
        )
    }

    /// Reconcile exact semantic orderbook success committed through another ingress.
    pub fn mark_orderbook_transaction_semantic_finalized(
        &self,
        operation_id: [u8; 32],
        finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .mark_semantic_finalized(operation_id, finalized_cursor)
    }

    /// Dead-letter an orderbook operation that conflicts with finalized state.
    pub fn mark_orderbook_transaction_finalized_conflict(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .mark_finalized_conflict(operation_id, observed_finalized_cursor)
    }

    /// Clear a rejected orderbook envelope for bounded replacement signing.
    pub fn mark_orderbook_transaction_rejected(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: OrderbookFinalizedCursorV1,
    ) -> Result<(), OrderbookTransactionForwarderError> {
        self.orderbook_transaction_forwarder
            .mark_transaction_rejected(operation_id, observed_finalized_cursor)
    }

    /// Durably enqueue one native reserve/rent operation bound to finalized governance state.
    pub fn enqueue_reserve_transaction(
        &self,
        operation: ReserveOperationV1,
        context: &ReserveTransactionContextV1,
    ) -> Result<ReserveTransactionEnqueueResultV1, ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .enqueue_unsigned_operation(operation, context)
    }

    /// Durably enqueue one canonical signed native reserve/rent transaction.
    ///
    /// The forwarder binds both the exact governed authority and active chain
    /// identity from the finalized context before persisting any bytes.
    pub fn enqueue_signed_reserve_transaction(
        &self,
        signed_transaction_bytes: &[u8],
        context: &ReserveTransactionContextV1,
    ) -> Result<ReserveTransactionEnqueueResultV1, ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .enqueue_signed_transaction(signed_transaction_bytes, context)
    }

    /// Return the oldest bounded page of pending native reserve/rent transactions.
    pub fn pending_reserve_transactions(
        &self,
        limit: usize,
    ) -> Result<Vec<ReserveTransactionPendingV1>, ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder.pending(limit)
    }

    /// Return a fair circular page of pending native reserve/rent transactions.
    pub fn pending_reserve_transactions_after(
        &self,
        after_sequence: Option<u64>,
        limit: usize,
    ) -> Result<Vec<ReserveTransactionPendingV1>, ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .pending_after(after_sequence, limit)
    }

    /// Read exact reserve/rent semantics for finalized reconciliation without claiming an attempt.
    pub fn reserve_transaction_operation_for_reconciliation(
        &self,
        operation_id: [u8; 32],
    ) -> Result<ReserveTransactionReconciliationV1, ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .operation_for_reconciliation(operation_id)
    }

    /// Return payload-free terminal reserve-transaction dead letters.
    pub fn reserve_transaction_dead_letters(
        &self,
        limit: usize,
    ) -> Result<Vec<ReserveTransactionDeadLetterV1>, ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder.dead_letters(limit)
    }

    /// Claim one native reserve/rent operation for isolated runtime signing.
    pub fn claim_reserve_transaction_for_signing(
        &self,
        operation_id: [u8; 32],
    ) -> Result<ReserveTransactionSigningRequestV1, ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .claim_for_signing(operation_id)
    }

    /// Persist exact signed reserve/rent transaction bytes before submitter exposure.
    pub fn store_signed_reserve_transaction(
        &self,
        operation_id: [u8; 32],
        signed_transaction_bytes: &[u8],
    ) -> Result<[u8; 32], ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .store_signed_transaction(operation_id, signed_transaction_bytes)
    }

    /// Release an interrupted reserve-transaction signing claim.
    pub fn release_reserve_transaction_signing_claim(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .release_signing_claim(operation_id)
    }

    /// Mark exact signed reserve/rent bytes ambiguous before transaction ingress.
    pub fn begin_reserve_transaction_submission(
        &self,
        operation_id: [u8; 32],
    ) -> Result<Vec<u8>, ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .begin_submission(operation_id)
    }

    /// Record that an exact reserve/rent transaction is pending or applied.
    pub fn mark_reserve_transaction_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .mark_submitted(operation_id)
    }

    /// Record a reserve/rent queue failure proven to precede submission.
    pub fn mark_reserve_transaction_not_submitted(
        &self,
        operation_id: [u8; 32],
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .mark_not_submitted(operation_id)
    }

    /// Permit exact-byte retry after finalized absence of a reserve/rent transaction.
    pub fn mark_reserve_transaction_finalized_absent(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: ReserveFinalizedCursorV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .mark_finalized_absent(operation_id, observed_finalized_cursor)
    }

    /// Reconcile exact finalized reserve/rent success by retained transaction digest.
    pub fn mark_reserve_transaction_finalized(
        &self,
        operation_id: [u8; 32],
        expected_transaction_digest: [u8; 32],
        finalized_cursor: ReserveFinalizedCursorV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder.mark_finalized(
            operation_id,
            expected_transaction_digest,
            finalized_cursor,
        )
    }

    /// Reconcile exact reserve/rent semantics committed through another ingress.
    pub fn mark_reserve_transaction_semantic_finalized(
        &self,
        operation_id: [u8; 32],
        finalized_cursor: ReserveFinalizedCursorV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .mark_semantic_finalized(operation_id, finalized_cursor)
    }

    /// Dead-letter a reserve/rent operation that conflicts with finalized state.
    pub fn mark_reserve_transaction_finalized_conflict(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: ReserveFinalizedCursorV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .mark_finalized_conflict(operation_id, observed_finalized_cursor)
    }

    /// Clear a rejected reserve/rent envelope for bounded replacement signing.
    pub fn mark_reserve_transaction_rejected(
        &self,
        operation_id: [u8; 32],
        observed_finalized_cursor: ReserveFinalizedCursorV1,
    ) -> Result<(), ReserveTransactionForwarderError> {
        self.reserve_transaction_forwarder
            .mark_transaction_rejected(operation_id, observed_finalized_cursor)
    }

    /// Returns a reference to the repair scheduler configuration.
    #[must_use]
    pub fn repair_config(&self) -> &RepairConfig {
        &self.repair_config
    }

    /// Returns a reference to the GC scheduler configuration.
    #[must_use]
    pub fn gc_config(&self) -> &GcConfig {
        &self.gc_config
    }

    /// Return the first durability failure that forced this handle into fail-closed mode.
    ///
    /// Once set, durable mutation APIs reject new work because in-memory and
    /// checkpoint state could no longer be proven equivalent.
    #[must_use]
    pub fn durability_failure_reason(&self) -> Option<String> {
        match self.durability_failure.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned
                .into_inner()
                .clone()
                .or_else(|| Some("durability health lock poisoned".to_owned())),
        }
    }

    fn ensure_durability_healthy(&self) -> Result<(), String> {
        match self.durability_failure_reason() {
            Some(reason) => Err(format!(
                "durable mutations are disabled after an unrecoverable checkpoint failure: {reason}"
            )),
            None => Ok(()),
        }
    }

    fn mark_durability_unhealthy(&self, reason: String) {
        match self.durability_failure.lock() {
            Ok(mut guard) => {
                if guard.is_none() {
                    *guard = Some(reason);
                }
            }
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                if guard.is_none() {
                    *guard = Some(reason);
                }
            }
        }
    }

    fn record_unrecoverable_rollback(
        &self,
        context: &str,
        rollback_error: impl std::fmt::Display,
    ) -> String {
        let reason = format!("{context}: {rollback_error}");
        self.mark_durability_unhealthy(reason.clone());
        reason
    }

    fn finish_local_checkpoint_write(
        &self,
        component: &'static str,
        path: &Path,
        result: Result<(), LocalCheckpointWriteError>,
    ) -> Result<(), RuntimeCheckpointPersistError> {
        match result {
            Ok(()) => Ok(()),
            Err(err) => {
                let committed = err.committed;
                let message = format!("persist {component} checkpoint `{}`: {err}", path.display());
                if committed {
                    self.mark_durability_unhealthy(message.clone());
                }
                Err(RuntimeCheckpointPersistError { message, committed })
            }
        }
    }

    fn initialize_runtime_checkpoints(&self) -> Result<(), NodeInitError> {
        let pricing_path = self
            .pricing_checkpoint_path
            .as_ref()
            .expect("storage-backed node has a governed-pricing checkpoint path");
        self.persist_pricing_checkpoint().map_err(|err| {
            NodeInitError::checkpoint("governed pricing runtime", pricing_path, err)
        })?;

        let hedging_path = self
            .hedging_checkpoint_path
            .as_ref()
            .expect("storage-backed node has a signed-feed checkpoint path");
        self.persist_hedging_checkpoint().map_err(|err| {
            NodeInitError::checkpoint("signed hedging-feed runtime", hedging_path, err)
        })?;

        let model_path = self
            .moderation_model_registry_checkpoint_path
            .as_ref()
            .expect("storage-backed node has a model registry checkpoint path");
        self.persist_moderation_model_registry_snapshot(&ModerationModelRegistrySnapshot::default())
            .map_err(|err| {
                NodeInitError::checkpoint("moderation model registry", model_path, err)
            })?;

        let screening_path = self
            .moderation_screening_checkpoint_path
            .as_ref()
            .expect("storage-backed node has a screening checkpoint path");
        self.persist_moderation_screening_snapshot(&ModerationScreeningSnapshot::default())
            .map_err(|err| {
                NodeInitError::checkpoint("moderation screening", screening_path, err)
            })?;

        let object_index_path = self
            .moderation_quarantine_object_index_path
            .as_ref()
            .expect("storage-backed node has an object-index checkpoint path");
        self.persist_moderation_quarantine_object_index_snapshot(
            &ModerationQuarantineObjectSnapshot::default(),
        )
        .map_err(|err| {
            NodeInitError::checkpoint("moderation quarantine object index", object_index_path, err)
        })?;

        let viewer_path = self
            .moderation_evidence_viewer_checkpoint_path
            .as_ref()
            .expect("storage-backed node has an evidence viewer checkpoint path");
        self.persist_moderation_evidence_viewer_snapshot(
            &ModerationEvidenceViewerSnapshot::default(),
        )
        .map_err(|err| NodeInitError::checkpoint("moderation evidence viewer", viewer_path, err))?;

        let auxiliary_path = self
            .auxiliary_runtime_checkpoint_path
            .as_ref()
            .expect("storage-backed node has an auxiliary checkpoint path");
        self.persist_auxiliary_runtime_checkpoint_unlocked()
            .map_err(|err| NodeInitError::checkpoint("auxiliary runtime", auxiliary_path, err))?;

        let marker_path = runtime_state_initialization_path(self.config.data_dir());
        self.finish_local_checkpoint_write(
            "runtime initialization marker",
            &marker_path,
            write_local_private_checkpoint_atomic(&marker_path, RUNTIME_STATE_INITIALIZATION_BYTES),
        )
        .map_err(|err| {
            NodeInitError::checkpoint("runtime initialization marker", &marker_path, err)
        })
    }

    /// Returns a clone of the embedded deal engine handle.
    #[must_use]
    pub fn deal_engine(&self) -> DealEngine {
        self.deal_engine.clone()
    }

    /// Deposit provider collateral without an external funding sequence in tests.
    #[cfg(test)]
    pub fn deposit_provider_bond(
        &self,
        provider_id: ProviderId,
        amount: XorQuantity,
    ) -> Result<ProviderSnapshot, DealEngineError> {
        self.mutate_deal_engine_durably(|engine| {
            engine
                .deposit_provider_bond(provider_id, amount)
                .map(|snapshot| (snapshot, true))
        })
    }

    /// Admit one exact-canonical threshold-governed pricing manifest durably.
    ///
    /// The configured external policy supplies all trust roots. The submitted
    /// bytes are decoded with protocol bounds, required to be canonical, and
    /// never interpreted as a source of trust policy or signing secrets.
    pub fn admit_governed_pricing_manifest(
        &self,
        canonical_envelope: &[u8],
        admitted_at_unix: u64,
    ) -> Result<GovernedPricingAdmissionOutcome, EconomicsRuntimeError> {
        let governed = decode_governed_pricing_manifest(canonical_envelope)?;
        let pricing_id = governed.pricing_id;
        let effective_from_unix = governed.manifest.effective_from_unix;
        let policy = self
            .pricing_trust_policy
            .as_deref()
            .ok_or(EconomicsRuntimeError::PricingNotConfigured)?;
        if self.pricing_checkpoint_path.is_none() {
            return Err(EconomicsRuntimeError::Checkpoint(
                "governed pricing admission requires enabled durable storage".to_owned(),
            ));
        }
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| EconomicsRuntimeError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(EconomicsRuntimeError::Checkpoint)?;
        let mut state = self
            .governed_pricing
            .write()
            .map_err(|_| EconomicsRuntimeError::StateLockPoisoned)?;
        let previous = state.clone();
        let series = state
            .as_mut()
            .ok_or(EconomicsRuntimeError::PricingNotConfigured)?;
        if let Err(error) = series.admit(policy, governed, admitted_at_unix) {
            *state = previous;
            return Err(error.into());
        }
        let admission_count = series.len();
        if let Err(error) = self.persist_pricing_checkpoint_state(Some(series)) {
            if !error.committed {
                *state = previous;
            }
            return Err(EconomicsRuntimeError::Checkpoint(error.to_string()));
        }
        Ok(GovernedPricingAdmissionOutcome {
            pricing_id,
            effective_from_unix,
            admitted_at_unix,
            admission_count,
        })
    }

    /// Return a validated clone of the durable governed-pricing series.
    pub fn governed_pricing_series(
        &self,
    ) -> Result<GovernedPricingSeriesV1, EconomicsRuntimeError> {
        let policy = self
            .pricing_trust_policy
            .as_deref()
            .ok_or(EconomicsRuntimeError::PricingNotConfigured)?;
        let state = self
            .governed_pricing
            .read()
            .map_err(|_| EconomicsRuntimeError::StateLockPoisoned)?;
        let series = state
            .as_ref()
            .ok_or(EconomicsRuntimeError::PricingNotConfigured)?;
        series.validate(policy)?;
        Ok(series.clone())
    }

    /// Return the latest governed pricing manifest effective at `observed_at_unix`.
    pub fn active_governed_pricing(
        &self,
        observed_at_unix: u64,
    ) -> Result<Option<GovernedPricingManifestV1>, EconomicsRuntimeError> {
        let policy = self
            .pricing_trust_policy
            .as_deref()
            .ok_or(EconomicsRuntimeError::PricingNotConfigured)?;
        let state = self
            .governed_pricing
            .read()
            .map_err(|_| EconomicsRuntimeError::StateLockPoisoned)?;
        let series = state
            .as_ref()
            .ok_or(EconomicsRuntimeError::PricingNotConfigured)?;
        Ok(series.active_at(policy, observed_at_unix)?.cloned())
    }

    /// Admit one exact-canonical externally signed hedging-feed sample durably.
    pub fn admit_signed_hedging_feed(
        &self,
        canonical_envelope: &[u8],
        admitted_at_unix: u64,
    ) -> Result<SignedHedgingFeedAdmissionOutcome, EconomicsRuntimeError> {
        let envelope = decode_signed_hedging_price_feed(canonical_envelope)?;
        let feed_id = envelope.feed.feed_id.clone();
        let source = envelope.feed.source.clone();
        let observed_at_unix = envelope.feed.observed_at_unix;
        let policy = self
            .hedging_feed_trust_policy
            .as_deref()
            .ok_or(EconomicsRuntimeError::HedgingNotConfigured)?;
        if self.hedging_checkpoint_path.is_none() {
            return Err(EconomicsRuntimeError::Checkpoint(
                "signed hedging-feed admission requires enabled durable storage".to_owned(),
            ));
        }
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| EconomicsRuntimeError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(EconomicsRuntimeError::Checkpoint)?;
        let mut state = self
            .signed_hedging_feeds
            .write()
            .map_err(|_| EconomicsRuntimeError::StateLockPoisoned)?;
        let previous = state.clone();
        let ledger = state
            .as_mut()
            .ok_or(EconomicsRuntimeError::HedgingNotConfigured)?;
        if let Err(error) = ledger.admit(policy, envelope, admitted_at_unix) {
            *state = previous;
            return Err(error.into());
        }
        let feed_count = ledger.len();
        if let Err(error) = self.persist_hedging_checkpoint_state(Some(ledger)) {
            if !error.committed {
                *state = previous;
            }
            return Err(EconomicsRuntimeError::Checkpoint(error.to_string()));
        }
        drop(state);
        let cluster = self.config.alias().map_or("local", String::as_str);
        global_or_default().set_sorafs_hedging_feed_lag_seconds(
            cluster,
            &source,
            admitted_at_unix.saturating_sub(observed_at_unix),
        );
        Ok(SignedHedgingFeedAdmissionOutcome {
            feed_id,
            source,
            observed_at_unix,
            admitted_at_unix,
            feed_count,
        })
    }

    /// Return a validated clone of the durable signed-feed high-water ledger.
    pub fn signed_hedging_feed_ledger(
        &self,
    ) -> Result<SignedHedgingFeedLedgerV1, EconomicsRuntimeError> {
        let policy = self
            .hedging_feed_trust_policy
            .as_deref()
            .ok_or(EconomicsRuntimeError::HedgingNotConfigured)?;
        let state = self
            .signed_hedging_feeds
            .read()
            .map_err(|_| EconomicsRuntimeError::StateLockPoisoned)?;
        let ledger = state
            .as_ref()
            .ok_or(EconomicsRuntimeError::HedgingNotConfigured)?;
        ledger.validate(policy)?;
        Ok(ledger.clone())
    }

    /// Return the latest authenticated sample retained for every feed id.
    pub fn latest_signed_hedging_feeds(
        &self,
    ) -> Result<Vec<SignedHedgingPriceFeedV1>, EconomicsRuntimeError> {
        let policy = self
            .hedging_feed_trust_policy
            .as_deref()
            .ok_or(EconomicsRuntimeError::HedgingNotConfigured)?;
        let state = self
            .signed_hedging_feeds
            .read()
            .map_err(|_| EconomicsRuntimeError::StateLockPoisoned)?;
        let ledger = state
            .as_ref()
            .ok_or(EconomicsRuntimeError::HedgingNotConfigured)?;
        ledger.latest_signed_feeds(policy).map_err(Into::into)
    }

    /// Return the maximum feed age authorized by the configured hedging policy.
    pub fn hedging_max_sample_age_secs(&self) -> Result<u64, EconomicsRuntimeError> {
        self.hedging_feed_trust_policy
            .as_deref()
            .map(|policy| policy.max_sample_age_secs)
            .ok_or(EconomicsRuntimeError::HedgingNotConfigured)
    }

    /// Derive a governed reference-price decision from durable latest samples.
    pub fn derive_latest_hedging_reference_price(
        &self,
        effective_at_unix: u64,
        admitted_at_unix: u64,
        max_feed_age_secs: u64,
        max_divergence_bps: u16,
    ) -> Result<GovernedHedgingReferencePriceDecisionV1, EconomicsRuntimeError> {
        let policy = self
            .hedging_feed_trust_policy
            .as_deref()
            .ok_or(EconomicsRuntimeError::HedgingNotConfigured)?;
        let state = self
            .signed_hedging_feeds
            .read()
            .map_err(|_| EconomicsRuntimeError::StateLockPoisoned)?;
        let ledger = state
            .as_ref()
            .ok_or(EconomicsRuntimeError::HedgingNotConfigured)?;
        let governed = ledger.derive_latest_reference_price(
            policy,
            effective_at_unix,
            admitted_at_unix,
            max_feed_age_secs,
            max_divergence_bps,
        )?;
        drop(state);
        self.record_hedging_reference_price_metrics(&governed);
        Ok(governed)
    }

    /// Apply an authenticated provider bond funding request at the exact next sequence.
    ///
    /// External callers must authenticate the request and bind `provider_id` to the admitted
    /// provider identity before invoking this trusted runtime boundary.
    pub fn fund_provider_bond_sequenced(
        &self,
        provider_id: ProviderId,
        amount: XorQuantity,
        funding_sequence: u64,
    ) -> Result<ProviderSnapshot, DealEngineError> {
        self.mutate_deal_engine_durably(|engine| {
            engine
                .deposit_provider_bond_sequenced(provider_id, amount, funding_sequence)
                .map(|snapshot| (snapshot, true))
        })
    }

    /// Deposit client credit without an external funding sequence in tests.
    #[cfg(test)]
    pub fn deposit_client_credit(
        &self,
        client_id: ClientId,
        amount: XorQuantity,
    ) -> Result<ClientSnapshot, DealEngineError> {
        self.mutate_deal_engine_durably(|engine| {
            engine
                .deposit_client_credit(client_id, amount)
                .map(|snapshot| (snapshot, true))
        })
    }

    /// Apply an authenticated operator client-credit funding request at the exact next sequence.
    ///
    /// External callers must authenticate a configured operator before invoking this trusted
    /// runtime boundary.
    pub fn fund_client_credit_sequenced(
        &self,
        client_id: ClientId,
        amount: XorQuantity,
        funding_sequence: u64,
    ) -> Result<ClientSnapshot, DealEngineError> {
        self.mutate_deal_engine_durably(|engine| {
            engine
                .deposit_client_credit_sequenced(client_id, amount, funding_sequence)
                .map(|snapshot| (snapshot, true))
        })
    }

    /// Open a deal using the supplied proposal and activation epoch.
    ///
    /// External callers must authenticate a configured operator and require a current admitted
    /// advert for the proposal's provider before invoking this trusted runtime boundary.
    pub fn open_deal(
        &self,
        proposal: DealProposal,
        activation_epoch: u64,
    ) -> Result<DealRecord, DealEngineError> {
        self.mutate_deal_engine_durably(|engine| {
            engine
                .open_deal(proposal, activation_epoch)
                .map(|record| (record, true))
        })
    }

    /// Record usage attributed to a deal and evaluate probabilistic micropayments.
    ///
    /// External callers must bind an authenticated request signer to the deal provider's current
    /// admitted advert before invoking this trusted runtime boundary.
    pub fn record_deal_usage(
        &self,
        report: DealUsageReport,
    ) -> Result<UsageOutcome, DealEngineError> {
        let outcome = self.mutate_deal_engine_durably(|engine| {
            engine.record_usage(report).map(|outcome| (outcome, true))
        })?;
        let provider_hex = hex::encode(outcome.provider_id.as_bytes());
        global_sorafs_node_otel().record_micropayment_sample(
            &provider_hex,
            MicropaymentCreditSnapshot {
                deterministic_charge: outcome.deterministic_charge.clone().into_quantity(),
                credit_generated: outcome
                    .micropayment_credit_generated
                    .clone()
                    .into_quantity(),
                credit_applied: outcome.micropayment_credit_applied.clone().into_quantity(),
                credit_carry: outcome.micropayment_credit_carry.clone().into_quantity(),
                outstanding: outcome.outstanding.clone().into_quantity(),
            },
            MicropaymentTicketCounters {
                processed: outcome.tickets_processed as u64,
                won: outcome.tickets_won as u64,
                duplicate: outcome.tickets_duplicate as u64,
            },
        );
        Ok(outcome)
    }

    /// Register a repair orchestrator used to rehydrate missing chunks remotely.
    pub fn set_repair_orchestrator(&self, orchestrator: Arc<dyn RepairOrchestrator>) {
        if let Ok(mut guard) = self.repair_orchestrator.write() {
            *guard = Some(orchestrator);
        }
    }

    /// Remove any configured repair orchestrator.
    pub fn clear_repair_orchestrator(&self) {
        if let Ok(mut guard) = self.repair_orchestrator.write() {
            *guard = None;
        }
    }

    fn repair_orchestrator(&self) -> Option<Arc<dyn RepairOrchestrator>> {
        self.repair_orchestrator
            .read()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// Register the governance publisher used to surface settlement artefacts.
    pub fn set_governance_publisher(&self, publisher: Arc<dyn GovernancePublisher>) {
        if let Err(err) = self.try_set_governance_publisher(publisher) {
            iroha_logger::error!(%err, "failed to drain SoraFS governance outbox after publisher registration");
        }
    }

    /// Register a governance publisher and replay every durable pending artifact.
    ///
    /// # Errors
    ///
    /// Returns an error if the publisher lock is poisoned or a pending artifact
    /// cannot be published and durably acknowledged.
    pub fn try_set_governance_publisher(
        &self,
        publisher: Arc<dyn GovernancePublisher>,
    ) -> Result<(), GovernancePublishError> {
        *self
            .governance_publisher
            .write()
            .map_err(|_| GovernancePublishError::other("governance publisher lock poisoned"))? =
            Some(publisher);
        self.flush_governance_outbox()?;
        Ok(())
    }

    /// Remove any configured governance publisher.
    pub fn clear_governance_publisher(&self) {
        if let Ok(mut guard) = self.governance_publisher.write() {
            *guard = None;
        }
    }

    /// Return whether this node currently has a governance publisher configured.
    #[must_use]
    pub fn has_governance_publisher(&self) -> bool {
        self.governance_publisher
            .read()
            .is_ok_and(|guard| guard.is_some())
    }

    fn governance_publisher(&self) -> Option<Arc<dyn GovernancePublisher>> {
        self.governance_publisher
            .read()
            .ok()
            .and_then(|guard| guard.clone())
    }

    fn enqueue_governance_outbox_unlocked(
        &self,
        kind: GovernanceOutboxKindV1,
        payload_bytes: Vec<u8>,
    ) -> Result<(u64, bool), GovernancePublishError> {
        self.enqueue_governance_outbox_unlocked_with_reservation(
            kind,
            payload_bytes,
            GovernanceOutboxReservationUse::None,
        )
    }

    fn enqueue_gc_eviction_governance_outbox_unlocked(
        &self,
        payload_bytes: Vec<u8>,
    ) -> Result<(u64, bool), GovernancePublishError> {
        self.enqueue_governance_outbox_unlocked_with_reservation(
            GovernanceOutboxKindV1::GcAudit,
            payload_bytes,
            GovernanceOutboxReservationUse::GcEviction,
        )
    }

    fn enqueue_governance_outbox_unlocked_with_reservation(
        &self,
        kind: GovernanceOutboxKindV1,
        payload_bytes: Vec<u8>,
        reservation_use: GovernanceOutboxReservationUse,
    ) -> Result<(u64, bool), GovernancePublishError> {
        let gc_reserved_slots = {
            let intents = self
                .gc_eviction_intents
                .read()
                .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))?;
            gc_eviction_reserved_outbox_slots(&intents)?
        };
        let reserved_slots = match reservation_use {
            GovernanceOutboxReservationUse::None => gc_reserved_slots,
            GovernanceOutboxReservationUse::GcEviction => {
                if gc_reserved_slots == 0 {
                    return Err(GovernancePublishError::other(
                        "GC eviction publication has no active outbox reservation",
                    ));
                }
                0
            }
        };
        let mut outbox = self
            .governance_outbox
            .write()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?;
        insert_governance_outbox_entry(
            &mut outbox,
            kind,
            payload_bytes,
            self.config.runtime_retention().state_entry_limit(),
            reserved_slots,
        )
    }

    fn enqueue_governance_outbox(
        &self,
        kind: GovernanceOutboxKindV1,
        payload_bytes: Vec<u8>,
    ) -> Result<u64, GovernancePublishError> {
        let checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        let previous = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .clone();
        let (sequence, inserted) = self.enqueue_governance_outbox_unlocked(kind, payload_bytes)?;
        if inserted && let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            *self.governance_outbox.write().map_err(|_| {
                GovernancePublishError::other("governance outbox rollback lock poisoned")
            })? = previous;
            return Err(GovernancePublishError::other(err.to_string()));
        }
        drop(checkpoint_guard);
        Ok(sequence)
    }

    fn reconcile_gc_eviction_intents_on_startup(&self) -> Result<(), GovernancePublishError> {
        let storage = self.storage.as_ref().ok_or_else(|| {
            GovernancePublishError::other("GC eviction reconciliation requires the storage backend")
        })?;
        let gc_guard = self
            .gc_mutation_lock
            .lock()
            .map_err(|_| GovernancePublishError::other("GC mutation lock poisoned"))?;
        let drain_guard = self
            .governance_outbox_drain_lock
            .lock()
            .map_err(|_| GovernancePublishError::other("governance outbox drain lock poisoned"))?;
        let intents = self
            .gc_eviction_intents
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))?
            .entries
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for intent in &intents {
            self.settle_gc_eviction_intent_against_storage(
                &gc_guard,
                &drain_guard,
                storage,
                intent,
                false,
            )?;
        }
        self.validate_gc_eviction_links_against_storage(storage)
    }

    fn enqueue_governance_outbox_with_transparency_entry(
        &self,
        kind: GovernanceOutboxKindV1,
        payload_bytes: Vec<u8>,
        source_entry: TransparencyLedgerSourceEntry,
    ) -> Result<u64, GovernancePublishError> {
        source_entry.validate().map_err(|err| {
            GovernancePublishError::other(format!(
                "invalid transparency ledger source entry: {err}"
            ))
        })?;
        let checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        let previous_outbox = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .clone();
        let previous_sources = self
            .transparency_ledger_source_entries
            .read()
            .map_err(|_| GovernancePublishError::other("transparency source-entry index poisoned"))?
            .clone();

        let mut sources = self
            .transparency_ledger_source_entries
            .write()
            .map_err(|_| {
                GovernancePublishError::other("transparency source-entry index poisoned")
            })?;
        let source_inserted = match sources.get(&source_entry.event_id) {
            Some(existing) if existing == &source_entry => false,
            Some(_) => {
                return Err(GovernancePublishError::other(format!(
                    "transparency ledger source entry `{}` conflicts with retained canonical data",
                    source_entry.event_id
                )));
            }
            None => {
                let limit = self.config.runtime_retention().state_entry_limit();
                if sources.len() >= limit {
                    return Err(GovernancePublishError::other(format!(
                        "transparency source-entry retention exhausted (limit {limit})"
                    )));
                }
                sources.insert(source_entry.event_id.clone(), source_entry);
                true
            }
        };
        drop(sources);

        let (sequence, outbox_inserted) =
            match self.enqueue_governance_outbox_unlocked(kind, payload_bytes) {
                Ok(outcome) => outcome,
                Err(err) => {
                    if source_inserted {
                        *self
                            .transparency_ledger_source_entries
                            .write()
                            .map_err(|_| {
                                GovernancePublishError::other(
                                    "transparency source-entry rollback lock poisoned",
                                )
                            })? = previous_sources;
                    }
                    return Err(err);
                }
            };
        if (source_inserted || outbox_inserted)
            && let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked()
        {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            *self.governance_outbox.write().map_err(|_| {
                GovernancePublishError::other("governance outbox rollback lock poisoned")
            })? = previous_outbox;
            *self
                .transparency_ledger_source_entries
                .write()
                .map_err(|_| {
                    GovernancePublishError::other(
                        "transparency source-entry rollback lock poisoned",
                    )
                })? = previous_sources;
            return Err(GovernancePublishError::other(err.to_string()));
        }
        drop(checkpoint_guard);
        Ok(sequence)
    }

    fn enqueue_sequenced_governance_outbox(
        &self,
        kind: GovernanceOutboxKindV1,
        build_payload: impl FnOnce(u64) -> Result<Vec<u8>, GovernancePublishError>,
    ) -> Result<u64, GovernancePublishError> {
        let checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        let previous = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .clone();
        let expected_sequence = previous.next_sequence;
        let payload_bytes = build_payload(expected_sequence)?;
        let (sequence, inserted) = self.enqueue_governance_outbox_unlocked(kind, payload_bytes)?;
        if !inserted || sequence != expected_sequence {
            *self.governance_outbox.write().map_err(|_| {
                GovernancePublishError::other("governance outbox rollback lock poisoned")
            })? = previous;
            return Err(GovernancePublishError::other(
                "sequenced governance artifact collided with an existing outbox entry",
            ));
        }
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            *self.governance_outbox.write().map_err(|_| {
                GovernancePublishError::other("governance outbox rollback lock poisoned")
            })? = previous;
            return Err(GovernancePublishError::other(err.to_string()));
        }
        drop(checkpoint_guard);
        Ok(sequence)
    }

    /// Return the number of durably pending governance publications.
    #[must_use]
    pub fn pending_governance_publication_count(&self) -> usize {
        self.governance_outbox
            .read()
            .map_or(0, |outbox| outbox.entries.len())
    }

    /// Publish and durably acknowledge all queued governance artifacts.
    ///
    /// Publication is at-least-once. Publishers must therefore treat identical
    /// canonical payload bytes idempotently.
    ///
    /// # Errors
    ///
    /// Returns an error if publication fails, the outbox is corrupt, or its
    /// acknowledgement checkpoint cannot be committed safely.
    pub fn flush_governance_outbox(&self) -> Result<usize, GovernancePublishError> {
        let _drain_guard = self
            .governance_outbox_drain_lock
            .lock()
            .map_err(|_| GovernancePublishError::other("governance outbox drain lock poisoned"))?;
        let Some(publisher) = self.governance_publisher() else {
            return Ok(0);
        };
        let mut published = 0usize;
        loop {
            self.ensure_durability_healthy()
                .map_err(GovernancePublishError::other)?;
            let Some(entry) = self
                .governance_outbox
                .read()
                .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
                .entries
                .first_key_value()
                .map(|(_, entry)| entry.clone())
            else {
                return Ok(published);
            };
            publish_governance_outbox_entry(publisher.as_ref(), &entry)?;

            let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
                GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
            })?;
            self.ensure_durability_healthy()
                .map_err(GovernancePublishError::other)?;
            let removed = self
                .governance_outbox
                .write()
                .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
                .entries
                .remove(&entry.sequence);
            if removed.as_ref() != Some(&entry) {
                return Err(GovernancePublishError::other(
                    "governance outbox changed while publication was in flight",
                ));
            }
            if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
                if err.committed {
                    return Err(GovernancePublishError::other(err.to_string()));
                }
                self.governance_outbox
                    .write()
                    .map_err(|_| {
                        GovernancePublishError::other(
                            "governance outbox acknowledgement rollback lock poisoned",
                        )
                    })?
                    .entries
                    .insert(entry.sequence, entry);
                return Err(GovernancePublishError::other(err.to_string()));
            }
            published = published.saturating_add(1);
        }
    }

    /// Admit, persist, and publish an externally authorized reputation snapshot.
    ///
    /// The local snapshot, head linkage, replay event, and publication intent
    /// are committed together before external delivery. Retrying the exact same
    /// snapshot id is idempotent and retries publication; conflicting ids or
    /// non-monotonic heads are rejected.
    pub fn publish_signed_reputation_snapshot(
        &self,
        envelope: SignedReputationSnapshotV1,
    ) -> Result<(), GovernancePublishError> {
        let checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        let policy = self.reputation_trust_policy.as_deref().ok_or_else(|| {
            GovernancePublishError::other(
                "signed reputation admission is disabled: no external trust policy is configured",
            )
        })?;
        let admitted_at_unix = unix_now_secs();
        envelope.verify(policy, admitted_at_unix).map_err(|err| {
            GovernancePublishError::other(format!("signed reputation admission failed: {err}"))
        })?;
        let snapshot = &envelope.snapshot;
        let encoded = envelope.canonical_bytes().map_err(|err| {
            GovernancePublishError::other(format!("encode signed reputation snapshot: {err}"))
        })?;
        let encoded_len = u64::try_from(encoded.len()).map_err(|_| {
            GovernancePublishError::other(
                "signed reputation snapshot length does not fit checkpoint accounting",
            )
        })?;
        let mut snapshots = self
            .reputation_snapshots
            .write()
            .map_err(|_| GovernancePublishError::other("reputation snapshot index poisoned"))?;
        if snapshots
            .get(&snapshot.snapshot_id)
            .is_some_and(|existing| existing.envelope != envelope)
        {
            return Err(GovernancePublishError::other(format!(
                "signed reputation snapshot id {} conflicts with retained canonical envelope bytes",
                hex::encode(snapshot.snapshot_id)
            )));
        }
        let retained_envelope_bytes = snapshots.values().try_fold(0_u64, |total, admitted| {
            total.checked_add(admitted.encoded_len).ok_or_else(|| {
                GovernancePublishError::other(
                    "retained signed reputation checkpoint byte accounting overflow",
                )
            })
        })?;
        let outbox = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?;
        let mut pending_envelope_bytes = 0_u64;
        let mut exact_envelope_pending = false;
        for entry in outbox
            .entries
            .values()
            .filter(|entry| entry.kind == GovernanceOutboxKindV1::SignedReputationSnapshot)
        {
            let len = u64::try_from(entry.payload_bytes.len()).map_err(|_| {
                GovernancePublishError::other(
                    "pending signed reputation outbox length does not fit u64",
                )
            })?;
            pending_envelope_bytes = pending_envelope_bytes.checked_add(len).ok_or_else(|| {
                GovernancePublishError::other(
                    "pending signed reputation checkpoint byte accounting overflow",
                )
            })?;
            exact_envelope_pending |= entry.payload_bytes == encoded;
        }
        drop(outbox);
        let additional_map_bytes = if snapshots.contains_key(&snapshot.snapshot_id) {
            0
        } else {
            encoded_len
        };
        let additional_outbox_bytes = if exact_envelope_pending {
            0
        } else {
            encoded_len
        };
        let required_reputation_bytes = retained_envelope_bytes
            .checked_add(pending_envelope_bytes)
            .and_then(|total| total.checked_add(additional_map_bytes))
            .and_then(|total| total.checked_add(additional_outbox_bytes))
            .ok_or_else(|| {
                GovernancePublishError::other(
                    "signed reputation checkpoint byte accounting overflow",
                )
            })?;
        let checkpoint_max_bytes = self.config.runtime_retention().checkpoint_max_bytes();
        if required_reputation_bytes > checkpoint_max_bytes {
            return Err(GovernancePublishError::other(format!(
                "signed reputation state requires at least {required_reputation_bytes} checkpoint bytes, exceeding configured limit {checkpoint_max_bytes}"
            )));
        }
        if snapshots.contains_key(&snapshot.snapshot_id) {
            drop(snapshots);
            let previous_outbox = self
                .governance_outbox
                .read()
                .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
                .clone();
            let (_, inserted) = self.enqueue_governance_outbox_unlocked(
                GovernanceOutboxKindV1::SignedReputationSnapshot,
                encoded,
            )?;
            if inserted && let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
                if err.committed {
                    return Err(GovernancePublishError::other(err.to_string()));
                }
                *self.governance_outbox.write().map_err(|_| {
                    GovernancePublishError::other("governance outbox rollback lock poisoned")
                })? = previous_outbox;
                return Err(GovernancePublishError::other(err.to_string()));
            }
            drop(checkpoint_guard);
            self.flush_governance_outbox()?;
            return Ok(());
        }
        let previous_snapshots = snapshots.clone();
        let mut events = self
            .reputation_events
            .write()
            .map_err(|_| GovernancePublishError::other("reputation event history poisoned"))?;
        let previous_events = events.clone();
        let mut latest = self
            .latest_reputation_snapshot
            .write()
            .map_err(|_| GovernancePublishError::other("reputation snapshot cache poisoned"))?;
        let previous_latest = latest.clone();
        let previous_outbox = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .clone();
        validate_reputation_snapshot_transition(latest.as_ref(), snapshot).map_err(|err| {
            GovernancePublishError::other(format!(
                "signed reputation snapshot does not extend the exact retained head: {err}"
            ))
        })?;
        let next_sequence = events
            .latest_sequence
            .checked_add(1)
            .ok_or_else(|| GovernancePublishError::other("event sequence exhausted"))?;
        let event =
            ReputationSnapshotEventV1::from_snapshot(next_sequence, snapshot).map_err(|err| {
                GovernancePublishError::other(format!(
                    "validated reputation snapshot could not produce an event: {err}"
                ))
            })?;
        snapshots.insert(
            snapshot.snapshot_id,
            AdmittedReputationSnapshotV1 {
                version: ADMITTED_REPUTATION_SNAPSHOT_VERSION_V1,
                admitted_at_unix,
                encoded_len,
                envelope: envelope.clone(),
            },
        );
        let event = match events.append(|sequence| {
            debug_assert_eq!(sequence, next_sequence);
            event
        }) {
            Ok(event) => event,
            Err(err) => {
                *snapshots = previous_snapshots;
                return Err(err);
            }
        };
        let retained_snapshot_ids = events
            .events
            .iter()
            .map(|event| event.snapshot_id)
            .chain(std::iter::once(snapshot.snapshot_id))
            .collect::<BTreeSet<_>>();
        let state_limit = self.config.runtime_retention().state_entry_limit();
        while snapshots.len() > state_limit {
            let Some(evict) = snapshots
                .values()
                .filter(|candidate| {
                    !retained_snapshot_ids.contains(&candidate.envelope.snapshot.snapshot_id)
                })
                .min_by_key(|candidate| {
                    (
                        candidate.envelope.snapshot.generated_at_unix,
                        candidate.envelope.snapshot.snapshot_id,
                    )
                })
                .map(|candidate| candidate.envelope.snapshot.snapshot_id)
            else {
                *snapshots = previous_snapshots;
                *events = previous_events;
                return Err(GovernancePublishError::other(format!(
                    "reputation snapshot retention exhausted (limit {state_limit}); retained events prevent safe eviction"
                )));
            };
            snapshots.remove(&evict);
        }
        if events
            .events
            .iter()
            .any(|event| !snapshots.contains_key(&event.snapshot_id))
        {
            *snapshots = previous_snapshots;
            *events = previous_events;
            return Err(GovernancePublishError::other(
                "reputation event suffix references an evicted snapshot",
            ));
        }
        *latest = Some(snapshot.clone());
        drop(snapshots);
        drop(events);
        drop(latest);
        if let Err(err) = self.enqueue_governance_outbox_unlocked(
            GovernanceOutboxKindV1::SignedReputationSnapshot,
            encoded,
        ) {
            *self.reputation_snapshots.write().map_err(|_| {
                GovernancePublishError::other("reputation snapshot rollback lock poisoned")
            })? = previous_snapshots;
            *self.reputation_events.write().map_err(|_| {
                GovernancePublishError::other("reputation event rollback lock poisoned")
            })? = previous_events;
            *self.latest_reputation_snapshot.write().map_err(|_| {
                GovernancePublishError::other("reputation latest rollback lock poisoned")
            })? = previous_latest;
            return Err(err);
        }
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            *self.reputation_snapshots.write().map_err(|_| {
                GovernancePublishError::other("reputation snapshot rollback lock poisoned")
            })? = previous_snapshots;
            *self.reputation_events.write().map_err(|_| {
                GovernancePublishError::other("reputation event rollback lock poisoned")
            })? = previous_events;
            *self.latest_reputation_snapshot.write().map_err(|_| {
                GovernancePublishError::other("reputation latest rollback lock poisoned")
            })? = previous_latest;
            *self.governance_outbox.write().map_err(|_| {
                GovernancePublishError::other("governance outbox rollback lock poisoned")
            })? = previous_outbox;
            return Err(GovernancePublishError::other(err.to_string()));
        }
        drop(checkpoint_guard);
        let _ = self.reputation_event_sender.send(event);
        self.flush_governance_outbox()?;
        Ok(())
    }

    /// Publish a typed SoraFS appeal finance report to the governance pipeline.
    pub fn publish_appeal_finance_report(
        &self,
        report: SoraFsAppealFinanceReportV1,
    ) -> Result<(), GovernancePublishError> {
        report.validate().map_err(|err| {
            GovernancePublishError::other(format!("invalid appeal finance report: {err}"))
        })?;
        let encoded = norito::to_bytes(&report).map_err(|err| {
            GovernancePublishError::other(format!("encode appeal finance report: {err}"))
        })?;
        let source_entry =
            transparency::appeal_finance_report_source_entry(&report).map_err(|err| {
                GovernancePublishError::other(format!(
                    "derive appeal finance report transparency source entry: {err}"
                ))
            })?;
        self.enqueue_governance_outbox_with_transparency_entry(
            GovernanceOutboxKindV1::AppealFinanceReport,
            encoded,
            source_entry,
        )?;
        self.flush_governance_outbox()?;
        Ok(())
    }

    /// Publish a typed SoraFS transparency ledger cycle to the governance pipeline.
    pub fn publish_transparency_ledger_publication(
        &self,
        publication: ModerationLedgerCyclePublicationV1,
    ) -> Result<(), GovernancePublishError> {
        publication.validate().map_err(|err| {
            GovernancePublishError::other(format!("invalid transparency ledger publication: {err}"))
        })?;
        let encoded = norito::to_bytes(&publication).map_err(|err| {
            GovernancePublishError::other(format!("encode transparency ledger publication: {err}"))
        })?;
        self.enqueue_governance_outbox(
            GovernanceOutboxKindV1::TransparencyLedgerPublication,
            encoded,
        )?;
        self.flush_governance_outbox()?;
        Ok(())
    }

    /// Publish a typed proof-token issuance summary to the governance pipeline.
    pub fn publish_proof_token_issuance(
        &self,
        issuance: ProofTokenIssuanceV1,
    ) -> Result<(), GovernancePublishError> {
        issuance.validate().map_err(|err| {
            GovernancePublishError::other(format!("invalid proof-token issuance: {err}"))
        })?;
        let encoded = norito::to_bytes(&issuance).map_err(|err| {
            GovernancePublishError::other(format!("encode proof-token issuance: {err}"))
        })?;
        self.enqueue_governance_outbox(GovernanceOutboxKindV1::ProofTokenIssuance, encoded)?;
        self.flush_governance_outbox()?;
        Ok(())
    }

    /// Derive and publish a proof-token issuance summary from an issued `SFGT` frame.
    ///
    /// The frame signature is verified with `signer_key` before publication.
    /// Runtime digest keys are deliberately not accepted or persisted here.
    pub fn publish_proof_token_frame_issuance(
        &self,
        encoded_token: &[u8],
        signer_key: [u8; 32],
        evidence_digest: Option<[u8; 32]>,
        policy_digest: Option<[u8; 32]>,
        metadata: Vec<ModerationLedgerMetadataV1>,
    ) -> Result<ProofTokenIssuanceV1, GovernancePublishError> {
        let issuance = transparency::proof_token_issuance_from_frame(
            encoded_token,
            signer_key,
            evidence_digest,
            policy_digest,
            metadata,
        )
        .map_err(|err| {
            GovernancePublishError::other(format!("ingest proof-token issuance: {err}"))
        })?;
        self.publish_proof_token_issuance(issuance.clone())?;
        Ok(issuance)
    }

    /// Derive and publish a proof-token issuance summary from URL-safe base64.
    ///
    /// This is the transport-friendly counterpart to
    /// [`Self::publish_proof_token_frame_issuance`].
    pub fn publish_proof_token_base64_issuance(
        &self,
        token_b64: &str,
        signer_key: [u8; 32],
        evidence_digest: Option<[u8; 32]>,
        policy_digest: Option<[u8; 32]>,
        metadata: Vec<ModerationLedgerMetadataV1>,
    ) -> Result<ProofTokenIssuanceV1, GovernancePublishError> {
        let issuance = transparency::proof_token_issuance_from_base64(
            token_b64,
            signer_key,
            evidence_digest,
            policy_digest,
            metadata,
        )
        .map_err(|err| {
            GovernancePublishError::other(format!("ingest proof-token issuance: {err}"))
        })?;
        self.publish_proof_token_issuance(issuance.clone())?;
        Ok(issuance)
    }

    /// Record one privacy-safe source entry for later transparency ledger publication.
    pub fn record_transparency_ledger_source_entry(
        &self,
        entry: TransparencyLedgerSourceEntry,
    ) -> Result<(), GovernancePublishError> {
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        entry.validate().map_err(|err| {
            GovernancePublishError::other(format!(
                "invalid transparency ledger source entry: {err}"
            ))
        })?;
        let mut guard = self
            .transparency_ledger_source_entries
            .write()
            .map_err(|_| {
                GovernancePublishError::other("transparency ledger source-entry index poisoned")
            })?;
        if let Some(existing) = guard.get(&entry.event_id) {
            if existing == &entry {
                return Ok(());
            }
            return Err(GovernancePublishError::other(format!(
                "transparency ledger source entry `{}` conflicts with retained canonical data",
                entry.event_id
            )));
        }
        let limit = self.config.runtime_retention().state_entry_limit();
        if guard.len() >= limit {
            return Err(GovernancePublishError::other(format!(
                "transparency source-entry retention exhausted (limit {limit})"
            )));
        }
        let event_id = entry.event_id.clone();
        guard.insert(event_id.clone(), entry);
        drop(guard);
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            self.transparency_ledger_source_entries
                .write()
                .map_err(|_| {
                    GovernancePublishError::other(
                        "transparency source-entry rollback lock poisoned",
                    )
                })?
                .remove(&event_id);
            return Err(GovernancePublishError::other(err.to_string()));
        }
        Ok(())
    }

    /// Derive and record a transparency source entry from a GAR enforcement receipt.
    pub fn record_gar_enforcement_receipt_transparency_entry(
        &self,
        receipt: &GarEnforcementReceiptV1,
    ) -> Result<(), GovernancePublishError> {
        let entry = transparency::gar_enforcement_receipt_source_entry(receipt).map_err(|err| {
            GovernancePublishError::other(format!(
                "derive GAR enforcement receipt transparency source entry: {err}"
            ))
        })?;
        self.record_transparency_ledger_source_entry(entry)
    }

    /// Derive and record a transparency source entry from a moderation governance event.
    #[cfg(test)]
    pub fn record_moderation_ballot_governance_transparency_entry(
        &self,
        event: &SoraFsModerationBallotGovernanceEventV1,
    ) -> Result<(), GovernancePublishError> {
        let entry = transparency::moderation_ballot_governance_event_source_entry(event).map_err(
            |err| {
                GovernancePublishError::other(format!(
                    "derive moderation governance transparency source entry: {err}"
                ))
            },
        )?;
        self.record_transparency_ledger_source_entry(entry)
    }

    /// Derive and record a transparency source entry from an appeal finance report.
    pub fn record_appeal_finance_report_transparency_entry(
        &self,
        report: &SoraFsAppealFinanceReportV1,
    ) -> Result<(), GovernancePublishError> {
        let entry = transparency::appeal_finance_report_source_entry(report).map_err(|err| {
            GovernancePublishError::other(format!(
                "derive appeal finance report transparency source entry: {err}"
            ))
        })?;
        self.record_transparency_ledger_source_entry(entry)
    }

    /// Derive and record a transparency source entry from an appeal finance settlement receipt.
    pub fn record_appeal_finance_settlement_receipt_transparency_entry(
        &self,
        receipt: &SoraFsAppealFinanceSettlementReceiptV1,
    ) -> Result<(), GovernancePublishError> {
        let entry = transparency::appeal_finance_settlement_receipt_source_entry(receipt).map_err(
            |err| {
                GovernancePublishError::other(format!(
                    "derive appeal finance settlement receipt transparency source entry: {err}"
                ))
            },
        )?;
        self.record_transparency_ledger_source_entry(entry)
    }

    /// Return the number of source entries currently retained by the transparency worker.
    #[must_use]
    pub fn transparency_ledger_source_entry_count(&self) -> usize {
        self.transparency_ledger_source_entries
            .read()
            .map(|guard| guard.len())
            .unwrap_or_default()
    }

    /// Build and publish a transparency cycle from locally recorded source entries.
    ///
    /// The worker selects retained source entries whose occurrence timestamps
    /// fall inside the requested cycle window, sorts them deterministically,
    /// assigns stable entry ids and sequence numbers, and publishes the
    /// resulting `ModerationLedgerCyclePublicationV1`.
    pub fn publish_transparency_ledger_cycle_from_source_entries(
        &self,
        cycle_id: [u8; 16],
        cycle_start_unix: u64,
        cycle_end_unix: u64,
        generated_at_unix: u64,
        previous_block_hash: Option<[u8; 32]>,
    ) -> Result<ModerationLedgerCyclePublicationV1, GovernancePublishError> {
        let events = self
            .transparency_ledger_source_entries
            .read()
            .map_err(|_| {
                GovernancePublishError::other("transparency ledger source-entry index poisoned")
            })?
            .values()
            .filter(|entry| {
                entry.occurred_at_unix >= cycle_start_unix
                    && entry.occurred_at_unix < cycle_end_unix
            })
            .cloned()
            .collect::<Vec<_>>();
        let entries: Vec<ModerationLedgerEntryV1> =
            transparency::build_transparency_ledger_entries_from_source_events(
                cycle_id,
                cycle_start_unix,
                cycle_end_unix,
                generated_at_unix,
                &events,
            )
            .map_err(|err| {
                GovernancePublishError::other(format!(
                    "build transparency ledger source cycle: {err}"
                ))
            })?;
        let publication = ModerationLedgerCyclePublicationV1::from_entries(
            cycle_id,
            cycle_start_unix,
            cycle_end_unix,
            generated_at_unix,
            previous_block_hash,
            &entries,
        )
        .map_err(|err| {
            GovernancePublishError::other(format!(
                "build transparency ledger source publication: {err}"
            ))
        })?;
        self.publish_transparency_ledger_publication(publication.clone())?;
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        let mut source_entries = self
            .transparency_ledger_source_entries
            .write()
            .map_err(|_| {
                GovernancePublishError::other("transparency ledger source-entry index poisoned")
            })?;
        let previous_entries = source_entries.clone();
        source_entries.retain(|_, entry| {
            entry.occurred_at_unix < cycle_start_unix || entry.occurred_at_unix >= cycle_end_unix
        });
        drop(source_entries);
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            *self
                .transparency_ledger_source_entries
                .write()
                .map_err(|_| {
                    GovernancePublishError::other(
                        "transparency source-entry rollback lock poisoned",
                    )
                })? = previous_entries;
            return Err(GovernancePublishError::other(err.to_string()));
        }
        Ok(publication)
    }

    /// Record one source event for later SFM-4c privacy aggregate publication.
    pub fn record_privacy_aggregate_source_event(
        &self,
        event: PrivacyAggregateSourceEvent,
    ) -> Result<PrivacySourceEventRecordOutcomeV1, GovernancePublishError> {
        let _mutation_guard = self.runtime_mutation_lock.lock().map_err(|_| {
            GovernancePublishError::other("runtime publication transaction lock poisoned")
        })?;
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        event.validate().map_err(|err| {
            GovernancePublishError::other(format!("invalid privacy aggregate source event: {err}"))
        })?;
        let canonical_digest = event.canonical_digest().map_err(|err| {
            GovernancePublishError::other(format!("digest privacy aggregate source event: {err}"))
        })?;
        let configured_policy = self.config.privacy_aggregate_policy();
        let configured_schedule = self.config.privacy_aggregate_schedule();
        let active_query = match (configured_schedule, configured_policy) {
            (Some(schedule), Some(policy)) => {
                schedule.validate().map_err(|err| {
                    GovernancePublishError::other(format!("privacy aggregate schedule: {err}"))
                })?;
                let config = policy.cycle_config();
                config.validate().map_err(|err| {
                    GovernancePublishError::other(format!("privacy aggregate policy: {err}"))
                })?;
                if schedule.first_cycle_start_unix != config.first_cycle_start_unix
                    || schedule.cycle_seconds != config.cycle_seconds
                {
                    return Err(GovernancePublishError::other(
                        "privacy aggregate schedule conflicts with the governed query cadence",
                    ));
                }
                let release_ledger = self
                    .privacy_release_ledger
                    .read()
                    .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))?
                    .clone();
                release_ledger.validate().map_err(|error| {
                    GovernancePublishError::other(format!(
                        "invalid privacy release ledger: {error}"
                    ))
                })?;
                validate_privacy_release_lineage(&release_ledger, schedule, &config)?;
                Some((schedule, config))
            }
            (None, None) => None,
            _ => {
                return Err(GovernancePublishError::other(
                    "privacy aggregate schedule and governed policy must be enabled together",
                ));
            }
        };
        if let Some(existing_digest) = self
            .privacy_source_event_receipts
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy source-event receipt index poisoned")
            })?
            .get(&event.event_id)
            .copied()
        {
            return if existing_digest == canonical_digest {
                Ok(PrivacySourceEventRecordOutcomeV1::AlreadyRecorded)
            } else {
                Err(GovernancePublishError::other(
                    "privacy aggregate source-event idempotency key equivocation",
                ))
            };
        }
        if let Some((schedule, config)) = active_query {
            config.validate_source_event(&event).map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid privacy aggregate source event under governed policy: {err}"
                ))
            })?;
            let window = schedule
                .event_window(event.occurred_at_unix)
                .map_err(|err| {
                    GovernancePublishError::other(format!("privacy aggregate schedule: {err}"))
                })?;
            let cycle_id = transparency::privacy_aggregate_cycle_id(
                config.query_id,
                window.cycle_start_unix,
                window.cycle_end_unix,
            );
            if self
                .published_privacy_aggregate_cycles
                .read()
                .map_err(|_| GovernancePublishError::other("privacy cycle index poisoned"))?
                .contains(&cycle_id)
            {
                return Err(GovernancePublishError::other(
                    "privacy aggregate source event targets a finalized release window",
                ));
            }
        }
        let mut event_guard = self
            .privacy_aggregate_source_events
            .write()
            .map_err(|_| GovernancePublishError::other("privacy aggregate event index poisoned"))?;
        if event_guard.contains_key(&event.event_id) {
            return Err(GovernancePublishError::other(
                "privacy aggregate source event exists without its durable receipt",
            ));
        }
        let limit = self.config.runtime_retention().state_entry_limit();
        if event_guard.len() >= limit {
            return Err(GovernancePublishError::other(format!(
                "privacy aggregate source-event retention exhausted (limit {limit})"
            )));
        }
        let mut receipt_guard = self.privacy_source_event_receipts.write().map_err(|_| {
            GovernancePublishError::other("privacy source-event receipt index poisoned")
        })?;
        if receipt_guard.len() >= limit {
            return Err(GovernancePublishError::other(format!(
                "privacy aggregate source-event receipt retention exhausted (limit {limit})"
            )));
        }
        let event_id = event.event_id.clone();
        event_guard.insert(event_id.clone(), event);
        receipt_guard.insert(event_id.clone(), canonical_digest);
        drop(receipt_guard);
        drop(event_guard);
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            self.privacy_aggregate_source_events
                .write()
                .map_err(|_| {
                    GovernancePublishError::other("privacy source-event rollback lock poisoned")
                })?
                .remove(&event_id);
            self.privacy_source_event_receipts
                .write()
                .map_err(|_| {
                    GovernancePublishError::other(
                        "privacy source-event receipt rollback lock poisoned",
                    )
                })?
                .remove(&event_id);
            return Err(GovernancePublishError::other(err.to_string()));
        }
        Ok(PrivacySourceEventRecordOutcomeV1::Recorded)
    }

    /// Return the number of source events currently retained by the aggregate worker.
    #[must_use]
    pub fn privacy_aggregate_source_event_count(&self) -> usize {
        self.privacy_aggregate_source_events
            .read()
            .map(|guard| guard.len())
            .unwrap_or_default()
    }

    /// Return a consistent copy of the durable privacy composition-budget ledger.
    ///
    /// # Errors
    ///
    /// Returns an error if the local budget lock is poisoned.
    pub fn privacy_composition_budget_snapshot(
        &self,
    ) -> Result<PrivacyCompositionBudgetLedgerV1, GovernancePublishError> {
        self.privacy_composition_budget
            .read()
            .map_err(|_| GovernancePublishError::other("privacy composition budget poisoned"))
            .map(|budget| budget.clone())
    }

    fn reconcile_privacy_release_anchor(&self) -> Result<(), GovernancePublishError> {
        let policy = self.config.privacy_aggregate_policy();
        let schedule = self.configured_privacy_aggregate_schedule();
        let release_ledger = self
            .privacy_release_ledger
            .read()
            .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))?
            .clone();
        release_ledger.validate().map_err(|error| {
            GovernancePublishError::other(format!(
                "invalid durable privacy release ledger: {error}"
            ))
        })?;
        let configured_query_id = policy.map(config::PrivacyAggregatePolicyConfig::query_id);
        let query_id = match release_ledger.records.last() {
            Some(record) => {
                if configured_query_id.is_some_and(|query_id| query_id != record.query_id) {
                    return Err(GovernancePublishError::other(
                        "durable privacy release state does not match the configured query",
                    ));
                }
                if let Some(policy) = policy {
                    let config = policy.cycle_config();
                    let schedule = schedule.ok_or_else(|| {
                        GovernancePublishError::other(
                            "durable privacy release cadence does not match the configured query lineage",
                        )
                    })?;
                    validate_privacy_release_lineage(&release_ledger, schedule, &config)?;
                }
                record.query_id
            }
            None => {
                let Some(query_id) = configured_query_id else {
                    return Ok(());
                };
                query_id
            }
        };
        let local_head = release_ledger.head(query_id).map_err(|error| {
            GovernancePublishError::other(format!(
                "privacy release ledger does not match the configured query: {error}"
            ))
        })?;
        let requires_anchor = policy.is_some() || !release_ledger.records.is_empty();
        let Some(anchor) = self.privacy_release_anchor.as_deref() else {
            if requires_anchor {
                return Err(GovernancePublishError::other(
                    "finalized privacy release anchor is unavailable",
                ));
            }
            return Ok(());
        };
        let mut finalized = anchor.finalized_head(query_id).map_err(|error| {
            GovernancePublishError::other(format!("read finalized privacy release anchor: {error}"))
        })?;
        if !finalized.validate() || finalized.query_id() != query_id {
            return Err(GovernancePublishError::other(
                "finalized privacy release anchor is malformed or bound to another query",
            ));
        }
        if finalized.sequence() > local_head.sequence()
            || (finalized.sequence() == local_head.sequence() && finalized != local_head)
        {
            return Err(GovernancePublishError::other(
                "local privacy release checkpoint is behind or equivocates with the finalized anchor",
            ));
        }
        if finalized == local_head {
            return Ok(());
        }

        let published_cycles = self
            .published_privacy_aggregate_cycles
            .read()
            .map_err(|_| GovernancePublishError::other("privacy cycle index poisoned"))?
            .clone();
        let outbox = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .clone();
        validate_privacy_release_outbox(&release_ledger, &published_cycles, &outbox)?;
        while finalized.sequence() < local_head.sequence() {
            let next_index = usize::try_from(finalized.sequence()).map_err(|_| {
                GovernancePublishError::other("privacy release anchor sequence exceeds usize")
            })?;
            let record = release_ledger.records.get(next_index).ok_or_else(|| {
                GovernancePublishError::other(
                    "privacy release anchor is not a prefix of the local release chain",
                )
            })?;
            let expected = if next_index == 0 {
                PrivacyReleaseAnchorHeadV1::genesis(query_id)
            } else {
                PrivacyReleaseAnchorHeadV1::from_record(
                    &release_ledger.records[next_index.saturating_sub(1)],
                )
            };
            if expected != finalized {
                return Err(GovernancePublishError::other(
                    "privacy release anchor predecessor conflicts with the local release chain",
                ));
            }
            if record.status == transparency::PrivacyReleaseStatusV1::Published {
                let has_exact_pending_publication = outbox.entries.values().any(|entry| {
                    entry.kind == GovernanceOutboxKindV1::TransparencyLedgerPublication
                        && record.publication_payload_digest == Some(entry.payload_digest)
                });
                if !has_exact_pending_publication {
                    return Err(GovernancePublishError::other(
                        "unanchored privacy release lacks its exact durable publication outbox entry",
                    ));
                }
            }
            let next = PrivacyReleaseAnchorHeadV1::from_record(record);
            anchor
                .compare_and_set_finalized_head(finalized, next)
                .map_err(|error| {
                    GovernancePublishError::other(format!(
                        "advance finalized privacy release anchor: {error}"
                    ))
                })?;
            let observed = anchor.finalized_head(query_id).map_err(|error| {
                GovernancePublishError::other(format!(
                    "confirm finalized privacy release anchor: {error}"
                ))
            })?;
            if observed != next {
                return Err(GovernancePublishError::other(
                    "finalized privacy release anchor did not confirm the exact committed head",
                ));
            }
            finalized = observed;
        }
        Ok(())
    }

    /// Return the config-backed privacy aggregate scheduler, when enabled.
    #[must_use]
    pub fn configured_privacy_aggregate_schedule(&self) -> Option<PrivacyAggregateScheduleConfig> {
        self.config.privacy_aggregate_schedule()
    }

    fn derive_privacy_cycle_prf_input(
        &self,
        config: &PrivacyAggregateCycleConfig,
        window: PrivacyAggregateCycleWindow,
    ) -> Result<Option<PrivacyCyclePrfInputV1>, GovernancePublishError> {
        if config.privacy.per_subject_metric_cap.is_none() {
            return Ok(None);
        }
        let request = PrivacyCyclePrfRequestV1::new(
            config.query_id,
            config.policy_digest,
            privacy_population_inventory_digest(&config.populations),
            privacy_metric_schema_digest(&config.metrics),
            window,
        )
        .map_err(|_| {
            GovernancePublishError::other("privacy cycle PRF request binding is invalid")
        })?;
        let provider = self.privacy_cycle_prf_provider.as_deref().ok_or_else(|| {
            GovernancePublishError::other(
                "runtime threshold PRF provider is unavailable for differential privacy",
            )
        })?;
        let output = provider
            .derive_cycle_output(&request)
            .map_err(|error| match error {
                PrivacyCyclePrfProviderErrorV1::Unavailable => {
                    GovernancePublishError::other("runtime threshold PRF provider is unavailable")
                }
                PrivacyCyclePrfProviderErrorV1::AuthenticationFailed => {
                    GovernancePublishError::other(
                        "runtime threshold PRF provider authentication failed",
                    )
                }
                PrivacyCyclePrfProviderErrorV1::RateLimited => {
                    GovernancePublishError::other("runtime threshold PRF provider rate limited")
                }
                PrivacyCyclePrfProviderErrorV1::Internal => {
                    GovernancePublishError::other("runtime threshold PRF provider internal failure")
                }
            })?;
        Ok(Some(PrivacyCyclePrfInputV1::new(request, output)))
    }

    /// Return the config-backed evidence-viewer audit scheduler, when enabled.
    #[must_use]
    pub fn configured_evidence_viewer_audit_schedule(
        &self,
    ) -> Option<PrivacyAggregateScheduleConfig> {
        self.config.evidence_viewer_audit_schedule()
    }

    /// Build and publish a transparency cycle from privacy-safe moderation aggregates.
    ///
    /// Aggregates are validated, required to fit inside the supplied cycle
    /// window, sorted deterministically by source window and aggregate id, then
    /// converted into `PrivacyAggregate` ledger entries before being published
    /// through the configured governance pipeline.
    #[cfg(test)]
    fn publish_privacy_aggregate_cycle(
        &self,
        cycle_id: [u8; 16],
        cycle_start_unix: u64,
        cycle_end_unix: u64,
        _generated_at_unix: u64,
        previous_block_hash: Option<[u8; 32]>,
        aggregates: Vec<ModerationPrivacyAggregateV1>,
    ) -> Result<ModerationLedgerCyclePublicationV1, GovernancePublishError> {
        let publication = Self::build_privacy_aggregate_publication(
            cycle_id,
            cycle_start_unix,
            cycle_end_unix,
            cycle_end_unix,
            previous_block_hash,
            aggregates,
        )?;
        self.publish_transparency_ledger_publication(publication.clone())?;
        Ok(publication)
    }

    fn build_privacy_aggregate_publication(
        cycle_id: [u8; 16],
        cycle_start_unix: u64,
        cycle_end_unix: u64,
        generated_at_unix: u64,
        previous_block_hash: Option<[u8; 32]>,
        aggregates: Vec<ModerationPrivacyAggregateV1>,
    ) -> Result<ModerationLedgerCyclePublicationV1, GovernancePublishError> {
        if aggregates.is_empty() {
            return Err(GovernancePublishError::other(
                "privacy aggregate cycle requires at least one aggregate",
            ));
        }
        if generated_at_unix != cycle_end_unix {
            return Err(GovernancePublishError::other(
                "privacy aggregate publication timestamp must equal the exact cycle end",
            ));
        }

        let mut seen_aggregate_ids = BTreeSet::new();
        let mut cycle_policy_digest = None;
        let mut cycle_noise_source = None;
        let mut keyed = Vec::with_capacity(aggregates.len());
        for aggregate in aggregates {
            aggregate.validate().map_err(|err| {
                GovernancePublishError::other(format!("invalid privacy aggregate: {err}"))
            })?;
            if aggregate.window_start_unix != cycle_start_unix
                || aggregate.window_end_unix != cycle_end_unix
            {
                return Err(GovernancePublishError::other(format!(
                    "privacy aggregate `{}` window must equal the publication cycle",
                    aggregate.aggregate_id
                )));
            }
            if aggregate.generated_at_unix != cycle_end_unix {
                return Err(GovernancePublishError::other(format!(
                    "privacy aggregate `{}` generated_at timestamp must equal the exact cycle end",
                    aggregate.aggregate_id
                )));
            }
            if !seen_aggregate_ids.insert(aggregate.aggregate_id.clone()) {
                return Err(GovernancePublishError::other(format!(
                    "duplicate privacy aggregate id `{}` in cycle",
                    aggregate.aggregate_id
                )));
            }
            match cycle_policy_digest {
                Some(policy_digest) if policy_digest != aggregate.policy_digest => {
                    return Err(GovernancePublishError::other(
                        "privacy aggregate cycle contains mixed policy digests",
                    ));
                }
                None => cycle_policy_digest = Some(aggregate.policy_digest),
                Some(_) => {}
            }
            match cycle_noise_source {
                Some(noise_source) if noise_source != aggregate.noise_source => {
                    return Err(GovernancePublishError::other(
                        "privacy aggregate cycle contains mixed privacy noise sources",
                    ));
                }
                None => cycle_noise_source = Some(aggregate.noise_source),
                Some(_) => {}
            }
            let aggregate_hash = aggregate.aggregate_hash().map_err(|err| {
                GovernancePublishError::other(format!("hash privacy aggregate: {err}"))
            })?;
            keyed.push((
                aggregate.window_start_unix,
                aggregate.window_end_unix,
                aggregate.aggregate_id.clone(),
                aggregate_hash,
                aggregate,
            ));
        }
        keyed.sort_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| left.3.cmp(&right.3))
        });

        let mut entries = Vec::with_capacity(keyed.len());
        for (index, (_, _, _, aggregate_hash, aggregate)) in keyed.iter().enumerate() {
            let sequence = u64::try_from(index)
                .map_err(|_| GovernancePublishError::other("privacy aggregate index overflow"))?
                .saturating_add(1);
            let entry_id =
                privacy_aggregate_entry_id(cycle_id, *aggregate_hash, &aggregate.aggregate_id);
            let entry = aggregate
                .to_ledger_entry(cycle_id, entry_id, sequence)
                .map_err(|err| {
                    GovernancePublishError::other(format!(
                        "convert privacy aggregate `{}` to ledger entry: {err}",
                        aggregate.aggregate_id
                    ))
                })?;
            entries.push(entry);
        }
        let ordered_aggregates = keyed
            .iter()
            .map(|(_, _, _, _, aggregate)| aggregate.clone())
            .collect::<Vec<_>>();

        let publication = ModerationLedgerCyclePublicationV1::from_entries_with_privacy_aggregates(
            cycle_id,
            cycle_start_unix,
            cycle_end_unix,
            generated_at_unix,
            previous_block_hash,
            &entries,
            &ordered_aggregates,
        )
        .map_err(|err| {
            GovernancePublishError::other(format!(
                "build privacy aggregate transparency publication: {err}"
            ))
        })?;
        Ok(publication)
    }

    /// Build and publish a privacy aggregate cycle from locally recorded source events.
    ///
    /// The worker filters recorded events to the requested cycle window, applies
    /// suppression/noise policy from `config`, builds aggregate payloads, and
    /// publishes the resulting transparency cycle through
    /// [`Self::publish_privacy_aggregate_cycle`].
    #[cfg(test)]
    fn publish_privacy_aggregate_cycle_from_source_events(
        &self,
        input: PrivacyAggregateSourceCycleInput,
    ) -> Result<ModerationLedgerCyclePublicationV1, GovernancePublishError> {
        let PrivacyAggregateSourceCycleInput {
            cycle_id,
            cycle_start_unix,
            cycle_end_unix,
            previous_block_hash,
            config,
            cycle_prf_input,
        } = input;
        let events = self
            .privacy_aggregate_source_events
            .read()
            .map_err(|_| GovernancePublishError::other("privacy aggregate event index poisoned"))?
            .values()
            .filter(|event| {
                event.occurred_at_unix >= cycle_start_unix
                    && event.occurred_at_unix < cycle_end_unix
            })
            .cloned()
            .collect::<Vec<_>>();
        let aggregates = transparency::build_privacy_aggregates_from_source_events(
            cycle_start_unix,
            cycle_end_unix,
            &config,
            cycle_prf_input,
            &events,
        )
        .map_err(|err| {
            GovernancePublishError::other(format!("build privacy aggregate cycle: {err}"))
        })?;
        self.publish_privacy_aggregate_cycle(
            cycle_id,
            cycle_start_unix,
            cycle_end_unix,
            cycle_end_unix,
            previous_block_hash,
            aggregates,
        )
    }

    /// Publish exactly one direct-successor privacy aggregate cycle.
    ///
    /// Window selection depends only on the governed activation/cadence and the
    /// durable release cursor. Source-event presence never affects which cycle
    /// is selected.
    fn publish_due_privacy_aggregate_cycle_from_source_events(
        &self,
        now_unix: u64,
        expected_cycle_id: [u8; 16],
        idempotency_key: String,
        schedule: PrivacyAggregateScheduleConfig,
        config: PrivacyAggregateCycleConfig,
        composition_budget: Option<PrivacyCompositionBudgetPolicyV1>,
    ) -> Result<PrivacyAggregateScheduleOutcome, GovernancePublishError> {
        let _mutation_guard = self.runtime_mutation_lock.lock().map_err(|_| {
            GovernancePublishError::other("runtime publication transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        validate_privacy_publish_idempotency_key(&idempotency_key)?;
        schedule.validate().map_err(|err| {
            GovernancePublishError::other(format!("privacy aggregate schedule: {err}"))
        })?;
        config.validate().map_err(|err| {
            GovernancePublishError::other(format!("privacy aggregate policy: {err}"))
        })?;
        if schedule.first_cycle_start_unix != config.first_cycle_start_unix
            || schedule.cycle_seconds != config.cycle_seconds
        {
            return Err(GovernancePublishError::other(
                "privacy aggregate schedule conflicts with the governed query cadence",
            ));
        }
        let release_ledger = self
            .privacy_release_ledger
            .read()
            .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))?
            .clone();
        release_ledger.validate().map_err(|error| {
            GovernancePublishError::other(format!("invalid privacy release ledger: {error}"))
        })?;
        validate_privacy_release_lineage(&release_ledger, schedule, &config)?;
        if self.privacy_release_anchor.is_none() {
            return Err(GovernancePublishError::other(
                "finalized privacy release anchor is unavailable",
            ));
        }
        self.reconcile_privacy_release_anchor()?;
        let request_digest =
            privacy_publish_request_digest(config.query_id, &idempotency_key, expected_cycle_id);
        let head = release_ledger.head(config.query_id).map_err(|error| {
            GovernancePublishError::other(format!("invalid privacy release head: {error}"))
        })?;
        let cycle_start_unix = release_ledger
            .records
            .last()
            .map_or(config.first_cycle_start_unix, |record| {
                record.cycle_end_unix
            });
        let cycle_end_unix = cycle_start_unix
            .checked_add(config.cycle_seconds)
            .ok_or_else(|| GovernancePublishError::other("privacy cycle window overflow"))?;
        let window = PrivacyAggregateCycleWindow {
            cycle_start_unix,
            cycle_end_unix,
            due_at_unix: cycle_end_unix
                .checked_add(schedule.publish_delay_seconds)
                .ok_or_else(|| {
                    GovernancePublishError::other("privacy cycle due timestamp overflow")
                })?,
        };
        let cycle_id = transparency::privacy_aggregate_cycle_id(
            config.query_id,
            window.cycle_start_unix,
            window.cycle_end_unix,
        );
        if let Some(receipt) = self
            .privacy_publish_request_receipts
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy publish-request receipt index poisoned")
            })?
            .get(&idempotency_key)
            .cloned()
        {
            if receipt.request_digest != request_digest || receipt.cycle_id != expected_cycle_id {
                return Err(GovernancePublishError::other(
                    "privacy publish idempotency key equivocation",
                ));
            }
            let record = release_ledger
                .records
                .iter()
                .find(|record| record.release_id == receipt.cycle_id)
                .ok_or_else(|| {
                    GovernancePublishError::other(
                        "privacy publish-request receipt references an unknown terminal release",
                    )
                })?;
            validate_privacy_publish_receipt_release(&receipt, record)?;
            self.flush_governance_outbox()?;
            return receipt.outcome();
        }
        if cycle_id != expected_cycle_id {
            return Err(GovernancePublishError::other(
                "privacy publish expected cycle does not match the direct successor",
            ));
        }
        let Some(latest_window) = schedule.due_window(now_unix).map_err(|err| {
            GovernancePublishError::other(format!("privacy aggregate schedule: {err}"))
        })?
        else {
            return Ok(PrivacyAggregateScheduleOutcome::NotDue);
        };
        if window.cycle_end_unix > latest_window.cycle_end_unix {
            return Ok(PrivacyAggregateScheduleOutcome::NotDue);
        }
        let published_cycles = self
            .published_privacy_aggregate_cycles
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy aggregate published-cycle index poisoned")
            })?
            .clone();
        let state_limit = self.config.runtime_retention().state_entry_limit();
        if published_cycles.contains(&cycle_id) {
            return Err(GovernancePublishError::other(
                "privacy published-cycle guard is ahead of the durable release cursor",
            ));
        }
        if published_cycles.len() >= state_limit {
            return Err(GovernancePublishError::other(format!(
                "privacy aggregate published-cycle retention exhausted (limit {state_limit})"
            )));
        }
        let events = self
            .privacy_aggregate_source_events
            .read()
            .map_err(|_| GovernancePublishError::other("privacy aggregate event index poisoned"))?
            .values()
            .filter(|event| {
                event.occurred_at_unix >= window.cycle_start_unix
                    && event.occurred_at_unix < window.cycle_end_unix
            })
            .cloned()
            .collect::<Vec<_>>();
        let private_source_digest = transparency::canonical_private_source_digest(
            window.cycle_start_unix,
            window.cycle_end_unix,
            &config,
            &events,
        )
        .map_err(|error| {
            GovernancePublishError::other(format!(
                "digest scheduled privacy aggregate source: {error}"
            ))
        })?;
        let cycle_prf_input = self.derive_privacy_cycle_prf_input(&config, window)?;
        let prf_evidence = cycle_prf_input.as_ref().map(|input| {
            (
                input.request().binding_digest(),
                input.commitment().commitment,
            )
        });
        let aggregates = match transparency::build_privacy_aggregates_from_source_events(
            window.cycle_start_unix,
            window.cycle_end_unix,
            &config,
            cycle_prf_input,
            &events,
        ) {
            Ok(aggregates) => aggregates,
            Err(
                transparency::PrivacyAggregateWorkerError::AllBucketsSuppressed
                | transparency::PrivacyAggregateWorkerError::NoSourceEvents,
            ) => {
                let receipt = PrivacyPublishRequestReceiptV1 {
                    idempotency_key,
                    request_digest,
                    query_id: config.query_id,
                    requested_now_unix: now_unix,
                    cycle_id,
                    cycle_start_unix: window.cycle_start_unix,
                    cycle_end_unix: window.cycle_end_unix,
                    publish_delay_seconds: schedule.publish_delay_seconds,
                    due_at_unix: window.due_at_unix,
                    outcome: PrivacyPublishRequestOutcomeV1::AllBucketsSuppressed,
                };
                self.commit_processed_privacy_cycle(
                    cycle_id,
                    window,
                    &config,
                    private_source_digest,
                    head.latest_publication_block_hash(),
                    receipt,
                )?;
                return Ok(PrivacyAggregateScheduleOutcome::AllBucketsSuppressed {
                    window,
                    cycle_id,
                });
            }
            Err(err) => {
                return Err(GovernancePublishError::other(format!(
                    "build scheduled privacy aggregate cycle: {err}"
                )));
            }
        };
        let publication = Self::build_privacy_aggregate_publication(
            cycle_id,
            window.cycle_start_unix,
            window.cycle_end_unix,
            window.cycle_end_unix,
            head.latest_publication_block_hash(),
            aggregates,
        )?;
        let publication_bytes = norito::to_bytes(&publication).map_err(|err| {
            GovernancePublishError::other(format!(
                "encode privacy publication idempotency receipt: {err}"
            ))
        })?;
        let receipt = PrivacyPublishRequestReceiptV1 {
            idempotency_key,
            request_digest,
            query_id: config.query_id,
            requested_now_unix: now_unix,
            cycle_id,
            cycle_start_unix: window.cycle_start_unix,
            cycle_end_unix: window.cycle_end_unix,
            publish_delay_seconds: schedule.publish_delay_seconds,
            due_at_unix: window.due_at_unix,
            outcome: PrivacyPublishRequestOutcomeV1::Published { publication_bytes },
        };
        self.commit_published_privacy_cycle(
            window,
            &publication,
            &config,
            private_source_digest,
            prf_evidence,
            composition_budget,
            receipt,
        )?;
        Ok(PrivacyAggregateScheduleOutcome::Published {
            window,
            publication,
        })
    }

    fn commit_published_privacy_cycle(
        &self,
        window: PrivacyAggregateCycleWindow,
        publication: &ModerationLedgerCyclePublicationV1,
        config: &PrivacyAggregateCycleConfig,
        private_source_digest: [u8; 32],
        prf_evidence: Option<([u8; 32], [u8; 32])>,
        composition_budget: Option<PrivacyCompositionBudgetPolicyV1>,
        request_receipt: PrivacyPublishRequestReceiptV1,
    ) -> Result<(), GovernancePublishError> {
        publication.validate().map_err(|err| {
            GovernancePublishError::other(format!(
                "invalid scheduled privacy aggregate publication: {err}"
            ))
        })?;
        let cycle_id = publication.block.cycle_id;
        if publication.block.cycle_start_unix != window.cycle_start_unix
            || publication.block.cycle_end_unix != window.cycle_end_unix
            || publication.block.generated_at_unix != window.cycle_end_unix
            || cycle_id
                != transparency::privacy_aggregate_cycle_id(
                    config.query_id,
                    window.cycle_start_unix,
                    window.cycle_end_unix,
                )
        {
            return Err(GovernancePublishError::other(
                "scheduled privacy aggregate publication identity or window mismatch",
            ));
        }
        let encoded = norito::to_bytes(publication).map_err(|err| {
            GovernancePublishError::other(format!(
                "encode scheduled privacy aggregate publication: {err}"
            ))
        })?;
        request_receipt.validate()?;
        if request_receipt.cycle_id != cycle_id
            || request_receipt.query_id != config.query_id
            || request_receipt.cycle_start_unix != window.cycle_start_unix
            || request_receipt.cycle_end_unix != window.cycle_end_unix
            || request_receipt.due_at_unix != window.due_at_unix
            || !matches!(
                &request_receipt.outcome,
                PrivacyPublishRequestOutcomeV1::Published { publication_bytes }
                    if publication_bytes == &encoded
            )
        {
            return Err(GovernancePublishError::other(
                "privacy publish-request receipt does not match the publication",
            ));
        }
        let checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;

        let previous_budget = self
            .privacy_composition_budget
            .read()
            .map_err(|_| GovernancePublishError::other("privacy composition budget poisoned"))?
            .clone();
        let previous_release_ledger = self
            .privacy_release_ledger
            .read()
            .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))?
            .clone();
        let previous_published = self
            .published_privacy_aggregate_cycles
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy aggregate published-cycle index poisoned")
            })?
            .clone();
        let previous_events = self
            .privacy_aggregate_source_events
            .read()
            .map_err(|_| GovernancePublishError::other("privacy aggregate event index poisoned"))?
            .clone();
        let previous_request_receipts = self
            .privacy_publish_request_receipts
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy publish-request receipt index poisoned")
            })?
            .clone();
        let previous_outbox = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .clone();

        let mut next_budget = previous_budget.clone();
        let budget_charge = match (
            config.privacy.epsilon_numerator,
            config.privacy.epsilon_denominator,
            composition_budget,
        ) {
            (Some(numerator), Some(denominator), Some(policy)) => Some(
                next_budget
                    .charge(
                        policy,
                        cycle_id,
                        publication.block.generated_at_unix,
                        numerator,
                        denominator,
                    )
                    .map_err(|err| {
                        GovernancePublishError::other(format!(
                            "charge privacy composition budget: {err}"
                        ))
                    })?,
            ),
            (None, None, _) => None,
            (Some(_), Some(_), None) => {
                return Err(GovernancePublishError::other(
                    "differential-privacy publication requires a governed composition budget",
                ));
            }
            _ => {
                return Err(GovernancePublishError::other(
                    "privacy epsilon numerator and denominator must be supplied together",
                ));
            }
        };
        if config.privacy.per_subject_metric_cap.is_some() != prf_evidence.is_some() {
            return Err(GovernancePublishError::other(
                "privacy release PRF evidence does not match the governed mode",
            ));
        }

        let publication_payload_digest = *blake3::hash(&encoded).as_bytes();
        let published_aggregate_inventory_digest =
            privacy_published_aggregate_inventory_digest(&publication.privacy_aggregates)?;
        let publication_block_hash = publication.block.block_hash().map_err(|err| {
            GovernancePublishError::other(format!("hash privacy publication block: {err}"))
        })?;
        let mut next_release_ledger = previous_release_ledger.clone();
        let (prf_request_binding, prf_commitment) = prf_evidence
            .map_or((None, None), |(binding, commitment)| {
                (Some(binding), Some(commitment))
            });
        next_release_ledger
            .append(transparency::PrivacyReleaseRecordV1 {
                sequence: 0,
                release_id: cycle_id,
                query_id: config.query_id,
                first_cycle_start_unix: config.first_cycle_start_unix,
                cycle_seconds: config.cycle_seconds,
                publish_delay_seconds: window
                    .due_at_unix
                    .checked_sub(window.cycle_end_unix)
                    .ok_or_else(|| {
                        GovernancePublishError::other(
                            "privacy publication due timestamp precedes its release window",
                        )
                    })?,
                cycle_start_unix: window.cycle_start_unix,
                cycle_end_unix: window.cycle_end_unix,
                due_at_unix: window.due_at_unix,
                private_source_digest,
                policy_digest: config.policy_digest,
                population_inventory_digest: privacy_population_inventory_digest(
                    &config.populations,
                ),
                metric_schema_digest: privacy_metric_schema_digest(&config.metrics),
                privacy: config.privacy,
                prf_request_binding,
                prf_commitment,
                budget_charge_digest: budget_charge.map(|charge| charge.charge_digest),
                publication_payload_digest: Some(publication_payload_digest),
                published_aggregate_inventory_digest: Some(published_aggregate_inventory_digest),
                previous_publication_block_hash: publication.block.previous_block_hash,
                publication_block_hash: Some(publication_block_hash),
                status: transparency::PrivacyReleaseStatusV1::Published,
                previous_record_digest: None,
                record_digest: [0; 32],
            })
            .map_err(|error| {
                GovernancePublishError::other(format!(
                    "append durable privacy release record: {error}"
                ))
            })?;

        let mut next_published = previous_published.clone();
        if !next_published.insert(cycle_id) {
            return Err(GovernancePublishError::other(
                "privacy aggregate cycle was already committed",
            ));
        }
        let mut next_events = previous_events.clone();
        next_events.retain(|_, event| {
            event.occurred_at_unix < window.cycle_start_unix
                || event.occurred_at_unix >= window.cycle_end_unix
        });
        let mut next_request_receipts = previous_request_receipts.clone();
        if next_request_receipts.len() >= self.config.runtime_retention().state_entry_limit()
            || next_request_receipts
                .insert(request_receipt.idempotency_key.clone(), request_receipt)
                .is_some()
        {
            return Err(GovernancePublishError::other(
                "privacy publish-request receipt retention exhausted or duplicated",
            ));
        }
        let gc_reserved_slots = {
            let intents = self
                .gc_eviction_intents
                .read()
                .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))?;
            gc_eviction_reserved_outbox_slots(&intents)?
        };
        let mut next_outbox = previous_outbox.clone();
        insert_governance_outbox_entry(
            &mut next_outbox,
            GovernanceOutboxKindV1::TransparencyLedgerPublication,
            encoded,
            self.config.runtime_retention().state_entry_limit(),
            gc_reserved_slots,
        )?;
        self.install_privacy_publication_state(
            next_budget,
            next_release_ledger,
            next_published,
            next_events,
            next_request_receipts,
            next_outbox,
        )?;
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            if let Err(rollback_err) = self.install_privacy_publication_state(
                previous_budget,
                previous_release_ledger,
                previous_published,
                previous_events,
                previous_request_receipts,
                previous_outbox,
            ) {
                self.mark_durability_unhealthy(format!(
                    "privacy publication precommit failure `{err}` could not be rolled back: {rollback_err}"
                ));
                return Err(GovernancePublishError::other(format!(
                    "{err}; privacy publication rollback failed: {rollback_err}"
                )));
            }
            return Err(GovernancePublishError::other(err.to_string()));
        }
        drop(checkpoint_guard);
        self.reconcile_privacy_release_anchor()?;
        self.flush_governance_outbox()?;
        Ok(())
    }

    fn install_privacy_publication_state(
        &self,
        budget: PrivacyCompositionBudgetLedgerV1,
        release_ledger: transparency::PrivacyReleaseLedgerV1,
        published: BTreeSet<[u8; 16]>,
        events: BTreeMap<String, PrivacyAggregateSourceEvent>,
        request_receipts: BTreeMap<String, PrivacyPublishRequestReceiptV1>,
        outbox: GovernanceOutboxRuntime,
    ) -> Result<(), GovernancePublishError> {
        let mut budget_guard = self
            .privacy_composition_budget
            .write()
            .map_err(|_| GovernancePublishError::other("privacy composition budget poisoned"))?;
        let mut release_guard = self
            .privacy_release_ledger
            .write()
            .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))?;
        let mut published_guard =
            self.published_privacy_aggregate_cycles
                .write()
                .map_err(|_| {
                    GovernancePublishError::other(
                        "privacy aggregate published-cycle index poisoned",
                    )
                })?;
        let mut events_guard = self
            .privacy_aggregate_source_events
            .write()
            .map_err(|_| GovernancePublishError::other("privacy aggregate event index poisoned"))?;
        let mut outbox_guard = self
            .governance_outbox
            .write()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?;
        let mut request_receipts_guard =
            self.privacy_publish_request_receipts.write().map_err(|_| {
                GovernancePublishError::other("privacy publish-request receipt index poisoned")
            })?;
        *budget_guard = budget;
        *release_guard = release_ledger;
        *published_guard = published;
        *events_guard = events;
        *request_receipts_guard = request_receipts;
        *outbox_guard = outbox;
        Ok(())
    }

    fn commit_processed_privacy_cycle(
        &self,
        cycle_id: [u8; 16],
        window: PrivacyAggregateCycleWindow,
        config: &PrivacyAggregateCycleConfig,
        private_source_digest: [u8; 32],
        previous_publication_block_hash: Option<[u8; 32]>,
        request_receipt: PrivacyPublishRequestReceiptV1,
    ) -> Result<(), GovernancePublishError> {
        let checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        if config.privacy.per_subject_metric_cap.is_some()
            || cycle_id
                != transparency::privacy_aggregate_cycle_id(
                    config.query_id,
                    window.cycle_start_unix,
                    window.cycle_end_unix,
                )
        {
            return Err(GovernancePublishError::other(
                "only suppression-only releases may commit without a publication",
            ));
        }
        request_receipt.validate()?;
        if request_receipt.cycle_id != cycle_id
            || request_receipt.query_id != config.query_id
            || request_receipt.cycle_start_unix != window.cycle_start_unix
            || request_receipt.cycle_end_unix != window.cycle_end_unix
            || request_receipt.due_at_unix != window.due_at_unix
            || !matches!(
                &request_receipt.outcome,
                PrivacyPublishRequestOutcomeV1::AllBucketsSuppressed
            )
        {
            return Err(GovernancePublishError::other(
                "privacy publish-request receipt does not match the suppressed release",
            ));
        }
        let previous_budget = self
            .privacy_composition_budget
            .read()
            .map_err(|_| GovernancePublishError::other("privacy composition budget poisoned"))?
            .clone();
        let previous_release_ledger = self
            .privacy_release_ledger
            .read()
            .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))?
            .clone();
        let previous_published = self
            .published_privacy_aggregate_cycles
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy aggregate published-cycle index poisoned")
            })?
            .clone();
        let previous_events = self
            .privacy_aggregate_source_events
            .read()
            .map_err(|_| GovernancePublishError::other("privacy aggregate event index poisoned"))?
            .clone();
        let previous_request_receipts = self
            .privacy_publish_request_receipts
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy publish-request receipt index poisoned")
            })?
            .clone();
        let previous_outbox = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .clone();

        let mut next_release_ledger = previous_release_ledger.clone();
        next_release_ledger
            .append(transparency::PrivacyReleaseRecordV1 {
                sequence: 0,
                release_id: cycle_id,
                query_id: config.query_id,
                first_cycle_start_unix: config.first_cycle_start_unix,
                cycle_seconds: config.cycle_seconds,
                publish_delay_seconds: window
                    .due_at_unix
                    .checked_sub(window.cycle_end_unix)
                    .ok_or_else(|| {
                        GovernancePublishError::other(
                            "privacy publication due timestamp precedes its release window",
                        )
                    })?,
                cycle_start_unix: window.cycle_start_unix,
                cycle_end_unix: window.cycle_end_unix,
                due_at_unix: window.due_at_unix,
                private_source_digest,
                policy_digest: config.policy_digest,
                population_inventory_digest: privacy_population_inventory_digest(
                    &config.populations,
                ),
                metric_schema_digest: privacy_metric_schema_digest(&config.metrics),
                privacy: config.privacy,
                prf_request_binding: None,
                prf_commitment: None,
                budget_charge_digest: None,
                publication_payload_digest: None,
                published_aggregate_inventory_digest: None,
                previous_publication_block_hash,
                publication_block_hash: None,
                status: transparency::PrivacyReleaseStatusV1::Suppressed,
                previous_record_digest: None,
                record_digest: [0; 32],
            })
            .map_err(|error| {
                GovernancePublishError::other(format!(
                    "append durable suppressed privacy release record: {error}"
                ))
            })?;
        let mut next_published = previous_published.clone();
        if !next_published.insert(cycle_id) {
            return Err(GovernancePublishError::other(
                "privacy aggregate cycle was already committed",
            ));
        }
        let mut next_events = previous_events.clone();
        next_events.retain(|_, event| {
            event.occurred_at_unix < window.cycle_start_unix
                || event.occurred_at_unix >= window.cycle_end_unix
        });
        let mut next_request_receipts = previous_request_receipts.clone();
        if next_request_receipts.len() >= self.config.runtime_retention().state_entry_limit()
            || next_request_receipts
                .insert(request_receipt.idempotency_key.clone(), request_receipt)
                .is_some()
        {
            return Err(GovernancePublishError::other(
                "privacy publish-request receipt retention exhausted or duplicated",
            ));
        }
        self.install_privacy_publication_state(
            previous_budget.clone(),
            next_release_ledger,
            next_published,
            next_events,
            next_request_receipts,
            previous_outbox.clone(),
        )?;
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            self.install_privacy_publication_state(
                previous_budget,
                previous_release_ledger,
                previous_published,
                previous_events,
                previous_request_receipts,
                previous_outbox,
            )?;
            return Err(GovernancePublishError::other(err.to_string()));
        }
        drop(checkpoint_guard);
        self.reconcile_privacy_release_anchor()?;
        Ok(())
    }

    /// Publish the next due privacy aggregate cycle using storage configuration.
    ///
    /// The complete public policy and composition budget come from
    /// `iroha_config`; hidden cycle randomness comes exclusively from the
    /// runtime threshold-PRF provider. The predecessor and trusted evaluation
    /// time are derived inside the node; callers supply only the expected
    /// direct-successor cycle and a bounded idempotency key.
    pub fn publish_due_configured_privacy_aggregate_cycle_from_source_events(
        &self,
        expected_cycle_id: [u8; 16],
        idempotency_key: String,
    ) -> Result<PrivacyAggregateScheduleOutcome, GovernancePublishError> {
        let Some(schedule) = self.configured_privacy_aggregate_schedule() else {
            return Ok(PrivacyAggregateScheduleOutcome::Disabled);
        };
        let (config, composition_budget) = {
            let policy = self.config.privacy_aggregate_policy().ok_or_else(|| {
                GovernancePublishError::other(
                    "enabled privacy aggregate scheduler is missing its governed policy",
                )
            })?;
            (policy.cycle_config(), policy.composition_budget())
        };
        self.publish_due_privacy_aggregate_cycle_from_source_events(
            unix_now_secs(),
            expected_cycle_id,
            idempotency_key,
            schedule,
            config,
            Some(composition_budget),
        )
    }

    /// Publish a typed SoraFS weekly appeal finance rollup to the governance pipeline.
    pub fn publish_appeal_finance_weekly_rollup(
        &self,
        rollup: SoraFsAppealFinanceWeeklyRollupV1,
    ) -> Result<(), GovernancePublishError> {
        rollup.validate().map_err(|err| {
            GovernancePublishError::other(format!("invalid appeal finance weekly rollup: {err}"))
        })?;
        let encoded = norito::to_bytes(&rollup).map_err(|err| {
            GovernancePublishError::other(format!("encode appeal finance weekly rollup: {err}"))
        })?;
        self.enqueue_governance_outbox(GovernanceOutboxKindV1::AppealFinanceWeeklyRollup, encoded)?;
        self.flush_governance_outbox()?;
        Ok(())
    }

    /// Publish a typed SoraFS appeal finance settlement receipt to the governance pipeline.
    pub fn publish_appeal_finance_settlement_receipt(
        &self,
        receipt: SoraFsAppealFinanceSettlementReceiptV1,
    ) -> Result<(), GovernancePublishError> {
        receipt.validate().map_err(|err| {
            GovernancePublishError::other(format!(
                "invalid appeal finance settlement receipt: {err}"
            ))
        })?;
        let encoded = norito::to_bytes(&receipt).map_err(|err| {
            GovernancePublishError::other(format!(
                "encode appeal finance settlement receipt: {err}"
            ))
        })?;
        let source_entry = transparency::appeal_finance_settlement_receipt_source_entry(&receipt)
            .map_err(|err| {
            GovernancePublishError::other(format!(
                "derive appeal finance settlement receipt transparency source entry: {err}"
            ))
        })?;
        self.enqueue_governance_outbox_with_transparency_entry(
            GovernanceOutboxKindV1::AppealFinanceSettlementReceipt,
            encoded,
            source_entry,
        )?;
        self.flush_governance_outbox()?;
        Ok(())
    }

    /// Return the latest reputation snapshot accepted by this node.
    #[must_use]
    pub fn latest_reputation_snapshot(&self) -> Option<ReputationSnapshotV1> {
        self.latest_reputation_snapshot
            .read()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// Return a published reputation snapshot by snapshot identifier.
    #[must_use]
    pub fn reputation_snapshot(&self, snapshot_id: [u8; 16]) -> Option<ReputationSnapshotV1> {
        self.reputation_snapshots.read().ok().and_then(|guard| {
            guard
                .get(&snapshot_id)
                .map(|admitted| admitted.envelope.snapshot.clone())
        })
    }

    /// Return the latest full signed reputation envelope and scoring evidence.
    #[must_use]
    pub fn latest_signed_reputation_snapshot(&self) -> Option<SignedReputationSnapshotV1> {
        let snapshot_id = self
            .latest_reputation_snapshot
            .read()
            .ok()
            .and_then(|guard| guard.as_ref().map(|snapshot| snapshot.snapshot_id))?;
        self.signed_reputation_snapshot(snapshot_id)
    }

    /// Return a retained full signed reputation envelope by snapshot identifier.
    #[must_use]
    pub fn signed_reputation_snapshot(
        &self,
        snapshot_id: [u8; 16],
    ) -> Option<SignedReputationSnapshotV1> {
        self.reputation_snapshots.read().ok().and_then(|guard| {
            guard
                .get(&snapshot_id)
                .map(|admitted| admitted.envelope.clone())
        })
    }

    /// Return reputation snapshot events after `since_sequence`, capped by `limit`.
    #[must_use]
    pub fn reputation_events_since(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
    ) -> Vec<ReputationSnapshotEventV1> {
        self.reputation_events_replay(since_sequence, limit).events
    }

    /// Return a gap-aware bounded replay of reputation snapshot events.
    #[must_use]
    pub fn reputation_events_replay(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
    ) -> EventReplay<ReputationSnapshotEventV1> {
        self.reputation_events.read().map_or_else(
            |_| EventReplay {
                oldest_available_sequence: None,
                latest_sequence: None,
                gap: false,
                events: Vec::new(),
            },
            |guard| guard.replay(since_sequence, limit, |event| event.sequence),
        )
    }

    /// Return the latest reputation snapshot event sequence accepted by this node.
    #[must_use]
    pub fn latest_reputation_event_sequence(&self) -> Option<u64> {
        self.reputation_events
            .read()
            .ok()
            .and_then(|guard| (guard.latest_sequence != 0).then_some(guard.latest_sequence))
    }

    /// Subscribe to live reputation snapshot publication events.
    #[must_use]
    pub fn subscribe_reputation_events(&self) -> broadcast::Receiver<ReputationSnapshotEventV1> {
        self.reputation_event_sender.subscribe()
    }

    /// Admit a governance-signed moderation reproducibility manifest into the local registry.
    ///
    /// The manifest is validated with the canonical data-model validator before
    /// it is recorded. Re-admitting the same manifest id is idempotent when the
    /// recorded summary matches and fails closed when the summary conflicts.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails, the registry lock is poisoned, or
    /// the manifest id conflicts with a previously admitted record.
    pub fn admit_moderation_repro_manifest(
        &self,
        manifest: ModerationReproManifestV1,
    ) -> Result<ModerationReproRegistryRecord, ModerationModelRegistryError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationModelRegistryError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationModelRegistryError::Checkpoint { message })?;
        let mut registry = self
            .moderation_model_registry
            .write()
            .map_err(|_| ModerationModelRegistryError::StateLockPoisoned)?;
        let previous = registry.snapshot();
        let record = registry.admit_repro_manifest(manifest)?;
        let committed = registry.snapshot();
        if let Err(err) = self.persist_moderation_model_registry_snapshot(&committed) {
            if err.committed {
                return Err(ModerationModelRegistryError::Checkpoint {
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = registry.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation model registry checkpoint failure",
                    rollback,
                );
                return Err(ModerationModelRegistryError::Checkpoint { message });
            }
            return Err(ModerationModelRegistryError::Checkpoint {
                message: err.to_string(),
            });
        }
        Ok(record)
    }

    /// Admit an adversarial corpus manifest into the local moderation registry.
    ///
    /// The corpus is validated and keyed by the BLAKE3 digest of its canonical
    /// Norito bytes. Re-admission of the same corpus digest is idempotent.
    ///
    /// # Errors
    ///
    /// Returns an error if validation, canonical encoding, or registry locking fails.
    pub fn admit_moderation_corpus_manifest(
        &self,
        manifest: AdversarialCorpusManifestV1,
    ) -> Result<ModerationCorpusRegistryRecord, ModerationModelRegistryError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationModelRegistryError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationModelRegistryError::Checkpoint { message })?;
        let mut registry = self
            .moderation_model_registry
            .write()
            .map_err(|_| ModerationModelRegistryError::StateLockPoisoned)?;
        let previous = registry.snapshot();
        let record = registry.admit_corpus_manifest(manifest)?;
        let committed = registry.snapshot();
        if let Err(err) = self.persist_moderation_model_registry_snapshot(&committed) {
            if err.committed {
                return Err(ModerationModelRegistryError::Checkpoint {
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = registry.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation model registry checkpoint failure",
                    rollback,
                );
                return Err(ModerationModelRegistryError::Checkpoint { message });
            }
            return Err(ModerationModelRegistryError::Checkpoint {
                message: err.to_string(),
            });
        }
        Ok(record)
    }

    /// Return a deterministic snapshot of the local moderation model registry.
    ///
    /// If the registry lock is poisoned, an empty snapshot is returned so callers
    /// can treat the local registry as unavailable without panicking.
    #[must_use]
    pub fn moderation_model_registry_snapshot(&self) -> ModerationModelRegistrySnapshot {
        self.moderation_model_registry.read().map_or_else(
            |_| ModerationModelRegistrySnapshot::default(),
            |guard| guard.snapshot(),
        )
    }

    /// Export a deterministic snapshot of the local moderation model registry.
    ///
    /// Unlike [`Self::moderation_model_registry_snapshot`], this method reports
    /// lock poisoning so callers can distinguish an unavailable registry from an
    /// empty registry.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry lock is poisoned.
    pub fn export_moderation_model_registry_snapshot(
        &self,
    ) -> Result<ModerationModelRegistrySnapshot, ModerationModelRegistryError> {
        Ok(self
            .moderation_model_registry
            .read()
            .map_err(|_| ModerationModelRegistryError::StateLockPoisoned)?
            .snapshot())
    }

    /// Replace the local moderation model registry from a validated snapshot.
    ///
    /// The snapshot is duplicate-checked before it replaces local state, then it
    /// is persisted to the local checkpoint when SoraFS storage is enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot is internally inconsistent or the
    /// registry lock is poisoned.
    pub fn restore_moderation_model_registry_snapshot(
        &self,
        snapshot: ModerationModelRegistrySnapshot,
    ) -> Result<(), ModerationModelRegistryError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationModelRegistryError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationModelRegistryError::Checkpoint { message })?;
        let mut registry = self
            .moderation_model_registry
            .write()
            .map_err(|_| ModerationModelRegistryError::StateLockPoisoned)?;
        let previous = registry.snapshot();
        registry.restore_snapshot(snapshot)?;
        let committed = registry.snapshot();
        if let Err(err) = self.persist_moderation_model_registry_snapshot(&committed) {
            if err.committed {
                return Err(ModerationModelRegistryError::Checkpoint {
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = registry.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation model registry snapshot failure",
                    rollback,
                );
                return Err(ModerationModelRegistryError::Checkpoint { message });
            }
            return Err(ModerationModelRegistryError::Checkpoint {
                message: err.to_string(),
            });
        }
        Ok(())
    }

    /// Install or rotate the active externally anchored screening authority.
    ///
    /// Reinstalling the byte-identical authority is idempotent. Policies with
    /// an older issue timestamp, or a different digest at the same timestamp,
    /// are rejected to prevent in-process rollback/equivocation. Operators must
    /// reconstruct this non-secret authority from canonical `iroha_config`
    /// inputs after every restart; it is never accepted from an HTTP request.
    ///
    /// # Errors
    ///
    /// Returns an error if the authority lock is poisoned or the candidate
    /// policy rolls back/equivocates against the active policy.
    pub fn install_moderation_screening_authority(
        &self,
        authority: ModerationScreeningAuthorityV1,
    ) -> Result<(), ModerationScreeningAuthenticationError> {
        let mut active = self.moderation_screening_authority.write().map_err(|_| {
            ModerationScreeningAuthenticationError::AuthorityUnavailable {
                message: "active authority lock is poisoned".to_owned(),
            }
        })?;
        if let Some(current) = active.as_ref() {
            if authority.policy_issued_at_unix() < current.policy_issued_at_unix() {
                return Err(ModerationScreeningAuthenticationError::PolicyRollback {
                    active_issued_at_unix: current.policy_issued_at_unix(),
                    candidate_issued_at_unix: authority.policy_issued_at_unix(),
                });
            }
            if authority.policy_issued_at_unix() == current.policy_issued_at_unix()
                && authority.policy_digest() != current.policy_digest()
            {
                return Err(ModerationScreeningAuthenticationError::PolicyEquivocation {
                    issued_at_unix: authority.policy_issued_at_unix(),
                });
            }
            if current == &authority {
                return Ok(());
            }
        }
        *active = Some(authority);
        Ok(())
    }

    /// Return whether a validated config-authoritative screening bundle is active.
    #[must_use]
    pub fn has_moderation_screening_authority(&self) -> bool {
        self.moderation_screening_authority
            .read()
            .is_ok_and(|authority| authority.is_some())
    }

    /// Return whether authenticated moderation screening was enabled by the
    /// immutable node configuration used at startup.
    #[must_use]
    pub fn moderation_screening_enabled(&self) -> bool {
        self.config.moderation_screening_enabled()
    }

    /// Return the reviewed digest of the authority bundle loaded at startup.
    #[must_use]
    pub fn moderation_screening_authority_bundle_digest(&self) -> Option<[u8; 32]> {
        self.config.moderation_screening_authority_bundle_digest()
    }

    /// Return the active non-secret PKCS#11/KMS quarantine key handle.
    ///
    /// The wrapping provider and its credentials remain runtime-only.
    #[must_use]
    pub fn moderation_quarantine_key_id(&self) -> Option<&str> {
        self.moderation_quarantine_key_wrapper
            .as_deref()
            .map(ModerationQuarantineKeyWrapper::active_key_id)
    }

    /// Return whether `candidate` is the exact runtime wrapper retained by this
    /// node handle.
    #[must_use]
    pub fn uses_moderation_quarantine_key_wrapper(
        &self,
        candidate: &Arc<dyn ModerationQuarantineKeyWrapper>,
    ) -> bool {
        self.moderation_quarantine_key_wrapper
            .as_ref()
            .is_some_and(|active| Arc::ptr_eq(&active.0, candidate))
    }

    /// Return whether the configured privacy policy requires threshold-PRF
    /// cycle outputs.
    #[must_use]
    pub fn privacy_cycle_prf_required(&self) -> bool {
        self.config.privacy_aggregate_schedule().is_some()
            && self
                .config
                .privacy_aggregate_policy()
                .is_some_and(config::PrivacyAggregatePolicyConfig::requires_cycle_prf)
    }

    /// Return whether `candidate` is the exact runtime threshold-PRF provider
    /// retained by this node.
    #[must_use]
    pub fn uses_privacy_cycle_prf_provider(
        &self,
        candidate: &Arc<dyn PrivacyCyclePrfProviderV1>,
    ) -> bool {
        self.privacy_cycle_prf_provider
            .as_ref()
            .is_some_and(|active| Arc::ptr_eq(&active.0, candidate))
    }

    /// Return whether `candidate` is the exact finalized privacy-release
    /// anchor retained by this node.
    #[must_use]
    pub fn uses_privacy_release_anchor(&self, candidate: &Arc<dyn PrivacyReleaseAnchorV1>) -> bool {
        self.privacy_release_anchor
            .as_ref()
            .is_some_and(|active| Arc::ptr_eq(&active.0, candidate))
    }

    /// Authenticate and durably record one governed moderation screening result.
    ///
    /// The result must be either a canonical runner-signed result under a
    /// single-signer policy or an exact committee aggregate reconstructed from
    /// its bounded signed member inventory. The durable snapshot atomically
    /// binds the idempotency key, authenticated authority digest, and resulting
    /// screening record so replay under a different key fails closed.
    ///
    /// # Errors
    ///
    /// Returns an error if governance signatures, runner authorization,
    /// subject/evidence bindings, score/verdict consistency, freshness,
    /// revocation, committee uniqueness/quorum, or replay validation fails, or
    /// if the authenticated projection cannot be durably committed.
    pub fn record_authenticated_moderation_screening_result(
        &self,
        request: ModerationAuthenticatedScreeningRequestV1,
        now_unix: u64,
    ) -> Result<
        ModerationAuthenticatedScreeningOutcomeV1,
        ModerationAuthenticatedScreeningAdmissionError,
    > {
        let authority = self
            .moderation_screening_authority
            .read()
            .map_err(
                |_| ModerationScreeningAuthenticationError::AuthorityUnavailable {
                    message: "active authority lock is poisoned".to_owned(),
                },
            )?
            .clone()
            .ok_or_else(
                || ModerationScreeningAuthenticationError::AuthorityUnavailable {
                    message: "no active governed manifest/policy/anchor bundle is installed"
                        .to_owned(),
                },
            )?;
        let verified = authority.verify(request, now_unix)?;
        self.commit_authenticated_moderation_screening(verified)
    }

    fn commit_authenticated_moderation_screening(
        &self,
        verified: moderation::ModerationVerifiedScreeningAdmissionV1,
    ) -> Result<
        ModerationAuthenticatedScreeningOutcomeV1,
        ModerationAuthenticatedScreeningAdmissionError,
    > {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationScreeningError::Checkpoint { message })?;
        let mut runtime = self
            .moderation_screening
            .write()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        let previous = runtime.snapshot();
        let outcome = runtime.record_authenticated_screening(verified)?;
        let committed = runtime.snapshot();
        if let Err(err) = self.persist_moderation_screening_snapshot(&committed) {
            if err.committed {
                return Err(ModerationScreeningError::Checkpoint {
                    message: err.to_string(),
                }
                .into());
            }
            if let Err(rollback) = runtime.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back authenticated moderation screening checkpoint failure",
                    rollback,
                );
                return Err(ModerationScreeningError::Checkpoint { message }.into());
            }
            return Err(ModerationScreeningError::Checkpoint {
                message: err.to_string(),
            }
            .into());
        }
        Ok(outcome)
    }

    /// Record one deterministic local moderation screening projection.
    ///
    /// This unsigned projection hook exists for tests and development tooling.
    /// Production request handlers must use
    /// [`Self::record_authenticated_moderation_screening_result`].
    ///
    /// `quarantine` and `escalate` verdicts also create a pending quarantine
    /// queue record. Successful updates are persisted to the local checkpoint
    /// when SoraFS storage is enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is invalid, the screening id conflicts
    /// with existing local state, or the runtime lock is poisoned.
    #[cfg(test)]
    fn record_moderation_screening_result(
        &self,
        input: ModerationScreeningInput,
    ) -> Result<ModerationScreeningOutcome, ModerationScreeningError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationScreeningError::Checkpoint { message })?;
        let mut runtime = self
            .moderation_screening
            .write()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        let previous = runtime.snapshot();
        let outcome = runtime.record_screening(input)?;
        let committed = runtime.snapshot();
        if let Err(err) = self.persist_moderation_screening_snapshot(&committed) {
            if err.committed {
                return Err(ModerationScreeningError::Checkpoint {
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = runtime.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation screening checkpoint failure",
                    rollback,
                );
                return Err(ModerationScreeningError::Checkpoint { message });
            }
            return Err(ModerationScreeningError::Checkpoint {
                message: err.to_string(),
            });
        }
        Ok(outcome)
    }

    /// Mark a pending local quarantine record as reviewed.
    ///
    /// Successful updates are persisted to the local checkpoint when SoraFS
    /// storage is enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the quarantine id is unknown, the transition is
    /// invalid, or the runtime lock is poisoned.
    pub fn review_moderation_quarantine_record(
        &self,
        input: ModerationQuarantineReviewInput,
    ) -> Result<ModerationQuarantineRecord, ModerationScreeningError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationScreeningError::Checkpoint { message })?;
        let mut runtime = self
            .moderation_screening
            .write()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        let previous = runtime.snapshot();
        let record = runtime.review_quarantine(input)?;
        let committed = runtime.snapshot();
        if let Err(err) = self.persist_moderation_screening_snapshot(&committed) {
            if err.committed {
                return Err(ModerationScreeningError::Checkpoint {
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = runtime.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation quarantine review checkpoint failure",
                    rollback,
                );
                return Err(ModerationScreeningError::Checkpoint { message });
            }
            return Err(ModerationScreeningError::Checkpoint {
                message: err.to_string(),
            });
        }
        Ok(record)
    }

    /// Release a reviewed local quarantine record.
    ///
    /// Successful updates are persisted to the local checkpoint when SoraFS
    /// storage is enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the quarantine id is unknown, the record has not
    /// been reviewed, the transition is invalid, or the runtime lock is
    /// poisoned.
    pub fn release_moderation_quarantine_record(
        &self,
        input: ModerationQuarantineReleaseInput,
    ) -> Result<ModerationQuarantineRecord, ModerationScreeningError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationScreeningError::Checkpoint { message })?;
        let mut runtime = self
            .moderation_screening
            .write()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        let previous = runtime.snapshot();
        let record = runtime.release_quarantine(input)?;
        let committed = runtime.snapshot();
        if let Err(err) = self.persist_moderation_screening_snapshot(&committed) {
            if err.committed {
                return Err(ModerationScreeningError::Checkpoint {
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = runtime.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation quarantine release checkpoint failure",
                    rollback,
                );
                return Err(ModerationScreeningError::Checkpoint { message });
            }
            return Err(ModerationScreeningError::Checkpoint {
                message: err.to_string(),
            });
        }
        Ok(record)
    }

    /// Return a deterministic snapshot of local screening and quarantine state.
    #[must_use]
    pub fn moderation_screening_snapshot(&self) -> ModerationScreeningSnapshot {
        self.moderation_screening.read().map_or_else(
            |_| ModerationScreeningSnapshot::default(),
            |guard| guard.snapshot(),
        )
    }

    /// Export local screening and quarantine state, reporting lock failures.
    ///
    /// # Errors
    ///
    /// Returns an error if the runtime lock is poisoned.
    pub fn export_moderation_screening_snapshot(
        &self,
    ) -> Result<ModerationScreeningSnapshot, ModerationScreeningError> {
        Ok(self
            .moderation_screening
            .read()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?
            .snapshot())
    }

    /// Replace local screening/quarantine state from a validated snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot is internally inconsistent or the
    /// runtime lock is poisoned.
    pub fn restore_moderation_screening_snapshot(
        &self,
        snapshot: ModerationScreeningSnapshot,
    ) -> Result<(), ModerationScreeningError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationScreeningError::Checkpoint { message })?;
        self.validate_moderation_screening_snapshot_downstream_refs(&snapshot)?;
        let mut runtime = self
            .moderation_screening
            .write()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?;
        let previous = runtime.snapshot();
        runtime.restore_snapshot(snapshot)?;
        let committed = runtime.snapshot();
        if let Err(err) = self.persist_moderation_screening_snapshot(&committed) {
            if err.committed {
                return Err(ModerationScreeningError::Checkpoint {
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = runtime.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation screening snapshot failure",
                    rollback,
                );
                return Err(ModerationScreeningError::Checkpoint { message });
            }
            return Err(ModerationScreeningError::Checkpoint {
                message: err.to_string(),
            });
        }
        Ok(())
    }

    /// Seal and store quarantined payload bytes in the local encrypted object store.
    ///
    /// The plaintext BLAKE3 digest must match the referenced quarantine record
    /// subject digest. Successful writes persist an encrypted Norito envelope
    /// and update the local object index checkpoint.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is disabled, the quarantine id is unknown,
    /// the payload digest does not match the quarantine record, encryption or
    /// filesystem persistence fails, or the object index lock is poisoned.
    pub fn store_moderation_quarantine_object(
        &self,
        input: ModerationQuarantineObjectInput,
    ) -> Result<ModerationQuarantineObjectRecord, ModerationQuarantineObjectError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?;
        self.ensure_durability_healthy().map_err(|message| {
            ModerationQuarantineObjectError::Io {
                path: "durability-state".to_owned(),
                message,
            }
        })?;
        let root = self
            .moderation_quarantine_object_root
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::StorageDisabled)?;
        let input = normalize_moderation_quarantine_object_input(input)?;
        let quarantine = self.moderation_quarantine_record_for_object(&input.quarantine_id)?;
        let payload_digest = *blake3::hash(&input.payload).as_bytes();
        if payload_digest != quarantine.subject_digest {
            return Err(ModerationQuarantineObjectError::DigestMismatch {
                quarantine_id_hex: hex::encode(input.quarantine_id),
                expected_digest_hex: hex::encode(quarantine.subject_digest),
                actual_digest_hex: hex::encode(payload_digest),
            });
        }
        let key_wrapper = self
            .moderation_quarantine_key_wrapper
            .as_deref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnavailable)?;

        let mut objects = self
            .moderation_quarantine_objects
            .write()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?;
        let previous = objects.snapshot();
        if let Some(existing) = objects.get(&input.quarantine_id) {
            let (_, existing_envelope, _) =
                self.read_moderation_quarantine_object_envelope(root, &existing)?;
            let plaintext =
                open_moderation_quarantine_object(&existing_envelope, &existing, key_wrapper)?;
            if existing.payload_digest != payload_digest
                || existing.captured_at_unix != input.captured_at_unix
                || existing.content_type.as_deref() != input.content_type.as_deref()
                || existing.notes.as_deref() != input.notes.as_deref()
                || plaintext.as_slice() != input.payload.as_slice()
            {
                return Err(ModerationQuarantineObjectError::ConflictingObject {
                    quarantine_id_hex: hex::encode(input.quarantine_id),
                });
            }
            return Ok(existing);
        }
        objects.ensure_insert_capacity(&input.quarantine_id)?;
        let (record, envelope_bytes) = seal_moderation_quarantine_object(input, key_wrapper)?;
        let envelope_path = self.resolve_moderation_quarantine_object_path(root, &record)?;
        match fs::symlink_metadata(&envelope_path) {
            Ok(_) => {
                return Err(ModerationQuarantineObjectError::ConflictingObject {
                    quarantine_id_hex: hex::encode(record.quarantine_id),
                });
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => {
                return Err(ModerationQuarantineObjectError::Io {
                    path: envelope_path.display().to_string(),
                    message: err.to_string(),
                });
            }
        }

        self.finish_local_checkpoint_write(
            "moderation quarantine object envelope",
            &envelope_path,
            write_local_checkpoint_atomic_bounded(
                &envelope_path,
                &envelope_bytes,
                self.config.runtime_retention().checkpoint_max_bytes(),
            ),
        )
        .map_err(|err| ModerationQuarantineObjectError::Io {
            path: envelope_path.display().to_string(),
            message: err.to_string(),
        })?;

        let stored = match objects.insert(record) {
            Ok(stored) => stored,
            Err(err) => {
                if let Err(cleanup) = remove_local_checkpoint_file_durably(&envelope_path) {
                    let message = format!(
                        "failed to remove quarantine envelope after rejected index insertion: {cleanup}"
                    );
                    self.mark_durability_unhealthy(message.clone());
                    return Err(ModerationQuarantineObjectError::Io {
                        path: envelope_path.display().to_string(),
                        message,
                    });
                }
                return Err(err);
            }
        };
        let committed = objects.snapshot();
        if let Err(err) = self.persist_moderation_quarantine_object_index_snapshot(&committed) {
            if err.committed {
                return Err(ModerationQuarantineObjectError::Io {
                    path: "durability-state".to_owned(),
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = objects.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation quarantine object index checkpoint failure",
                    rollback,
                );
                return Err(ModerationQuarantineObjectError::Io {
                    path: "durability-state".to_owned(),
                    message,
                });
            }
            if let Err(cleanup) = remove_local_checkpoint_file_durably(&envelope_path) {
                let message = self.record_unrecoverable_rollback(
                    "failed to remove quarantine envelope after index checkpoint failure",
                    cleanup,
                );
                return Err(ModerationQuarantineObjectError::Io {
                    path: envelope_path.display().to_string(),
                    message,
                });
            }
            return Err(ModerationQuarantineObjectError::Io {
                path: "durability-state".to_owned(),
                message: err.to_string(),
            });
        }
        Ok(stored)
    }

    /// Read and decrypt a local quarantine payload object.
    ///
    /// # Errors
    ///
    /// Returns an error if storage is disabled, the quarantine/object record is
    /// missing, the envelope cannot be read or decoded, authentication fails,
    /// or the decrypted payload no longer matches the quarantine record digest.
    pub fn read_moderation_quarantine_object(
        &self,
        quarantine_id: [u8; 16],
    ) -> Result<ModerationQuarantineObjectPayload, ModerationQuarantineObjectError> {
        let root = self
            .moderation_quarantine_object_root
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::StorageDisabled)?;
        let key_wrapper = self
            .moderation_quarantine_key_wrapper
            .as_deref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnavailable)?;
        let quarantine = self.moderation_quarantine_record_for_object(&quarantine_id)?;
        let record = self
            .moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .get(&quarantine_id)
            .ok_or_else(|| ModerationQuarantineObjectError::MissingObject {
                quarantine_id_hex: hex::encode(quarantine_id),
            })?;
        let (_, envelope, _) = self.read_moderation_quarantine_object_envelope(root, &record)?;
        let payload = open_moderation_quarantine_object(&envelope, &record, key_wrapper)?;
        if *blake3::hash(&payload).as_bytes() != quarantine.subject_digest {
            return Err(ModerationQuarantineObjectError::AuthenticationFailed {
                quarantine_id_hex: hex::encode(quarantine_id),
            });
        }
        Ok(ModerationQuarantineObjectPayload { record, payload })
    }

    /// Read and authenticate an inclusive-exclusive plaintext byte range.
    ///
    /// Only ciphertext chunks intersecting `start..end` are decrypted. Each
    /// returned byte is independently authenticated against the immutable
    /// object metadata, chunk index, offset, and length.
    ///
    /// # Errors
    ///
    /// Returns an error if the range is invalid, storage or the runtime
    /// PKCS#11/KMS wrapper is unavailable, the object is missing, or any
    /// envelope/chunk authentication check fails.
    pub fn read_moderation_quarantine_object_range(
        &self,
        quarantine_id: [u8; 16],
        start: u64,
        end: u64,
    ) -> Result<ModerationQuarantineObjectRangePayload, ModerationQuarantineObjectError> {
        let root = self
            .moderation_quarantine_object_root
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::StorageDisabled)?;
        let key_wrapper = self
            .moderation_quarantine_key_wrapper
            .as_deref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnavailable)?;
        self.moderation_quarantine_record_for_object(&quarantine_id)?;
        let record = self
            .moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .get(&quarantine_id)
            .ok_or_else(|| ModerationQuarantineObjectError::MissingObject {
                quarantine_id_hex: hex::encode(quarantine_id),
            })?;
        let (_, envelope, _) = self.read_moderation_quarantine_object_envelope(root, &record)?;
        let payload =
            open_moderation_quarantine_object_range(&envelope, &record, key_wrapper, start..end)?;
        Ok(ModerationQuarantineObjectRangePayload {
            record,
            start,
            end,
            payload,
        })
    }

    /// Rewrap one object's DEK under the wrapper's current active key.
    ///
    /// The injected wrapper must remain able to unwrap the historical key
    /// handle stored in the object envelope. Ciphertext chunks, object id, and
    /// the durable index stay byte-identical; only the context-bound wrapped
    /// DEK and its non-secret key handle are atomically replaced.
    ///
    /// # Errors
    ///
    /// Returns an error if storage or the runtime PKCS#11/KMS wrapper is
    /// unavailable, the object is missing, old/new key operations fail, the
    /// replacement cannot be authenticated, or the atomic write fails.
    pub fn rewrap_moderation_quarantine_object_dek(
        &self,
        quarantine_id: [u8; 16],
    ) -> Result<ModerationQuarantineObjectRecord, ModerationQuarantineObjectError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?;
        self.ensure_durability_healthy().map_err(|message| {
            ModerationQuarantineObjectError::Io {
                path: "durability-state".to_owned(),
                message,
            }
        })?;
        let root = self
            .moderation_quarantine_object_root
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::StorageDisabled)?;
        let key_wrapper = self
            .moderation_quarantine_key_wrapper
            .as_deref()
            .ok_or(ModerationQuarantineObjectError::KeyWrapperUnavailable)?;
        self.moderation_quarantine_record_for_object(&quarantine_id)?;
        let record = self
            .moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .get(&quarantine_id)
            .ok_or_else(|| ModerationQuarantineObjectError::MissingObject {
                quarantine_id_hex: hex::encode(quarantine_id),
            })?;
        let (envelope_path, envelope, original_bytes) =
            self.read_moderation_quarantine_object_envelope(root, &record)?;
        let (replacement_record, replacement_bytes) =
            rewrap_moderation_quarantine_object(&envelope, &record, key_wrapper, key_wrapper)?;
        if replacement_record != record {
            return Err(ModerationQuarantineObjectError::InvalidSnapshot {
                message: "DEK rewrap changed immutable object index metadata".to_owned(),
            });
        }
        if replacement_bytes == original_bytes {
            return Ok(record);
        }
        let replacement_envelope =
            norito::decode_from_bytes::<ModerationQuarantineObjectEnvelopeV1>(&replacement_bytes)
                .map_err(|error| ModerationQuarantineObjectError::Codec {
                message: error.to_string(),
            })?;
        open_moderation_quarantine_object(&replacement_envelope, &replacement_record, key_wrapper)?;
        self.finish_local_checkpoint_write(
            "moderation quarantine object DEK rewrap",
            &envelope_path,
            write_local_checkpoint_atomic_bounded(
                &envelope_path,
                &replacement_bytes,
                self.config.runtime_retention().checkpoint_max_bytes(),
            ),
        )
        .map_err(|error| ModerationQuarantineObjectError::Io {
            path: envelope_path.display().to_string(),
            message: error.to_string(),
        })?;
        Ok(record)
    }

    /// Return a deterministic snapshot of the local quarantine object index.
    #[must_use]
    pub fn moderation_quarantine_object_snapshot(&self) -> ModerationQuarantineObjectSnapshot {
        self.moderation_quarantine_objects.read().map_or_else(
            |_| ModerationQuarantineObjectSnapshot::default(),
            |guard| guard.snapshot(),
        )
    }

    /// Export local quarantine object index state, reporting lock failures.
    ///
    /// # Errors
    ///
    /// Returns an error if the object index lock is poisoned.
    pub fn export_moderation_quarantine_object_snapshot(
        &self,
    ) -> Result<ModerationQuarantineObjectSnapshot, ModerationQuarantineObjectError> {
        Ok(self
            .moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .snapshot())
    }

    /// Replace the local quarantine object index from a validated snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot is internally inconsistent, references
    /// an unknown quarantine id, or the object index lock is poisoned.
    pub fn restore_moderation_quarantine_object_snapshot(
        &self,
        snapshot: ModerationQuarantineObjectSnapshot,
    ) -> Result<(), ModerationQuarantineObjectError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?;
        self.ensure_durability_healthy().map_err(|message| {
            ModerationQuarantineObjectError::Io {
                path: "durability-state".to_owned(),
                message,
            }
        })?;
        self.moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .ensure_snapshot_capacity(&snapshot)?;
        self.validate_moderation_quarantine_object_snapshot_refs(&snapshot)?;
        self.validate_moderation_quarantine_snapshot_viewer_refs(&snapshot)?;
        let mut objects = self
            .moderation_quarantine_objects
            .write()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?;
        let previous = objects.snapshot();
        objects.restore_snapshot(snapshot)?;
        let committed = objects.snapshot();
        if let Err(err) = self.persist_moderation_quarantine_object_index_snapshot(&committed) {
            if err.committed {
                return Err(ModerationQuarantineObjectError::Io {
                    path: "durability-state".to_owned(),
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = objects.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation quarantine object index snapshot failure",
                    rollback,
                );
                return Err(ModerationQuarantineObjectError::Io {
                    path: "durability-state".to_owned(),
                    message,
                });
            }
            return Err(ModerationQuarantineObjectError::Io {
                path: "durability-state".to_owned(),
                message: err.to_string(),
            });
        }
        Ok(())
    }

    /// Create or return a payload-free local evidence viewer session record.
    ///
    /// The session is bound to an existing encrypted quarantine object by
    /// object id and payload digest. The request is rejected if it includes raw
    /// evidence, signed URLs, session tokens, watermark secrets, or a session
    /// duration longer than the local short-lived window.
    ///
    /// # Errors
    ///
    /// Returns an error if the quarantine/object reference is unknown, the
    /// session metadata is invalid or conflicts with existing local state, the
    /// checkpoint cannot be persisted, or the state lock is poisoned.
    pub fn create_moderation_evidence_viewer_session(
        &self,
        input: ModerationEvidenceViewerSessionInput,
    ) -> Result<ModerationEvidenceViewerSessionRecord, ModerationEvidenceViewerError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationEvidenceViewerError::Io {
                path: "durability-state".to_owned(),
                message,
            })?;
        let object = self.moderation_quarantine_object_record_for_viewer(&input.quarantine_id)?;
        let mut runtime = self
            .moderation_evidence_viewer
            .write()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)?;
        let previous = runtime.snapshot();
        let record = runtime.create_session(input, &object)?;
        let committed = runtime.snapshot();
        if let Err(err) = self.persist_moderation_evidence_viewer_snapshot(&committed) {
            if err.committed {
                return Err(ModerationEvidenceViewerError::Io {
                    path: "durability-state".to_owned(),
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = runtime.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation evidence viewer checkpoint failure",
                    rollback,
                );
                return Err(ModerationEvidenceViewerError::Io {
                    path: "durability-state".to_owned(),
                    message,
                });
            }
            return Err(ModerationEvidenceViewerError::Io {
                path: "durability-state".to_owned(),
                message: err.to_string(),
            });
        }
        Ok(record)
    }

    /// Append a payload-free local evidence viewer access-log event.
    ///
    /// # Errors
    ///
    /// Returns an error if the session is unknown, the event arrives outside the
    /// active session window, the event carries raw payload/token/body markers,
    /// the checkpoint cannot be persisted, or the state lock is poisoned.
    pub fn record_moderation_evidence_viewer_access(
        &self,
        input: ModerationEvidenceViewerAccessInput,
    ) -> Result<ModerationEvidenceViewerAccessEventRecord, ModerationEvidenceViewerError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationEvidenceViewerError::Io {
                path: "durability-state".to_owned(),
                message,
            })?;
        let mut runtime = self
            .moderation_evidence_viewer
            .write()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)?;
        let previous = runtime.snapshot();
        let record = runtime.record_access(input)?;
        let committed = runtime.snapshot();
        if let Err(err) = self.persist_moderation_evidence_viewer_snapshot(&committed) {
            if err.committed {
                return Err(ModerationEvidenceViewerError::Io {
                    path: "durability-state".to_owned(),
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = runtime.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation evidence viewer access checkpoint failure",
                    rollback,
                );
                return Err(ModerationEvidenceViewerError::Io {
                    path: "durability-state".to_owned(),
                    message,
                });
            }
            return Err(ModerationEvidenceViewerError::Io {
                path: "durability-state".to_owned(),
                message: err.to_string(),
            });
        }
        Ok(record)
    }

    /// Return a deterministic snapshot of local evidence viewer audit state.
    #[must_use]
    pub fn moderation_evidence_viewer_snapshot(&self) -> ModerationEvidenceViewerSnapshot {
        self.moderation_evidence_viewer.read().map_or_else(
            |_| ModerationEvidenceViewerSnapshot::default(),
            |guard| guard.snapshot(),
        )
    }

    /// Export local evidence viewer audit state, reporting lock failures.
    ///
    /// # Errors
    ///
    /// Returns an error if the evidence-viewer runtime lock is poisoned.
    pub fn export_moderation_evidence_viewer_snapshot(
        &self,
    ) -> Result<ModerationEvidenceViewerSnapshot, ModerationEvidenceViewerError> {
        Ok(self
            .moderation_evidence_viewer
            .read()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)?
            .snapshot())
    }

    /// Replace local evidence viewer audit state from a validated snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot is internally inconsistent, references
    /// missing quarantine object state, cannot be persisted, or the state lock is
    /// poisoned.
    pub fn restore_moderation_evidence_viewer_snapshot(
        &self,
        snapshot: ModerationEvidenceViewerSnapshot,
    ) -> Result<(), ModerationEvidenceViewerError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(|message| ModerationEvidenceViewerError::Io {
                path: "durability-state".to_owned(),
                message,
            })?;
        self.validate_moderation_evidence_viewer_snapshot_refs(&snapshot)?;
        let mut runtime = self
            .moderation_evidence_viewer
            .write()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)?;
        let previous = runtime.snapshot();
        runtime.restore_snapshot(snapshot)?;
        let committed = runtime.snapshot();
        if let Err(err) = self.persist_moderation_evidence_viewer_snapshot(&committed) {
            if err.committed {
                return Err(ModerationEvidenceViewerError::Io {
                    path: "durability-state".to_owned(),
                    message: err.to_string(),
                });
            }
            if let Err(rollback) = runtime.restore_snapshot(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation evidence viewer snapshot failure",
                    rollback,
                );
                return Err(ModerationEvidenceViewerError::Io {
                    path: "durability-state".to_owned(),
                    message,
                });
            }
            return Err(ModerationEvidenceViewerError::Io {
                path: "durability-state".to_owned(),
                message: err.to_string(),
            });
        }
        Ok(())
    }

    /// Build a payload-free local evidence-viewer audit report for one closed window.
    ///
    /// # Errors
    ///
    /// Returns an error if the report window is invalid, the input attempts to
    /// carry raw payload material, or the evidence-viewer state lock is poisoned.
    pub fn build_moderation_evidence_viewer_audit_report(
        &self,
        input: ModerationEvidenceViewerAuditReportInput,
    ) -> Result<ModerationEvidenceViewerAuditReport, ModerationEvidenceViewerError> {
        let snapshot = self.export_moderation_evidence_viewer_snapshot()?;
        moderation::moderation_evidence_viewer_audit_report_from_snapshot(input, &snapshot)
    }

    /// Build and record a payload-free evidence-viewer audit report as a transparency source entry.
    ///
    /// The report is recorded into the local transparency source-entry worker
    /// under the `EvidenceAccess` ledger kind. Existing transparency publication
    /// APIs can then include it in a ledger cycle and publish that cycle through
    /// the configured Governance DAG publisher.
    ///
    /// # Errors
    ///
    /// Returns an error if report derivation fails, source-entry derivation
    /// fails, or the transparency source-entry worker rejects the entry.
    pub fn record_moderation_evidence_viewer_audit_report(
        &self,
        input: ModerationEvidenceViewerAuditReportInput,
    ) -> Result<ModerationEvidenceViewerAuditReportOutcome, ModerationEvidenceViewerError> {
        let report = self.build_moderation_evidence_viewer_audit_report(input)?;
        let source_entry = transparency::moderation_evidence_viewer_audit_report_source_entry(
            &report,
        )
        .map_err(|err| ModerationEvidenceViewerError::TransparencyExport {
            message: err.to_string(),
        })?;
        self.record_transparency_ledger_source_entry(source_entry.clone())
            .map_err(|err| ModerationEvidenceViewerError::TransparencyExport {
                message: err.to_string(),
            })?;
        Ok(ModerationEvidenceViewerAuditReportOutcome {
            report,
            source_entry,
        })
    }

    /// Publish the oldest due payload-free evidence-viewer audit report cycle.
    ///
    /// The scheduler derives report windows from local viewer sessions and
    /// access events, records the oldest due report as an `EvidenceAccess`
    /// source entry, then publishes the matching transparency ledger cycle
    /// through the configured governance pipeline. Duplicate cycle publication
    /// is suppressed within the node runtime.
    ///
    /// # Errors
    ///
    /// Returns an error if schedule validation fails, the evidence-viewer state
    /// cannot be read, report/source-entry derivation fails, or publication
    /// through the transparency ledger/Governance DAG pipeline fails.
    pub fn publish_due_moderation_evidence_viewer_audit_report(
        &self,
        now_unix: u64,
        schedule: PrivacyAggregateScheduleConfig,
        report_scope: String,
        policy_digest: Option<[u8; 32]>,
        previous_block_hash: Option<[u8; 32]>,
    ) -> Result<ModerationEvidenceViewerAuditScheduleOutcome, GovernancePublishError> {
        let _mutation_guard = self.runtime_mutation_lock.lock().map_err(|_| {
            GovernancePublishError::other("runtime publication transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        let Some(latest_window) = schedule.due_window(now_unix).map_err(|err| {
            GovernancePublishError::other(format!("evidence viewer audit schedule: {err}"))
        })?
        else {
            return Ok(ModerationEvidenceViewerAuditScheduleOutcome::NotDue);
        };
        let published_cycles = self
            .published_evidence_viewer_audit_cycles
            .read()
            .map_err(|_| {
                GovernancePublishError::other(
                    "evidence viewer audit published-cycle index poisoned",
                )
            })?
            .clone();
        let snapshot = self
            .export_moderation_evidence_viewer_snapshot()
            .map_err(|err| {
                GovernancePublishError::other(format!(
                    "export evidence viewer audit snapshot: {err}"
                ))
            })?;

        let mut due_windows = BTreeSet::new();
        for session in &snapshot.sessions {
            collect_due_evidence_viewer_audit_window(
                &schedule,
                latest_window,
                &published_cycles,
                session.issued_at_unix_ms,
                &mut due_windows,
            )?;
            collect_due_evidence_viewer_audit_window(
                &schedule,
                latest_window,
                &published_cycles,
                session.expires_at_unix_ms.saturating_sub(1),
                &mut due_windows,
            )?;
        }
        for event in &snapshot.access_events {
            collect_due_evidence_viewer_audit_window(
                &schedule,
                latest_window,
                &published_cycles,
                event.event_at_unix_ms,
                &mut due_windows,
            )?;
        }

        if due_windows.is_empty() {
            let cycle_id = evidence_viewer_audit_cycle_id(latest_window);
            if published_cycles.contains(&cycle_id) {
                return Ok(
                    ModerationEvidenceViewerAuditScheduleOutcome::AlreadyPublished {
                        window: latest_window,
                        cycle_id,
                    },
                );
            }
            return Ok(
                ModerationEvidenceViewerAuditScheduleOutcome::NoSourceEvents {
                    window: latest_window,
                    cycle_id,
                },
            );
        }

        for window in due_windows {
            let cycle_id = evidence_viewer_audit_cycle_id(window);
            {
                let published_cycles =
                    self.published_evidence_viewer_audit_cycles
                        .read()
                        .map_err(|_| {
                            GovernancePublishError::other(
                                "evidence viewer audit published-cycle index poisoned",
                            )
                        })?;
                if published_cycles.contains(&cycle_id) {
                    continue;
                }
                let state_limit = self.config.runtime_retention().state_entry_limit();
                if published_cycles.len() >= state_limit {
                    return Err(GovernancePublishError::other(format!(
                        "evidence viewer published-cycle retention exhausted (limit {state_limit})"
                    )));
                }
            }
            let outcome = self
                .record_moderation_evidence_viewer_audit_report(
                    ModerationEvidenceViewerAuditReportInput {
                        report_scope: report_scope.clone(),
                        window_start_unix: window.cycle_start_unix,
                        window_end_unix: window.cycle_end_unix,
                        generated_at_unix: now_unix,
                        policy_digest,
                        raw_evidence_included: false,
                        raw_access_logs_included: false,
                        viewer_accounts_included: false,
                        signed_urls_included: false,
                        session_tokens_included: false,
                        response_bodies_included: false,
                    },
                )
                .map_err(|err| {
                    GovernancePublishError::other(format!(
                        "record evidence viewer audit report: {err}"
                    ))
                })?;
            if outcome.report.session_count == 0 && outcome.report.access_event_count == 0 {
                continue;
            }
            let publication = self.publish_transparency_ledger_cycle_from_source_entries(
                cycle_id,
                window.cycle_start_unix,
                window.cycle_end_unix,
                now_unix,
                previous_block_hash,
            )?;
            let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
                GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
            })?;
            self.ensure_durability_healthy()
                .map_err(GovernancePublishError::other)?;
            self.published_evidence_viewer_audit_cycles
                .write()
                .map_err(|_| {
                    GovernancePublishError::other(
                        "evidence viewer audit published-cycle index poisoned",
                    )
                })?
                .insert(cycle_id);
            if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
                if err.committed {
                    return Err(GovernancePublishError::other(err.to_string()));
                }
                self.published_evidence_viewer_audit_cycles
                    .write()
                    .map_err(|_| {
                        GovernancePublishError::other(
                            "evidence viewer cycle rollback lock poisoned",
                        )
                    })?
                    .remove(&cycle_id);
                return Err(GovernancePublishError::other(err.to_string()));
            }
            return Ok(ModerationEvidenceViewerAuditScheduleOutcome::Published {
                window,
                report: Box::new(outcome.report),
                source_entry: Box::new(outcome.source_entry),
                publication,
            });
        }

        let cycle_id = evidence_viewer_audit_cycle_id(latest_window);
        Ok(
            ModerationEvidenceViewerAuditScheduleOutcome::NoSourceEvents {
                window: latest_window,
                cycle_id,
            },
        )
    }

    /// Publish the next due evidence-viewer audit-report cycle using storage configuration.
    ///
    /// Report scope, policy digest, and previous block hash remain explicit
    /// runtime inputs; persisted config only controls whether scheduled
    /// publication is enabled and which cadence is used.
    pub fn publish_due_configured_moderation_evidence_viewer_audit_report(
        &self,
        now_unix: u64,
        report_scope: String,
        policy_digest: Option<[u8; 32]>,
        previous_block_hash: Option<[u8; 32]>,
    ) -> Result<ModerationEvidenceViewerAuditScheduleOutcome, GovernancePublishError> {
        let Some(schedule) = self.configured_evidence_viewer_audit_schedule() else {
            return Ok(ModerationEvidenceViewerAuditScheduleOutcome::Disabled);
        };
        self.publish_due_moderation_evidence_viewer_audit_report(
            now_unix,
            schedule,
            report_scope,
            policy_digest,
            previous_block_hash,
        )
    }

    fn load_moderation_model_registry_checkpoint(&self) -> Result<(), NodeInitError> {
        let Some(path) = self.moderation_model_registry_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let Some(bytes) = read_local_checkpoint_bounded(
            path,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|err| NodeInitError::checkpoint("moderation model registry", path, err))?
        else {
            return Ok(());
        };
        let retention = self.config.runtime_retention();
        let snapshot = decode_local_checkpoint_canonical::<ModerationModelRegistrySnapshot>(
            &bytes,
            retention.checkpoint_max_bytes(),
            retention
                .state_entry_limit()
                .max(retention.event_history_limit()),
        )
        .map_err(|err| NodeInitError::checkpoint("moderation model registry", path, err))?;
        let repro_count = snapshot.reproducibility_manifests.len();
        let corpus_count = snapshot.adversarial_corpora.len();
        self.moderation_model_registry
            .write()
            .map_err(|_| ModerationModelRegistryError::StateLockPoisoned)
            .and_then(|mut registry| registry.restore_snapshot(snapshot))
            .map_err(|err| NodeInitError::checkpoint("moderation model registry", path, err))?;
        iroha_logger::info!(
            path = %path.display(),
            repro_count,
            corpus_count,
            "restored SoraFS moderation model registry checkpoint"
        );
        Ok(())
    }

    fn persist_moderation_model_registry_snapshot(
        &self,
        snapshot: &ModerationModelRegistrySnapshot,
    ) -> Result<(), RuntimeCheckpointPersistError> {
        let Some(path) = self.moderation_model_registry_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let bytes = norito::to_bytes(snapshot).map_err(|err| {
            RuntimeCheckpointPersistError::precommit(format!(
                "encode moderation model registry checkpoint `{}`: {err}",
                path.display()
            ))
        })?;
        self.finish_local_checkpoint_write(
            "moderation model registry",
            path,
            write_local_checkpoint_atomic_bounded(
                path,
                &bytes,
                self.config.runtime_retention().checkpoint_max_bytes(),
            ),
        )
    }

    fn load_moderation_screening_checkpoint(&self) -> Result<(), NodeInitError> {
        let Some(path) = self.moderation_screening_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let Some(bytes) = read_local_checkpoint_bounded(
            path,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|err| NodeInitError::checkpoint("moderation screening", path, err))?
        else {
            return Ok(());
        };
        let retention = self.config.runtime_retention();
        let snapshot = decode_local_checkpoint_canonical::<ModerationScreeningSnapshot>(
            &bytes,
            retention.checkpoint_max_bytes(),
            retention
                .state_entry_limit()
                .max(retention.event_history_limit()),
        )
        .map_err(|err| NodeInitError::checkpoint("moderation screening", path, err))?;
        let screening_count = snapshot.screening_records.len();
        let quarantine_count = snapshot.quarantine_records.len();
        self.moderation_screening
            .write()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)
            .and_then(|mut runtime| runtime.restore_snapshot(snapshot))
            .map_err(|err| NodeInitError::checkpoint("moderation screening", path, err))?;
        iroha_logger::info!(
            path = %path.display(),
            screening_count,
            quarantine_count,
            "restored SoraFS moderation screening checkpoint"
        );
        Ok(())
    }

    fn persist_moderation_screening_snapshot(
        &self,
        snapshot: &ModerationScreeningSnapshot,
    ) -> Result<(), RuntimeCheckpointPersistError> {
        let Some(path) = self.moderation_screening_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let bytes = norito::to_bytes(snapshot).map_err(|err| {
            RuntimeCheckpointPersistError::precommit(format!(
                "encode moderation screening checkpoint `{}`: {err}",
                path.display()
            ))
        })?;
        self.finish_local_checkpoint_write(
            "moderation screening",
            path,
            write_local_checkpoint_atomic_bounded(
                path,
                &bytes,
                self.config.runtime_retention().checkpoint_max_bytes(),
            ),
        )
    }

    fn load_moderation_quarantine_object_index_checkpoint(&self) -> Result<(), NodeInitError> {
        let Some(path) = self.moderation_quarantine_object_index_path.as_ref() else {
            return Ok(());
        };
        let Some(bytes) = read_local_checkpoint_bounded(
            path,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|err| {
            NodeInitError::checkpoint("moderation quarantine object index", path, err)
        })?
        else {
            return Ok(());
        };
        let retention = self.config.runtime_retention();
        let snapshot = decode_local_checkpoint_canonical::<ModerationQuarantineObjectSnapshot>(
            &bytes,
            retention.checkpoint_max_bytes(),
            retention
                .state_entry_limit()
                .max(retention.event_history_limit()),
        )
        .map_err(|err| {
            NodeInitError::checkpoint("moderation quarantine object index", path, err)
        })?;
        let object_count = snapshot.objects.len();
        self.moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)
            .and_then(|objects| objects.ensure_snapshot_capacity(&snapshot))
            .map_err(|err| {
                NodeInitError::checkpoint("moderation quarantine object index", path, err)
            })?;
        self.validate_moderation_quarantine_object_snapshot_refs(&snapshot)
            .map_err(|err| {
                NodeInitError::checkpoint("moderation quarantine object index", path, err)
            })?;
        self.moderation_quarantine_objects
            .write()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)
            .and_then(|mut objects| objects.restore_snapshot(snapshot))
            .map_err(|err| {
                NodeInitError::checkpoint("moderation quarantine object index", path, err)
            })?;
        iroha_logger::info!(
            path = %path.display(),
            object_count,
            "restored SoraFS moderation quarantine object index"
        );
        Ok(())
    }

    fn audit_moderation_quarantine_object_store(&self) -> Result<(), NodeInitError> {
        let root = self
            .moderation_quarantine_object_root
            .as_ref()
            .expect("storage-backed node has a quarantine object root");
        let index_path = self
            .moderation_quarantine_object_index_path
            .as_ref()
            .expect("storage-backed node has a quarantine index path");
        let snapshot = self
            .moderation_quarantine_objects
            .read()
            .map_err(|_| {
                NodeInitError::checkpoint(
                    "moderation quarantine object store",
                    root,
                    "object index lock poisoned",
                )
            })?
            .snapshot();
        let key_wrapper = self.moderation_quarantine_key_wrapper.as_deref();
        if !snapshot.objects.is_empty() && key_wrapper.is_none() {
            return Err(NodeInitError::checkpoint(
                "moderation quarantine key wrapper",
                root,
                "runtime PKCS#11/KMS key wrapper is missing while indexed objects exist",
            ));
        }
        let mut expected_files = BTreeSet::from([index_path.clone()]);
        for record in &snapshot.objects {
            let envelope_path = self
                .resolve_moderation_quarantine_object_path(root, record)
                .map_err(|err| {
                    NodeInitError::checkpoint("moderation quarantine object store", root, err)
                })?;
            let bytes = read_local_checkpoint_bounded(
                &envelope_path,
                self.config.runtime_retention().checkpoint_max_bytes(),
            )
            .map_err(|err| {
                NodeInitError::checkpoint(
                    "moderation quarantine object envelope",
                    &envelope_path,
                    err,
                )
            })?
            .ok_or_else(|| {
                NodeInitError::checkpoint(
                    "moderation quarantine object envelope",
                    &envelope_path,
                    "indexed envelope is missing",
                )
            })?;
            let envelope = norito::decode_from_bytes::<ModerationQuarantineObjectEnvelopeV1>(
                &bytes,
            )
            .map_err(|err| {
                NodeInitError::checkpoint(
                    "moderation quarantine object envelope",
                    &envelope_path,
                    err,
                )
            })?;
            let canonical = norito::to_bytes(&envelope).map_err(|err| {
                NodeInitError::checkpoint(
                    "moderation quarantine object envelope",
                    &envelope_path,
                    err,
                )
            })?;
            if canonical != bytes {
                return Err(NodeInitError::checkpoint(
                    "moderation quarantine object envelope",
                    &envelope_path,
                    "envelope is not canonically encoded",
                ));
            }
            let payload = open_moderation_quarantine_object(
                &envelope,
                record,
                key_wrapper.expect("non-empty object index requires a runtime key wrapper"),
            )
            .map_err(|err| {
                NodeInitError::checkpoint(
                    "moderation quarantine object envelope",
                    &envelope_path,
                    err,
                )
            })?;
            let quarantine = self
                .moderation_quarantine_record_for_object(&record.quarantine_id)
                .map_err(|err| {
                    NodeInitError::checkpoint(
                        "moderation quarantine object envelope",
                        &envelope_path,
                        err,
                    )
                })?;
            if *blake3::hash(&payload).as_bytes() != quarantine.subject_digest {
                return Err(NodeInitError::checkpoint(
                    "moderation quarantine object envelope",
                    &envelope_path,
                    "decrypted payload digest does not match quarantine subject",
                ));
            }
            expected_files.insert(envelope_path);
        }

        let file_limit = self
            .config
            .runtime_retention()
            .state_entry_limit()
            .saturating_add(2);
        let mut actual_files = BTreeSet::new();
        collect_secure_object_store_files(root, root, 0, file_limit, &mut actual_files).map_err(
            |err| NodeInitError::checkpoint("moderation quarantine object store", root, err),
        )?;
        let orphan_files = actual_files
            .difference(&expected_files)
            .cloned()
            .collect::<Vec<_>>();
        for orphan_path in orphan_files {
            if !self
                .recover_unindexed_moderation_quarantine_envelope(root, &orphan_path)
                .map_err(|error| {
                    NodeInitError::checkpoint(
                        "moderation quarantine object store",
                        &orphan_path,
                        error,
                    )
                })?
            {
                return Err(NodeInitError::checkpoint(
                    "moderation quarantine object store",
                    &orphan_path,
                    "unindexed object-store file is not a canonical crash orphan",
                ));
            }
            actual_files.remove(&orphan_path);
        }
        if actual_files != expected_files {
            let unexpected = actual_files
                .difference(&expected_files)
                .next()
                .or_else(|| expected_files.difference(&actual_files).next())
                .cloned()
                .unwrap_or_else(|| root.clone());
            return Err(NodeInitError::checkpoint(
                "moderation quarantine object store",
                &unexpected,
                "object store files do not exactly match the durable index",
            ));
        }
        Ok(())
    }

    fn persist_moderation_quarantine_object_index_snapshot(
        &self,
        snapshot: &ModerationQuarantineObjectSnapshot,
    ) -> Result<(), RuntimeCheckpointPersistError> {
        let Some(path) = self.moderation_quarantine_object_index_path.as_ref() else {
            return Ok(());
        };
        let bytes = norito::to_bytes(snapshot).map_err(|err| {
            RuntimeCheckpointPersistError::precommit(format!(
                "encode moderation quarantine object index checkpoint `{}`: {err}",
                path.display()
            ))
        })?;
        self.finish_local_checkpoint_write(
            "moderation quarantine object index",
            path,
            write_local_checkpoint_atomic_bounded(
                path,
                &bytes,
                self.config.runtime_retention().checkpoint_max_bytes(),
            ),
        )
    }

    fn load_moderation_evidence_viewer_checkpoint(&self) -> Result<(), NodeInitError> {
        let Some(path) = self.moderation_evidence_viewer_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let Some(bytes) = read_local_checkpoint_bounded(
            path,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|err| NodeInitError::checkpoint("moderation evidence viewer", path, err))?
        else {
            return Ok(());
        };
        let retention = self.config.runtime_retention();
        let snapshot = decode_local_checkpoint_canonical::<ModerationEvidenceViewerSnapshot>(
            &bytes,
            retention.checkpoint_max_bytes(),
            retention
                .state_entry_limit()
                .max(retention.event_history_limit()),
        )
        .map_err(|err| NodeInitError::checkpoint("moderation evidence viewer", path, err))?;
        let session_count = snapshot.sessions.len();
        let access_event_count = snapshot.access_events.len();
        self.validate_moderation_evidence_viewer_snapshot_refs(&snapshot)
            .map_err(|err| NodeInitError::checkpoint("moderation evidence viewer", path, err))?;
        self.moderation_evidence_viewer
            .write()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)
            .and_then(|mut runtime| runtime.restore_snapshot(snapshot))
            .map_err(|err| NodeInitError::checkpoint("moderation evidence viewer", path, err))?;
        iroha_logger::info!(
            path = %path.display(),
            session_count,
            access_event_count,
            "restored SoraFS moderation evidence viewer checkpoint"
        );
        Ok(())
    }

    fn persist_moderation_evidence_viewer_snapshot(
        &self,
        snapshot: &ModerationEvidenceViewerSnapshot,
    ) -> Result<(), RuntimeCheckpointPersistError> {
        let Some(path) = self.moderation_evidence_viewer_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let bytes = norito::to_bytes(snapshot).map_err(|err| {
            RuntimeCheckpointPersistError::precommit(format!(
                "encode moderation evidence viewer checkpoint `{}`: {err}",
                path.display()
            ))
        })?;
        self.finish_local_checkpoint_write(
            "moderation evidence viewer",
            path,
            write_local_checkpoint_atomic_bounded(
                path,
                &bytes,
                self.config.runtime_retention().checkpoint_max_bytes(),
            ),
        )
    }

    fn export_auxiliary_runtime_checkpoint(
        &self,
    ) -> Result<AuxiliaryRuntimeCheckpointV2, GovernancePublishError> {
        let capacity_runtime = self.capacity.checkpoint().map_err(|err| {
            GovernancePublishError::other(format!("export capacity runtime checkpoint: {err}"))
        })?;
        let deal_runtime = self.deal_engine.checkpoint().map_err(|err| {
            GovernancePublishError::other(format!("export deal runtime checkpoint: {err}"))
        })?;
        let mut por_history = self
            .por_history
            .read()
            .map_err(|_| GovernancePublishError::other("PoR history lock poisoned"))?
            .iter()
            .map(
                |((manifest_digest, provider_id), entry)| PorHistoryCheckpointEntryV1 {
                    manifest_digest: *manifest_digest,
                    provider_id: *provider_id,
                    last_success_unix: entry.last_success_unix,
                    last_failure_unix: entry.last_failure_unix,
                    failures_total: entry.failures_total,
                    consecutive_failures: entry.consecutive_failures,
                    last_slash_unix: entry.last_slash_unix,
                },
            )
            .collect::<Vec<_>>();
        por_history.sort_by_key(|entry| (entry.manifest_digest, entry.provider_id));
        let gc_eviction_intents = self
            .gc_eviction_intents
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))?;
        let gc_eviction_intent_next_sequence = gc_eviction_intents.next_sequence;
        let gc_eviction_intents = gc_eviction_intents.entries.values().cloned().collect();
        let gc_eviction_audit_links = self
            .gc_eviction_audit_links
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction audit link lock poisoned"))?
            .values()
            .cloned()
            .collect();
        let reputation_snapshots = self
            .reputation_snapshots
            .read()
            .map_err(|_| GovernancePublishError::other("reputation snapshot index poisoned"))?
            .values()
            .cloned()
            .collect();
        let latest_reputation_snapshot_id = self
            .latest_reputation_snapshot
            .read()
            .map_err(|_| GovernancePublishError::other("reputation snapshot cache poisoned"))?
            .as_ref()
            .map(|snapshot| snapshot.snapshot_id);
        let reputation_events = self
            .reputation_events
            .read()
            .map_err(|_| GovernancePublishError::other("reputation event history poisoned"))?
            .retained();
        let transparency_source_entries = self
            .transparency_ledger_source_entries
            .read()
            .map_err(|_| GovernancePublishError::other("transparency source-entry index poisoned"))?
            .values()
            .cloned()
            .collect();
        let privacy_source_events = self
            .privacy_aggregate_source_events
            .read()
            .map_err(|_| GovernancePublishError::other("privacy source-event index poisoned"))?
            .values()
            .cloned()
            .collect();
        let privacy_source_event_receipts = self
            .privacy_source_event_receipts
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy source-event receipt index poisoned")
            })?
            .iter()
            .map(
                |(event_id, canonical_digest)| transparency::PrivacySourceEventReceiptV1 {
                    event_id: event_id.clone(),
                    canonical_digest: *canonical_digest,
                },
            )
            .collect();
        let privacy_publish_request_receipts = self
            .privacy_publish_request_receipts
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy publish-request receipt index poisoned")
            })?
            .values()
            .cloned()
            .collect();
        let published_privacy_aggregate_cycles = self
            .published_privacy_aggregate_cycles
            .read()
            .map_err(|_| GovernancePublishError::other("privacy cycle index poisoned"))?
            .iter()
            .copied()
            .collect();
        let privacy_composition_budget = self
            .privacy_composition_budget
            .read()
            .map_err(|_| GovernancePublishError::other("privacy composition budget poisoned"))?
            .clone();
        let privacy_release_ledger = self
            .privacy_release_ledger
            .read()
            .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))?
            .clone();
        let published_evidence_viewer_audit_cycles = self
            .published_evidence_viewer_audit_cycles
            .read()
            .map_err(|_| GovernancePublishError::other("evidence cycle index poisoned"))?
            .iter()
            .copied()
            .collect();
        let governance_outbox = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?;
        let governance_outbox_next_sequence = governance_outbox.next_sequence;
        let governance_outbox_entries = governance_outbox.entries.values().cloned().collect();
        Ok(AuxiliaryRuntimeCheckpointV2 {
            version: AUX_RUNTIME_STATE_VERSION_V2,
            capacity_runtime,
            deal_runtime,
            por_tracker: self.por.checkpoint(),
            por_history,
            gc_eviction_intent_next_sequence,
            gc_eviction_intents,
            gc_eviction_audit_links,
            reputation_snapshots,
            latest_reputation_snapshot_id,
            reputation_events,
            transparency_source_entries,
            privacy_source_events,
            privacy_source_event_receipts,
            privacy_publish_request_receipts,
            published_privacy_aggregate_cycles,
            privacy_composition_budget,
            privacy_release_ledger,
            published_evidence_viewer_audit_cycles,
            governance_outbox_next_sequence,
            governance_outbox_entries,
        })
    }

    fn persist_auxiliary_runtime_checkpoint_unlocked(
        &self,
    ) -> Result<(), RuntimeCheckpointPersistError> {
        let Some(path) = self.auxiliary_runtime_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let checkpoint = self.export_auxiliary_runtime_checkpoint().map_err(|err| {
            RuntimeCheckpointPersistError::precommit(format!(
                "export auxiliary runtime checkpoint: {err}"
            ))
        })?;
        let bytes = norito::to_bytes(&checkpoint).map_err(|err| {
            RuntimeCheckpointPersistError::precommit(format!(
                "encode auxiliary runtime checkpoint: {err}"
            ))
        })?;
        self.finish_local_checkpoint_write(
            "auxiliary runtime",
            path,
            write_local_checkpoint_atomic_bounded(
                path,
                &bytes,
                self.config.runtime_retention().checkpoint_max_bytes(),
            ),
        )
    }

    fn mutate_deal_engine_durably<T>(
        &self,
        mutate: impl FnOnce(&DealEngine) -> Result<(T, bool), DealEngineError>,
    ) -> Result<T, DealEngineError> {
        let _checkpoint_guard = self
            .auxiliary_checkpoint_lock
            .lock()
            .map_err(|_| DealEngineError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(DealEngineError::Checkpoint)?;
        let previous = self.deal_engine.checkpoint()?;
        let (outcome, changed) = match mutate(&self.deal_engine) {
            Ok(outcome) => outcome,
            Err(err) => {
                if let Err(restore_err) = self.deal_engine.restore_checkpoint(previous) {
                    let message = self.record_unrecoverable_rollback(
                        "failed to roll back rejected deal mutation",
                        restore_err,
                    );
                    return Err(DealEngineError::Checkpoint(message));
                }
                return Err(err);
            }
        };
        if !changed {
            return Ok(outcome);
        }
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(DealEngineError::Checkpoint(err.to_string()));
            }
            if let Err(restore_err) = self.deal_engine.restore_checkpoint(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back deal checkpoint failure",
                    restore_err,
                );
                return Err(DealEngineError::Checkpoint(message));
            }
            return Err(DealEngineError::Checkpoint(err.to_string()));
        }
        Ok(outcome)
    }

    fn mutate_deal_engine_durably_with_governance<T>(
        &self,
        mutate: impl FnOnce(&DealEngine) -> Result<(T, bool), DealEngineError>,
        artifact: impl FnOnce(&T) -> Result<(GovernanceOutboxKindV1, Vec<u8>), GovernancePublishError>,
    ) -> Result<T, DealEngineError> {
        let _checkpoint_guard = self
            .auxiliary_checkpoint_lock
            .lock()
            .map_err(|_| DealEngineError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(DealEngineError::Checkpoint)?;
        let previous_deal = self.deal_engine.checkpoint()?;
        let previous_outbox = self
            .governance_outbox
            .read()
            .map_err(|_| DealEngineError::StateLockPoisoned)?
            .clone();
        let (outcome, changed) = match mutate(&self.deal_engine) {
            Ok(outcome) => outcome,
            Err(err) => {
                if let Err(restore_err) = self.deal_engine.restore_checkpoint(previous_deal) {
                    let message = self.record_unrecoverable_rollback(
                        "failed to roll back rejected deal mutation",
                        restore_err,
                    );
                    return Err(DealEngineError::Checkpoint(message));
                }
                return Err(err);
            }
        };
        if !changed {
            return Ok(outcome);
        }
        let (kind, payload_bytes) = match artifact(&outcome) {
            Ok(artifact) => artifact,
            Err(err) => {
                if let Err(restore_err) = self.deal_engine.restore_checkpoint(previous_deal) {
                    let message = self.record_unrecoverable_rollback(
                        "failed to roll back deal mutation after governance artifact failure",
                        restore_err,
                    );
                    return Err(DealEngineError::Checkpoint(message));
                }
                return Err(DealEngineError::Checkpoint(err.to_string()));
            }
        };
        if let Err(err) = self.enqueue_governance_outbox_unlocked(kind, payload_bytes) {
            if let Err(restore_err) = self.deal_engine.restore_checkpoint(previous_deal) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back deal mutation after governance outbox rejection",
                    restore_err,
                );
                return Err(DealEngineError::Checkpoint(message));
            }
            return Err(DealEngineError::Checkpoint(err.to_string()));
        }
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(DealEngineError::Checkpoint(err.to_string()));
            }
            if let Err(restore_err) = self.deal_engine.restore_checkpoint(previous_deal) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back deal checkpoint failure",
                    restore_err,
                );
                return Err(DealEngineError::Checkpoint(message));
            }
            if self
                .governance_outbox
                .write()
                .map(|mut outbox| *outbox = previous_outbox)
                .is_err()
            {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back governance outbox after deal checkpoint failure",
                    "state lock poisoned",
                );
                return Err(DealEngineError::Checkpoint(message));
            }
            return Err(DealEngineError::Checkpoint(err.to_string()));
        }
        Ok(outcome)
    }

    fn mutate_capacity_durably<T>(
        &self,
        mutate: impl FnOnce(&CapacityManager) -> Result<(T, bool), CapacityError>,
    ) -> Result<T, CapacityError> {
        let _checkpoint_guard = self
            .auxiliary_checkpoint_lock
            .lock()
            .map_err(|_| CapacityError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(CapacityError::Checkpoint)?;
        let previous = self.capacity.checkpoint()?;
        let (outcome, changed) = match mutate(&self.capacity) {
            Ok(outcome) => outcome,
            Err(err) => {
                if let Err(restore_err) = self.capacity.restore_checkpoint(previous) {
                    let message = self.record_unrecoverable_rollback(
                        "failed to roll back rejected capacity mutation",
                        restore_err,
                    );
                    return Err(CapacityError::Checkpoint(message));
                }
                return Err(err);
            }
        };
        if !changed {
            return Ok(outcome);
        }
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(CapacityError::Checkpoint(err.to_string()));
            }
            if let Err(restore_err) = self.capacity.restore_checkpoint(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back capacity checkpoint failure",
                    restore_err,
                );
                return Err(CapacityError::Checkpoint(message));
            }
            return Err(CapacityError::Checkpoint(err.to_string()));
        }
        Ok(outcome)
    }

    fn load_auxiliary_runtime_checkpoint(&self) -> Result<(), NodeInitError> {
        let Some(path) = self.auxiliary_runtime_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let Some(bytes) = read_local_checkpoint_bounded(
            path,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|err| NodeInitError::checkpoint("auxiliary runtime", path, err))?
        else {
            return Ok(());
        };
        let retention = self.config.runtime_retention();
        let maximum_sequence_elements = retention
            .state_entry_limit()
            .saturating_mul(2)
            .saturating_add(retention.event_history_limit())
            // Checkpoints contain bounded byte blobs whose element count is
            // independent of the number of retained domain records. The
            // archive's already-enforced byte length is a strict upper bound
            // for those sequences; domain restore validation still enforces
            // the configured state and event limits.
            .max(bytes.len());
        let checkpoint = decode_local_checkpoint_canonical::<AuxiliaryRuntimeCheckpointV2>(
            &bytes,
            retention.checkpoint_max_bytes(),
            maximum_sequence_elements,
        )
        .map_err(|err| NodeInitError::checkpoint("auxiliary runtime", path, err))?;
        self.restore_auxiliary_runtime_checkpoint(checkpoint)
            .map_err(|err| NodeInitError::checkpoint("auxiliary runtime", path, err))?;
        Ok(())
    }

    fn restore_auxiliary_runtime_checkpoint(
        &self,
        checkpoint: AuxiliaryRuntimeCheckpointV2,
    ) -> Result<(), GovernancePublishError> {
        if checkpoint.version != AUX_RUNTIME_STATE_VERSION_V2 {
            return Err(GovernancePublishError::other(format!(
                "unsupported auxiliary runtime checkpoint version {}",
                checkpoint.version
            )));
        }
        let retention = self.config.runtime_retention();
        let state_limit = retention.state_entry_limit();
        let event_limit = retention.event_history_limit();
        for (label, count, limit) in [
            ("PoR history", checkpoint.por_history.len(), state_limit),
            (
                "reputation snapshots",
                checkpoint.reputation_snapshots.len(),
                state_limit,
            ),
            (
                "transparency source entries",
                checkpoint.transparency_source_entries.len(),
                state_limit,
            ),
            (
                "privacy source events",
                checkpoint.privacy_source_events.len(),
                state_limit,
            ),
            (
                "privacy source-event receipts",
                checkpoint.privacy_source_event_receipts.len(),
                state_limit,
            ),
            (
                "privacy publish-request receipts",
                checkpoint.privacy_publish_request_receipts.len(),
                state_limit,
            ),
            (
                "published privacy cycles",
                checkpoint.published_privacy_aggregate_cycles.len(),
                state_limit,
            ),
            (
                "privacy release records",
                checkpoint.privacy_release_ledger.records.len(),
                state_limit,
            ),
            (
                "published evidence cycles",
                checkpoint.published_evidence_viewer_audit_cycles.len(),
                state_limit,
            ),
            (
                "governance outbox",
                checkpoint.governance_outbox_entries.len(),
                state_limit,
            ),
            (
                "GC eviction intents",
                checkpoint.gc_eviction_intents.len(),
                1,
            ),
            (
                "GC eviction audit links",
                checkpoint.gc_eviction_audit_links.len(),
                state_limit,
            ),
            (
                "reputation events",
                checkpoint.reputation_events.len(),
                event_limit,
            ),
        ] {
            if count > limit {
                return Err(GovernancePublishError::other(format!(
                    "{label} checkpoint count {count} exceeds configured limit {limit}"
                )));
            }
        }

        let mut por_history = HashMap::with_capacity(checkpoint.por_history.len());
        for entry in checkpoint.por_history {
            let key = (entry.manifest_digest, entry.provider_id);
            if por_history
                .insert(
                    key,
                    PorHistoryEntry {
                        last_success_unix: entry.last_success_unix,
                        last_failure_unix: entry.last_failure_unix,
                        failures_total: entry.failures_total,
                        consecutive_failures: entry.consecutive_failures,
                        last_slash_unix: entry.last_slash_unix,
                    },
                )
                .is_some()
            {
                return Err(GovernancePublishError::other(
                    "duplicate PoR history key in auxiliary checkpoint",
                ));
            }
        }
        let mut snapshots = BTreeMap::new();
        let mut previous_reputation_snapshot_id = None;
        for admitted in checkpoint.reputation_snapshots {
            if admitted.version != ADMITTED_REPUTATION_SNAPSHOT_VERSION_V1
                || admitted.admitted_at_unix == 0
                || admitted.encoded_len == 0
            {
                return Err(GovernancePublishError::other(
                    "invalid admitted reputation snapshot version or timestamp in auxiliary checkpoint",
                ));
            }
            let policy = self.reputation_trust_policy.as_deref().ok_or_else(|| {
                GovernancePublishError::other(
                    "auxiliary checkpoint contains signed reputation state but no external trust policy is configured",
                )
            })?;
            admitted
                .envelope
                .verify(policy, admitted.admitted_at_unix)
                .map_err(|err| {
                    GovernancePublishError::other(format!(
                        "invalid signed reputation snapshot in auxiliary checkpoint: {err}"
                    ))
                })?;
            let canonical_len = u64::try_from(
                admitted
                    .envelope
                    .canonical_bytes()
                    .map_err(|err| {
                        GovernancePublishError::other(format!(
                            "failed to canonicalize signed reputation snapshot in auxiliary checkpoint: {err}"
                        ))
                    })?
                    .len(),
            )
            .map_err(|_| {
                GovernancePublishError::other(
                    "signed reputation checkpoint envelope length does not fit u64",
                )
            })?;
            if admitted.encoded_len != canonical_len {
                return Err(GovernancePublishError::other(
                    "signed reputation checkpoint encoded length mismatch",
                ));
            }
            let snapshot_id = admitted.envelope.snapshot.snapshot_id;
            if previous_reputation_snapshot_id.is_some_and(|previous| previous >= snapshot_id) {
                return Err(GovernancePublishError::other(
                    "reputation snapshot checkpoint entries must be strictly ordered by snapshot id",
                ));
            }
            previous_reputation_snapshot_id = Some(snapshot_id);
            if snapshots.insert(snapshot_id, admitted).is_some() {
                return Err(GovernancePublishError::other(
                    "duplicate reputation snapshot id in auxiliary checkpoint",
                ));
            }
        }
        let latest_snapshot = match checkpoint.latest_reputation_snapshot_id {
            Some(snapshot_id) => Some(
                snapshots
                    .get(&snapshot_id)
                    .map(|admitted| admitted.envelope.snapshot.clone())
                    .ok_or_else(|| {
                        GovernancePublishError::other(
                            "latest reputation snapshot id is absent from auxiliary checkpoint",
                        )
                    })?,
            ),
            None if snapshots.is_empty() => None,
            None => {
                return Err(GovernancePublishError::other(
                    "non-empty reputation snapshot checkpoint is missing latest id",
                ));
            }
        };
        let mut reputation_chain = Vec::new();
        reputation_chain
            .try_reserve_exact(snapshots.len())
            .map_err(|_| {
                GovernancePublishError::other(
                    "failed to reserve retained reputation-chain validation state",
                )
            })?;
        reputation_chain.extend(snapshots.values());
        reputation_chain.sort_by_key(|admitted| {
            (
                admitted.envelope.snapshot.generated_at_unix,
                admitted.envelope.snapshot.snapshot_id,
            )
        });
        for pair in reputation_chain.windows(2) {
            validate_reputation_snapshot_transition(
                Some(&pair[0].envelope.snapshot),
                &pair[1].envelope.snapshot,
            )
            .map_err(|err| {
                GovernancePublishError::other(format!(
                    "retained reputation snapshots are not one exact monotonic chain: {err}"
                ))
            })?;
        }
        if let (Some(latest), Some(last)) = (latest_snapshot.as_ref(), reputation_chain.last())
            && latest.snapshot_id != last.envelope.snapshot.snapshot_id
        {
            return Err(GovernancePublishError::other(
                "latest reputation snapshot is not the final retained signed envelope",
            ));
        }
        drop(reputation_chain);
        let gc_eviction_intents = restore_gc_eviction_intents(
            checkpoint.gc_eviction_intent_next_sequence,
            checkpoint.gc_eviction_intents,
        )?;
        let mut gc_eviction_audit_links = BTreeMap::new();
        let mut previous_gc_intent_sequence = None;
        let mut gc_linked_outbox_sequences = BTreeSet::new();
        for link in checkpoint.gc_eviction_audit_links {
            validate_gc_eviction_audit_link(&link)?;
            if link.intent_sequence >= gc_eviction_intents.next_sequence
                || gc_eviction_intents
                    .entries
                    .contains_key(&link.intent_sequence)
                || previous_gc_intent_sequence
                    .is_some_and(|previous| previous >= link.intent_sequence)
                || !gc_linked_outbox_sequences.insert(link.outbox_sequence)
            {
                return Err(GovernancePublishError::other(
                    "GC eviction audit links must reference finalized intents below the high-water mark and be ordered and unique by intent/outbox sequence",
                ));
            }
            previous_gc_intent_sequence = Some(link.intent_sequence);
            gc_eviction_audit_links.insert(link.intent_sequence, link);
        }
        let mut reputation_events = BoundedEventHistory::new(event_limit);
        let mut previous_reputation_event: Option<&ReputationSnapshotEventV1> = None;
        for event in &checkpoint.reputation_events {
            event.validate().map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid reputation event in auxiliary checkpoint: {err}"
                ))
            })?;
            let Some(admitted) = snapshots.get(&event.snapshot_id) else {
                return Err(GovernancePublishError::other(
                    "reputation event references a missing retained snapshot",
                ));
            };
            let snapshot = &admitted.envelope.snapshot;
            let provider_count = u32::try_from(snapshot.providers.len()).map_err(|_| {
                GovernancePublishError::other(
                    "retained reputation snapshot provider count exceeds u32",
                )
            })?;
            if event.generated_at_unix != snapshot.generated_at_unix
                || event.merkle_root != snapshot.merkle_root
                || event.provider_count != provider_count
                || event.previous_snapshot_id != snapshot.previous_snapshot_id
            {
                return Err(GovernancePublishError::other(
                    "reputation event metadata does not match its retained snapshot",
                ));
            }
            if let Some(previous) = previous_reputation_event {
                if event.previous_snapshot_id != Some(previous.snapshot_id)
                    || event.generated_at_unix <= previous.generated_at_unix
                {
                    return Err(GovernancePublishError::other(
                        "reputation event suffix is not a monotonic snapshot chain",
                    ));
                }
            }
            previous_reputation_event = Some(event);
        }
        if let (Some(latest), Some(last_event)) = (
            latest_snapshot.as_ref(),
            checkpoint.reputation_events.last(),
        ) && latest.snapshot_id != last_event.snapshot_id
        {
            return Err(GovernancePublishError::other(
                "latest reputation snapshot does not match the final retained event",
            ));
        }
        reputation_events.restore(checkpoint.reputation_events, |event| event.sequence)?;
        let validated_capacity = CapacityManager::with_entry_limit(state_limit);
        validated_capacity
            .restore_checkpoint(checkpoint.capacity_runtime)
            .map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid capacity runtime auxiliary checkpoint: {err}"
                ))
            })?;
        let validated_capacity_checkpoint = validated_capacity.checkpoint().map_err(|err| {
            GovernancePublishError::other(format!(
                "failed to normalize capacity runtime auxiliary checkpoint: {err}"
            ))
        })?;
        let validated_deal_engine = DealEngine::with_entry_limit(state_limit);
        validated_deal_engine
            .restore_checkpoint(checkpoint.deal_runtime)
            .map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid deal runtime auxiliary checkpoint: {err}"
                ))
            })?;
        let validated_deal_checkpoint = validated_deal_engine.checkpoint().map_err(|err| {
            GovernancePublishError::other(format!(
                "failed to normalize deal runtime auxiliary checkpoint: {err}"
            ))
        })?;
        let mut transparency_entries = BTreeMap::new();
        for entry in checkpoint.transparency_source_entries {
            entry.validate().map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid transparency source entry in auxiliary checkpoint: {err}"
                ))
            })?;
            if transparency_entries
                .insert(entry.event_id.clone(), entry)
                .is_some()
            {
                return Err(GovernancePublishError::other(
                    "duplicate transparency source entry in auxiliary checkpoint",
                ));
            }
        }
        let mut privacy_receipts = BTreeMap::new();
        for receipt in checkpoint.privacy_source_event_receipts {
            if receipt.event_id.is_empty()
                || receipt.event_id.len() > MODERATION_LEDGER_MAX_PUBLIC_TEXT_BYTES_V1
                || receipt.event_id.trim() != receipt.event_id
                || receipt.event_id.chars().any(char::is_control)
                || receipt.canonical_digest == [0; 32]
                || privacy_receipts
                    .insert(receipt.event_id, receipt.canonical_digest)
                    .is_some()
            {
                return Err(GovernancePublishError::other(
                    "invalid or duplicate privacy source-event receipt in auxiliary checkpoint",
                ));
            }
        }
        let mut privacy_publish_receipts = BTreeMap::new();
        for receipt in checkpoint.privacy_publish_request_receipts {
            receipt.validate()?;
            if privacy_publish_receipts
                .insert(receipt.idempotency_key.clone(), receipt)
                .is_some()
            {
                return Err(GovernancePublishError::other(
                    "duplicate privacy publish-request receipt in auxiliary checkpoint",
                ));
            }
        }
        let mut privacy_events = BTreeMap::new();
        for event in checkpoint.privacy_source_events {
            event.validate().map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid privacy source event in auxiliary checkpoint: {err}"
                ))
            })?;
            let event_digest = event.canonical_digest().map_err(|err| {
                GovernancePublishError::other(format!(
                    "digest privacy source event in auxiliary checkpoint: {err}"
                ))
            })?;
            if privacy_receipts.get(&event.event_id) != Some(&event_digest)
                || privacy_events
                    .insert(event.event_id.clone(), event)
                    .is_some()
            {
                return Err(GovernancePublishError::other(
                    "privacy source event is duplicate or lacks its exact durable receipt",
                ));
            }
        }
        let privacy_cycle_count = checkpoint.published_privacy_aggregate_cycles.len();
        let evidence_cycle_count = checkpoint.published_evidence_viewer_audit_cycles.len();
        let privacy_cycles = checkpoint
            .published_privacy_aggregate_cycles
            .into_iter()
            .collect::<BTreeSet<_>>();
        let privacy_composition_budget = checkpoint.privacy_composition_budget;
        privacy_composition_budget.validate().map_err(|err| {
            GovernancePublishError::other(format!(
                "invalid privacy composition budget in auxiliary checkpoint: {err}"
            ))
        })?;
        if privacy_composition_budget
            .chains
            .iter()
            .flat_map(|chain| chain.charges.iter())
            .any(|charge| !privacy_cycles.contains(&charge.cycle_id))
        {
            return Err(GovernancePublishError::other(
                "privacy composition budget charge references an unpublished cycle",
            ));
        }
        let privacy_release_ledger = checkpoint.privacy_release_ledger;
        privacy_release_ledger.validate().map_err(|err| {
            GovernancePublishError::other(format!(
                "invalid privacy release ledger in auxiliary checkpoint: {err}"
            ))
        })?;
        let release_cycle_ids = privacy_release_ledger
            .records
            .iter()
            .map(|record| record.release_id)
            .collect::<BTreeSet<_>>();
        if release_cycle_ids != privacy_cycles {
            return Err(GovernancePublishError::other(
                "privacy release records and published-cycle guards differ",
            ));
        }
        let mut receipt_cycle_ids = BTreeSet::new();
        for receipt in privacy_publish_receipts.values() {
            if !receipt_cycle_ids.insert(receipt.cycle_id) {
                return Err(GovernancePublishError::other(
                    "multiple privacy publish-request receipts reference one release",
                ));
            }
            let Some(record) = privacy_release_ledger
                .records
                .iter()
                .find(|record| record.release_id == receipt.cycle_id)
            else {
                return Err(GovernancePublishError::other(
                    "privacy publish-request receipt references an unknown release",
                ));
            };
            validate_privacy_publish_receipt_release(receipt, record)?;
        }
        if receipt_cycle_ids != release_cycle_ids {
            return Err(GovernancePublishError::other(
                "privacy release ledger and publish-request receipt inventory differ",
            ));
        }
        let budget_charges = privacy_composition_budget
            .chains
            .iter()
            .flat_map(|chain| chain.charges.iter())
            .map(|charge| (charge.cycle_id, charge))
            .collect::<BTreeMap<_, _>>();
        let dp_release_count = privacy_release_ledger
            .records
            .iter()
            .filter(|record| record.privacy.per_subject_metric_cap.is_some())
            .count();
        if budget_charges.len() != dp_release_count {
            return Err(GovernancePublishError::other(
                "privacy release records and composition-budget charges differ",
            ));
        }
        for record in &privacy_release_ledger.records {
            if record.privacy.per_subject_metric_cap.is_some() {
                let charge = budget_charges.get(&record.release_id).ok_or_else(|| {
                    GovernancePublishError::other(
                        "differential-privacy release has no composition-budget charge",
                    )
                })?;
                if record.budget_charge_digest != Some(charge.charge_digest)
                    || record.status != transparency::PrivacyReleaseStatusV1::Published
                {
                    return Err(GovernancePublishError::other(
                        "differential-privacy release charge linkage is invalid",
                    ));
                }
            }
        }
        let evidence_cycles = checkpoint
            .published_evidence_viewer_audit_cycles
            .into_iter()
            .collect::<BTreeSet<_>>();
        if privacy_cycles.len() != privacy_cycle_count
            || evidence_cycles.len() != evidence_cycle_count
        {
            return Err(GovernancePublishError::other(
                "duplicate published cycle ids in auxiliary checkpoint",
            ));
        }
        let governance_outbox = restore_governance_outbox(
            checkpoint.governance_outbox_next_sequence,
            checkpoint.governance_outbox_entries,
            state_limit,
        )?;
        validate_privacy_release_outbox(
            &privacy_release_ledger,
            &privacy_cycles,
            &governance_outbox,
        )?;
        for entry in governance_outbox
            .entries
            .values()
            .filter(|entry| entry.kind == GovernanceOutboxKindV1::SignedReputationSnapshot)
        {
            let envelope = decode_canonical_signed_reputation_payload(&entry.payload_bytes)?;
            let Some(admitted) = snapshots.get(&envelope.snapshot.snapshot_id) else {
                return Err(GovernancePublishError::other(format!(
                    "pending signed reputation outbox entry {} has no retained admitted envelope",
                    entry.sequence
                )));
            };
            if admitted.envelope != envelope {
                return Err(GovernancePublishError::other(format!(
                    "pending signed reputation outbox entry {} conflicts with retained admitted bytes",
                    entry.sequence
                )));
            }
        }
        if gc_eviction_audit_links
            .values()
            .any(|link| link.outbox_sequence >= governance_outbox.next_sequence)
        {
            return Err(GovernancePublishError::other(
                "GC eviction audit link references an outbox sequence beyond its high-water mark",
            ));
        }
        let reserved_gc_slots = gc_eviction_reserved_outbox_slots(&gc_eviction_intents)?;
        if governance_outbox
            .entries
            .len()
            .checked_add(reserved_gc_slots)
            .is_none_or(|required| required > state_limit)
        {
            return Err(GovernancePublishError::other(format!(
                "auxiliary checkpoint intents reserve {reserved_gc_slots} outbox slots beyond retention limit {state_limit}"
            )));
        }

        let mut pending_linked_gc_sequences = BTreeSet::new();
        for link in gc_eviction_audit_links.values() {
            if let Some(entry) = governance_outbox.entries.get(&link.outbox_sequence) {
                if entry.kind != GovernanceOutboxKindV1::GcAudit
                    || entry.payload_digest != link.outbox_payload_digest
                {
                    return Err(GovernancePublishError::other(format!(
                        "GC eviction audit link {} conflicts with its pending outbox entry",
                        link.intent_sequence
                    )));
                }
                let expected = norito::to_bytes(&gc_eviction_audit_event_from_link(link)?)
                    .map_err(|err| {
                        GovernancePublishError::other(format!(
                            "encode linked GC eviction audit event: {err}"
                        ))
                    })?;
                if entry.payload_bytes != expected {
                    return Err(GovernancePublishError::other(format!(
                        "GC eviction audit link {} payload bytes mismatch",
                        link.intent_sequence
                    )));
                }
                pending_linked_gc_sequences.insert(entry.sequence);
            }
        }
        for entry in governance_outbox
            .entries
            .values()
            .filter(|entry| entry.kind == GovernanceOutboxKindV1::GcAudit)
        {
            let event =
                decode_canonical_governance_payload::<GcAuditEventV1>(&entry.payload_bytes)?;
            if event.payload.blocked_reason.is_none()
                && !pending_linked_gc_sequences.contains(&entry.sequence)
            {
                return Err(GovernancePublishError::other(format!(
                    "pending successful GC audit entry {} lacks eviction linkage",
                    entry.sequence
                )));
            }
        }

        self.por
            .restore_checkpoint(checkpoint.por_tracker)
            .map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid PoR tracker auxiliary checkpoint: {err}"
                ))
            })?;

        *self
            .por_history
            .write()
            .map_err(|_| GovernancePublishError::other("PoR history lock poisoned"))? = por_history;
        *self
            .gc_eviction_intents
            .write()
            .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))? =
            gc_eviction_intents;
        *self
            .gc_eviction_audit_links
            .write()
            .map_err(|_| GovernancePublishError::other("GC eviction audit link lock poisoned"))? =
            gc_eviction_audit_links;
        *self
            .reputation_snapshots
            .write()
            .map_err(|_| GovernancePublishError::other("reputation snapshot index poisoned"))? =
            snapshots;
        *self
            .latest_reputation_snapshot
            .write()
            .map_err(|_| GovernancePublishError::other("reputation snapshot cache poisoned"))? =
            latest_snapshot;
        *self
            .reputation_events
            .write()
            .map_err(|_| GovernancePublishError::other("reputation event history poisoned"))? =
            reputation_events;
        self.deal_engine
            .restore_checkpoint(validated_deal_checkpoint)
            .map_err(|err| {
                GovernancePublishError::other(format!(
                    "failed to install deal runtime auxiliary checkpoint: {err}"
                ))
            })?;
        self.capacity
            .restore_checkpoint(validated_capacity_checkpoint)
            .map_err(|err| {
                GovernancePublishError::other(format!(
                    "failed to install capacity runtime auxiliary checkpoint: {err}"
                ))
            })?;
        let capacity_usage = self.capacity.usage_snapshot();
        if capacity_usage.provider_id.is_some() {
            self.meter.restore_capacity_runtime(
                capacity_usage.committed_total_gib,
                capacity_usage.declaration_window,
                &capacity_usage.outstanding_orders,
            );
            if let Some(record) = self.capacity.active_declaration_record().map_err(|err| {
                GovernancePublishError::other(format!(
                    "failed to reconstruct capacity declaration after restore: {err}"
                ))
            })? {
                self.seed_telemetry_accumulator(&record);
            }
        }
        *self
            .transparency_ledger_source_entries
            .write()
            .map_err(|_| {
                GovernancePublishError::other("transparency source-entry index poisoned")
            })? = transparency_entries;
        *self
            .privacy_aggregate_source_events
            .write()
            .map_err(|_| GovernancePublishError::other("privacy source-event index poisoned"))? =
            privacy_events;
        *self.privacy_source_event_receipts.write().map_err(|_| {
            GovernancePublishError::other("privacy source-event receipt index poisoned")
        })? = privacy_receipts;
        *self.privacy_publish_request_receipts.write().map_err(|_| {
            GovernancePublishError::other("privacy publish-request receipt index poisoned")
        })? = privacy_publish_receipts;
        *self
            .published_privacy_aggregate_cycles
            .write()
            .map_err(|_| GovernancePublishError::other("privacy cycle index poisoned"))? =
            privacy_cycles;
        *self
            .privacy_composition_budget
            .write()
            .map_err(|_| GovernancePublishError::other("privacy composition budget poisoned"))? =
            privacy_composition_budget;
        *self
            .privacy_release_ledger
            .write()
            .map_err(|_| GovernancePublishError::other("privacy release ledger poisoned"))? =
            privacy_release_ledger;
        *self
            .published_evidence_viewer_audit_cycles
            .write()
            .map_err(|_| GovernancePublishError::other("evidence cycle index poisoned"))? =
            evidence_cycles;
        *self
            .governance_outbox
            .write()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))? =
            governance_outbox;
        self.reconcile_privacy_release_anchor()?;
        Ok(())
    }

    fn moderation_quarantine_record_for_object(
        &self,
        quarantine_id: &[u8; 16],
    ) -> Result<ModerationQuarantineRecord, ModerationQuarantineObjectError> {
        self.moderation_screening
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .quarantine_record(quarantine_id)
            .ok_or_else(|| ModerationQuarantineObjectError::UnknownQuarantine {
                quarantine_id_hex: hex::encode(quarantine_id),
            })
    }

    fn moderation_quarantine_object_record_for_viewer(
        &self,
        quarantine_id: &[u8; 16],
    ) -> Result<ModerationQuarantineObjectRecord, ModerationEvidenceViewerError> {
        self.moderation_quarantine_record_for_object(quarantine_id)
            .map_err(moderation_evidence_viewer_error_from_object_error)?;
        self.moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationEvidenceViewerError::StateLockPoisoned)?
            .get(quarantine_id)
            .ok_or_else(|| ModerationEvidenceViewerError::MissingObject {
                quarantine_id_hex: hex::encode(quarantine_id),
            })
    }

    fn validate_moderation_quarantine_object_snapshot_refs(
        &self,
        snapshot: &ModerationQuarantineObjectSnapshot,
    ) -> Result<(), ModerationQuarantineObjectError> {
        let screening = self
            .moderation_screening
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?;
        for record in &snapshot.objects {
            let quarantine = screening
                .quarantine_record(&record.quarantine_id)
                .ok_or_else(|| ModerationQuarantineObjectError::UnknownQuarantine {
                    quarantine_id_hex: hex::encode(record.quarantine_id),
                })?;
            if quarantine.subject_digest != record.payload_digest {
                return Err(ModerationQuarantineObjectError::DigestMismatch {
                    quarantine_id_hex: hex::encode(record.quarantine_id),
                    expected_digest_hex: hex::encode(quarantine.subject_digest),
                    actual_digest_hex: hex::encode(record.payload_digest),
                });
            }
        }
        Ok(())
    }

    fn validate_moderation_screening_snapshot_downstream_refs(
        &self,
        snapshot: &ModerationScreeningSnapshot,
    ) -> Result<(), ModerationScreeningError> {
        let quarantine_ids = snapshot
            .quarantine_records
            .iter()
            .map(|record| record.quarantine_id)
            .collect::<BTreeSet<_>>();
        let objects = self
            .moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?
            .snapshot();
        for object in objects.objects {
            if !quarantine_ids.contains(&object.quarantine_id) {
                return Err(ModerationScreeningError::InvalidSnapshot {
                    message: format!(
                        "screening snapshot removes quarantine `{}` referenced by object `{}`",
                        hex::encode(object.quarantine_id),
                        hex::encode(object.object_id)
                    ),
                });
            }
        }
        let viewer = self
            .moderation_evidence_viewer
            .read()
            .map_err(|_| ModerationScreeningError::StateLockPoisoned)?
            .snapshot();
        for session in viewer.sessions {
            if !quarantine_ids.contains(&session.quarantine_id) {
                return Err(ModerationScreeningError::InvalidSnapshot {
                    message: format!(
                        "screening snapshot removes quarantine `{}` referenced by viewer session `{}`",
                        hex::encode(session.quarantine_id),
                        hex::encode(session.session_id)
                    ),
                });
            }
        }
        Ok(())
    }

    fn validate_moderation_quarantine_snapshot_viewer_refs(
        &self,
        snapshot: &ModerationQuarantineObjectSnapshot,
    ) -> Result<(), ModerationQuarantineObjectError> {
        let objects = snapshot
            .objects
            .iter()
            .map(|record| (record.quarantine_id, record))
            .collect::<BTreeMap<_, _>>();
        let viewer = self
            .moderation_evidence_viewer
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .snapshot();
        for session in viewer.sessions {
            let Some(object) = objects.get(&session.quarantine_id) else {
                return Err(ModerationQuarantineObjectError::InvalidSnapshot {
                    message: format!(
                        "object snapshot removes quarantine `{}` referenced by viewer session `{}`",
                        hex::encode(session.quarantine_id),
                        hex::encode(session.session_id)
                    ),
                });
            };
            if session.object_id != object.object_id
                || session.evidence_digest != object.payload_digest
            {
                return Err(ModerationQuarantineObjectError::InvalidSnapshot {
                    message: format!(
                        "viewer session `{}` does not match candidate object metadata",
                        hex::encode(session.session_id)
                    ),
                });
            }
        }
        Ok(())
    }

    fn validate_moderation_evidence_viewer_snapshot_refs(
        &self,
        snapshot: &ModerationEvidenceViewerSnapshot,
    ) -> Result<(), ModerationEvidenceViewerError> {
        for session in &snapshot.sessions {
            let object =
                self.moderation_quarantine_object_record_for_viewer(&session.quarantine_id)?;
            if session.object_id != object.object_id
                || session.evidence_digest != object.payload_digest
            {
                return Err(ModerationEvidenceViewerError::InvalidSnapshot {
                    message: format!(
                        "evidence viewer session `{}` does not match local quarantine object metadata",
                        hex::encode(session.session_id)
                    ),
                });
            }
        }
        Ok(())
    }

    fn pricing_checkpoint_max_bytes(&self) -> u64 {
        self.config
            .runtime_retention()
            .checkpoint_max_bytes()
            .min(u64::try_from(MAX_PRICING_RUNTIME_CHECKPOINT_BYTES).unwrap_or(u64::MAX))
    }

    fn hedging_checkpoint_max_bytes(&self) -> u64 {
        self.config
            .runtime_retention()
            .checkpoint_max_bytes()
            .min(u64::try_from(MAX_HEDGING_RUNTIME_CHECKPOINT_BYTES).unwrap_or(u64::MAX))
    }

    fn persist_pricing_checkpoint(&self) -> Result<(), RuntimeCheckpointPersistError> {
        let state = self.governed_pricing.read().map_err(|_| {
            RuntimeCheckpointPersistError::precommit(
                "governed-pricing state lock poisoned while checkpointing",
            )
        })?;
        self.persist_pricing_checkpoint_state(state.as_ref())
    }

    fn persist_pricing_checkpoint_state(
        &self,
        series: Option<&GovernedPricingSeriesV1>,
    ) -> Result<(), RuntimeCheckpointPersistError> {
        let Some(path) = self.pricing_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let bytes = encode_pricing_checkpoint(self.pricing_trust_policy.as_deref(), series)
            .map_err(|error| {
                RuntimeCheckpointPersistError::precommit(format!(
                    "encode governed-pricing checkpoint `{}`: {error}",
                    path.display()
                ))
            })?;
        self.finish_local_checkpoint_write(
            "governed pricing runtime",
            path,
            write_local_checkpoint_atomic_bounded(
                path,
                &bytes,
                self.pricing_checkpoint_max_bytes(),
            ),
        )
    }

    fn load_pricing_checkpoint(&self) -> Result<(), NodeInitError> {
        let Some(path) = self.pricing_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let Some(bytes) = read_local_checkpoint_bounded(path, self.pricing_checkpoint_max_bytes())
            .map_err(|error| NodeInitError::checkpoint("governed pricing runtime", path, error))?
        else {
            return Ok(());
        };
        let restored = decode_pricing_checkpoint(&bytes, self.pricing_trust_policy.as_deref())
            .map_err(|error| NodeInitError::checkpoint("governed pricing runtime", path, error))?;
        *self.governed_pricing.write().map_err(|_| {
            NodeInitError::checkpoint("governed pricing runtime", path, "state lock poisoned")
        })? = restored;
        Ok(())
    }

    fn persist_hedging_checkpoint(&self) -> Result<(), RuntimeCheckpointPersistError> {
        let state = self.signed_hedging_feeds.read().map_err(|_| {
            RuntimeCheckpointPersistError::precommit(
                "signed hedging-feed state lock poisoned while checkpointing",
            )
        })?;
        self.persist_hedging_checkpoint_state(state.as_ref())
    }

    fn persist_hedging_checkpoint_state(
        &self,
        ledger: Option<&SignedHedgingFeedLedgerV1>,
    ) -> Result<(), RuntimeCheckpointPersistError> {
        let Some(path) = self.hedging_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let bytes = encode_hedging_checkpoint(self.hedging_feed_trust_policy.as_deref(), ledger)
            .map_err(|error| {
                RuntimeCheckpointPersistError::precommit(format!(
                    "encode signed hedging-feed checkpoint `{}`: {error}",
                    path.display()
                ))
            })?;
        self.finish_local_checkpoint_write(
            "signed hedging-feed runtime",
            path,
            write_local_checkpoint_atomic_bounded(
                path,
                &bytes,
                self.hedging_checkpoint_max_bytes(),
            ),
        )
    }

    fn load_hedging_checkpoint(&self) -> Result<(), NodeInitError> {
        let Some(path) = self.hedging_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let Some(bytes) = read_local_checkpoint_bounded(path, self.hedging_checkpoint_max_bytes())
            .map_err(|error| {
                NodeInitError::checkpoint("signed hedging-feed runtime", path, error)
            })?
        else {
            return Ok(());
        };
        let restored = decode_hedging_checkpoint(&bytes, self.hedging_feed_trust_policy.as_deref())
            .map_err(|error| {
                NodeInitError::checkpoint("signed hedging-feed runtime", path, error)
            })?;
        *self.signed_hedging_feeds.write().map_err(|_| {
            NodeInitError::checkpoint("signed hedging-feed runtime", path, "state lock poisoned")
        })? = restored;
        Ok(())
    }

    fn record_hedging_reference_price_metrics(
        &self,
        governed: &GovernedHedgingReferencePriceDecisionV1,
    ) {
        let cluster = self.config.alias().map_or("local", String::as_str);
        let reference = &governed.decision.xor_usd_price;
        let metrics = global_or_default();
        metrics.set_sorafs_hedging_reference_price_micro_usd(
            cluster,
            u64::try_from(quantity_to_metric_micro_saturating(reference)).unwrap_or(u64::MAX),
        );
        for feed in &governed.decision.feeds {
            metrics.set_sorafs_hedging_feed_divergence_bps(
                cluster,
                &feed.source,
                quantity_divergence_bps_saturating(&feed.xor_usd_price, reference),
            );
        }
    }

    fn resolve_moderation_quarantine_object_path(
        &self,
        root: &Path,
        record: &ModerationQuarantineObjectRecord,
    ) -> Result<PathBuf, ModerationQuarantineObjectError> {
        validate_relative_object_path(&record.envelope_path)
            .map_err(|message| ModerationQuarantineObjectError::InvalidSnapshot { message })?;
        Ok(root.join(&record.envelope_path))
    }

    fn read_moderation_quarantine_object_envelope(
        &self,
        root: &Path,
        record: &ModerationQuarantineObjectRecord,
    ) -> Result<
        (PathBuf, ModerationQuarantineObjectEnvelopeV1, Vec<u8>),
        ModerationQuarantineObjectError,
    > {
        let envelope_path = self.resolve_moderation_quarantine_object_path(root, record)?;
        let envelope_bytes = read_local_checkpoint_bounded(
            &envelope_path,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|error| ModerationQuarantineObjectError::Io {
            path: envelope_path.display().to_string(),
            message: error.to_string(),
        })?
        .ok_or_else(|| ModerationQuarantineObjectError::MissingObject {
            quarantine_id_hex: hex::encode(record.quarantine_id),
        })?;
        let envelope =
            norito::decode_from_bytes::<ModerationQuarantineObjectEnvelopeV1>(&envelope_bytes)
                .map_err(|error| ModerationQuarantineObjectError::Codec {
                    message: error.to_string(),
                })?;
        let canonical = norito::to_bytes(&envelope).map_err(|error| {
            ModerationQuarantineObjectError::Codec {
                message: error.to_string(),
            }
        })?;
        if canonical != envelope_bytes {
            return Err(ModerationQuarantineObjectError::AuthenticationFailed {
                quarantine_id_hex: hex::encode(record.quarantine_id),
            });
        }
        Ok((envelope_path, envelope, envelope_bytes))
    }

    fn recover_unindexed_moderation_quarantine_envelope(
        &self,
        root: &Path,
        path: &Path,
    ) -> Result<bool, ModerationQuarantineObjectError> {
        let bytes = read_local_checkpoint_bounded(
            path,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|error| ModerationQuarantineObjectError::Io {
            path: path.display().to_string(),
            message: error.to_string(),
        })?
        .ok_or_else(|| ModerationQuarantineObjectError::Io {
            path: path.display().to_string(),
            message: "unindexed envelope disappeared during startup audit".to_owned(),
        })?;
        let envelope =
            match norito::decode_from_bytes::<ModerationQuarantineObjectEnvelopeV1>(&bytes) {
                Ok(envelope) => envelope,
                Err(_) => return Ok(false),
            };
        let canonical = norito::to_bytes(&envelope).map_err(|error| {
            ModerationQuarantineObjectError::Codec {
                message: error.to_string(),
            }
        })?;
        if canonical != bytes {
            return Ok(false);
        }
        if validate_quarantine_object_envelope(&envelope).is_err() {
            return Ok(false);
        }
        let expected_path = root.join(moderation_quarantine_object_relative_path(
            envelope.quarantine_id,
            envelope.object_id,
        ));
        if expected_path != path {
            return Ok(false);
        }
        remove_local_checkpoint_file_durably(path).map_err(|error| {
            ModerationQuarantineObjectError::Io {
                path: path.display().to_string(),
                message: format!("remove unindexed crash-recovery envelope: {error}"),
            }
        })?;
        iroha_logger::warn!(
            quarantine_id = %hex::encode(envelope.quarantine_id),
            object_id = %hex::encode(envelope.object_id),
            "removed unindexed moderation quarantine envelope left by an interrupted commit"
        );
        Ok(true)
    }

    /// Finalise a deal settlement for the supplied epoch.
    ///
    /// External callers must authenticate a configured operator before invoking this trusted
    /// runtime boundary.
    pub fn settle_deal(
        &self,
        deal_id: DealId,
        settlement_epoch: u64,
    ) -> Result<DealSettlementOutcome, DealEngineError> {
        let outcome = self.mutate_deal_engine_durably_with_governance(
            |engine| {
                engine
                    .settle(deal_id, settlement_epoch)
                    .map(|outcome| (outcome, true))
            },
            |outcome| {
                let encoded = norito::to_bytes(&outcome.governance).map_err(|err| {
                    GovernancePublishError::other(format!(
                        "encode deal settlement governance artifact: {err}"
                    ))
                })?;
                Ok((GovernanceOutboxKindV1::DealSettlement, encoded))
            },
        )?;
        let provider_hex = hex::encode(outcome.record.provider_id.as_bytes());
        let status_label = match outcome.governance.status {
            DealSettlementStatusV1::WindowSettled => "window_settled",
            DealSettlementStatusV1::Completed => "completed",
            DealSettlementStatusV1::Cancelled => "cancelled",
            DealSettlementStatusV1::Defaulted => "defaulted",
        };
        global_sorafs_node_otel().record_deal_settlement(
            &provider_hex,
            status_label,
            &outcome.record.expected_charge,
            &outcome.record.client_credit_debit,
            &outcome.record.bond_slash,
            &outcome.record.outstanding,
        );
        if let Err(err) = self.flush_governance_outbox() {
            global_sorafs_node_otel().record_settlement_publish(&provider_hex, "pending");
            iroha_logger::warn!(
                %err,
                %provider_hex,
                "SoraFS settlement artefact remains pending in the governance outbox"
            );
        } else {
            let status = if self.pending_governance_publication_count() == 0 {
                "success"
            } else {
                "pending"
            };
            global_sorafs_node_otel().record_settlement_publish(&provider_hex, status);
        }
        Ok(outcome)
    }

    /// Cancel an idle active deal at its exact next settlement boundary.
    ///
    /// The caller is a trusted runtime boundary and must authenticate a configured operator
    /// authority before invoking this method.
    pub fn cancel_deal(
        &self,
        deal_id: DealId,
        cancellation_epoch: u64,
        reason: String,
    ) -> Result<DealSettlementOutcome, DealEngineError> {
        let outcome = self.mutate_deal_engine_durably_with_governance(
            |engine| {
                engine
                    .cancel(deal_id, cancellation_epoch, reason)
                    .map(|outcome| (outcome, true))
            },
            |outcome| {
                let encoded = norito::to_bytes(&outcome.governance).map_err(|err| {
                    GovernancePublishError::other(format!(
                        "encode deal cancellation governance artifact: {err}"
                    ))
                })?;
                Ok((GovernanceOutboxKindV1::DealSettlement, encoded))
            },
        )?;
        let provider_hex = hex::encode(outcome.record.provider_id.as_bytes());
        let zero = Quantity::zero();
        global_sorafs_node_otel().record_deal_settlement(
            &provider_hex,
            "cancelled",
            &zero,
            &zero,
            &zero,
            &zero,
        );
        if let Err(err) = self.flush_governance_outbox() {
            global_sorafs_node_otel().record_settlement_publish(&provider_hex, "pending");
            iroha_logger::warn!(
                %err,
                %provider_hex,
                "SoraFS cancellation artefact remains pending in the governance outbox"
            );
        } else {
            let status = if self.pending_governance_publication_count() == 0 {
                "success"
            } else {
                "pending"
            };
            global_sorafs_node_otel().record_settlement_publish(&provider_hex, status);
        }
        Ok(outcome)
    }

    fn prepare_gc_eviction_intent(
        &self,
        _lock_guards: (
            &std::sync::MutexGuard<'_, ()>,
            &std::sync::MutexGuard<'_, ()>,
        ),
        storage: &StorageBackend,
        target: &StoredManifest,
        provider_id: [u8; 32],
        audit_timestamp_unix: u64,
        reason: &str,
    ) -> Result<GcEvictionIntentV1, GovernancePublishError> {
        let snapshot = gc_storage_snapshot(storage)?;
        let authoritative_target = snapshot
            .manifests
            .iter()
            .find(|manifest| manifest.manifest_id() == target.manifest_id())
            .ok_or_else(|| {
                GovernancePublishError::other(format!(
                    "GC target {} disappeared before intent preparation",
                    target.manifest_id()
                ))
            })?;
        if gc_manifest_identity_digest(authoritative_target) != gc_manifest_identity_digest(target)
        {
            return Err(GovernancePublishError::other(format!(
                "GC target {} changed canonical storage identity before intent preparation",
                target.manifest_id()
            )));
        }
        for index in 0..authoritative_target.chunk_count() {
            let chunk = authoritative_target.chunk(index).ok_or_else(|| {
                GovernancePublishError::other("GC target chunk metadata is incomplete")
            })?;
            let refcount = snapshot
                .chunk_refcounts
                .iter()
                .find(|entry| entry.digest == chunk.digest)
                .ok_or_else(|| {
                    GovernancePublishError::other("GC target chunk lacks a storage refcount")
                })?;
            if refcount.count > 1 {
                return Err(GovernancePublishError::other(format!(
                    "GC target {} acquired shared chunks before intent preparation",
                    target.manifest_id()
                )));
            }
        }
        let storage_after = gc_expected_post_storage_identity(&snapshot, authoritative_target)?;

        let checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        let previous_intents = self
            .gc_eviction_intents
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))?
            .clone();
        if !previous_intents.entries.is_empty() {
            return Err(GovernancePublishError::other(
                "unreconciled GC eviction intent blocks a new eviction",
            ));
        }
        let sequence = previous_intents.next_sequence;
        let next_sequence = sequence.checked_add(1).ok_or_else(|| {
            GovernancePublishError::other("GC eviction intent sequence exhausted")
        })?;
        let mut intent = GcEvictionIntentV1 {
            version: GC_EVICTION_INTENT_VERSION_V1,
            sequence,
            manifest_id: authoritative_target.manifest_id().to_owned(),
            manifest_digest: *authoritative_target.manifest_digest(),
            manifest_identity_digest: gc_manifest_identity_digest(authoritative_target),
            provider_id,
            audit_timestamp_unix,
            reason: reason.to_owned(),
            expected_freed_bytes: authoritative_target.content_length(),
            storage_before: snapshot.identity,
            storage_after,
            reserved_outbox_slots: GC_EVICTION_RESERVED_OUTBOX_SLOTS,
            binding_digest: [0; 32],
        };
        intent.binding_digest = gc_eviction_intent_binding_digest(&intent)?;
        validate_gc_eviction_intent(&intent)?;

        let pending_outbox_entries = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .entries
            .len();
        let limit = self.config.runtime_retention().state_entry_limit();
        let required = pending_outbox_entries
            .checked_add(usize::from(intent.reserved_outbox_slots))
            .ok_or_else(|| GovernancePublishError::other("outbox reservation count overflow"))?;
        if required > limit {
            return Err(GovernancePublishError::other(format!(
                "GC eviction requires one reserved governance outbox slot, but {pending_outbox_entries} entries already consume limit {limit}"
            )));
        }

        let previous_links = self
            .gc_eviction_audit_links
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction audit link lock poisoned"))?
            .clone();
        let pending_sequences = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .entries
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut links = previous_links.clone();
        while links.len() >= limit {
            let candidate = links
                .iter()
                .find_map(|(sequence, link)| {
                    (!pending_sequences.contains(&link.outbox_sequence)).then_some(*sequence)
                })
                .ok_or_else(|| {
                    GovernancePublishError::other(format!(
                        "GC eviction audit linkage retention exhausted (limit {limit})"
                    ))
                })?;
            links.remove(&candidate);
        }

        let mut runtime = previous_intents.clone();
        runtime.next_sequence = next_sequence;
        runtime.entries.insert(sequence, intent.clone());
        let mut intent_runtime = self
            .gc_eviction_intents
            .write()
            .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))?;
        let mut audit_links = self
            .gc_eviction_audit_links
            .write()
            .map_err(|_| GovernancePublishError::other("GC eviction audit link lock poisoned"))?;
        *intent_runtime = runtime;
        *audit_links = links;
        drop(audit_links);
        drop(intent_runtime);
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            let mut intent_runtime = self.gc_eviction_intents.write().map_err(|_| {
                GovernancePublishError::other("GC eviction intent rollback lock poisoned")
            })?;
            let mut audit_links = self.gc_eviction_audit_links.write().map_err(|_| {
                GovernancePublishError::other("GC eviction link rollback lock poisoned")
            })?;
            *intent_runtime = previous_intents;
            *audit_links = previous_links;
            return Err(GovernancePublishError::other(err.to_string()));
        }
        drop(checkpoint_guard);
        Ok(intent)
    }

    fn append_gc_eviction_audit_unlocked(
        &self,
        intent: &GcEvictionIntentV1,
    ) -> Result<(), GovernancePublishError> {
        validate_gc_eviction_intent(intent)?;
        if self
            .gc_eviction_audit_links
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction audit link lock poisoned"))?
            .contains_key(&intent.sequence)
        {
            return Err(GovernancePublishError::other(format!(
                "GC eviction intent {} already has publication linkage while still pending",
                intent.sequence
            )));
        }
        let expected_outbox_sequence = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .next_sequence;
        let payload = gc_eviction_payload(intent);
        let payload_digest = gc_audit_payload_digest_v1(&payload)
            .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        let audit = GcAuditEventV1 {
            version: GC_AUDIT_EVENT_VERSION_V1,
            header: SorafsAuditHeaderV1 {
                sequence: expected_outbox_sequence,
                occurred_at_unix: intent.audit_timestamp_unix,
                signer: GC_AUDIT_SIGNER_V1.to_owned(),
                payload_digest,
            },
            payload,
        };
        audit
            .validate()
            .map_err(|err| GovernancePublishError::other(err.to_string()))?;
        let encoded = norito::to_bytes(&audit).map_err(|err| {
            GovernancePublishError::other(format!("encode GC eviction audit event: {err}"))
        })?;
        let outbox_payload_digest = *blake3::hash(&encoded).as_bytes();
        let (outbox_sequence, inserted) =
            self.enqueue_gc_eviction_governance_outbox_unlocked(encoded)?;
        if !inserted || outbox_sequence != expected_outbox_sequence {
            return Err(GovernancePublishError::other(
                "GC eviction audit collided with an existing outbox entry",
            ));
        }
        let mut link = GcEvictionAuditLinkV1 {
            version: GC_EVICTION_AUDIT_LINK_VERSION_V1,
            intent_sequence: intent.sequence,
            outbox_sequence,
            manifest_id: intent.manifest_id.clone(),
            manifest_digest: intent.manifest_digest,
            provider_id: intent.provider_id,
            occurred_at_unix: intent.audit_timestamp_unix,
            freed_bytes: intent.expected_freed_bytes,
            reason: intent.reason.clone(),
            storage_gc_evictions_total: intent.storage_after.gc_evictions_total,
            payload_digest,
            outbox_payload_digest,
            binding_digest: [0; 32],
        };
        link.binding_digest = gc_eviction_audit_link_binding_digest(&link);
        validate_gc_eviction_audit_link(&link)?;
        let replaced = self
            .gc_eviction_audit_links
            .write()
            .map_err(|_| GovernancePublishError::other("GC eviction audit link lock poisoned"))?
            .insert(intent.sequence, link);
        if replaced.is_some() {
            return Err(GovernancePublishError::other(
                "GC eviction audit link sequence changed during finalization",
            ));
        }
        Ok(())
    }

    fn restore_gc_publication_runtime(
        &self,
        intents: GcEvictionIntentRuntime,
        links: BTreeMap<u64, GcEvictionAuditLinkV1>,
        outbox: GovernanceOutboxRuntime,
    ) -> Result<(), GovernancePublishError> {
        let mut intent_runtime = self.gc_eviction_intents.write().map_err(|_| {
            GovernancePublishError::other("GC eviction intent rollback lock poisoned")
        })?;
        let mut audit_links = self.gc_eviction_audit_links.write().map_err(|_| {
            GovernancePublishError::other("GC eviction link rollback lock poisoned")
        })?;
        let mut governance_outbox = self.governance_outbox.write().map_err(|_| {
            GovernancePublishError::other("governance outbox rollback lock poisoned")
        })?;
        *intent_runtime = intents;
        *audit_links = links;
        *governance_outbox = outbox;
        Ok(())
    }

    fn settle_gc_eviction_intent_against_storage(
        &self,
        _gc_guard: &std::sync::MutexGuard<'_, ()>,
        _drain_guard: &std::sync::MutexGuard<'_, ()>,
        storage: &StorageBackend,
        intent: &GcEvictionIntentV1,
        fail_closed_on_error: bool,
    ) -> Result<GcIntentDisposition, GovernancePublishError> {
        validate_gc_eviction_intent(intent)?;
        let checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        if fail_closed_on_error {
            self.ensure_durability_healthy()
                .map_err(GovernancePublishError::other)?;
        }
        let reconciliation_error = |message: String| {
            if fail_closed_on_error {
                self.mark_durability_unhealthy(message.clone());
            }
            GovernancePublishError::other(message)
        };
        let snapshot = gc_storage_snapshot(storage).map_err(|err| {
            reconciliation_error(format!(
                "cannot classify storage generation for GC intent {}: {err}",
                intent.sequence
            ))
        })?;
        let target = snapshot
            .manifests
            .iter()
            .find(|manifest| manifest.manifest_id() == intent.manifest_id);
        let disposition = if snapshot.identity == intent.storage_before {
            let target = target.ok_or_else(|| {
                reconciliation_error(
                    "pre-domain GC generation is missing its intended manifest".to_owned(),
                )
            })?;
            if gc_manifest_identity_digest(target) != intent.manifest_identity_digest
                || target.manifest_digest() != &intent.manifest_digest
            {
                return Err(reconciliation_error(
                    "pre-domain GC manifest conflicts with the persisted intent".to_owned(),
                ));
            }
            GcIntentDisposition::DiscardedPreDomain
        } else if snapshot.identity == intent.storage_after {
            if target.is_some() {
                return Err(reconciliation_error(
                    "post-domain GC generation still contains the evicted manifest".to_owned(),
                ));
            }
            GcIntentDisposition::FinalizedCommitted
        } else {
            return Err(reconciliation_error(format!(
                "storage generation drift prevents deterministic reconciliation of GC intent {}",
                intent.sequence
            )));
        };

        let previous_intents = self
            .gc_eviction_intents
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))?
            .clone();
        if previous_intents.entries.get(&intent.sequence) != Some(intent) {
            return Err(reconciliation_error(format!(
                "GC eviction intent {} changed before reconciliation",
                intent.sequence
            )));
        }
        let previous_links = self
            .gc_eviction_audit_links
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction audit link lock poisoned"))?
            .clone();
        let previous_outbox = self
            .governance_outbox
            .read()
            .map_err(|_| GovernancePublishError::other("governance outbox lock poisoned"))?
            .clone();

        let mutation = (|| {
            if disposition == GcIntentDisposition::FinalizedCommitted {
                self.append_gc_eviction_audit_unlocked(intent)?;
            }
            let removed = self
                .gc_eviction_intents
                .write()
                .map_err(|_| GovernancePublishError::other("GC eviction intent lock poisoned"))?
                .entries
                .remove(&intent.sequence);
            if removed.as_ref() != Some(intent) {
                return Err(GovernancePublishError::other(format!(
                    "GC eviction intent {} changed during finalization",
                    intent.sequence
                )));
            }
            Ok(())
        })();
        if let Err(err) = mutation {
            if let Err(rollback) = self.restore_gc_publication_runtime(
                previous_intents,
                previous_links,
                previous_outbox,
            ) {
                let reason = self.record_unrecoverable_rollback(
                    "failed to roll back GC publication finalization",
                    rollback,
                );
                return Err(GovernancePublishError::other(reason));
            }
            if fail_closed_on_error && disposition == GcIntentDisposition::FinalizedCommitted {
                self.mark_durability_unhealthy(err.to_string());
            }
            return Err(err);
        }
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            if let Err(rollback) = self.restore_gc_publication_runtime(
                previous_intents,
                previous_links,
                previous_outbox,
            ) {
                let reason = self.record_unrecoverable_rollback(
                    "failed to roll back GC publication checkpoint failure",
                    rollback,
                );
                return Err(GovernancePublishError::other(reason));
            }
            let message =
                format!("GC storage state may have committed without its audit publication: {err}");
            if fail_closed_on_error && disposition == GcIntentDisposition::FinalizedCommitted {
                self.mark_durability_unhealthy(message.clone());
            }
            return Err(GovernancePublishError::other(message));
        }
        drop(checkpoint_guard);
        Ok(disposition)
    }

    fn evict_manifest_with_gc_audit(
        &self,
        gc_guard: &std::sync::MutexGuard<'_, ()>,
        storage: &StorageBackend,
        target: &StoredManifest,
        provider_id: [u8; 32],
        audit_timestamp_unix: u64,
        reason: &str,
    ) -> Result<GcEvictionTransactionOutcome, GovernancePublishError> {
        let drain_guard = self
            .governance_outbox_drain_lock
            .lock()
            .map_err(|_| GovernancePublishError::other("governance outbox drain lock poisoned"))?;
        let intent = self.prepare_gc_eviction_intent(
            (gc_guard, &drain_guard),
            storage,
            target,
            provider_id,
            audit_timestamp_unix,
            reason,
        )?;
        let before_eviction = gc_storage_snapshot(storage).map_err(|err| {
            let message = format!(
                "cannot revalidate storage generation for GC intent {}: {err}",
                intent.sequence
            );
            self.mark_durability_unhealthy(message.clone());
            GovernancePublishError::other(message)
        })?;
        if before_eviction.identity != intent.storage_before
            || before_eviction
                .manifests
                .iter()
                .find(|manifest| manifest.manifest_id() == intent.manifest_id)
                .is_none_or(|manifest| {
                    gc_manifest_identity_digest(manifest) != intent.manifest_identity_digest
                })
        {
            let message = format!(
                "storage generation changed after preparing GC intent {}",
                intent.sequence
            );
            self.mark_durability_unhealthy(message.clone());
            return Err(GovernancePublishError::other(message));
        }

        let freed_bytes = match storage.evict_manifest(&intent.manifest_id) {
            Ok(freed_bytes) => freed_bytes,
            Err(storage_error) => {
                let current = gc_storage_snapshot_unchecked(storage);
                if storage.ensure_durability_healthy().is_ok()
                    && current.as_ref().is_ok_and(|snapshot| {
                        snapshot.identity == intent.storage_before
                            && snapshot.manifests.iter().any(|manifest| {
                                manifest.manifest_id() == intent.manifest_id
                                    && gc_manifest_identity_digest(manifest)
                                        == intent.manifest_identity_digest
                            })
                    })
                {
                    self.settle_gc_eviction_intent_against_storage(
                        gc_guard,
                        &drain_guard,
                        storage,
                        &intent,
                        false,
                    )?;
                    return Err(GovernancePublishError::other(storage_error.to_string()));
                }
                let message = format!(
                    "GC eviction result for intent {} is ambiguous after storage error: {storage_error}",
                    intent.sequence
                );
                self.mark_durability_unhealthy(message.clone());
                return Err(GovernancePublishError::other(message));
            }
        };
        if freed_bytes != intent.expected_freed_bytes {
            let message = format!(
                "GC eviction intent {} freed {freed_bytes} bytes, expected {}",
                intent.sequence, intent.expected_freed_bytes
            );
            self.mark_durability_unhealthy(message.clone());
            return Err(GovernancePublishError::other(message));
        }
        let disposition = self.settle_gc_eviction_intent_against_storage(
            gc_guard,
            &drain_guard,
            storage,
            &intent,
            true,
        )?;
        if disposition != GcIntentDisposition::FinalizedCommitted {
            let message = format!(
                "GC intent {} reverted to a pre-domain generation after eviction",
                intent.sequence
            );
            self.mark_durability_unhealthy(message.clone());
            return Err(GovernancePublishError::other(message));
        }
        drop(drain_guard);
        let publish_error = self
            .flush_governance_outbox()
            .err()
            .map(|err| err.to_string());
        Ok(GcEvictionTransactionOutcome {
            freed_bytes,
            publish_error,
        })
    }

    fn validate_gc_eviction_links_against_storage(
        &self,
        storage: &StorageBackend,
    ) -> Result<(), GovernancePublishError> {
        let (_, current_evictions_total) = storage.gc_counters();
        let links = self
            .gc_eviction_audit_links
            .read()
            .map_err(|_| GovernancePublishError::other("GC eviction audit link lock poisoned"))?;
        let mut previous_counter = None;
        for link in links.values() {
            validate_gc_eviction_audit_link(link)?;
            if link.storage_gc_evictions_total > current_evictions_total
                || previous_counter
                    .is_some_and(|previous| previous >= link.storage_gc_evictions_total)
            {
                return Err(GovernancePublishError::other(
                    "GC eviction audit links conflict with storage counter generation",
                ));
            }
            previous_counter = Some(link.storage_gc_evictions_total);
        }
        Ok(())
    }

    fn publish_gc_audit_event(
        &self,
        payload: GcAuditPayloadV1,
    ) -> Result<(), GovernancePublishError> {
        if payload.blocked_reason.is_none() {
            return Err(GovernancePublishError::other(
                "successful GC audits must be committed through an eviction transaction",
            ));
        }
        let manifest_digest = payload.manifest_digest;
        self.enqueue_sequenced_governance_outbox(GovernanceOutboxKindV1::GcAudit, |sequence| {
            let payload_digest = gc_audit_payload_digest_v1(&payload)
                .map_err(|err| GovernancePublishError::other(err.to_string()))?;
            let header = SorafsAuditHeaderV1 {
                sequence,
                occurred_at_unix: payload.evicted_at_unix,
                signer: GC_AUDIT_SIGNER_V1.to_owned(),
                payload_digest,
            };
            let audit_event = GcAuditEventV1 {
                version: GC_AUDIT_EVENT_VERSION_V1,
                header,
                payload,
            };
            audit_event
                .validate()
                .map_err(|err| GovernancePublishError::other(err.to_string()))?;
            norito::to_bytes(&audit_event).map_err(|err| {
                GovernancePublishError::other(format!("encode GC audit event: {err}"))
            })
        })?;
        self.flush_governance_outbox().map_err(|err| {
            GovernancePublishError::other(format!(
                "GC audit for manifest {} remains durably pending: {err}",
                hex::encode(manifest_digest)
            ))
        })?;
        Ok(())
    }

    fn publish_reconciliation_report(&self, report: &SorafsReconciliationReportV1) {
        let encoded = match norito::to_bytes(report) {
            Ok(encoded) => encoded,
            Err(err) => {
                iroha_logger::error!(%err, "failed to encode reconciliation report");
                return;
            }
        };
        if let Err(err) =
            self.enqueue_governance_outbox(GovernanceOutboxKindV1::ReconciliationReport, encoded)
        {
            iroha_logger::error!(
                %err,
                "failed to durably queue reconciliation report"
            );
            return;
        }
        if let Err(err) = self.flush_governance_outbox() {
            iroha_logger::warn!(
                %err,
                "reconciliation report remains pending in the governance outbox"
            );
        }
    }

    /// Capture a snapshot of the deal ledger.
    #[must_use]
    pub fn deal_snapshot(&self, deal_id: DealId) -> Option<DealSnapshot> {
        self.deal_engine.deal_snapshot(deal_id)
    }

    /// Run a GC sweep against expired manifests using one complete finalized
    /// native repair-ledger projection.
    ///
    /// The caller must collect `repair_projection` from one immutable query
    /// view. A partial or provider-filtered task page cannot be constructed as
    /// this projection and therefore cannot authorize local deletion.
    pub fn run_gc_once(
        &self,
        now_unix: u64,
        repair_projection: &RepairLedgerTaskProjectionV1,
    ) -> GcSweepReport {
        const REASON_LIMIT_REACHED: &str = "limit_reached";
        const RESULT_SUCCESS: &str = "success";
        const RESULT_ERROR: &str = "error";

        let mut report = GcSweepReport::default();
        if !self.gc_config.enabled() || now_unix == 0 {
            return report;
        }
        let Some(storage) = self.storage.as_ref() else {
            report.errors = report.errors.saturating_add(1);
            iroha_logger::warn!("GC sweep skipped: storage backend disabled");
            global_or_default().inc_sorafs_gc_runs(RESULT_ERROR);
            global_sorafs_gc_otel().record_run(RESULT_ERROR);
            return report;
        };
        if let Err(err) = storage.ensure_durability_healthy() {
            report.errors = report.errors.saturating_add(1);
            iroha_logger::error!(%err, "GC sweep skipped: storage backend fail-stopped");
            global_or_default().inc_sorafs_gc_runs(RESULT_ERROR);
            global_sorafs_gc_otel().record_run(RESULT_ERROR);
            return report;
        }
        let gc_guard = match self.gc_mutation_lock.lock() {
            Ok(guard) => guard,
            Err(_) => {
                report.errors = report.errors.saturating_add(1);
                iroha_logger::error!("GC sweep skipped: mutation lock poisoned");
                global_or_default().inc_sorafs_gc_runs(RESULT_ERROR);
                global_sorafs_gc_otel().record_run(RESULT_ERROR);
                return report;
            }
        };
        let grace_secs = self.gc_config.retention_grace_secs();
        let max_deletions = self.gc_config.max_deletions_per_run() as usize;
        if max_deletions == 0 {
            global_or_default().inc_sorafs_gc_runs(RESULT_SUCCESS);
            global_sorafs_gc_otel().record_run(RESULT_SUCCESS);
            return report;
        }

        let Some(provider_id) = self.capacity_usage().provider_id else {
            report.errors = report.errors.saturating_add(1);
            iroha_logger::error!(
                "GC sweep skipped: provider identity is unavailable for repair-ledger filtering"
            );
            global_or_default().inc_sorafs_gc_runs(RESULT_ERROR);
            global_sorafs_gc_otel().record_run(RESULT_ERROR);
            return report;
        };
        let active_repair_tasks = repair_projection
            .active_tasks_for_provider(provider_id)
            .collect::<Vec<_>>();

        let mut expired = Vec::new();
        let mut expired_count = 0u64;
        let mut oldest_expired_age: Option<u64> = None;

        for manifest in storage.manifests() {
            let retention_epoch = manifest.retention_epoch();
            if retention_epoch == 0 {
                continue;
            }
            let expires_at = retention_epoch.saturating_add(grace_secs);
            if now_unix < expires_at {
                continue;
            }
            expired_count = expired_count.saturating_add(1);
            let age_secs = now_unix.saturating_sub(expires_at);
            oldest_expired_age =
                Some(oldest_expired_age.map_or(age_secs, |prev| prev.max(age_secs)));
            expired.push(manifest);
        }

        expired.sort_by(|left, right| {
            left.retention_epoch()
                .cmp(&right.retention_epoch())
                .then(left.manifest_id().cmp(right.manifest_id()))
        });

        let mut evicted_count = 0usize;

        for manifest in expired {
            if evicted_count >= max_deletions {
                report.skipped.push(GcSkip {
                    manifest_id: manifest.manifest_id().to_owned(),
                    reason: REASON_LIMIT_REACHED.to_string(),
                });
                break;
            }

            let digest = *manifest.manifest_digest();
            if active_repair_tasks
                .iter()
                .any(|task| task.manifest_digest == digest)
            {
                report.skipped.push(GcSkip {
                    manifest_id: manifest.manifest_id().to_owned(),
                    reason: GC_AUDIT_BLOCKED_REPAIR_ACTIVE_V1.to_string(),
                });
                iroha_logger::warn!(
                    manifest_id = %manifest.manifest_id(),
                    "GC retention blocked by active repair tasks"
                );
                global_or_default().inc_sorafs_gc_blocked(GC_AUDIT_BLOCKED_REPAIR_ACTIVE_V1);
                global_sorafs_gc_otel().record_blocked(GC_AUDIT_BLOCKED_REPAIR_ACTIVE_V1);
                let payload = GcAuditPayloadV1 {
                    version: GC_AUDIT_PAYLOAD_VERSION_V1,
                    manifest_digest: digest,
                    provider_id,
                    evicted_at_unix: now_unix,
                    freed_bytes: 0,
                    reason: GC_AUDIT_REASON_RETENTION_EXPIRED_V1.to_string(),
                    blocked_reason: Some(GC_AUDIT_BLOCKED_REPAIR_ACTIVE_V1.to_string()),
                };
                if let Err(err) = self.publish_gc_audit_event(payload) {
                    report.errors = report.errors.saturating_add(1);
                    iroha_logger::error!(
                        %err,
                        manifest_id = %manifest.manifest_id(),
                        "GC blocked outcome could not be durably audited"
                    );
                    break;
                }
                continue;
            }

            if manifest
                .retention_source()
                .and_then(|source| source.deal_end_epoch)
                .is_some_and(|deal_end| deal_end > now_unix)
            {
                report.skipped.push(GcSkip {
                    manifest_id: manifest.manifest_id().to_owned(),
                    reason: GC_AUDIT_BLOCKED_DEAL_ACTIVE_V1.to_string(),
                });
                iroha_logger::warn!(
                    manifest_id = %manifest.manifest_id(),
                    "GC retention blocked by active deal window"
                );
                global_or_default().inc_sorafs_gc_blocked(GC_AUDIT_BLOCKED_DEAL_ACTIVE_V1);
                global_sorafs_gc_otel().record_blocked(GC_AUDIT_BLOCKED_DEAL_ACTIVE_V1);
                let payload = GcAuditPayloadV1 {
                    version: GC_AUDIT_PAYLOAD_VERSION_V1,
                    manifest_digest: digest,
                    provider_id,
                    evicted_at_unix: now_unix,
                    freed_bytes: 0,
                    reason: GC_AUDIT_REASON_RETENTION_EXPIRED_V1.to_string(),
                    blocked_reason: Some(GC_AUDIT_BLOCKED_DEAL_ACTIVE_V1.to_string()),
                };
                if let Err(err) = self.publish_gc_audit_event(payload) {
                    report.errors = report.errors.saturating_add(1);
                    iroha_logger::error!(
                        %err,
                        manifest_id = %manifest.manifest_id(),
                        "GC blocked outcome could not be durably audited"
                    );
                    break;
                }
                continue;
            }

            match storage.manifest_has_shared_chunks(manifest.manifest_id()) {
                Ok(true) => {
                    report.skipped.push(GcSkip {
                        manifest_id: manifest.manifest_id().to_owned(),
                        reason: GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1.to_string(),
                    });
                    iroha_logger::warn!(
                        manifest_id = %manifest.manifest_id(),
                        "GC retention blocked by shared chunks"
                    );
                    global_or_default().inc_sorafs_gc_blocked(GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1);
                    global_sorafs_gc_otel().record_blocked(GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1);
                    let payload = GcAuditPayloadV1 {
                        version: GC_AUDIT_PAYLOAD_VERSION_V1,
                        manifest_digest: digest,
                        provider_id,
                        evicted_at_unix: now_unix,
                        freed_bytes: 0,
                        reason: GC_AUDIT_REASON_RETENTION_EXPIRED_V1.to_string(),
                        blocked_reason: Some(GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1.to_string()),
                    };
                    if let Err(err) = self.publish_gc_audit_event(payload) {
                        report.errors = report.errors.saturating_add(1);
                        iroha_logger::error!(
                            %err,
                            manifest_id = %manifest.manifest_id(),
                            "GC blocked outcome could not be durably audited"
                        );
                        break;
                    }
                    continue;
                }
                Ok(false) => {}
                Err(err) => {
                    report.errors = report.errors.saturating_add(1);
                    iroha_logger::warn!(
                        %err,
                        manifest_id = %manifest.manifest_id(),
                        "GC eviction skipped: failed to inspect shared chunks"
                    );
                    continue;
                }
            }

            let reason = GC_AUDIT_REASON_RETENTION_EXPIRED_V1;
            let transaction = match self.evict_manifest_with_gc_audit(
                &gc_guard,
                storage,
                &manifest,
                provider_id,
                now_unix,
                reason,
            ) {
                Ok(outcome) => outcome,
                Err(err) => {
                    report.errors = report.errors.saturating_add(1);
                    iroha_logger::warn!(
                        %err,
                        manifest_id = %manifest.manifest_id(),
                        "GC eviction transaction failed"
                    );
                    break;
                }
            };
            let freed_bytes = transaction.freed_bytes;
            if let Some(err) = transaction.publish_error {
                report.errors = report.errors.saturating_add(1);
                iroha_logger::warn!(
                    error = %err,
                    manifest_id = %manifest.manifest_id(),
                    "GC eviction audit remains durably pending"
                );
            }

            evicted_count += 1;
            report.freed_bytes = report.freed_bytes.saturating_add(freed_bytes);
            report.evictions.push(GcEviction {
                manifest_id: manifest.manifest_id().to_owned(),
                manifest_digest: digest,
                retention_epoch: manifest.retention_epoch(),
                freed_bytes,
                reason: reason.to_string(),
            });

            global_or_default().inc_sorafs_gc_evictions(reason);
            global_or_default().add_sorafs_gc_freed_bytes(reason, freed_bytes);
            global_sorafs_gc_otel().record_eviction(reason, freed_bytes);
        }

        global_or_default()
            .set_sorafs_gc_expired_snapshot(expired_count, oldest_expired_age.unwrap_or(0));

        let result = if report.errors == 0 {
            RESULT_SUCCESS
        } else {
            RESULT_ERROR
        };
        global_or_default().inc_sorafs_gc_runs(result);
        global_sorafs_gc_otel().record_run(result);
        self.schedulers
            .update_storage_bytes(storage.total_bytes(), self.config.max_capacity_bytes().0);

        report
    }

    /// Run a reconciliation snapshot across repair and GC state.
    pub fn run_reconciliation_once(
        &self,
        now_unix: u64,
        repair_projection: &RepairLedgerTaskProjectionV1,
    ) -> Result<SorafsReconciliationReportV1, ReconciliationError> {
        let result = self.compute_reconciliation_report(now_unix, repair_projection);
        match &result {
            Ok(report) => {
                self.publish_reconciliation_report(report);
                global_or_default().inc_sorafs_reconciliation_runs("success");
                global_or_default()
                    .set_sorafs_reconciliation_divergence_count(u64::from(report.divergence_count));
                global_sorafs_reconciliation_otel().record_run("success");
                global_sorafs_reconciliation_otel()
                    .record_divergence(u64::from(report.divergence_count));
            }
            Err(_) => {
                global_or_default().inc_sorafs_reconciliation_runs("error");
                global_sorafs_reconciliation_otel().record_run("error");
            }
        }
        result
    }

    fn compute_reconciliation_report(
        &self,
        now_unix: u64,
        repair_projection: &RepairLedgerTaskProjectionV1,
    ) -> Result<SorafsReconciliationReportV1, ReconciliationError> {
        if now_unix == 0 {
            return Err(ReconciliationError::InvalidTimestamp);
        }
        let Some(storage) = self.storage.as_ref() else {
            return Err(ReconciliationError::StorageDisabled);
        };

        let provider_id = self
            .capacity_usage()
            .provider_id
            .ok_or(ReconciliationError::ProviderBindingUnavailable)?;

        let repair_snapshot = reconciliation::RepairReconciliationSnapshot {
            version: reconciliation::RECONCILIATION_SNAPSHOT_VERSION_V1,
            finalized_cursor: repair_projection.finalized_cursor(),
            tasks: repair_projection.tasks().to_vec(),
        };
        let repair_snapshot_hash = reconciliation::hash_snapshot(&repair_snapshot)?;

        let manifests = storage.manifests();
        let mut retention_entries = manifests
            .iter()
            .map(|manifest| reconciliation::RetentionReconciliationEntry {
                manifest_id: manifest.manifest_id().to_string(),
                manifest_digest: *manifest.manifest_digest(),
                retention_epoch: manifest.retention_epoch(),
                retention_source: manifest.retention_source().cloned(),
            })
            .collect::<Vec<_>>();
        retention_entries.sort_by(|left, right| {
            left.manifest_id
                .cmp(&right.manifest_id)
                .then(left.manifest_digest.cmp(&right.manifest_digest))
        });
        let retention_snapshot = reconciliation::RetentionReconciliationSnapshot {
            version: reconciliation::RECONCILIATION_SNAPSHOT_VERSION_V1,
            manifests: retention_entries,
        };
        let retention_snapshot_hash = reconciliation::hash_snapshot(&retention_snapshot)?;

        let (gc_freed_bytes_total, gc_evictions_total) = storage.gc_counters();
        let mut chunk_refcounts = storage.chunk_refcount_snapshot();
        chunk_refcounts.sort_by(|left, right| left.digest.cmp(&right.digest));
        let gc_snapshot = reconciliation::GcReconciliationSnapshot {
            version: reconciliation::RECONCILIATION_SNAPSHOT_VERSION_V1,
            gc_freed_bytes_total,
            gc_evictions_total,
            chunk_refcounts,
        };
        let gc_snapshot_hash = reconciliation::hash_snapshot(&gc_snapshot)?;

        let divergence_count = reconciliation_divergence_count(storage, &manifests);
        let appeal_finance = self.appeal_finance_reconciliation_summary()?;

        let report = SorafsReconciliationReportV1 {
            version: SORAFS_RECONCILIATION_REPORT_VERSION_V1,
            provider_id,
            generated_at_unix: now_unix,
            repair_snapshot_hash,
            retention_snapshot_hash,
            gc_snapshot_hash,
            repair_task_count: u32::try_from(repair_projection.len()).unwrap_or(u32::MAX),
            retention_manifest_count: u32::try_from(manifests.len()).unwrap_or(u32::MAX),
            gc_evictions_total,
            gc_freed_bytes_total,
            divergence_count,
            appeal_finance,
        };
        report.validate()?;
        Ok(report)
    }

    fn appeal_finance_reconciliation_summary(
        &self,
    ) -> Result<Option<AppealFinanceReconciliationSummaryV1>, ReconciliationError> {
        let Some(governance_dir) = self.config.governance_dir() else {
            return Ok(None);
        };
        let index_path = governance_dir.join(GOVERNANCE_PUBLISH_INDEX_FILE);
        let index = match fs::read(&index_path) {
            Ok(bytes) => norito::json::from_slice::<JsonValue>(&bytes).map_err(|err| {
                ReconciliationError::AppealFinance(format!(
                    "failed to decode governance publish index `{}`: {err}",
                    index_path.display()
                ))
            })?,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return empty_appeal_finance_reconciliation_summary().map(Some);
            }
            Err(err) => {
                return Err(ReconciliationError::AppealFinance(format!(
                    "failed to read governance publish index `{}`: {err}",
                    index_path.display()
                )));
            }
        };
        if index.get("schema").and_then(JsonValue::as_str) != Some(GOVERNANCE_PUBLISH_INDEX_SCHEMA)
        {
            return Err(ReconciliationError::AppealFinance(
                "governance publish index uses an unsupported schema".to_string(),
            ));
        }

        let mut entries = Vec::new();
        let Some(index_entries) = index.get("entries").and_then(JsonValue::as_array) else {
            return Err(ReconciliationError::AppealFinance(
                "governance publish index is missing `entries` array".to_string(),
            ));
        };
        for entry in index_entries {
            if entry.get("payload_kind").and_then(JsonValue::as_str)
                != Some(APPEAL_FINANCE_WEEKLY_ROLLUP_KIND)
            {
                continue;
            }
            entries.push(appeal_finance_rollup_reconciliation_entry(
                governance_dir,
                entry,
            )?);
        }
        entries.sort_by(|left, right| {
            left.cycle
                .cmp(&right.cycle)
                .then(left.encoded_blake3.cmp(&right.encoded_blake3))
        });

        let snapshot = reconciliation::AppealFinanceRollupReconciliationSnapshot {
            version: reconciliation::RECONCILIATION_SNAPSHOT_VERSION_V1,
            rollups: entries.clone(),
        };
        let rollup_snapshot_hash = reconciliation::hash_snapshot(&snapshot)?;
        let mut total_treasury_xor = XorQuantity::zero();
        let mut total_rewards_forfeited_treasury_xor = XorQuantity::zero();
        let mut source_report_count = 0_u64;
        let mut case_count = 0_u64;
        for entry in &entries {
            source_report_count = source_report_count
                .checked_add(entry.report_count)
                .ok_or_else(|| {
                    ReconciliationError::AppealFinance("source report count overflow".to_string())
                })?;
            case_count = case_count.checked_add(entry.case_count).ok_or_else(|| {
                ReconciliationError::AppealFinance("appeal case count overflow".to_string())
            })?;
            total_treasury_xor = total_treasury_xor
                .checked_add(&entry.total_treasury_xor)
                .map_err(|error| {
                    ReconciliationError::AppealFinance(format!(
                        "treasury total arithmetic failed: {error}"
                    ))
                })?;
            total_rewards_forfeited_treasury_xor = total_rewards_forfeited_treasury_xor
                .checked_add(&entry.total_rewards_forfeited_treasury_xor)
                .map_err(|error| {
                    ReconciliationError::AppealFinance(format!(
                        "forfeited reward total arithmetic failed: {error}"
                    ))
                })?;
        }

        Ok(Some(AppealFinanceReconciliationSummaryV1 {
            rollup_snapshot_hash,
            rollup_count: u32::try_from(entries.len()).unwrap_or(u32::MAX),
            source_report_count,
            case_count,
            total_treasury_xor,
            total_rewards_forfeited_treasury_xor,
        }))
    }

    /// Whether the storage worker is currently enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.storage.is_some()
    }

    /// Record a capacity declaration captured by Torii.
    pub fn record_capacity_declaration(
        &self,
        record: &CapacityDeclarationRecord,
    ) -> Result<(), CapacityError> {
        self.mutate_capacity_durably(|capacity| {
            capacity.record_declaration(record).map(|()| ((), true))
        })?;
        let window = DeclarationWindow {
            registered_epoch: record.registered_epoch,
            valid_from_epoch: record.valid_from_epoch,
            valid_until_epoch: record.valid_until_epoch,
        };
        self.meter
            .reset_for_declaration(record.committed_capacity_gib, window);
        self.seed_telemetry_accumulator(record);
        Ok(())
    }

    /// Return a snapshot describing the currently tracked capacity usage.
    #[must_use]
    pub fn capacity_usage(&self) -> CapacityUsageSnapshot {
        self.capacity.usage_snapshot()
    }

    /// Schedule a replication order if the active declaration matches the provider.
    pub fn schedule_replication_order(
        &self,
        order: &ReplicationOrderV1,
    ) -> Result<Option<ReplicationPlan>, CapacityError> {
        let maybe_plan = self.mutate_capacity_durably(|capacity| {
            capacity.schedule_order(order).map(|plan| {
                let changed = plan.is_some();
                (plan, changed)
            })
        })?;
        if let Some(plan) = maybe_plan.as_ref() {
            self.meter.on_order_scheduled(plan);
        }
        Ok(maybe_plan)
    }

    /// Mark a replication order as completed and release its reserved capacity.
    pub fn complete_replication_order(
        &self,
        order_id: [u8; 32],
    ) -> Result<ReplicationRelease, CapacityError> {
        let release = self.mutate_capacity_durably(|capacity| {
            capacity
                .complete_order(order_id)
                .map(|release| (release, true))
        })?;
        let usage_sample = self.meter.on_order_completed(&release);
        self.record_replication_success(order_id, usage_sample);
        Ok(release)
    }

    /// Expose the underlying capacity manager for advanced integrations.
    #[must_use]
    pub fn capacity_manager(&self) -> Arc<CapacityManager> {
        self.capacity.clone()
    }

    /// Expose the capacity meter so callers can populate telemetry windows.
    #[must_use]
    pub fn capacity_meter(&self) -> CapacityMeter {
        self.meter.clone()
    }

    /// Access the telemetry accumulator backing the current declaration.
    #[must_use]
    pub fn telemetry_handle(&self) -> Arc<RwLock<Option<TelemetryAccumulator>>> {
        self.telemetry.clone()
    }

    /// Expose the scheduler runtime for telemetry and queue inspection.
    #[must_use]
    pub fn schedulers(&self) -> StorageSchedulersRuntime {
        self.schedulers.clone()
    }

    /// Record a governance-issued PoR challenge from the trusted scheduler.
    ///
    /// External callers must not treat structural challenge validation as
    /// beacon or VRF authentication. Torii retires direct challenge ingestion;
    /// the coordinator scheduler is the production authority for this method.
    pub fn record_por_challenge(&self, challenge: &PorChallengeV1) -> Result<(), PorTrackerError> {
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            PorTrackerError::RuntimeCheckpoint(
                "auxiliary checkpoint transaction lock poisoned".to_owned(),
            )
        })?;
        self.ensure_durability_healthy()
            .map_err(PorTrackerError::RuntimeCheckpoint)?;
        let previous = self.por.checkpoint();
        self.por.record_challenge(challenge)?;
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(PorTrackerError::RuntimeCheckpoint(err.to_string()));
            }
            if let Err(rollback) = self.por.restore_checkpoint(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back PoR challenge after checkpoint error",
                    rollback,
                );
                return Err(PorTrackerError::RuntimeCheckpoint(message));
            }
            return Err(PorTrackerError::RuntimeCheckpoint(err.to_string()));
        }
        Ok(())
    }

    /// Record a provider PoR proof response bound to its admitted provider key.
    pub fn record_por_proof(
        &self,
        proof: &PorProofV1,
        admitted_provider_key: &[u8],
    ) -> Result<(), PorTrackerError> {
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            PorTrackerError::RuntimeCheckpoint(
                "auxiliary checkpoint transaction lock poisoned".to_owned(),
            )
        })?;
        self.ensure_durability_healthy()
            .map_err(PorTrackerError::RuntimeCheckpoint)?;
        let previous = self.por.checkpoint();
        self.por.record_proof(proof, admitted_provider_key)?;
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(PorTrackerError::RuntimeCheckpoint(err.to_string()));
            }
            if let Err(rollback) = self.por.restore_checkpoint(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back PoR proof after checkpoint error",
                    rollback,
                );
                return Err(PorTrackerError::RuntimeCheckpoint(message));
            }
            return Err(PorTrackerError::RuntimeCheckpoint(err.to_string()));
        }
        Ok(())
    }

    /// Return process-local PoR latency, VRF, and seed-binding metrics.
    #[must_use]
    pub fn por_protocol_metrics(&self) -> PorProtocolMetricsSnapshot {
        self.por.protocol_metrics()
    }

    /// Record an audit verdict and update telemetry counters accordingly.
    pub fn record_por_verdict(
        &self,
        verdict: &AuditVerdictV1,
        trusted_auditor_keys: &[Vec<u8>],
        auditor_threshold: usize,
        repair_handoff: &dyn PorRepairHandoff,
    ) -> Result<PorVerdictOutcome, PorTrackerError> {
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            PorTrackerError::RuntimeCheckpoint(
                "auxiliary checkpoint transaction lock poisoned".to_owned(),
            )
        })?;
        self.ensure_durability_healthy()
            .map_err(PorTrackerError::RuntimeCheckpoint)?;
        {
            let history = self.por_history.read().map_err(|_| {
                PorTrackerError::RuntimeCheckpoint("PoR history lock poisoned".to_owned())
            })?;
            let key = (verdict.manifest_digest, verdict.provider_id);
            let limit = self.config.runtime_retention().state_entry_limit();
            if !history.contains_key(&key) && history.len() >= limit {
                return Err(PorTrackerError::RuntimeCheckpoint(format!(
                    "PoR history retention exhausted (limit {limit})"
                )));
            }
        }
        let previous_tracker = self.por.checkpoint();
        let transition = self.por.record_verdict_with(
            verdict,
            trusted_auditor_keys,
            auditor_threshold,
            |intent| repair_handoff.enqueue_failed_por_repair(intent),
        )?;
        let stats = transition.stats;
        if !transition.newly_finalized {
            let consecutive_failures = self
                .por_history
                .read()
                .map_err(|_| {
                    PorTrackerError::RuntimeCheckpoint("PoR history lock poisoned".to_owned())
                })?
                .get(&(verdict.manifest_digest, verdict.provider_id))
                .map_or(0, |entry| entry.consecutive_failures);
            return Ok(PorVerdictOutcome {
                stats,
                repair_task_id: transition.repair_task_id,
                consecutive_failures,
                slash: None,
            });
        }
        let previous_history = self
            .por_history
            .read()
            .map_err(|_| {
                PorTrackerError::RuntimeCheckpoint("PoR history lock poisoned".to_owned())
            })?
            .clone();
        let (consecutive_failures, slash) =
            match self
                .update_por_history_entry(verdict)
                .and_then(|consecutive_failures| {
                    self.evaluate_por_penalty(verdict, &stats, consecutive_failures)
                        .map(|slash| (consecutive_failures, slash))
                }) {
                Ok(outcome) => outcome,
                Err(error) => {
                    if let Err(rollback) =
                        self.rollback_por_verdict_state(previous_tracker, previous_history)
                    {
                        let message = self.record_unrecoverable_rollback(
                            "failed to roll back finalized PoR state after bookkeeping error",
                            rollback,
                        );
                        return Err(PorTrackerError::RuntimeCheckpoint(message));
                    }
                    return Err(error);
                }
            };
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(PorTrackerError::RuntimeCheckpoint(err.to_string()));
            }
            if let Err(rollback) =
                self.rollback_por_verdict_state(previous_tracker, previous_history)
            {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back finalized PoR state after checkpoint error",
                    rollback,
                );
                return Err(PorTrackerError::RuntimeCheckpoint(message));
            }
            return Err(PorTrackerError::RuntimeCheckpoint(err.to_string()));
        }
        if stats.success_samples > 0 {
            self.meter.record_por_samples(stats.success_samples, 0);
        }
        if stats.failed_samples > 0 {
            self.meter.record_por_samples(0, stats.failed_samples);
        }
        self.schedulers
            .record_por_samples(stats.success_samples, stats.failed_samples);
        Ok(PorVerdictOutcome {
            stats,
            repair_task_id: transition.repair_task_id,
            consecutive_failures: slash.as_ref().map_or(consecutive_failures, |_| 0),
            slash,
        })
    }

    /// Attach stripe layout and chunk-role metadata to a stored manifest.
    pub fn attach_stripe_layout(
        &self,
        manifest_id: &str,
        stripe_layout: DaStripeLayout,
        chunk_roles: Vec<ChunkRoleMetadata>,
    ) -> Result<(), NodeStorageError> {
        let storage = self.storage.as_ref().ok_or(NodeStorageError::Disabled)?;
        storage.attach_stripe_layout(manifest_id, stripe_layout, chunk_roles)?;
        Ok(())
    }

    /// Generate PoR challenges for all stored manifests using the supplied randomness inputs.
    pub fn plan_por_challenges(
        &self,
        randomness: PorRandomness,
        vrf_records: &HashMap<ManifestVrfKey, ManifestVrfBundle>,
    ) -> Result<Vec<PlannedChallenge>, PorChallengePlannerError> {
        self.plan_por_challenges_with_forced_policy(randomness, vrf_records, true)
    }

    /// Generate PoR challenges while explicitly controlling whether missing
    /// provider VRFs may enter the forced-challenge path.
    pub fn plan_por_challenges_with_forced_policy(
        &self,
        randomness: PorRandomness,
        vrf_records: &HashMap<ManifestVrfKey, ManifestVrfBundle>,
        allow_forced: bool,
    ) -> Result<Vec<PlannedChallenge>, PorChallengePlannerError> {
        let storage = self
            .storage
            .as_ref()
            .ok_or(PorChallengePlannerError::StorageDisabled)?;
        let usage = self.capacity_usage();
        let provider_id = usage
            .provider_id
            .ok_or(PorChallengePlannerError::ProviderUnavailable)?;
        let sample_policy = por::PorSamplePolicy::from_metadata(provider_id, &usage.metadata)?;
        let grace_secs = self.gc_config.retention_grace_secs();
        let issued_at = randomness.issued_at_unix;

        let manifests = storage.manifests();
        let mut challenges = Vec::with_capacity(manifests.len());

        for manifest in manifests {
            let retention_epoch = manifest.retention_epoch();
            if retention_epoch != 0 {
                let expires_at = retention_epoch.saturating_add(grace_secs);
                if issued_at >= expires_at {
                    continue;
                }
            }
            let digest = *manifest.manifest_digest();
            let vrf = vrf_records.get(&ManifestVrfKey {
                provider_id,
                manifest_digest: digest,
            });
            let planned = build_por_challenge_for_manifest(
                &manifest,
                provider_id,
                &randomness,
                vrf,
                &sample_policy,
                allow_forced,
            )?;
            challenges.push(planned);
        }

        challenges.sort_by_key(|planned| planned.challenge.challenge_id);

        Ok(challenges)
    }

    /// Record a PoTR receipt captured by the gateway.
    pub fn record_potr_receipt(
        &self,
        receipt: PotrReceiptV1,
        gateway_public_key: &[u8; 32],
        admission: &AdmissionRecord,
    ) -> Result<PotrRecordOutcome, PotrTrackerError> {
        self.potr
            .record_receipt(receipt, gateway_public_key, admission, self)
    }

    /// Record a PoTR receipt using an explicit chain-authoritative repair handoff.
    pub fn record_potr_receipt_with_handoff(
        &self,
        receipt: PotrReceiptV1,
        gateway_public_key: &[u8; 32],
        admission: &AdmissionRecord,
        repair_handoff: &dyn potr::PotrLatencyRepairHandoff,
    ) -> Result<PotrRecordOutcome, PotrTrackerError> {
        self.potr
            .record_receipt(receipt, gateway_public_key, admission, repair_handoff)
    }

    /// Retrieve PoTR receipts matching the manifest/provider filters.
    pub fn potr_receipts(
        &self,
        manifest_digest: &[u8; 32],
        provider_id: &[u8; 32],
        tier: Option<ProofStreamTier>,
    ) -> Result<Vec<PotrReceiptV1>, PotrTrackerError> {
        self.potr.receipts_for(manifest_digest, provider_id, tier)
    }

    /// Return status for one exact final signed PoTR receipt.
    pub fn potr_receipt_status(
        &self,
        receipt_digest: &[u8; 32],
    ) -> Result<Option<PotrReceiptStatusV1>, PotrTrackerError> {
        self.potr.status(receipt_digest)
    }

    /// Export a bounded sequence-ordered page of PoTR receipt statuses.
    pub fn export_potr_receipt_statuses(
        &self,
        after_sequence: u64,
        limit: usize,
    ) -> Result<Vec<PotrReceiptStatusV1>, PotrTrackerError> {
        self.potr.export_statuses(after_sequence, limit)
    }

    /// Export a bounded sequence-ordered page of exact final signed receipts.
    pub fn export_potr_receipts(
        &self,
        after_sequence: u64,
        limit: usize,
    ) -> Result<Vec<PotrReceiptV1>, PotrTrackerError> {
        self.potr.export_receipts(after_sequence, limit)
    }

    /// Returns a clone of the persistent storage backend when enabled.
    #[must_use]
    pub fn storage(&self) -> Option<Arc<StorageBackend>> {
        self.storage.clone()
    }

    /// Ingest a manifest payload into the local storage backend.
    pub fn ingest_manifest<R: Read>(
        &self,
        manifest: &ManifestV1,
        plan: &CarBuildPlan,
        reader: &mut R,
    ) -> Result<String, NodeStorageError> {
        let layout = Self::derive_layout_and_roles(plan);
        let (stripe_layout, chunk_roles) = match layout {
            Some((layout, roles)) => (Some(layout), Some(roles)),
            None => (None, None),
        };
        self.ingest_manifest_with_layout(manifest, plan, reader, stripe_layout, chunk_roles)
    }

    /// Ingest a manifest payload with optional stripe layout and chunk-role annotations.
    pub fn ingest_manifest_with_layout<R: Read>(
        &self,
        manifest: &ManifestV1,
        plan: &CarBuildPlan,
        reader: &mut R,
        stripe_layout: Option<DaStripeLayout>,
        chunk_roles: Option<Vec<ChunkRoleMetadata>>,
    ) -> Result<String, NodeStorageError> {
        let storage = self.storage_backend()?;
        let result = self.schedulers.try_with_pin(|| {
            storage.ingest_manifest_with_layout(manifest, plan, reader, stripe_layout, chunk_roles)
        })?;
        match result {
            Ok(manifest_id) => {
                self.schedulers.update_storage_bytes(
                    storage.total_bytes(),
                    self.config.max_capacity_bytes().0,
                );
                Ok(manifest_id)
            }
            Err(err) => Err(NodeStorageError::from(err)),
        }
    }

    /// Read a byte range from a stored manifest payload.
    pub fn read_payload_range(
        &self,
        manifest_id: &str,
        offset: u64,
        len: usize,
    ) -> Result<Vec<u8>, NodeStorageError> {
        let storage = self.storage_backend()?;
        self.schedulers
            .try_run_fetch(len as u64, None, || {
                storage.read_payload_range(manifest_id, offset, len)
            })
            .map_err(NodeStorageError::from)?
            .map_err(NodeStorageError::from)
    }

    /// Retrieve stored manifest metadata.
    pub fn manifest_metadata(&self, manifest_id: &str) -> Result<StoredManifest, NodeStorageError> {
        let storage = self.storage_backend()?;
        storage.manifest(manifest_id).ok_or_else(|| {
            NodeStorageError::from(StorageError::ManifestNotFound {
                manifest_id: manifest_id.to_owned(),
            })
        })
    }

    /// Retrieve stored manifest metadata by digest.
    pub fn manifest_metadata_by_digest(
        &self,
        digest: &[u8; 32],
    ) -> Result<StoredManifest, NodeStorageError> {
        let storage = self.storage_backend()?;
        storage.manifest_by_digest(digest).ok_or_else(|| {
            NodeStorageError::from(StorageError::ManifestNotFound {
                manifest_id: hex::encode(digest),
            })
        })
    }

    /// Return stored manifest metadata ordered deterministically by manifest digest then identifier.
    pub fn stored_manifests(&self) -> Result<Vec<StoredManifest>, NodeStorageError> {
        let storage = self.storage_backend()?;
        let mut manifests = storage.manifests();
        manifests.sort_by(|left, right| {
            left.manifest_digest()
                .cmp(right.manifest_digest())
                .then_with(|| left.manifest_id().cmp(right.manifest_id()))
        });
        Ok(manifests)
    }

    fn derive_layout_and_roles(
        plan: &CarBuildPlan,
    ) -> Option<(DaStripeLayout, Vec<ChunkRoleMetadata>)> {
        if plan.chunks.is_empty() {
            return None;
        }
        let layout = DaStripeLayout {
            total_stripes: 1,
            shards_per_stripe: plan.chunks.len() as u32,
            row_parity_stripes: 0,
        };
        let roles = plan
            .chunks
            .iter()
            .enumerate()
            .map(|(idx, _)| ChunkRoleMetadata {
                role: iroha_data_model::da::manifest::ChunkRole::Data,
                group_id: idx as u32,
            })
            .collect();
        Some((layout, roles))
    }

    /// Retrieve chunk metadata identified by its digest.
    pub fn chunk_by_digest(
        &self,
        manifest_id: &str,
        digest: &[u8; 32],
    ) -> Result<ChunkFileRecord, NodeStorageError> {
        let storage = self.storage_backend()?;
        storage
            .chunk_by_digest(manifest_id, digest)
            .map_err(NodeStorageError::from)
    }

    /// Read chunk bytes identified by digest, returning metadata alongside the payload.
    pub fn read_chunk_by_digest(
        &self,
        manifest_id: &str,
        digest: &[u8; 32],
    ) -> Result<(ChunkFileRecord, Vec<u8>), NodeStorageError> {
        let storage = self.storage_backend()?;
        let record = storage
            .chunk_by_digest(manifest_id, digest)
            .map_err(NodeStorageError::from)?;
        let bytes = self
            .schedulers
            .try_run_fetch(record.length as u64, None, || {
                storage.read_chunk(manifest_id, digest)
            })
            .map_err(NodeStorageError::from)?
            .map_err(NodeStorageError::from)?;
        Ok((record, bytes))
    }

    /// Sample Proof-of-Retrievability leaves for a stored manifest.
    pub fn sample_por(
        &self,
        manifest_id: &str,
        count: usize,
        seed: u64,
    ) -> Result<Vec<(usize, PorProof)>, NodeStorageError> {
        let storage = self.storage_backend()?;
        let result = self
            .schedulers
            .try_with_por(|| storage.sample_por(manifest_id, count, seed))?;
        match result {
            Ok(samples) => {
                self.schedulers.record_por_samples(samples.len() as u64, 0);
                Ok(samples)
            }
            Err(err) => {
                self.schedulers.record_por_samples(0, count as u64);
                Err(NodeStorageError::from(err))
            }
        }
    }

    /// Produce a snapshot of the current metering counters.
    #[must_use]
    pub fn metering_snapshot(&self) -> MeteringSnapshot {
        self.meter.snapshot()
    }

    /// Record an uptime observation for the current declaration.
    pub fn record_uptime_observation(&self, uptime_secs: u64, observed_secs: u64) {
        let success = uptime_secs >= observed_secs && observed_secs > 0;
        self.meter.record_uptime_sample(success);
        if let Ok(mut guard) = self.telemetry.write()
            && let Some(acc) = guard.as_mut()
        {
            let _ = acc.record_uptime_sample(uptime_secs.min(observed_secs), observed_secs);
        }
    }

    /// Record a proof-of-retrievability observation for the current declaration.
    pub fn record_por_observation(&self, success: bool) {
        self.meter.record_por_sample(success);
        if let Ok(mut guard) = self.telemetry.write()
            && let Some(acc) = guard.as_mut()
        {
            acc.record_por_sample(success);
        }
    }

    /// Return the PoR ingestion status for the supplied manifest digest.
    pub fn por_ingestion_status(
        &self,
        manifest_digest: &[u8; 32],
    ) -> Result<PorIngestionStatus, NodeStorageError> {
        if !self.is_enabled() {
            return Err(NodeStorageError::Disabled);
        }
        let backlog = self.por.backlog_for_manifest(manifest_digest);
        let mut statuses = Self::build_ingestion_status_map(backlog);
        self.apply_por_history(&mut statuses, Some(manifest_digest));
        let mut providers: Vec<_> = statuses.into_values().collect();
        providers.sort_by_key(|entry| entry.provider_id);
        Ok(PorIngestionStatus {
            manifest_digest: *manifest_digest,
            providers,
        })
    }

    /// Return the PoR ingestion status for all manifests tracked by the node.
    #[must_use]
    pub fn por_ingestion_overview(&self) -> Vec<PorIngestionProviderStatus> {
        if !self.is_enabled() {
            return Vec::new();
        }
        let backlog = self.por.backlog_entries();
        let mut statuses = Self::build_ingestion_status_map(backlog);
        self.apply_por_history(&mut statuses, None);
        let mut entries: Vec<_> = statuses.into_values().collect();
        entries.sort_by(|left, right| {
            left.manifest_digest
                .cmp(&right.manifest_digest)
                .then(left.provider_id.cmp(&right.provider_id))
        });
        entries
    }

    /// Record that a replication order ultimately failed and release the reservation.
    ///
    /// Returns `true` when the order was tracked and the failure counters were updated.
    pub fn record_replication_failure(&self, order_id: [u8; 32]) -> bool {
        let Some(sample) = self.meter.record_replication_failure(order_id) else {
            return false;
        };
        if let Ok(mut guard) = self.telemetry.write()
            && let Some(acc) = guard.as_mut()
        {
            acc.record_replication_failure();
            if sample.slice_gib > 0 && sample.duration_secs > 0 {
                let _ = acc.record_utilisation(sample.slice_gib, sample.duration_secs);
            }
        }
        true
    }

    /// Build the current Norito telemetry payload, if the accumulator is initialised.
    pub fn build_capacity_telemetry(&self) -> Option<Result<CapacityTelemetryV1, TelemetryError>> {
        let guard = self.telemetry.read().ok()?;
        guard.as_ref().map(TelemetryAccumulator::build_payload)
    }

    /// Mutate the telemetry accumulator if it has been initialised.
    pub fn update_telemetry<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut TelemetryAccumulator) -> R,
    {
        let mut guard = self.telemetry.write().ok()?;
        let acc = guard.as_mut()?;
        Some(f(acc))
    }

    fn update_por_history_entry(&self, verdict: &AuditVerdictV1) -> Result<u64, PorTrackerError> {
        let mut history = self.por_history.write().map_err(|_| {
            PorTrackerError::RuntimeCheckpoint("PoR history lock poisoned".to_owned())
        })?;
        let entry = history
            .entry((verdict.manifest_digest, verdict.provider_id))
            .or_default();
        match verdict.outcome {
            AuditOutcomeV1::Success | AuditOutcomeV1::Repaired => {
                entry.last_success_unix = Some(verdict.decided_at);
                entry.consecutive_failures = 0;
            }
            AuditOutcomeV1::Failed => {
                entry.last_failure_unix = Some(verdict.decided_at);
                entry.failures_total = entry.failures_total.saturating_add(1);
                entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
            }
        }
        Ok(entry.consecutive_failures)
    }

    fn rollback_por_verdict_state(
        &self,
        previous_tracker: por::PorTrackerCheckpointV1,
        previous_history: HashMap<PorHistoryKey, PorHistoryEntry>,
    ) -> Result<(), String> {
        let history_rollback = self
            .por_history
            .write()
            .map(|mut history| {
                *history = previous_history;
            })
            .map_err(|_| "PoR history rollback lock poisoned".to_owned());
        let tracker_rollback = self
            .por
            .restore_checkpoint(previous_tracker)
            .map_err(|error| format!("PoR tracker rollback failed: {error}"));
        match (history_rollback, tracker_rollback) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(history), Ok(())) => Err(history),
            (Ok(()), Err(tracker)) => Err(tracker),
            (Err(history), Err(tracker)) => Err(format!("{history}; {tracker}")),
        }
    }

    fn evaluate_por_penalty(
        &self,
        verdict: &AuditVerdictV1,
        stats: &PorVerdictStats,
        consecutive_failures: u64,
    ) -> Result<Option<SlashRecommendation>, PorTrackerError> {
        if stats.failed_samples == 0 {
            return Ok(None);
        }
        let policy = self.config.penalty();
        if consecutive_failures < u64::from(policy.strike_threshold) {
            return Ok(None);
        }

        let mut history = self.por_history.write().map_err(|_| {
            PorTrackerError::RuntimeCheckpoint("PoR history lock poisoned".to_owned())
        })?;
        let entry = history
            .entry((verdict.manifest_digest, verdict.provider_id))
            .or_default();
        if let Some(last_slash) = entry.last_slash_unix {
            let elapsed = verdict.decided_at.saturating_sub(last_slash);
            if elapsed < policy.cooldown_secs {
                return Ok(None);
            }
        }

        let provider_id = ProviderId::new(verdict.provider_id);
        let Some(snapshot) = self.deal_engine.provider_snapshot(provider_id) else {
            return Ok(None);
        };
        let penalty = checked_por_penalty(
            &snapshot.bond_available,
            &snapshot.bond_locked,
            policy.penalty_bond_bps,
        )
        .map_err(|error| PorTrackerError::PenaltyArithmetic(error.to_string()))?;
        if penalty.is_zero() {
            return Ok(None);
        }

        entry.last_slash_unix = Some(verdict.decided_at);
        entry.consecutive_failures = 0;

        Ok(Some(SlashRecommendation {
            provider_id,
            manifest_digest: verdict.manifest_digest,
            penalty,
            strikes: policy.strike_threshold,
            reason: format!(
                "PoR failure streak reached {} (threshold {}), slashing {} bps of bonded collateral",
                consecutive_failures, policy.strike_threshold, policy.penalty_bond_bps
            ),
        }))
    }

    fn build_ingestion_status_map(
        backlog: Vec<por::PorBacklogEntry>,
    ) -> HashMap<PorHistoryKey, PorIngestionProviderStatus> {
        let mut statuses = HashMap::new();
        for entry in backlog {
            let slot = Self::ensure_ingestion_entry(
                &mut statuses,
                entry.manifest_digest,
                entry.provider_id,
            );
            slot.pending_challenges = entry.pending_challenges;
            slot.oldest_epoch_id = entry.oldest_epoch_id;
            slot.oldest_response_deadline_unix = entry.oldest_response_deadline_unix;
        }
        statuses
    }

    fn apply_por_history(
        &self,
        statuses: &mut HashMap<PorHistoryKey, PorIngestionProviderStatus>,
        manifest_filter: Option<&[u8; 32]>,
    ) {
        let Ok(history) = self.por_history.read() else {
            iroha_logger::error!(
                "PoR ingestion history is unavailable because its lock is poisoned"
            );
            return;
        };
        for ((manifest, provider), entry) in history.iter() {
            if manifest_filter.is_some_and(|filter| manifest != filter) {
                continue;
            }
            let slot = Self::ensure_ingestion_entry(statuses, *manifest, *provider);
            slot.last_success_unix = entry.last_success_unix;
            slot.last_failure_unix = entry.last_failure_unix;
            slot.failures_total = entry.failures_total;
            slot.consecutive_failures = entry.consecutive_failures;
        }
    }

    fn ensure_ingestion_entry(
        statuses: &mut HashMap<PorHistoryKey, PorIngestionProviderStatus>,
        manifest_digest: [u8; 32],
        provider_id: [u8; 32],
    ) -> &mut PorIngestionProviderStatus {
        statuses
            .entry((manifest_digest, provider_id))
            .or_insert_with(|| PorIngestionProviderStatus {
                manifest_digest,
                provider_id,
                pending_challenges: 0,
                oldest_epoch_id: None,
                oldest_response_deadline_unix: None,
                last_success_unix: None,
                last_failure_unix: None,
                failures_total: 0,
                consecutive_failures: 0,
            })
    }

    fn storage_backend(&self) -> Result<&StorageBackend, NodeStorageError> {
        let storage = self
            .storage
            .as_ref()
            .map(|arc| arc.as_ref())
            .ok_or(NodeStorageError::Disabled)?;
        storage.ensure_durability_healthy()?;
        Ok(storage)
    }

    fn seed_telemetry_accumulator(&self, record: &CapacityDeclarationRecord) {
        let mut provider_bytes = [0_u8; 32];
        provider_bytes.copy_from_slice(record.provider_id.as_bytes());
        let mut accumulator = TelemetryAccumulator::new(
            provider_bytes,
            record.committed_capacity_gib,
            record.valid_from_epoch,
        );

        let window_end = if record.valid_until_epoch <= record.valid_from_epoch {
            record.valid_from_epoch.saturating_add(1)
        } else {
            record.valid_until_epoch
        };
        let _ = accumulator.set_window_end_epoch(window_end);

        if let Ok(mut guard) = self.telemetry.write() {
            *guard = Some(accumulator);
        }
    }

    fn record_replication_success(&self, _order_id: [u8; 32], usage: ReplicationUsageSample) {
        if let Ok(mut guard) = self.telemetry.write()
            && let Some(acc) = guard.as_mut()
        {
            acc.record_replication_success();
            if usage.slice_gib > 0 && usage.duration_secs > 0 {
                let _ = acc.record_utilisation(usage.slice_gib, usage.duration_secs);
            }
        }
    }
}

fn moderation_evidence_viewer_error_from_object_error(
    err: ModerationQuarantineObjectError,
) -> ModerationEvidenceViewerError {
    match err {
        ModerationQuarantineObjectError::ResourceExhausted { resource, limit } => {
            ModerationEvidenceViewerError::ResourceExhausted { resource, limit }
        }
        ModerationQuarantineObjectError::StorageDisabled => {
            ModerationEvidenceViewerError::InvalidInput {
                message: "moderation quarantine object store is disabled".to_string(),
            }
        }
        ModerationQuarantineObjectError::InvalidInput { message }
        | ModerationQuarantineObjectError::InvalidSnapshot { message }
        | ModerationQuarantineObjectError::Codec { message } => {
            ModerationEvidenceViewerError::InvalidSnapshot { message }
        }
        ModerationQuarantineObjectError::UnknownQuarantine { quarantine_id_hex } => {
            ModerationEvidenceViewerError::UnknownQuarantine { quarantine_id_hex }
        }
        ModerationQuarantineObjectError::MissingObject { quarantine_id_hex } => {
            ModerationEvidenceViewerError::MissingObject { quarantine_id_hex }
        }
        ModerationQuarantineObjectError::DigestMismatch {
            quarantine_id_hex,
            expected_digest_hex,
            actual_digest_hex,
        } => ModerationEvidenceViewerError::InvalidSnapshot {
            message: format!(
                "quarantine object `{quarantine_id_hex}` digest mismatch: expected {expected_digest_hex}, got {actual_digest_hex}"
            ),
        },
        ModerationQuarantineObjectError::ConflictingObject { quarantine_id_hex } => {
            ModerationEvidenceViewerError::InvalidSnapshot {
                message: format!("quarantine object `{quarantine_id_hex}` conflicts"),
            }
        }
        ModerationQuarantineObjectError::AuthenticationFailed { quarantine_id_hex } => {
            ModerationEvidenceViewerError::InvalidSnapshot {
                message: format!("quarantine object `{quarantine_id_hex}` failed authentication"),
            }
        }
        ModerationQuarantineObjectError::KeyWrapperUnavailable => {
            ModerationEvidenceViewerError::InvalidInput {
                message: "runtime PKCS#11/KMS quarantine key wrapper is unavailable".to_owned(),
            }
        }
        ModerationQuarantineObjectError::KeyWrapping { key_id } => {
            ModerationEvidenceViewerError::InvalidSnapshot {
                message: format!("quarantine key operation failed for `{key_id}`"),
            }
        }
        ModerationQuarantineObjectError::InvalidRange {
            start,
            end,
            payload_len,
        } => ModerationEvidenceViewerError::InvalidInput {
            message: format!(
                "quarantine object range {start}..{end} exceeds plaintext length {payload_len}"
            ),
        },
        ModerationQuarantineObjectError::Io { path, message } => {
            ModerationEvidenceViewerError::Io { path, message }
        }
        ModerationQuarantineObjectError::StateLockPoisoned => {
            ModerationEvidenceViewerError::StateLockPoisoned
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::{
        str::FromStr,
        sync::{
            Arc, Barrier, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use iroha_crypto::{Algorithm, KeyPair, Signature as IrohaSignature, SignatureOf};
    use iroha_data_model::{
        metadata::Metadata,
        name::Name,
        sorafs::{
            capacity::{CapacityDeclarationRecord, ProviderId},
            deal::{
                BYTES_PER_GIB, ClientId, DealProposal, DealStatus, DealTerms, DealUsageReport,
                GIB_HOURS_PER_MONTH, MicropaymentTicket,
            },
            moderation::{
                ADVERSARIAL_CORPUS_VERSION_V1, AdversarialCorpusManifestV1,
                AdversarialPerceptualFamilyV1, AdversarialPerceptualVariantV1,
                MODERATION_REPRO_MANIFEST_VERSION_V1, MODERATION_TRUST_POLICY_VERSION_V1,
                ModerationModelFingerprintV1, ModerationReproBodyV1, ModerationReproManifestV1,
                ModerationReproSignatureV1, ModerationSeedMaterialV1, ModerationThresholdsV1,
                ModerationTrustPolicyBodyV1, ModerationTrustPolicySignatureV1,
                ModerationTrustPolicyV1, ModerationTrustedSignerV1,
            },
            moderation_ledger::{
                REPAIR_LEDGER_TASK_VERSION_V1, RepairFinalizedCursorV1, RepairFinalizedStatusV1,
                RepairLedgerStatusV1, RepairLedgerTaskPageV1, RepairLedgerTaskV1,
                RepairLedgerTerminalKindV1, sorafs_repair_task_id_v1,
            },
            pin_registry::StorageClass,
            reserve::{ReserveDuration, ReservePolicyV1, ReserveTier},
        },
    };
    use iroha_telemetry::metrics::global_or_default;
    use norito::to_bytes;
    use sorafs_car::{CarBuildPlan, compute_chunk_plan_digest_sha3};
    use sorafs_manifest::PorReportIsoWeek;
    use sorafs_manifest::{
        DagCodecId, ManifestBuilder, PinPolicy, REPUTATION_PROVIDER_INPUT_VERSION_V1,
        REPUTATION_PROVIDER_METRICS_VERSION_V1, REPUTATION_SCORING_EVIDENCE_VERSION_V1,
        REPUTATION_SNAPSHOT_TRUST_POLICY_VERSION_V1, REPUTATION_TRUSTED_SIGNER_VERSION_V1,
        ReputationDegradationFlagV1, ReputationProviderInputV1, ReputationProviderMetricsV1,
        ReputationReserveStageV1, ReputationScoringEvidenceV1, ReputationSnapshotSignatureV1,
        ReputationSnapshotTrustPolicyV1, ReputationTrustedSignerV1, ReputationWeightsV1,
        SIGNED_REPUTATION_SNAPSHOT_VERSION_V1, SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
        SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1,
        SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
        SORAFS_RECONCILIATION_REPORT_VERSION_V1, SignedReputationSnapshotV1,
        SoraFsAppealFinanceAccountFlowV1, SoraFsAppealFinanceJurorPayoutV1,
        SoraFsAppealFinanceOutcomeV1, SoraFsAppealFinanceReportV1,
        SoraFsAppealFinanceSettlementReceiptV1, SoraFsAppealFinanceWeeklyRollupV1,
        SoraFsModerationBallotGovernanceEventKindV1, SoraFsModerationBallotGovernanceEventV1,
        SoraFsModerationBallotGovernanceTallyV1, SoraFsModerationVoteChoiceV1,
        SoraFsModerationVoteCountsV1, SorafsReconciliationReportV1, build_reputation_snapshot,
        capacity::{
            CAPACITY_DECLARATION_VERSION_V1, CapacityDeclarationV1, CapacityMetadataEntry,
            ChunkerCommitmentV1, LaneCommitmentV1, REPLICATION_ORDER_VERSION_V1,
            ReplicationAssignmentV1, ReplicationOrderSlaV1, ReplicationOrderV1,
        },
        deal::{DealSettlementStatusV1, DealSettlementV1},
        repair::{
            GC_AUDIT_EVENT_VERSION_V1, GC_AUDIT_PAYLOAD_VERSION_V1, GcAuditEventV1,
            REPAIR_EVIDENCE_VERSION_V1, REPAIR_REPORT_VERSION_V1, RepairCauseV1, RepairEvidenceV1,
            RepairManualCauseV1, RepairReportV1, RepairTicketId,
        },
    };
    use tempfile::TempDir;

    use super::*;
    use crate::config::RuntimeRetentionPolicy;
    use crate::por::test_support::{
        resign_sample_verdict as resign_por_sample_verdict,
        sample_auditor_keys as por_sample_auditor_keys, sample_challenge as por_sample_challenge,
        sample_proof as por_sample_proof, sample_provider_key as por_sample_provider_key,
        sample_verdict as por_sample_verdict,
    };
    use crate::repair_ledger_projection::RepairLedgerTaskProjectionBuilderV1;

    #[derive(Debug)]
    struct SuccessfulPorRepairHandoff;

    impl PorRepairHandoff for SuccessfulPorRepairHandoff {
        fn enqueue_failed_por_repair(
            &self,
            intent: &PorFailedRepairIntentV1,
        ) -> Result<[u8; 32], PorRepairHandoffError> {
            Ok(intent.repair_task_id())
        }
    }

    fn xor(value: &str) -> XorQuantity {
        value.parse().expect("canonical XOR quantity")
    }

    fn quantity(value: &str) -> Quantity {
        value.parse().expect("canonical quantity")
    }

    fn manifest_builder_for_plan(payload: &[u8], plan: &CarBuildPlan) -> ManifestBuilder {
        ManifestBuilder::new()
            .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
            .por_root(
                sorafs_car::compute_por_root(payload, plan)
                    .expect("derive canonical fixture PoR root"),
            )
    }

    fn subsequent_por_challenge(base: &PorChallengeV1, seconds: u64) -> PorChallengeV1 {
        let mut challenge = base.clone();
        challenge.epoch_id = challenge.epoch_id.saturating_add(1);
        challenge.drand_round = challenge.drand_round.saturating_add(1);
        challenge.issued_at = challenge.issued_at.saturating_add(seconds);
        challenge.deadline_at = challenge.deadline_at.saturating_add(seconds);
        challenge.seed = sorafs_manifest::por::derive_challenge_seed(
            &challenge.drand_randomness,
            challenge.vrf_output.as_ref(),
            &challenge.manifest_digest,
            challenge.epoch_id,
        );
        challenge.challenge_id = sorafs_manifest::por::derive_challenge_id(
            &challenge.seed,
            &challenge.manifest_digest,
            &challenge.provider_id,
            challenge.epoch_id,
            challenge.drand_round,
        );
        challenge
    }

    fn storage_config_with_temp_dir() -> (StorageConfig, TempDir) {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(root.join("storage"))
            .build();
        (cfg, temp_dir)
    }

    #[test]
    fn orderbook_forwarder_survives_restart_when_worker_and_provider_are_disabled() {
        use iroha_data_model::{
            ChainId,
            isi::sorafs::MatchSorafsOrderbook,
            sorafs::orderbook::{
                ORDERBOOK_ADMISSION_POLICY_VERSION_V1, OrderbookAdmissionPolicyRecord,
                OrderbookAdmissionPolicyV1, OrderbookFinalizedCursorV1,
            },
        };

        let temp_dir = tempfile::tempdir().expect("create orderbook forwarder temp dir");
        let data_dir = temp_dir.path().join("validator-state");
        let config = StorageConfig::builder()
            .enabled(false)
            .data_dir(data_dir.clone())
            .build();
        assert!(!config.orderbook_worker_policy().enabled());

        let matcher =
            KeyPair::try_from_seed(vec![0x61; 32], Algorithm::Ed25519).expect("matcher key");
        let settlement =
            KeyPair::try_from_seed(vec![0x62; 32], Algorithm::Ed25519).expect("settlement key");
        let matcher_authority = AccountId::new(matcher.public_key().clone());
        let policy = OrderbookAdmissionPolicyV1 {
            version: ORDERBOOK_ADMISSION_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            market_id: [0x63; 32],
            matcher_authority: matcher_authority.clone(),
            settlement_authority: AccountId::new(settlement.public_key().clone()),
            paused: false,
            min_order_gib: 1,
            max_order_gib: 1_024,
            price_tick_micro_xor: 1,
            max_maker_fee_bps: 100,
            max_taker_fee_bps: 100,
            max_order_lifetime_secs: 86_400,
            max_receipt_age_secs: 3_600,
            max_clock_skew_secs: 30,
            max_receipt_bytes: 1 << 30,
            max_receipts_per_channel: 128,
        };
        let policy_digest = policy.digest().expect("digest orderbook policy");
        let context = OrderbookTransactionContextV1 {
            chain_id: ChainId::from("orderbook-forwarder-restart-test"),
            policy_record: OrderbookAdmissionPolicyRecord {
                policy,
                policy_digest,
                activated_at_unix: 1,
                activated_by: matcher_authority,
            },
            book_revision: 7,
            finalized_cursor: OrderbookFinalizedCursorV1 {
                height: 11,
                block_hash: [0x64; 32],
            },
        };
        let operation = OrderbookOperationV1::Match(MatchSorafsOrderbook::new(
            policy_digest,
            context.book_revision,
            8,
        ));

        let operation_id = {
            let node = NodeHandle::try_new(config.clone()).expect("start validator-only node");
            assert!(node.storage.is_none());
            let operation_id = node
                .enqueue_orderbook_transaction(operation, &context)
                .expect("persist orderbook operation")
                .operation_id();
            assert_eq!(
                node.pending_orderbook_transactions(1)
                    .expect("read pending operation")[0]
                    .operation_id,
                operation_id
            );
            operation_id
        };

        assert!(
            data_dir
                .join("orderbook-transaction-forwarder")
                .join(
                    crate::orderbook_transaction_forwarder::ORDERBOOK_TRANSACTION_FORWARDER_CHECKPOINT_FILE_NAME_V1,
                )
                .is_file()
        );
        let recovered = NodeHandle::try_new(config).expect("restart validator-only node");
        assert!(recovered.storage.is_none());
        let pending = recovered
            .pending_orderbook_transactions(1)
            .expect("recover pending orderbook operation");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].operation_id, operation_id);
        assert_eq!(pending[0].expected_book_revision, Some(7));
    }

    #[test]
    fn reserve_forwarder_survives_restart_when_worker_and_provider_are_disabled() {
        use iroha_data_model::{
            ChainId,
            asset::AssetDefinitionId,
            domain::DomainId,
            isi::sorafs::RegisterSorafsReserveAccount,
            sorafs::reserve::{
                RESERVE_AUTHORITY_POLICY_VERSION_V1, ReserveAuthorityPolicyRecordV1,
                ReserveAuthorityPolicyV1, ReserveFinalizedCursorV1, ReserveProviderTermsV1,
            },
        };

        let temp_dir = tempfile::tempdir().expect("create reserve forwarder temp dir");
        let data_dir = temp_dir.path().join("validator-state");
        let config = StorageConfig::builder()
            .enabled(false)
            .data_dir(data_dir.clone())
            .build();
        assert!(!config.reserve_worker_policy().enabled());

        let operations =
            KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519).expect("operations key");
        let decision =
            KeyPair::try_from_seed(vec![0x72; 32], Algorithm::Ed25519).expect("decision key");
        let provider =
            KeyPair::try_from_seed(vec![0x73; 32], Algorithm::Ed25519).expect("provider key");
        let operations_authority = AccountId::new(operations.public_key().clone());
        let provider_authority = AccountId::new(provider.public_key().clone());
        let policy = ReserveAuthorityPolicyV1 {
            version: RESERVE_AUTHORITY_POLICY_VERSION_V1,
            revision: 1,
            predecessor_policy_digest: None,
            economics: ReservePolicyV1::default(),
            asset_definition: AssetDefinitionId::new(
                DomainId::try_new("reserve", "universal").expect("reserve domain"),
                "xor".parse().expect("reserve asset name"),
            ),
            custody_account: AccountId::new(
                KeyPair::try_from_seed(vec![0x74; 32], Algorithm::Ed25519)
                    .expect("custody key")
                    .public_key()
                    .clone(),
            ),
            treasury_account: AccountId::new(
                KeyPair::try_from_seed(vec![0x75; 32], Algorithm::Ed25519)
                    .expect("treasury key")
                    .public_key()
                    .clone(),
            ),
            operations_authority: operations_authority.clone(),
            decision_authority: AccountId::new(decision.public_key().clone()),
            grace_period_days: 7,
            default_after_days: 30,
            max_provider_debt: XorQuantity::try_from_micro(1_000_000_000)
                .expect("valid reserve debt cap"),
            max_pending_movements_per_provider: 8,
            max_open_appeals_per_provider: 4,
        };
        let policy_digest = policy.digest().expect("digest reserve policy");
        let context = ReserveTransactionContextV1 {
            chain_id: ChainId::from("reserve-forwarder-restart-test"),
            policy_record: ReserveAuthorityPolicyRecordV1 {
                policy,
                policy_digest,
                activated_by: operations_authority,
                activated_at_unix: 1,
            },
            projection:
                crate::reserve_transaction_forwarder::ReserveTransactionProjectionV1::Registration {
                    provider_owner: provider_authority.clone(),
                },
            finalized_cursor: ReserveFinalizedCursorV1 {
                height: 11,
                block_hash: [0x76; 32],
            },
        };
        let operation = ReserveOperationV1::RegisterProvider(RegisterSorafsReserveAccount::new(
            ReserveProviderTermsV1 {
                provider_id: ProviderId::new([0x77; 32]),
                provider_account: provider_authority,
                tier: ReserveTier::TierA,
                storage_class: StorageClass::Hot,
                duration: ReserveDuration::Monthly,
                capacity_gib: 64,
            },
            policy_digest,
        ));

        let operation_id = {
            let node = NodeHandle::try_new(config.clone()).expect("start validator-only node");
            assert!(node.storage.is_none());
            let operation_id = node
                .enqueue_reserve_transaction(operation, &context)
                .expect("persist reserve operation")
                .operation_id();
            assert_eq!(
                node.pending_reserve_transactions(1)
                    .expect("read pending operation")[0]
                    .operation_id,
                operation_id
            );
            let claimed = node
                .claim_reserve_transaction_for_signing(operation_id)
                .expect("persist signer-only claim before restart");
            assert_eq!(claimed.operation_id, operation_id);
            let signing = node
                .pending_reserve_transactions(1)
                .expect("read signing handoff");
            assert_eq!(
                signing[0].state,
                crate::reserve_transaction_forwarder::ReserveTransactionDeliveryStateV1::Signing
            );
            operation_id
        };

        assert!(
            data_dir
                .join("reserve-transaction-forwarder")
                .join(
                    crate::reserve_transaction_forwarder::RESERVE_TRANSACTION_FORWARDER_CHECKPOINT_FILE_NAME_V1,
                )
                .is_file()
        );
        let recovered = NodeHandle::try_new(config).expect("restart validator-only node");
        assert!(recovered.storage.is_none());
        let pending = recovered
            .pending_reserve_transactions(1)
            .expect("recover pending reserve operation");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].operation_id, operation_id);
        assert_eq!(
            pending[0].state,
            crate::reserve_transaction_forwarder::ReserveTransactionDeliveryStateV1::Ready,
            "restart must recover an interrupted signer-only claim for handoff"
        );
        assert_eq!(
            pending[0].attempts, 1,
            "restart recovery must not refund the consumed signing attempt"
        );
    }

    #[test]
    fn node_startup_rejects_unsafe_programmatic_reserve_worker_policy() {
        let temp_dir = tempfile::tempdir().expect("create invalid reserve policy temp dir");
        let mut actual = iroha_config::parameters::actual::SorafsStorage::default();
        actual.enabled = false;
        actual.data_dir = temp_dir.path().join("validator-state");
        actual.reserve_worker.scan_batch_limit = 0;

        let error = NodeHandle::try_new(StorageConfig::from(actual))
            .expect_err("unsafe programmatic reserve worker policy must fail closed");
        assert!(matches!(error, NodeInitError::ReserveWorkerConfig { .. }));
    }

    #[derive(Debug)]
    struct TestQuarantineKeyWrapper {
        active_key_id: String,
        keys: BTreeMap<String, [u8; 32]>,
    }

    impl TestQuarantineKeyWrapper {
        fn single(key_id: &str, seed: u8) -> Self {
            Self {
                active_key_id: key_id.to_owned(),
                keys: BTreeMap::from([(key_id.to_owned(), [seed; 32])]),
            }
        }

        fn rotated(old_key_id: &str, old_seed: u8, new_key_id: &str, new_seed: u8) -> Self {
            Self {
                active_key_id: new_key_id.to_owned(),
                keys: BTreeMap::from([
                    (old_key_id.to_owned(), [old_seed; 32]),
                    (new_key_id.to_owned(), [new_seed; 32]),
                ]),
            }
        }

        fn wrapping_key(&self, key_id: &str) -> Result<[u8; 32], String> {
            self.keys
                .get(key_id)
                .copied()
                .ok_or_else(|| "unknown test wrapping key handle".to_owned())
        }

        fn nonce(key_id: &str, context_digest: [u8; 32], key: [u8; 32]) -> [u8; 12] {
            let mut hasher = blake3::Hasher::new_keyed(&key);
            hasher.update(b"sorafs.node.test-quarantine-wrapper.nonce.v1");
            hasher.update(key_id.as_bytes());
            hasher.update(&context_digest);
            let mut nonce = [0_u8; 12];
            nonce.copy_from_slice(&hasher.finalize().as_bytes()[..12]);
            nonce
        }
    }

    impl ModerationQuarantineKeyWrapper for TestQuarantineKeyWrapper {
        fn active_key_id(&self) -> &str {
            &self.active_key_id
        }

        fn wrap_dek(&self, context_digest: [u8; 32], dek: &[u8; 32]) -> Result<Vec<u8>, String> {
            use iroha_crypto::encryption::{ChaCha20Poly1305, SymmetricEncryptor};

            let key = self.wrapping_key(&self.active_key_id)?;
            let nonce = Self::nonce(&self.active_key_id, context_digest, key);
            SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(key)
                .map_err(|error| error.to_string())?
                .encrypt(nonce.as_slice(), context_digest.as_slice(), dek.as_slice())
                .map_err(|error| error.to_string())
        }

        fn unwrap_dek(
            &self,
            key_id: &str,
            context_digest: [u8; 32],
            wrapped_dek: &[u8],
        ) -> Result<[u8; 32], String> {
            use iroha_crypto::encryption::{ChaCha20Poly1305, SymmetricEncryptor};

            let key = self.wrapping_key(key_id)?;
            let nonce = Self::nonce(key_id, context_digest, key);
            SymmetricEncryptor::<ChaCha20Poly1305>::new_with_key(key)
                .map_err(|error| error.to_string())?
                .decrypt(nonce.as_slice(), context_digest.as_slice(), wrapped_dek)
                .map_err(|error| error.to_string())?
                .try_into()
                .map_err(|_| "unwrapped test DEK is not 32 bytes".to_owned())
        }
    }

    fn test_quarantine_key_wrapper() -> Arc<dyn ModerationQuarantineKeyWrapper> {
        Arc::new(TestQuarantineKeyWrapper::single(
            "kms:test/quarantine-v1",
            0xA5,
        ))
    }

    fn node_with_test_quarantine_key_wrapper(config: StorageConfig) -> NodeHandle {
        NodeHandle::try_new_with_quarantine_key_wrapper(config, test_quarantine_key_wrapper())
            .expect("initialise node with test-only quarantine key wrapper")
    }

    #[derive(Debug, Clone, Copy)]
    enum TestPrivacyCyclePrfMode {
        Bound,
        Failure(PrivacyCyclePrfProviderErrorV1),
    }

    struct TestPrivacyCyclePrfProvider {
        mode: TestPrivacyCyclePrfMode,
        requests: Mutex<Vec<PrivacyCyclePrfRequestV1>>,
    }

    impl std::fmt::Debug for TestPrivacyCyclePrfProvider {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("TEST-PRF-VENDOR-DIAGNOSTIC-MUST-NOT-LEAK")
        }
    }

    impl TestPrivacyCyclePrfProvider {
        fn bound() -> Self {
            Self {
                mode: TestPrivacyCyclePrfMode::Bound,
                requests: Mutex::new(Vec::new()),
            }
        }

        fn failing(error: PrivacyCyclePrfProviderErrorV1) -> Self {
            Self {
                mode: TestPrivacyCyclePrfMode::Failure(error),
                requests: Mutex::new(Vec::new()),
            }
        }

        fn requests(&self) -> Vec<PrivacyCyclePrfRequestV1> {
            self.requests.lock().expect("test PRF requests").clone()
        }
    }

    impl PrivacyCyclePrfProviderV1 for TestPrivacyCyclePrfProvider {
        fn derive_cycle_output(
            &self,
            request: &PrivacyCyclePrfRequestV1,
        ) -> Result<PrivacyCyclePrfOutputV1, PrivacyCyclePrfProviderErrorV1> {
            self.requests
                .lock()
                .expect("test PRF requests")
                .push(*request);
            match self.mode {
                TestPrivacyCyclePrfMode::Bound => {
                    let mut hasher = blake3::Hasher::new();
                    hasher.update(b"sorafs.node.test-privacy-cycle-prf.v1");
                    hasher.update(&request.binding_digest());
                    let output = *hasher.finalize().as_bytes();
                    debug_assert_ne!(output, [0; 32]);
                    Ok(PrivacyCyclePrfOutputV1::new(output)
                        .expect("test PRF hash cannot be all zeroes"))
                }
                TestPrivacyCyclePrfMode::Failure(error) => Err(error),
            }
        }
    }

    #[derive(Default)]
    struct TestPrivacyReleaseAnchor {
        heads: Mutex<BTreeMap<[u8; 32], PrivacyReleaseAnchorHeadV1>>,
    }

    impl PrivacyReleaseAnchorV1 for TestPrivacyReleaseAnchor {
        fn finalized_head(
            &self,
            query_id: [u8; 32],
        ) -> Result<PrivacyReleaseAnchorHeadV1, PrivacyReleaseAnchorErrorV1> {
            Ok(self
                .heads
                .lock()
                .map_err(|_| PrivacyReleaseAnchorErrorV1::Internal)?
                .get(&query_id)
                .copied()
                .unwrap_or_else(|| PrivacyReleaseAnchorHeadV1::genesis(query_id)))
        }

        fn compare_and_set_finalized_head(
            &self,
            expected: PrivacyReleaseAnchorHeadV1,
            next: PrivacyReleaseAnchorHeadV1,
        ) -> Result<(), PrivacyReleaseAnchorErrorV1> {
            if expected.query_id() != next.query_id()
                || next.sequence() != expected.sequence().saturating_add(1)
            {
                return Err(PrivacyReleaseAnchorErrorV1::InvalidState);
            }
            let mut heads = self
                .heads
                .lock()
                .map_err(|_| PrivacyReleaseAnchorErrorV1::Internal)?;
            let current = heads
                .get(&expected.query_id())
                .copied()
                .unwrap_or_else(|| PrivacyReleaseAnchorHeadV1::genesis(expected.query_id()));
            if current != expected {
                return Err(PrivacyReleaseAnchorErrorV1::Conflict);
            }
            heads.insert(next.query_id(), next);
            Ok(())
        }
    }

    fn test_privacy_cycle_prf_provider() -> Arc<dyn PrivacyCyclePrfProviderV1> {
        Arc::new(TestPrivacyCyclePrfProvider::bound())
    }

    fn test_privacy_release_anchor() -> Arc<dyn PrivacyReleaseAnchorV1> {
        Arc::new(TestPrivacyReleaseAnchor::default())
    }

    fn node_with_test_privacy_cycle_prf_provider(config: StorageConfig) -> NodeHandle {
        NodeHandle::try_new_with_runtime_deps(
            config,
            NodeRuntimeDeps::default()
                .with_privacy_cycle_prf_provider(test_privacy_cycle_prf_provider())
                .with_privacy_release_anchor(test_privacy_release_anchor()),
        )
        .expect("initialise node with test-only privacy cycle PRF provider")
    }

    fn reputation_signing_key() -> iroha_crypto::KeyPair {
        iroha_crypto::KeyPair::try_from_seed(vec![0x5A; 32], Algorithm::Ed25519)
            .expect("derive reputation signing key")
    }

    fn reputation_trust_policy_fixture() -> ReputationSnapshotTrustPolicyV1 {
        ReputationSnapshotTrustPolicyV1 {
            version: REPUTATION_SNAPSHOT_TRUST_POLICY_VERSION_V1,
            policy_id: [0xA5; 32],
            valid_from_unix: 1_700_000_000,
            valid_until_unix: 2_000_000_000,
            max_snapshot_age_secs: 600,
            max_future_skew_secs: 30,
            min_signatures: 1,
            signers: vec![ReputationTrustedSignerV1 {
                version: REPUTATION_TRUSTED_SIGNER_VERSION_V1,
                signer_id: "council-1".to_owned(),
                public_key: reputation_signing_key()
                    .public_key()
                    .try_to_bytes()
                    .expect("export reputation verifying key")
                    .1
                    .try_into()
                    .expect("Ed25519 public key is fixed-width"),
            }],
            revoked_signer_ids: Vec::new(),
        }
    }

    fn storage_config_with_reputation_policy() -> (StorageConfig, TempDir) {
        storage_config_with_reputation_policy_fixture(&reputation_trust_policy_fixture())
    }

    fn storage_config_with_reputation_policy_fixture(
        policy: &ReputationSnapshotTrustPolicyV1,
    ) -> (StorageConfig, TempDir) {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let policy_path = root.join("reputation-trust-policy.to");
        let policy_bytes = policy
            .canonical_bytes()
            .expect("encode reputation trust policy");
        write_local_checkpoint_atomic(&policy_path, &policy_bytes)
            .expect("write reputation trust policy");
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(root.join("storage"))
            .reputation_trust_policy_path(Some(policy_path))
            .build();
        (cfg, temp_dir)
    }

    fn enabled_repair_config(max_attempts: u32) -> RepairConfig {
        RepairConfig::from(&iroha_config::parameters::actual::SorafsRepair {
            enabled: true,
            max_attempts,
            ..Default::default()
        })
    }

    fn enabled_gc_config(max_deletions_per_run: u32) -> GcConfig {
        GcConfig::from(&iroha_config::parameters::actual::SorafsGc {
            enabled: true,
            retention_grace_secs: 0,
            max_deletions_per_run,
            ..Default::default()
        })
    }

    fn ensure_test_capacity_provider(handle: &NodeHandle) -> [u8; 32] {
        if let Some(provider_id) = handle.capacity_usage().provider_id {
            return provider_id;
        }
        let provider_id = [0xAB; 32];
        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id,
            stake: sorafs_manifest::provider_advert::StakePointer {
                pool_id: [0xAC; 32],
                stake_amount: xor("1"),
            },
            committed_capacity_gib: 100,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".to_owned(),
                profile_aliases: None,
                committed_gib: 100,
                capability_refs: Vec::new(),
            }],
            lane_commitments: vec![LaneCommitmentV1 {
                lane_id: "default".to_owned(),
                max_gib: 100,
            }],
            pricing: None,
            valid_from: 1,
            valid_until: u64::MAX,
            metadata: Vec::new(),
        };
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(provider_id),
            norito::to_bytes(&declaration).expect("encode capacity declaration"),
            declaration.committed_capacity_gib,
            1,
            declaration.valid_from,
            declaration.valid_until,
            Metadata::default(),
        );
        handle
            .record_capacity_declaration(&record)
            .expect("record test capacity declaration");
        provider_id
    }

    fn run_test_gc(
        handle: &NodeHandle,
        now_unix: u64,
        repair_projection: &RepairLedgerTaskProjectionV1,
    ) -> GcSweepReport {
        ensure_test_capacity_provider(handle);
        handle.run_gc_once(now_unix, repair_projection)
    }

    fn finalized_repair_projection(tasks: Vec<RepairLedgerTaskV1>) -> RepairLedgerTaskProjectionV1 {
        let finalized_cursor = RepairFinalizedCursorV1 {
            height: 42,
            block_hash: [0xA5; 32],
        };
        let mut status = RepairLedgerStatusV1::default();
        status.tasks = u64::try_from(tasks.len()).expect("test task count fits u64");
        if !tasks.is_empty() {
            status.updated_at_unix_ms = 1_700_000_000_000;
        }
        for task in &tasks {
            status.leased_tasks += u64::from(task.lease.is_some());
            status.slash_proposals += u64::from(task.slash.is_some());
            status.appeals += u64::from(task.appeal.is_some());
            let Some(terminal) = task.terminal_outcome.as_ref() else {
                continue;
            };
            status.terminal_outcomes += 1;
            match &terminal.kind {
                RepairLedgerTerminalKindV1::Completed(_) => status.completed += 1,
                RepairLedgerTerminalKindV1::Failed(_) => status.failed += 1,
                RepairLedgerTerminalKindV1::Escalated(_) => status.escalated += 1,
            }
        }
        let mut builder = RepairLedgerTaskProjectionBuilderV1::new(RepairFinalizedStatusV1 {
            finalized_cursor,
            status,
        })
        .expect("initialize finalized repair projection");
        builder
            .push_page(RepairLedgerTaskPageV1 {
                finalized_cursor,
                tasks,
                has_more: false,
                next_after_task_id: None,
            })
            .expect("append finalized repair projection page");
        builder.finish().expect("finish repair projection")
    }

    fn empty_finalized_repair_projection() -> RepairLedgerTaskProjectionV1 {
        finalized_repair_projection(Vec::new())
    }

    fn active_native_repair_task(
        manifest_digest: [u8; 32],
        provider_id: [u8; 32],
    ) -> RepairLedgerTaskV1 {
        let source_identity = [0x61; 32];
        let auditor = AccountId::new(
            KeyPair::try_from_seed(vec![0x60; 32], Algorithm::Ed25519)
                .expect("repair auditor key")
                .public_key()
                .clone(),
        );
        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-GC-NATIVE-001".to_owned()),
            auditor_account: auditor.to_string(),
            submitted_at_unix: 1_700_000_000,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest,
                provider_id,
                por_history_id: None,
                cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                    reason: "native finalized GC exclusion".to_owned(),
                }),
                evidence_json: None,
                notes: None,
            },
            notes: None,
        };
        RepairLedgerTaskV1 {
            version: REPAIR_LEDGER_TASK_VERSION_V1,
            task_id: sorafs_repair_task_id_v1(source_identity),
            source_identity,
            ticket_id: report.ticket_id.0.clone(),
            canonical_report: norito::to_bytes(&report).expect("encode repair report"),
            manifest_digest,
            provider_id,
            submitted_by: auditor,
            submitted_at_unix_ms: report.submitted_at_unix * 1_000,
            revision: 1,
            lease: None,
            terminal_outcome: None,
            slash: None,
            appeal: None,
            action_receipts: Vec::new(),
            updated_at_unix_ms: report.submitted_at_unix * 1_000,
        }
    }

    #[test]
    fn exact_quantity_metric_projection_is_explicit_and_saturating() {
        assert_eq!(
            quantity_to_metric_micro_saturating(&quantity("0.000001999")),
            1
        );
        assert_eq!(
            quantity_to_metric_micro_saturating(xor("0.0000001").as_quantity()),
            0
        );
        assert_eq!(
            quantity_to_metric_micro_saturating(&quantity(
                "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047",
            )),
            u128::MAX
        );
    }

    #[test]
    fn exact_quantity_metric_divergence_rounds_toward_zero() {
        let reference = quantity("2");
        assert_eq!(
            quantity_divergence_bps_saturating(&quantity("2.1"), &reference),
            500
        );
        assert_eq!(
            quantity_divergence_bps_saturating(&quantity("1.8"), &reference),
            1_000
        );
        assert_eq!(
            quantity_divergence_bps_saturating(&quantity("1"), &Quantity::zero()),
            u64::MAX
        );
    }

    #[test]
    fn bounded_event_history_preserves_monotonic_sequences_and_reports_gaps() {
        let mut history = BoundedEventHistory::new(2);
        assert_eq!(history.append(|sequence| sequence).unwrap(), 1);
        assert_eq!(history.append(|sequence| sequence).unwrap(), 2);
        assert_eq!(history.append(|sequence| sequence).unwrap(), 3);

        let replay = history.replay(Some(0), 10, |sequence| *sequence);
        assert_eq!(replay.oldest_available_sequence, Some(2));
        assert_eq!(replay.latest_sequence, Some(3));
        assert!(replay.gap);
        assert_eq!(replay.events, vec![2, 3]);
        assert_eq!(history.append(|sequence| sequence).unwrap(), 4);
        assert_eq!(history.retained(), vec![3, 4]);
    }

    #[test]
    fn bounded_event_history_rejects_non_monotonic_restore() {
        let mut history = BoundedEventHistory::new(4);
        let error = history
            .restore(vec![1_u64, 3, 3], |sequence| *sequence)
            .expect_err("duplicate checkpoint sequence must fail");
        assert!(error.to_string().contains("strictly consecutive"));
        assert!(history.retained().is_empty());
    }

    #[test]
    fn local_checkpoint_roundtrip_is_bounded_and_atomic() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().canonicalize().expect("canonical temp dir");
        let path = root.join("runtime").join("checkpoint.to");
        write_local_checkpoint_atomic_bounded(&path, b"checkpoint", 10)
            .expect("write bounded checkpoint");
        assert_eq!(
            read_local_checkpoint_bounded(&path, 10).expect("read checkpoint"),
            Some(b"checkpoint".to_vec())
        );
        assert!(write_local_checkpoint_atomic_bounded(&path, b"too-large", 4).is_err());
        assert!(read_local_checkpoint_bounded(&path, 4).is_err());
    }

    #[test]
    fn local_checkpoint_decoder_rejects_trailing_bytes_and_sequence_bombs() {
        let value = vec![1_u64, 2];
        let canonical = norito::to_bytes(&value).expect("encode canonical checkpoint fixture");
        assert_eq!(
            decode_local_checkpoint_canonical::<Vec<u64>>(&canonical, 4_096, 2)
                .expect("decode canonical checkpoint fixture"),
            value
        );

        let mut trailing = canonical;
        trailing.push(0);
        assert!(
            decode_local_checkpoint_canonical::<Vec<u64>>(&trailing, 4_096, 2).is_err(),
            "trailing bytes must not be accepted as an equivalent checkpoint"
        );

        let oversized_sequence =
            norito::to_bytes(&vec![1_u64, 2, 3]).expect("encode sequence bomb fixture");
        assert!(
            decode_local_checkpoint_canonical::<Vec<u64>>(&oversized_sequence, 4_096, 2).is_err(),
            "declared sequence length must fail before allocation beyond the configured bound"
        );
    }

    #[test]
    fn local_checkpoint_distinguishes_precommit_and_visible_uncertain_failures() {
        fn fail_parent_sync(_: &Path) -> io::Result<()> {
            Err(io::Error::other("injected parent sync failure"))
        }

        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().canonicalize().expect("canonical temp dir");
        let path = root.join("checkpoint.to");
        let precommit = write_local_checkpoint_atomic_bounded(&path, b"too-large", 4)
            .expect_err("size limit fails before commit");
        assert!(!precommit.committed);
        assert!(!path.exists());

        let uncertain = write_local_checkpoint_atomic_with_mode_and_parent_sync(
            &path,
            b"visible-state",
            false,
            fail_parent_sync,
        )
        .expect_err("parent sync failure follows rename");
        assert!(uncertain.committed);
        assert_eq!(
            fs::read(&path).expect("visible committed bytes"),
            b"visible-state"
        );
    }

    #[test]
    fn concurrent_local_checkpoint_writers_never_publish_partial_bytes() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().canonicalize().expect("canonical temp dir");
        let path = Arc::new(root.join("checkpoint.to"));
        let barrier = Arc::new(Barrier::new(8));
        let payloads = (0_u8..8)
            .map(|byte| Arc::<[u8]>::from(vec![byte; 4_096]))
            .collect::<Vec<_>>();
        let workers = payloads
            .iter()
            .map(|payload| {
                let path = Arc::clone(&path);
                let barrier = Arc::clone(&barrier);
                let payload = Arc::clone(payload);
                std::thread::spawn(move || {
                    barrier.wait();
                    write_local_checkpoint_atomic_bounded(&path, payload.as_ref(), 8_192)
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().expect("checkpoint writer joins").unwrap();
        }
        let bytes = fs::read(&*path).expect("read final checkpoint");
        assert!(
            payloads
                .iter()
                .any(|payload| payload.as_ref() == bytes.as_slice())
        );
        let leftovers = fs::read_dir(root)
            .expect("read temp dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".tmp-"))
            .count();
        assert_eq!(leftovers, 0);
    }

    #[cfg(unix)]
    #[test]
    fn local_checkpoint_rejects_symlink_targets_and_parents() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};

        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().canonicalize().expect("canonical temp dir");
        let victim = root.join("victim");
        fs::write(&victim, b"unchanged").expect("write victim");
        let target = root.join("checkpoint.to");
        symlink(&victim, &target).expect("symlink target");
        assert!(write_local_checkpoint_atomic(&target, b"replacement").is_err());
        assert!(read_local_checkpoint_bounded(&target, 128).is_err());
        assert_eq!(fs::read(&victim).unwrap(), b"unchanged");

        let real_parent = root.join("real-parent");
        fs::create_dir(&real_parent).expect("create real parent");
        let linked_parent = root.join("linked-parent");
        symlink(&real_parent, &linked_parent).expect("symlink parent");
        assert!(write_local_checkpoint_atomic(&linked_parent.join("state.to"), b"state").is_err());
        assert!(!real_parent.join("state.to").exists());

        let nested_target = linked_parent.join("nested").join("state.to");
        assert!(write_local_checkpoint_atomic(&nested_target, b"state").is_err());
        assert!(!real_parent.join("nested").exists());

        let hardlink_target = root.join("hardlink-target.to");
        write_local_checkpoint_atomic(&hardlink_target, b"state").expect("write hardlink target");
        let hardlink_alias = root.join("hardlink-alias.to");
        fs::hard_link(&hardlink_target, &hardlink_alias).expect("create hardlink alias");
        assert!(read_local_checkpoint_bounded(&hardlink_target, 128).is_err());
        assert!(write_local_checkpoint_atomic(&hardlink_target, b"replacement").is_err());

        let permissive_parent = root.join("permissive-parent");
        fs::create_dir(&permissive_parent).expect("create permissive parent");
        fs::set_permissions(&permissive_parent, fs::Permissions::from_mode(0o777))
            .expect("make parent writable");
        assert!(
            write_local_checkpoint_atomic(&permissive_parent.join("state.to"), b"state").is_err()
        );
        fs::set_permissions(&permissive_parent, fs::Permissions::from_mode(0o700))
            .expect("restore parent permissions");
    }

    #[test]
    fn local_checkpoint_rejects_parent_traversal_and_randomizes_temporary_names() {
        let temp = tempfile::tempdir().expect("temp dir");
        let traversal = temp.path().join("runtime").join("..").join("escaped.to");
        assert!(write_local_checkpoint_atomic(&traversal, b"state").is_err());
        assert!(!temp.path().join("escaped.to").exists());

        let path = temp.path().join("checkpoint.to");
        let first = local_checkpoint_tmp_path(&path).expect("first randomized temp path");
        let second = local_checkpoint_tmp_path(&path).expect("second randomized temp path");
        assert_ne!(first, second);
        assert!(
            first
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with('.')
        );
        assert!(
            second
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with('.')
        );
    }

    #[cfg(unix)]
    #[test]
    fn local_checkpoint_identity_rejects_same_length_file_swap() {
        let temp = tempfile::tempdir().expect("temp dir");
        let first = temp.path().join("first.to");
        let second = temp.path().join("second.to");
        fs::write(&first, b"same-size-a").expect("write first");
        fs::write(&second, b"same-size-b").expect("write second");
        let first_meta = fs::metadata(&first).expect("first metadata");
        let second_meta = fs::metadata(&second).expect("second metadata");
        assert!(same_local_file_identity(&first_meta, &first_meta));
        assert!(!same_local_file_identity(&first_meta, &second_meta));
    }

    #[test]
    fn runtime_initialization_requires_every_checkpoint_after_first_start() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::try_new(cfg.clone()).expect("initialize runtime checkpoints");
        drop(handle);

        let marker = runtime_state_initialization_path(cfg.data_dir());
        assert_eq!(
            fs::read(&marker).expect("read initialization marker"),
            RUNTIME_STATE_INITIALIZATION_BYTES
        );
        for (_, path) in required_runtime_checkpoint_paths(cfg.data_dir()) {
            assert!(
                path.is_file(),
                "missing initialized checkpoint {}",
                path.display()
            );
            let bytes = fs::read(&path).expect("read initialized checkpoint");
            fs::remove_file(&path).expect("remove initialized checkpoint");
            assert!(matches!(
                NodeHandle::try_new(cfg.clone()),
                Err(NodeInitError::Checkpoint { .. })
            ));
            write_local_checkpoint_atomic(&path, &bytes).expect("restore checkpoint for next case");
        }

        fs::remove_file(&marker).expect("remove initialization marker");
        assert!(matches!(
            NodeHandle::try_new(cfg),
            Err(NodeInitError::Checkpoint {
                component: "runtime initialization marker",
                ..
            })
        ));
    }

    #[test]
    fn runtime_initialization_rejects_retired_v1_marker_and_checkpoint() {
        for (retired_path, component) in [
            (
                retired_runtime_state_initialization_path_v1 as fn(&Path) -> PathBuf,
                "retired runtime initialization marker",
            ),
            (
                retired_auxiliary_runtime_checkpoint_path_v1 as fn(&Path) -> PathBuf,
                "retired auxiliary runtime checkpoint",
            ),
        ] {
            let (cfg, _dir) = storage_config_with_temp_dir();
            drop(NodeHandle::new(cfg.clone()));
            let path = retired_path(cfg.data_dir());
            write_local_checkpoint_atomic(&path, b"retired-v1")
                .expect("write retired runtime-state artifact");

            let error =
                NodeHandle::try_new(cfg).expect_err("retired runtime-state artifact must fail");
            assert!(
                matches!(
                    error,
                    NodeInitError::Checkpoint {
                        component: actual_component,
                        ..
                    } if actual_component == component
                ),
                "unexpected startup error: {error}"
            );
        }
    }

    #[test]
    fn runtime_initialization_rejects_noncanonical_v2_marker() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        drop(NodeHandle::new(cfg.clone()));
        let marker = runtime_state_initialization_path(cfg.data_dir());
        write_local_private_checkpoint_atomic(
            &marker,
            b"sorafs.node.runtime-state.initialized.v1\n",
        )
        .expect("replace runtime-state marker");

        let error = NodeHandle::try_new(cfg)
            .expect_err("noncanonical runtime-state v2 marker must fail startup");
        assert!(
            matches!(
                error,
                NodeInitError::Checkpoint {
                    component: "runtime initialization marker",
                    ref message,
                    ..
                } if message.contains("runtime-state v2")
            ),
            "unexpected startup error: {error}"
        );
    }

    #[test]
    fn auxiliary_runtime_checkpoint_rejects_retired_version() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        drop(NodeHandle::new(cfg.clone()));
        let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        let bytes = fs::read(&path).expect("read initialized auxiliary checkpoint");
        let mut checkpoint = norito::decode_from_bytes::<AuxiliaryRuntimeCheckpointV2>(&bytes)
            .expect("decode initialized auxiliary checkpoint");
        checkpoint.version = 1;
        let retired = norito::to_bytes(&checkpoint).expect("encode retired checkpoint version");
        write_local_checkpoint_atomic(&path, &retired)
            .expect("replace auxiliary checkpoint with retired version");

        let error =
            NodeHandle::try_new(cfg).expect_err("retired auxiliary version must fail startup");
        assert!(
            matches!(
                error,
                NodeInitError::Checkpoint {
                    component: "auxiliary runtime",
                    ref message,
                    ..
                } if message.contains("unsupported auxiliary runtime checkpoint version 1")
            ),
            "unexpected startup error: {error}"
        );
    }

    #[test]
    fn governance_outbox_rejects_retired_entry_version_before_kind_dispatch() {
        let payload_bytes = b"retired-governance-outbox-entry".to_vec();
        let payload_digest = *blake3::hash(&payload_bytes).as_bytes();
        let entry = GovernanceOutboxEntryV1 {
            version: 1,
            sequence: 1,
            kind: GovernanceOutboxKindV1::DealSettlement,
            payload_digest,
            binding_digest: governance_outbox_binding_digest(
                1,
                1,
                GovernanceOutboxKindV1::DealSettlement,
                payload_digest,
            ),
            payload_bytes,
        };

        let error =
            validate_governance_outbox_entry(&entry).expect_err("retired outbox version must fail");
        assert!(
            error
                .to_string()
                .contains("unsupported governance outbox entry version 1"),
            "unexpected validation error: {error}"
        );
    }

    #[test]
    fn auxiliary_runtime_checkpoint_restores_privacy_source_state() {
        let (base, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base.data_dir().clone())
            .runtime_retention(RuntimeRetentionPolicy::new(4, 8, 1024 * 1024))
            .build();
        let source = NodeHandle::new(cfg.clone());
        source
            .record_privacy_aggregate_source_event(privacy_source_event(
                "restart-event",
                "restart-population",
                0x42,
                1_800_000_001,
            ))
            .expect("persist privacy source event");
        let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        assert!(path.exists());
        drop(source);

        let restored = NodeHandle::new(cfg);
        assert_eq!(restored.privacy_aggregate_source_event_count(), 1);
        assert_eq!(
            restored
                .record_privacy_aggregate_source_event(privacy_source_event(
                    "restart-event",
                    "restart-population",
                    0x42,
                    1_800_000_001,
                ))
                .expect("restart retry is idempotent"),
            PrivacySourceEventRecordOutcomeV1::AlreadyRecorded
        );
    }

    #[test]
    fn concurrent_auxiliary_runtime_updates_survive_restart() {
        let (base, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base.data_dir().clone())
            .runtime_retention(RuntimeRetentionPolicy::new(8, 32, 1024 * 1024))
            .build();
        let source = NodeHandle::new(cfg.clone());
        let barrier = Arc::new(Barrier::new(16));
        let workers = (0_u8..16)
            .map(|index| {
                let handle = source.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    handle.record_privacy_aggregate_source_event(privacy_source_event(
                        &format!("concurrent-{index}"),
                        "concurrent-population",
                        index.saturating_add(1),
                        1_800_000_100 + u64::from(index),
                    ))
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().expect("runtime writer joins").unwrap();
        }
        assert_eq!(source.privacy_aggregate_source_event_count(), 16);
        drop(source);

        let restored = NodeHandle::new(cfg);
        assert_eq!(restored.privacy_aggregate_source_event_count(), 16);
    }

    #[test]
    fn auxiliary_runtime_checkpoint_corruption_and_oversize_fail_startup() {
        let (base, _dir) = storage_config_with_temp_dir();
        let checkpoint_max_bytes = 64 * 1024;
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base.data_dir().clone())
            .runtime_retention(RuntimeRetentionPolicy::new(4, 8, checkpoint_max_bytes))
            .build();
        drop(NodeHandle::new(cfg.clone()));
        let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"not-norito").unwrap();
        let corrupt_error = NodeHandle::try_new(cfg.clone())
            .expect_err("corrupt auxiliary checkpoint must fail startup");
        assert!(
            matches!(
                corrupt_error,
                NodeInitError::Checkpoint {
                    component: "auxiliary runtime",
                    ..
                }
            ),
            "unexpected startup error: {corrupt_error}"
        );

        let oversized_len = usize::try_from(checkpoint_max_bytes)
            .expect("test checkpoint limit fits usize")
            .checked_add(1)
            .expect("test checkpoint oversize length does not overflow");
        fs::write(&path, vec![0xAA; oversized_len]).unwrap();
        assert!(
            matches!(
                NodeHandle::try_new(cfg),
                Err(NodeInitError::Checkpoint {
                    component: "auxiliary runtime",
                    ..
                })
            ),
            "oversized auxiliary checkpoint must fail its own startup boundary"
        );
    }

    #[cfg(unix)]
    #[test]
    fn auxiliary_runtime_checkpoint_symlink_fails_startup() {
        use std::os::unix::fs::symlink;

        let (cfg, _dir) = storage_config_with_temp_dir();
        drop(NodeHandle::new(cfg.clone()));
        let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let victim = cfg.data_dir().join("victim.to");
        fs::write(&victim, b"not-a-checkpoint").unwrap();
        fs::remove_file(&path).expect("remove initialized auxiliary checkpoint");
        symlink(&victim, &path).unwrap();
        assert!(
            matches!(
                NodeHandle::try_new(cfg),
                Err(NodeInitError::Checkpoint {
                    component: "auxiliary runtime",
                    ..
                })
            ),
            "symlinked auxiliary checkpoint must fail its own startup boundary"
        );
    }

    fn moderation_repro_manifest_fixture(
        manifest_id_byte: u8,
        runner_hash_byte: u8,
    ) -> ModerationReproManifestV1 {
        let mut body = ModerationReproBodyV1 {
            schema_version: MODERATION_REPRO_MANIFEST_VERSION_V1,
            manifest_id: [manifest_id_byte; 16],
            // The digest is derived from the canonical body below; it is not
            // an operator-selected fixture label.
            manifest_digest: [0; 32],
            runner_hash: [runner_hash_byte; 32],
            runtime_version: format!("sorafs-ai-runner {runner_hash_byte}.0.0"),
            issued_at_unix: 1_800_000_000 + u64::from(runner_hash_byte),
            seed_material: ModerationSeedMaterialV1 {
                domain_tag: "sfm4a:calibration".to_string(),
                seed_version: 1,
                run_nonce: [0x44; 32],
            },
            thresholds: ModerationThresholdsV1 {
                quarantine: 6_000,
                escalate: 8_500,
            },
            models: vec![ModerationModelFingerprintV1 {
                model_id: [0x55; 16],
                artifact_path: "models/model-55.norito".to_string(),
                artifact_bytes: 1,
                artifact_digest: [0x66; 32],
                weights_digest: [0x77; 32],
                engine: iroha_data_model::sorafs::moderation::ModerationModelEngineV1::DeterministicLinearV1,
                feature_profile: iroha_data_model::sorafs::moderation::ModerationFeatureProfileV1::ByteHistogramAndBigramV1,
                calibration_knot_count: 2,
                max_input_bytes: 1024,
                max_operations: 3073,
                working_memory_bytes: 4096,
                weight: Some(10_000),
            }],
            notes: Some("registry fixture".to_string()),
        };
        body.refresh_manifest_digest()
            .expect("refresh moderation fixture digest");
        let keypair = iroha_crypto::KeyPair::try_from_seed(vec![0x9A; 32], Algorithm::Ed25519)
            .expect("moderation fixture seed must derive keypair");
        let signature = SignatureOf::try_new(keypair.private_key(), &body)
            .expect("moderation fixture signature");
        ModerationReproManifestV1 {
            body,
            signatures: vec![ModerationReproSignatureV1 {
                role: "council".to_string(),
                public_key: keypair.public_key().clone(),
                signature,
            }],
        }
    }

    fn moderation_screening_authority_bundle_fixture(
        now_unix: u64,
        policy_issued_at_unix: u64,
        policy_id_byte: u8,
    ) -> ModerationScreeningAuthorityBundleV1 {
        let manifest_key =
            KeyPair::try_from_seed(vec![0xD1; 32], Algorithm::Ed25519).expect("manifest key");
        let governance_key =
            KeyPair::try_from_seed(vec![0xD2; 32], Algorithm::Ed25519).expect("governance key");
        let runner_key =
            KeyPair::try_from_seed(vec![0xD3; 32], Algorithm::Ed25519).expect("runner key");
        let mut manifest_body = ModerationReproBodyV1 {
            schema_version: MODERATION_REPRO_MANIFEST_VERSION_V1,
            manifest_id: [0xD4; 16],
            manifest_digest: [0; 32],
            runner_hash: [0xD5; 32],
            runtime_version: "sorafs-ai-runner config-authority-v1".to_owned(),
            issued_at_unix: now_unix.saturating_sub(200),
            seed_material: ModerationSeedMaterialV1 {
                domain_tag: "sfm4a:config-authority".to_owned(),
                seed_version: 1,
                run_nonce: [0xD6; 32],
            },
            thresholds: ModerationThresholdsV1 {
                quarantine: 6_000,
                escalate: 8_500,
            },
            models: vec![ModerationModelFingerprintV1 {
                model_id: [0xD7; 16],
                artifact_path: "models/config-authority-v1.norito".to_owned(),
                artifact_bytes: 1,
                artifact_digest: [0xD8; 32],
                weights_digest: [0xD9; 32],
                engine: iroha_data_model::sorafs::moderation::ModerationModelEngineV1::DeterministicLinearV1,
                feature_profile: iroha_data_model::sorafs::moderation::ModerationFeatureProfileV1::ByteHistogramAndBigramV1,
                calibration_knot_count: 2,
                max_input_bytes: 1024,
                max_operations: 3073,
                working_memory_bytes: 4096,
                weight: Some(10_000),
            }],
            notes: None,
        };
        manifest_body
            .refresh_manifest_digest()
            .expect("manifest digest");
        let manifest = ModerationReproManifestV1 {
            signatures: vec![ModerationReproSignatureV1 {
                role: "model-governance".to_owned(),
                public_key: manifest_key.public_key().clone(),
                signature: SignatureOf::try_new(manifest_key.private_key(), &manifest_body)
                    .expect("manifest signature"),
            }],
            body: manifest_body,
        };
        let mut policy_body = ModerationTrustPolicyBodyV1 {
            schema_version: MODERATION_TRUST_POLICY_VERSION_V1,
            policy_id: [policy_id_byte; 16],
            policy_digest: [0; 32],
            manifest_id: manifest.body.manifest_id,
            manifest_digest: manifest.body.manifest_digest,
            runner_hash: manifest.body.runner_hash,
            issued_at_unix: policy_issued_at_unix,
            valid_from_unix: policy_issued_at_unix,
            valid_until_unix: now_unix.saturating_add(3_600),
            result_quorum: 1,
            governance_quorum: 1,
            max_result_age_secs: 600,
            max_result_ttl_secs: 300,
            max_clock_skew_secs: 30,
            trusted_signers: vec![ModerationTrustedSignerV1 {
                role: "runner".to_owned(),
                public_key: runner_key.public_key().clone(),
                valid_from_unix: policy_issued_at_unix,
                valid_until_unix: now_unix.saturating_add(3_600),
                revoked_at_unix: None,
            }],
            notes: None,
        };
        policy_body.refresh_policy_digest().expect("policy digest");
        let policy = ModerationTrustPolicyV1 {
            signatures: vec![ModerationTrustPolicySignatureV1 {
                role: "governance".to_owned(),
                public_key: governance_key.public_key().clone(),
                signature: SignatureOf::try_new(governance_key.private_key(), &policy_body)
                    .expect("policy signature"),
            }],
            body: policy_body,
        };
        ModerationScreeningAuthorityBundleV1 {
            version: MODERATION_SCREENING_AUTHORITY_BUNDLE_VERSION_V1,
            manifest,
            policy,
            governance_trust_anchors: vec![governance_key.public_key().clone()],
            minimum_governance_quorum: 1,
        }
    }

    fn write_moderation_screening_authority_bundle(
        root: &Path,
        bundle: &ModerationScreeningAuthorityBundleV1,
    ) -> (PathBuf, [u8; 32]) {
        let bytes = norito::to_bytes(bundle).expect("encode authority bundle");
        let root = fs::canonicalize(root).expect("canonicalize authority fixture root");
        let path = root.join("moderation-screening-authority.to");
        fs::write(&path, &bytes).expect("write authority bundle");
        #[cfg(unix)]
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("secure authority bundle permissions");
        (path, *blake3::hash(&bytes).as_bytes())
    }

    #[test]
    fn moderation_screening_authority_loads_from_digest_pinned_config_and_rejects_rotation_attacks()
    {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let now_unix = unix_now_secs();
        let policy_issued_at_unix = now_unix.saturating_sub(50);
        let bundle =
            moderation_screening_authority_bundle_fixture(now_unix, policy_issued_at_unix, 0xE1);
        let (path, digest) = write_moderation_screening_authority_bundle(temp_dir.path(), &bundle);
        let storage_dir = path
            .parent()
            .expect("authority fixture must have a parent")
            .join("storage");
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(storage_dir)
            .moderation_screening_enabled(true)
            .moderation_screening_authority_bundle_path(Some(path))
            .moderation_screening_authority_bundle_digest(Some(digest))
            .build();
        assert!(matches!(
            NodeHandle::try_new(cfg.clone()),
            Err(NodeInitError::ModerationQuarantineKeyWrapperUnavailable)
        ));
        let key_wrapper = test_quarantine_key_wrapper();
        let node = NodeHandle::try_new_with_quarantine_key_wrapper(cfg, Arc::clone(&key_wrapper))
            .expect("load config-authoritative screening bundle with runtime key wrapper");
        assert!(node.has_moderation_screening_authority());
        assert!(node.moderation_screening_enabled());
        assert!(node.uses_moderation_quarantine_key_wrapper(&key_wrapper));
        assert_eq!(
            node.moderation_quarantine_key_id(),
            Some("kms:test/quarantine-v1")
        );

        let older = moderation_screening_authority_bundle_fixture(
            now_unix,
            policy_issued_at_unix.saturating_sub(1),
            0xE0,
        )
        .into_authority(now_unix)
        .expect("older authority is otherwise valid");
        assert!(matches!(
            node.install_moderation_screening_authority(older),
            Err(ModerationScreeningAuthenticationError::PolicyRollback { .. })
        ));

        let equivocation =
            moderation_screening_authority_bundle_fixture(now_unix, policy_issued_at_unix, 0xE2)
                .into_authority(now_unix)
                .expect("equivocating authority is otherwise valid");
        assert!(matches!(
            node.install_moderation_screening_authority(equivocation),
            Err(ModerationScreeningAuthenticationError::PolicyEquivocation { .. })
        ));
    }

    #[test]
    fn moderation_screening_authority_startup_rejects_missing_mismatched_and_noncanonical_bundles()
    {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let missing = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_dir.path().join("missing-storage"))
            .moderation_screening_enabled(true)
            .build();
        assert!(matches!(
            NodeHandle::try_new(missing),
            Err(NodeInitError::ModerationScreeningAuthorityBundle { .. })
        ));

        let relative_path = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_dir.path().join("relative-path-storage"))
            .moderation_screening_enabled(true)
            .moderation_screening_authority_bundle_path(Some(PathBuf::from(
                "relative-authority.to",
            )))
            .moderation_screening_authority_bundle_digest(Some([0xA5; 32]))
            .build();
        assert!(matches!(
            NodeHandle::try_new(relative_path),
            Err(NodeInitError::ModerationScreeningAuthorityBundle { .. })
        ));

        let now_unix = unix_now_secs();
        let bundle = moderation_screening_authority_bundle_fixture(
            now_unix,
            now_unix.saturating_sub(50),
            0xE3,
        );
        let (path, digest) = write_moderation_screening_authority_bundle(temp_dir.path(), &bundle);
        let missing_digest = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_dir.path().join("missing-digest-storage"))
            .moderation_screening_enabled(true)
            .moderation_screening_authority_bundle_path(Some(path.clone()))
            .build();
        assert!(matches!(
            NodeHandle::try_new(missing_digest),
            Err(NodeInitError::ModerationScreeningAuthorityBundle { .. })
        ));

        let mismatched_digest = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_dir.path().join("mismatch-storage"))
            .moderation_screening_enabled(true)
            .moderation_screening_authority_bundle_path(Some(path.clone()))
            .moderation_screening_authority_bundle_digest(Some([0xFF; 32]))
            .build();
        assert!(matches!(
            NodeHandle::try_new(mismatched_digest),
            Err(NodeInitError::ModerationScreeningAuthorityBundle { .. })
        ));

        let mut noncanonical = fs::read(&path).expect("read canonical bundle");
        noncanonical.push(0);
        fs::write(&path, &noncanonical).expect("write noncanonical bundle");
        #[cfg(unix)]
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("secure noncanonical bundle permissions");
        let noncanonical_config = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_dir.path().join("noncanonical-storage"))
            .moderation_screening_enabled(true)
            .moderation_screening_authority_bundle_path(Some(path))
            .moderation_screening_authority_bundle_digest(Some(
                *blake3::hash(&noncanonical).as_bytes(),
            ))
            .build();
        assert!(matches!(
            NodeHandle::try_new(noncanonical_config),
            Err(NodeInitError::ModerationScreeningAuthorityBundle { .. })
        ));
        assert_ne!(digest, [0; 32]);
    }

    #[test]
    fn moderation_screening_authority_startup_bounds_oversized_and_sequence_bomb_inputs() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let temp_root = fs::canonicalize(temp_dir.path()).expect("canonicalize temp root");
        let oversized =
            vec![0_u8; MODERATION_SCREENING_AUTHORITY_BUNDLE_MAX_BYTES_V1.saturating_add(1)];
        let oversized_path = temp_root.join("oversized-authority.to");
        fs::write(&oversized_path, &oversized).expect("write oversized authority");
        #[cfg(unix)]
        fs::set_permissions(&oversized_path, fs::Permissions::from_mode(0o600))
            .expect("secure oversized bundle permissions");
        let oversized_config = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_root.join("oversized-storage"))
            .moderation_screening_enabled(true)
            .moderation_screening_authority_bundle_path(Some(oversized_path))
            .moderation_screening_authority_bundle_digest(Some(
                *blake3::hash(&oversized).as_bytes(),
            ))
            .build();
        assert!(matches!(
            NodeHandle::try_new(oversized_config),
            Err(NodeInitError::ModerationScreeningAuthorityBundle { .. })
        ));

        let now_unix = unix_now_secs();
        let mut sequence_bomb = moderation_screening_authority_bundle_fixture(
            now_unix,
            now_unix.saturating_sub(50),
            0xE4,
        );
        sequence_bomb.governance_trust_anchors =
            vec![sequence_bomb.governance_trust_anchors[0].clone(); 4_097];
        let sequence_bomb_bytes = norito::to_bytes(&sequence_bomb).expect("encode sequence bomb");
        assert!(
            sequence_bomb_bytes.len() < MODERATION_SCREENING_AUTHORITY_BUNDLE_MAX_BYTES_V1,
            "sequence bomb must exercise decode bounds rather than the byte cap"
        );
        let sequence_bomb_path = temp_root.join("sequence-bomb-authority.to");
        fs::write(&sequence_bomb_path, &sequence_bomb_bytes).expect("write sequence bomb");
        #[cfg(unix)]
        fs::set_permissions(&sequence_bomb_path, fs::Permissions::from_mode(0o600))
            .expect("secure sequence bomb permissions");
        let sequence_bomb_config = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_root.join("sequence-bomb-storage"))
            .moderation_screening_enabled(true)
            .moderation_screening_authority_bundle_path(Some(sequence_bomb_path))
            .moderation_screening_authority_bundle_digest(Some(
                *blake3::hash(&sequence_bomb_bytes).as_bytes(),
            ))
            .build();
        assert!(matches!(
            NodeHandle::try_new(sequence_bomb_config),
            Err(NodeInitError::ModerationScreeningAuthorityBundle { .. })
        ));

        let mut duplicate_anchors = moderation_screening_authority_bundle_fixture(
            now_unix,
            now_unix.saturating_sub(50),
            0xE6,
        );
        duplicate_anchors
            .governance_trust_anchors
            .push(duplicate_anchors.governance_trust_anchors[0].clone());
        let duplicate_anchor_bytes =
            norito::to_bytes(&duplicate_anchors).expect("encode duplicate anchors");
        let duplicate_anchor_path = temp_root.join("duplicate-anchor-authority.to");
        fs::write(&duplicate_anchor_path, &duplicate_anchor_bytes)
            .expect("write duplicate-anchor authority");
        #[cfg(unix)]
        fs::set_permissions(&duplicate_anchor_path, fs::Permissions::from_mode(0o600))
            .expect("secure duplicate-anchor permissions");
        let duplicate_anchor_config = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_root.join("duplicate-anchor-storage"))
            .moderation_screening_enabled(true)
            .moderation_screening_authority_bundle_path(Some(duplicate_anchor_path))
            .moderation_screening_authority_bundle_digest(Some(
                *blake3::hash(&duplicate_anchor_bytes).as_bytes(),
            ))
            .build();
        assert!(matches!(
            NodeHandle::try_new(duplicate_anchor_config),
            Err(NodeInitError::ModerationScreeningAuthorityBundle { .. })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn moderation_screening_authority_startup_rejects_symlink_and_hardlink_replacement_paths() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let now_unix = unix_now_secs();
        let bundle = moderation_screening_authority_bundle_fixture(
            now_unix,
            now_unix.saturating_sub(50),
            0xE5,
        );
        let (source_path, digest) =
            write_moderation_screening_authority_bundle(temp_dir.path(), &bundle);
        let temp_root = source_path
            .parent()
            .expect("authority fixture must have a parent");

        let symlink_path = temp_root.join("symlink-authority.to");
        std::os::unix::fs::symlink(&source_path, &symlink_path).expect("create authority symlink");
        let symlink_config = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_root.join("symlink-storage"))
            .moderation_screening_enabled(true)
            .moderation_screening_authority_bundle_path(Some(symlink_path))
            .moderation_screening_authority_bundle_digest(Some(digest))
            .build();
        assert!(matches!(
            NodeHandle::try_new(symlink_config),
            Err(NodeInitError::ModerationScreeningAuthorityBundle { .. })
        ));

        let hardlink_path = temp_root.join("hardlink-authority.to");
        fs::hard_link(&source_path, &hardlink_path).expect("create authority hard link");
        let hardlink_config = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_root.join("hardlink-storage"))
            .moderation_screening_enabled(true)
            .moderation_screening_authority_bundle_path(Some(hardlink_path))
            .moderation_screening_authority_bundle_digest(Some(digest))
            .build();
        assert!(matches!(
            NodeHandle::try_new(hardlink_config),
            Err(NodeInitError::ModerationScreeningAuthorityBundle { .. })
        ));

        let directory_path = temp_root.join("directory-authority");
        fs::create_dir(&directory_path).expect("create non-regular authority path");
        let directory_config = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_root.join("directory-storage"))
            .moderation_screening_enabled(true)
            .moderation_screening_authority_bundle_path(Some(directory_path))
            .moderation_screening_authority_bundle_digest(Some(digest))
            .build();
        assert!(matches!(
            NodeHandle::try_new(directory_config),
            Err(NodeInitError::ModerationScreeningAuthorityBundle { .. })
        ));
    }

    fn adversarial_corpus_manifest_fixture() -> AdversarialCorpusManifestV1 {
        AdversarialCorpusManifestV1 {
            schema_version: ADVERSARIAL_CORPUS_VERSION_V1,
            issued_at_unix: 1_800_000_010,
            cohort_label: Some("sfm4a-2026-q1".to_string()),
            families: vec![AdversarialPerceptualFamilyV1 {
                family_id: [0x21; 16],
                description: "jpeg jitter corpus".to_string(),
                variants: vec![
                    AdversarialPerceptualVariantV1 {
                        variant_id: [0x31; 16],
                        attack_vector: "jpeg_jitter".to_string(),
                        reference_cid_b64: None,
                        perceptual_hash: Some([0x41; 32]),
                        hamming_radius: 8,
                        embedding_digest: None,
                        notes: Some("hash match".to_string()),
                    },
                    AdversarialPerceptualVariantV1 {
                        variant_id: [0x32; 16],
                        attack_vector: "mosaic".to_string(),
                        reference_cid_b64: None,
                        perceptual_hash: None,
                        hamming_radius: 0,
                        embedding_digest: Some([0x42; 32]),
                        notes: Some("embedding match".to_string()),
                    },
                ],
            }],
        }
    }

    fn moderation_screening_input_fixture(
        subject: &str,
        verdict: ModerationScreeningVerdict,
    ) -> ModerationScreeningInput {
        ModerationScreeningInput {
            subject: subject.to_string(),
            subject_digest: [0xA1; 32],
            manifest_id: [0x12; 16],
            runner_hash: [0x34; 32],
            combined_score_bps: match verdict {
                ModerationScreeningVerdict::Pass => 1_000,
                ModerationScreeningVerdict::Warn => 3_000,
                ModerationScreeningVerdict::Quarantine => 6_500,
                ModerationScreeningVerdict::Escalate => 8_700,
                ModerationScreeningVerdict::Block => 9_900,
            },
            verdict,
            screened_at_unix: 1_800_000_050,
            evidence_digest: Some([0xE1; 32]),
            policy_digest: Some([0xC1; 32]),
            notes: Some("local screening fixture".to_string()),
        }
    }

    fn moderation_quarantine_review_input(
        quarantine_id: [u8; 16],
    ) -> ModerationQuarantineReviewInput {
        ModerationQuarantineReviewInput {
            quarantine_id,
            reviewed_by: "operator@moderation".to_string(),
            reviewed_at_unix: 1_800_000_060,
            notes: Some("reviewed locally".to_string()),
        }
    }

    fn moderation_quarantine_release_input(
        quarantine_id: [u8; 16],
    ) -> ModerationQuarantineReleaseInput {
        ModerationQuarantineReleaseInput {
            quarantine_id,
            release_authority: "release-authority@moderation".to_string(),
            released_at_unix: 1_800_000_070,
            notes: Some("released locally".to_string()),
        }
    }

    fn moderation_evidence_viewer_session_input(
        quarantine_id: [u8; 16],
        issued_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> ModerationEvidenceViewerSessionInput {
        ModerationEvidenceViewerSessionInput {
            quarantine_id,
            requested_by: "operator@moderation".to_string(),
            viewer_account: "juror-1@moderation".to_string(),
            viewer_role: "juror".to_string(),
            purpose: "appeal evidence review".to_string(),
            attestation_digest: [0xA7; 32],
            watermark_metadata_digest: [0xB7; 32],
            session_nonce_digest: [0xC7; 32],
            issued_at_unix_ms,
            expires_at_unix_ms,
            legal_hold_id: Some("legal-hold-2026-07".to_string()),
            notes: Some("payload-free viewer session".to_string()),
            raw_evidence_included: false,
            signed_url_included: false,
            session_token_included: false,
            watermark_secret_included: false,
        }
    }

    fn moderation_evidence_viewer_access_input(
        session_id: [u8; 16],
        kind: ModerationEvidenceViewerAccessKind,
        event_at_unix_ms: u64,
    ) -> ModerationEvidenceViewerAccessInput {
        ModerationEvidenceViewerAccessInput {
            session_id,
            kind,
            actor_account: "juror-1@moderation".to_string(),
            event_at_unix_ms,
            request_digest: [0xD7; 32],
            event_metadata_digest: Some([0xE7; 32]),
            notes: Some("payload-free viewer access".to_string()),
            raw_evidence_included: false,
            signed_url_included: false,
            session_token_included: false,
            response_body_included: false,
        }
    }

    fn moderation_evidence_viewer_audit_report_input(
        window_start_unix: u64,
        window_end_unix: u64,
        generated_at_unix: u64,
    ) -> ModerationEvidenceViewerAuditReportInput {
        ModerationEvidenceViewerAuditReportInput {
            report_scope: "local-daily".to_string(),
            window_start_unix,
            window_end_unix,
            generated_at_unix,
            policy_digest: Some([0xF7; 32]),
            raw_evidence_included: false,
            raw_access_logs_included: false,
            viewer_accounts_included: false,
            signed_urls_included: false,
            session_tokens_included: false,
            response_bodies_included: false,
        }
    }

    fn seed_moderation_evidence_viewer_activity(
        handle: &NodeHandle,
        subject: &str,
        payload: &[u8],
        issued_at_unix_ms: u64,
        expires_at_unix_ms: u64,
        access_events: &[(ModerationEvidenceViewerAccessKind, u64)],
    ) -> ModerationEvidenceViewerSessionRecord {
        let mut screening =
            moderation_screening_input_fixture(subject, ModerationScreeningVerdict::Quarantine);
        screening.subject_digest = *blake3::hash(payload).as_bytes();
        let outcome = handle
            .record_moderation_screening_result(screening)
            .expect("record quarantine result");
        let quarantine_id = outcome.quarantine.expect("quarantine record").quarantine_id;
        handle
            .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                quarantine_id,
                payload: payload.to_vec(),
                captured_at_unix: issued_at_unix_ms / 1_000,
                content_type: None,
                notes: None,
            })
            .expect("store quarantine object");
        let session = handle
            .create_moderation_evidence_viewer_session(moderation_evidence_viewer_session_input(
                quarantine_id,
                issued_at_unix_ms,
                expires_at_unix_ms,
            ))
            .expect("create viewer session");
        for (idx, (kind, event_at_unix_ms)) in access_events.iter().copied().enumerate() {
            let mut input =
                moderation_evidence_viewer_access_input(session.session_id, kind, event_at_unix_ms);
            input.request_digest = [0xD7_u8.wrapping_add(idx as u8); 32];
            input.event_metadata_digest = Some([0xE7_u8.wrapping_add(idx as u8); 32]);
            handle
                .record_moderation_evidence_viewer_access(input)
                .expect("record viewer access");
        }
        session
    }

    fn reputation_provider_input_fixture() -> ReputationProviderInputV1 {
        let metrics = ReputationProviderMetricsV1 {
            version: REPUTATION_PROVIDER_METRICS_VERSION_V1,
            por_success_bps: 9_800,
            pdp_success_bps: 9_700,
            potr_success_bps: 9_600,
            latency_health_bps: 9_000,
            dispute_rate_bps: 100,
            token_violation_rate_bps: 50,
            repair_breach_rate_bps: 0,
        };
        ReputationProviderInputV1 {
            version: REPUTATION_PROVIDER_INPUT_VERSION_V1,
            provider_id: "provider-a".to_string(),
            metrics,
            reserve_stage: ReputationReserveStageV1::Active,
            previous_score_bps: None,
            active_dispute: false,
            slashing_event: false,
        }
    }

    fn signed_reputation_snapshot_fixture_with(
        snapshot_id: [u8; 16],
        generated_at_unix: u64,
        previous_snapshot_id: Option<[u8; 16]>,
    ) -> SignedReputationSnapshotV1 {
        signed_reputation_snapshot_fixture_for_policy(
            &reputation_trust_policy_fixture(),
            snapshot_id,
            generated_at_unix,
            previous_snapshot_id,
        )
    }

    fn signed_reputation_snapshot_fixture_for_policy(
        policy: &ReputationSnapshotTrustPolicyV1,
        snapshot_id: [u8; 16],
        generated_at_unix: u64,
        previous_snapshot_id: Option<[u8; 16]>,
    ) -> SignedReputationSnapshotV1 {
        let input = reputation_provider_input_fixture();
        let scoring_evidence = ReputationScoringEvidenceV1 {
            version: REPUTATION_SCORING_EVIDENCE_VERSION_V1,
            provider_inputs: vec![input.clone()],
            trust_edges: Vec::new(),
        };
        let snapshot = build_reputation_snapshot(
            snapshot_id,
            generated_at_unix,
            ReputationWeightsV1::default(),
            &[input],
            previous_snapshot_id,
        )
        .expect("reputation snapshot fixture");
        let mut envelope = SignedReputationSnapshotV1 {
            version: SIGNED_REPUTATION_SNAPSHOT_VERSION_V1,
            policy_digest: policy.canonical_digest().expect("reputation policy digest"),
            snapshot,
            scoring_evidence_digest: scoring_evidence
                .canonical_digest()
                .expect("reputation evidence digest"),
            scoring_evidence,
            signatures: Vec::new(),
        };
        let signing_key = reputation_signing_key();
        let signature = IrohaSignature::try_new(
            signing_key.private_key(),
            &envelope
                .signing_digest()
                .expect("reputation signing digest"),
        )
        .expect("sign reputation snapshot");
        envelope.signatures.push(ReputationSnapshotSignatureV1 {
            signer_id: "council-1".to_owned(),
            signature: signature
                .payload()
                .try_into()
                .expect("Ed25519 signature is fixed-width"),
        });
        envelope
    }

    fn signed_reputation_snapshot_fixture() -> SignedReputationSnapshotV1 {
        signed_reputation_snapshot_fixture_with([0x42; 16], unix_now_secs(), None)
    }

    fn transparency_ledger_publication_fixture() -> ModerationLedgerCyclePublicationV1 {
        use iroha_data_model::sorafs::transparency::{
            MODERATION_LEDGER_ENTRY_VERSION_V1, ModerationLedgerEntryKindV1,
            ModerationLedgerEntryV1, ModerationLedgerMetadataV1,
        };

        let cycle_id = *b"cycle-2026-wk-02";
        let entries = [
            ModerationLedgerEntryV1 {
                version: MODERATION_LEDGER_ENTRY_VERSION_V1,
                cycle_id,
                entry_id: [0x22; 16],
                sequence: 2,
                occurred_at_unix: 1_800_000_020,
                kind: ModerationLedgerEntryKindV1::GarEnforcementReceipt,
                subject: "gar-receipt-22".to_string(),
                subject_digest: [0x22; 32],
                payload_digest: [0x23; 32],
                summary_digest: [0x24; 32],
                policy_digest: Some([0x25; 32]),
                evidence_uris: vec!["sora://transparency/22".to_string()],
                metadata: vec![ModerationLedgerMetadataV1 {
                    key: "source".to_string(),
                    value: "gar".to_string(),
                }],
            },
            ModerationLedgerEntryV1 {
                version: MODERATION_LEDGER_ENTRY_VERSION_V1,
                cycle_id,
                entry_id: [0x11; 16],
                sequence: 1,
                occurred_at_unix: 1_800_000_010,
                kind: ModerationLedgerEntryKindV1::AppealOutcome,
                subject: "appeal-case-11".to_string(),
                subject_digest: [0x11; 32],
                payload_digest: [0x12; 32],
                summary_digest: [0x13; 32],
                policy_digest: Some([0x14; 32]),
                evidence_uris: vec!["sora://transparency/11".to_string()],
                metadata: vec![ModerationLedgerMetadataV1 {
                    key: "source".to_string(),
                    value: "appeal".to_string(),
                }],
            },
        ];
        ModerationLedgerCyclePublicationV1::from_entries(
            cycle_id,
            1_800_000_000,
            1_800_604_800,
            1_800_604_801,
            None,
            &entries,
        )
        .expect("transparency ledger publication fixture")
    }

    fn privacy_aggregate_fixture(aggregate_id: &str, seed: u8) -> ModerationPrivacyAggregateV1 {
        use iroha_data_model::sorafs::transparency::{
            MODERATION_PRIVACY_AGGREGATE_VERSION_V1, MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
            ModerationLedgerMetadataV1, ModerationPrivacyAggregateMetricV1,
            ModerationPrivacyModeV1, ModerationPrivacyNoiseSourceV1, ModerationPrivacyParametersV1,
            ModerationPrivacyThresholdPrfCommitmentV1,
        };

        ModerationPrivacyAggregateV1 {
            version: MODERATION_PRIVACY_AGGREGATE_VERSION_V1,
            aggregate_id: aggregate_id.to_string(),
            window_start_unix: 1_800_000_000,
            window_end_unix: 1_800_604_800,
            generated_at_unix: 1_800_604_800,
            population_label: format!("{aggregate_id}-population"),
            population_digest: [seed; 32],
            privacy: ModerationPrivacyParametersV1 {
                version: MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
                mode: ModerationPrivacyModeV1::DifferentialPrivacyWithSuppression,
                epsilon_numerator: Some(4),
                epsilon_denominator: Some(5),
                delta_ppb: Some(0),
                per_subject_metric_cap: Some(1),
                suppression_threshold: Some(25),
            },
            noise_source: ModerationPrivacyNoiseSourceV1::ThresholdPrf(
                ModerationPrivacyThresholdPrfCommitmentV1 {
                    commitment: [0xCC; 32],
                },
            ),
            metrics: vec![
                ModerationPrivacyAggregateMetricV1 {
                    key: "appeals_upheld".to_string(),
                    value: u64::from(seed),
                    unit: "count".to_string(),
                },
                ModerationPrivacyAggregateMetricV1 {
                    key: "moderation_actions".to_string(),
                    value: u64::from(seed) + 10,
                    unit: "count".to_string(),
                },
            ],
            policy_digest: [0xC0; 32],
            metadata: vec![ModerationLedgerMetadataV1 {
                key: "publisher".to_string(),
                value: "sfm4c".to_string(),
            }],
        }
    }

    fn privacy_source_event(
        event_id: &str,
        population_label: &str,
        seed: u8,
        occurred_at_unix: u64,
    ) -> PrivacyAggregateSourceEvent {
        PrivacyAggregateSourceEvent {
            event_id: event_id.to_string(),
            occurred_at_unix,
            population_label: population_label.to_string(),
            population_digest: [seed; 32],
            subject_digest: *blake3::hash(event_id.as_bytes()).as_bytes(),
            metrics: vec![
                PrivacyAggregateSourceMetric {
                    key: "appeals_upheld".to_string(),
                    value: 1,
                    unit: "count".to_string(),
                },
                PrivacyAggregateSourceMetric {
                    key: "moderation_actions".to_string(),
                    value: 3,
                    unit: "count".to_string(),
                },
            ],
            policy_digest: [0xC0; 32],
        }
    }

    fn transparency_ledger_source_entry(
        event_id: &str,
        occurred_at_unix: u64,
        kind: iroha_data_model::sorafs::transparency::ModerationLedgerEntryKindV1,
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

    fn gar_enforcement_receipt_fixture(
        action: iroha_data_model::sorafs::gar::GarEnforcementActionV1,
    ) -> GarEnforcementReceiptV1 {
        GarEnforcementReceiptV1 {
            receipt_id: *b"gar-receipt-0001",
            gar_name: "docs.sora".to_string(),
            canonical_host: "docs.gateway.sora.net".to_string(),
            action,
            triggered_at_unix: 1_800_000_010,
            expires_at_unix: Some(1_800_086_410),
            policy_version: Some("2026-q2".to_string()),
            policy_digest: Some([0xAB; 32]),
            operator: iroha_data_model::account::AccountId::parse_encoded(
                "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE",
            )
            .map(iroha_data_model::account::ParsedAccountId::into_account_id)
            .expect("account id"),
            reason: "Guardian freeze window".to_string(),
            notes: Some("Escalated during SFM-4c drill".to_string()),
            evidence_uris: vec!["sora://gar/receipts/docs/0001".to_string()],
            labels: vec!["guardian-freeze".to_string(), "sfm4c".to_string()],
        }
    }

    fn privacy_aggregate_cycle_config() -> PrivacyAggregateCycleConfig {
        use iroha_data_model::sorafs::transparency::{
            MODERATION_PRIVACY_PARAMETERS_VERSION_V1, ModerationLedgerMetadataV1,
            ModerationPrivacyModeV1, ModerationPrivacyParametersV1,
        };

        PrivacyAggregateCycleConfig {
            query_id: [0xB0; 32],
            first_cycle_start_unix: 100,
            cycle_seconds: 100,
            aggregate_id_prefix: "sfm4c-weekly".to_string(),
            populations: vec![
                PrivacyAggregatePopulationV1 {
                    label: "jurisdiction-a".to_string(),
                    digest: [0xA0; 32],
                },
                PrivacyAggregatePopulationV1 {
                    label: "jurisdiction-b".to_string(),
                    digest: [0xB0; 32],
                },
            ],
            metrics: vec![
                PrivacyAggregateMetricSchemaV1 {
                    key: "appeals_upheld".to_string(),
                    unit: "count".to_string(),
                },
                PrivacyAggregateMetricSchemaV1 {
                    key: "moderation_actions".to_string(),
                    unit: "count".to_string(),
                },
            ],
            privacy: ModerationPrivacyParametersV1 {
                version: MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
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

    fn privacy_aggregate_schedule_config() -> PrivacyAggregateScheduleConfig {
        PrivacyAggregateScheduleConfig {
            first_cycle_start_unix: 100,
            cycle_seconds: 100,
            publish_delay_seconds: 10,
        }
    }

    fn privacy_cycle_prf_input(
        config: &PrivacyAggregateCycleConfig,
        cycle_start_unix: u64,
        cycle_end_unix: u64,
        due_at_unix: u64,
        output: [u8; 32],
    ) -> PrivacyCyclePrfInputV1 {
        let request = PrivacyCyclePrfRequestV1::new(
            config.query_id,
            config.policy_digest,
            privacy_population_inventory_digest(&config.populations),
            privacy_metric_schema_digest(&config.metrics),
            PrivacyAggregateCycleWindow {
                cycle_start_unix,
                cycle_end_unix,
                due_at_unix,
            },
        )
        .expect("test privacy PRF request");
        PrivacyCyclePrfInputV1::new(
            request,
            PrivacyCyclePrfOutputV1::new(output).expect("test privacy PRF input"),
        )
    }

    fn privacy_composition_budget_policy() -> PrivacyCompositionBudgetPolicyV1 {
        PrivacyCompositionBudgetPolicyV1 {
            budget_id: [0xB0; 32],
            epsilon_limit_numerator: 80,
            epsilon_limit_denominator: 1,
            max_publications: 100,
        }
    }

    fn privacy_aggregate_policy_config() -> config::PrivacyAggregatePolicyConfig {
        privacy_aggregate_policy_config_for_cycle(privacy_aggregate_cycle_config())
    }

    fn privacy_aggregate_policy_config_for_cycle(
        cycle: PrivacyAggregateCycleConfig,
    ) -> config::PrivacyAggregatePolicyConfig {
        config::PrivacyAggregatePolicyConfig::new(
            cycle.query_id,
            cycle.first_cycle_start_unix,
            cycle.cycle_seconds,
            cycle.aggregate_id_prefix,
            cycle.populations,
            cycle.metrics,
            cycle.privacy,
            cycle.policy_digest,
            privacy_composition_budget_policy(),
        )
        .expect("test privacy aggregate policy")
    }

    fn privacy_aggregate_storage_config(root: &Path) -> StorageConfig {
        StorageConfig::builder()
            .enabled(true)
            .data_dir(root.join("storage"))
            .privacy_aggregate_schedule(Some(privacy_aggregate_schedule_config()))
            .privacy_aggregate_policy(Some(privacy_aggregate_policy_config()))
            .build()
    }

    fn appeal_finance_report_fixture() -> SoraFsAppealFinanceReportV1 {
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

    fn proof_token_issuance_fixture() -> ProofTokenIssuanceV1 {
        ProofTokenIssuanceV1 {
            version: iroha_data_model::sorafs::transparency::PROOF_TOKEN_ISSUANCE_VERSION_V1,
            token_id: [0x61; 16],
            issued_at_unix: 1_800_000_030,
            expires_at_unix: Some(1_800_086_430),
            moderation_action_code: 2,
            signer_key: [0x62; 32],
            token_blake3: [0x63; 32],
            blinded_digest: [0x64; 32],
            entry_ids: vec!["denylist/global".to_string(), "gar/policy/42".to_string()],
            evidence_digest: Some([0x65; 32]),
            policy_digest: Some([0x66; 32]),
            metadata: vec![
                iroha_data_model::sorafs::transparency::ModerationLedgerMetadataV1 {
                    key: "issuer".to_string(),
                    value: "gateway-a".to_string(),
                },
            ],
        }
    }

    const VALID_PROOF_TOKEN_SIGNER_HEX: &str =
        "f4bfda67d38a409557e4a910dbdf0a862ee5aa6cf6c2284aa38b0b82c4f16532";
    const VALID_PROOF_TOKEN_B64: &str = "U0ZHVAEBAgAAAABrSdIeAAAAAGtLI55hYWFhYWFhYWFhYWFhYWFhAAIAD2RlbnlsaXN0L2dsb2JhbAANZ2FyL3BvbGljeS80MmRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkZGRkAEDHmshANx2cvkpmh1mCkrE94PJ6hL0A0qX4vQ-T3rWyTUKZG6uGoYM2sXbL36cYTahpsgcQ35z4R9bb1owinokB";

    fn proof_token_signer_key_fixture() -> [u8; 32] {
        hex::decode(VALID_PROOF_TOKEN_SIGNER_HEX)
            .expect("valid proof-token signer hex")
            .try_into()
            .expect("proof-token signer key length")
    }

    fn appeal_finance_weekly_rollup_fixture() -> SoraFsAppealFinanceWeeklyRollupV1 {
        let report = appeal_finance_report_fixture();
        SoraFsAppealFinanceWeeklyRollupV1::from_reports(
            PorReportIsoWeek {
                year: 2026,
                week: 26,
            },
            1_800_000_100_000,
            &[report],
        )
        .expect("appeal finance weekly rollup fixture")
    }

    fn appeal_finance_settlement_receipt_fixture() -> SoraFsAppealFinanceSettlementReceiptV1 {
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
            submitted_step: "drawdown_non_refund".to_string(),
            required_authority: "release-authority".to_string(),
            amount_xor: xor("420"),
            tx_hash_hex: "22".repeat(32),
            reconciliation_digest_hex: "33".repeat(32),
            reconciliation_status: "pending_client_submission".to_string(),
            observed_lifecycle_status: "locked".to_string(),
            observed_remaining_xor: xor("420"),
            deposit_xor: xor("420"),
            refund_xor: xor("0"),
            treasury_xor: xor("210"),
            held_xor: xor("210"),
            panel_size: 7,
            configured_signer_count: 1,
        }
    }

    #[test]
    fn moderation_model_registry_admits_repro_manifest_and_rejects_conflict() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let manifest = moderation_repro_manifest_fixture(0x10, 0x30);
        let expected_manifest_digest = manifest.body.manifest_digest;

        let record = handle
            .admit_moderation_repro_manifest(manifest.clone())
            .expect("admit repro manifest");
        assert_eq!(record.manifest_id, [0x10; 16]);
        assert_eq!(record.manifest_digest, expected_manifest_digest);
        assert_eq!(record.runner_hash, [0x30; 32]);
        assert_eq!(record.model_count, 1);
        assert_eq!(record.signer_count, 1);

        let repeated = handle
            .admit_moderation_repro_manifest(manifest)
            .expect("re-admit matching repro manifest");
        assert_eq!(repeated, record);

        let err = handle
            .admit_moderation_repro_manifest(moderation_repro_manifest_fixture(0x10, 0x31))
            .expect_err("conflicting manifest id rejected");
        assert!(matches!(
            err,
            ModerationModelRegistryError::ConflictingReproManifest { .. }
        ));

        let snapshot = handle.moderation_model_registry_snapshot();
        assert_eq!(snapshot.reproducibility_manifests, vec![record]);
        assert!(snapshot.adversarial_corpora.is_empty());
    }

    #[test]
    fn moderation_model_registry_admits_corpus_manifest_snapshot() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let manifest = adversarial_corpus_manifest_fixture();
        let expected_digest =
            *blake3::hash(&to_bytes(&manifest).expect("encode corpus fixture")).as_bytes();

        let record = handle
            .admit_moderation_corpus_manifest(manifest.clone())
            .expect("admit corpus manifest");
        assert_eq!(record.corpus_digest, expected_digest);
        assert_eq!(record.cohort_label.as_deref(), Some("sfm4a-2026-q1"));
        assert_eq!(record.family_count, 1);
        assert_eq!(record.variant_count, 2);

        let repeated = handle
            .admit_moderation_corpus_manifest(manifest)
            .expect("re-admit matching corpus manifest");
        assert_eq!(repeated, record);

        let snapshot = handle.moderation_model_registry_snapshot();
        assert!(snapshot.reproducibility_manifests.is_empty());
        assert_eq!(snapshot.adversarial_corpora, vec![record]);
    }

    #[test]
    fn moderation_model_registry_checkpoint_persists_and_reloads_snapshot() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let source = NodeHandle::new(cfg.clone());
        let repro_record = source
            .admit_moderation_repro_manifest(moderation_repro_manifest_fixture(0x12, 0x32))
            .expect("admit repro manifest");
        let corpus_record = source
            .admit_moderation_corpus_manifest(adversarial_corpus_manifest_fixture())
            .expect("admit corpus manifest");

        let checkpoint_path = moderation_model_registry_checkpoint_path(cfg.data_dir());
        let checkpoint_bytes = fs::read(&checkpoint_path).expect("read registry checkpoint");
        let checkpoint: ModerationModelRegistrySnapshot =
            norito::decode_from_bytes(&checkpoint_bytes).expect("decode registry checkpoint");
        assert_eq!(
            checkpoint.reproducibility_manifests,
            vec![repro_record.clone()]
        );
        assert_eq!(checkpoint.adversarial_corpora, vec![corpus_record.clone()]);

        drop(source);
        let restored = NodeHandle::new(cfg);
        let restored_snapshot = restored
            .export_moderation_model_registry_snapshot()
            .expect("export restored registry snapshot");
        assert_eq!(
            restored_snapshot,
            ModerationModelRegistrySnapshot {
                reproducibility_manifests: vec![repro_record],
                adversarial_corpora: vec![corpus_record],
            }
        );
    }

    #[test]
    fn moderation_model_registry_restore_rejects_duplicate_records() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let record = ModerationReproRegistryRecord {
            manifest_id: [0x14; 16],
            manifest_digest: [0x24; 32],
            runner_hash: [0x34; 32],
            runtime_version: "sorafs-ai-runner 1.0.0".to_string(),
            issued_at_unix: 1_800_000_040,
            model_count: 1,
            signer_count: 1,
        };
        let err = handle
            .restore_moderation_model_registry_snapshot(ModerationModelRegistrySnapshot {
                reproducibility_manifests: vec![record.clone(), record],
                adversarial_corpora: Vec::new(),
            })
            .expect_err("duplicate manifest ids rejected");
        assert!(matches!(
            err,
            ModerationModelRegistryError::InvalidRegistrySnapshot { .. }
        ));
        assert_eq!(
            handle.moderation_model_registry_snapshot(),
            ModerationModelRegistrySnapshot::default()
        );
    }

    #[test]
    fn moderation_screening_records_deterministic_quarantine_queue() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let input = moderation_screening_input_fixture(
            "cid:bafy-screening",
            ModerationScreeningVerdict::Quarantine,
        );

        let outcome = handle
            .record_moderation_screening_result(input.clone())
            .expect("record screening result");
        assert_eq!(outcome.record.subject, "cid:bafy-screening");
        assert_eq!(
            outcome.record.verdict,
            ModerationScreeningVerdict::Quarantine
        );
        assert_eq!(outcome.record.combined_score_bps, 6_500);
        assert_eq!(
            &outcome.record.record_digest[..16],
            outcome.record.record_id
        );
        let quarantine = outcome.quarantine.expect("quarantine record");
        assert_eq!(quarantine.screening_record_id, outcome.record.record_id);
        assert_eq!(quarantine.subject_digest, outcome.record.subject_digest);
        assert_eq!(quarantine.verdict, ModerationScreeningVerdict::Quarantine);
        assert_eq!(quarantine.state, ModerationQuarantineState::PendingReview);
        assert!(quarantine.reviewed_at_unix.is_none());
        assert!(quarantine.released_at_unix.is_none());

        let repeated = handle
            .record_moderation_screening_result(input)
            .expect("idempotent screening result");
        assert_eq!(repeated.record, outcome.record);
        assert_eq!(repeated.quarantine, Some(quarantine.clone()));

        let snapshot = handle.moderation_screening_snapshot();
        assert_eq!(snapshot.screening_records, vec![outcome.record]);
        assert_eq!(snapshot.quarantine_records, vec![quarantine]);
    }

    #[test]
    fn moderation_screening_pass_does_not_create_quarantine_record() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);

        let outcome = handle
            .record_moderation_screening_result(moderation_screening_input_fixture(
                "cid:bafy-pass",
                ModerationScreeningVerdict::Pass,
            ))
            .expect("record pass result");
        assert!(outcome.quarantine.is_none());
        let snapshot = handle.moderation_screening_snapshot();
        assert_eq!(snapshot.screening_records, vec![outcome.record]);
        assert!(snapshot.quarantine_records.is_empty());
    }

    #[test]
    fn moderation_screening_checkpoint_persists_and_reloads_snapshot() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let source = NodeHandle::new(cfg.clone());
        let quarantine_outcome = source
            .record_moderation_screening_result(moderation_screening_input_fixture(
                "cid:bafy-quarantine",
                ModerationScreeningVerdict::Quarantine,
            ))
            .expect("record quarantine result");
        let pass_outcome = source
            .record_moderation_screening_result(moderation_screening_input_fixture(
                "cid:bafy-pass",
                ModerationScreeningVerdict::Pass,
            ))
            .expect("record pass result");

        let checkpoint_path = moderation_screening_checkpoint_path(cfg.data_dir());
        let checkpoint_bytes = fs::read(&checkpoint_path).expect("read screening checkpoint");
        let checkpoint: ModerationScreeningSnapshot =
            norito::decode_from_bytes(&checkpoint_bytes).expect("decode screening checkpoint");
        assert_eq!(checkpoint.screening_records.len(), 2);
        assert_eq!(checkpoint.quarantine_records.len(), 1);

        drop(source);
        let restored = NodeHandle::new(cfg);
        let restored_snapshot = restored
            .export_moderation_screening_snapshot()
            .expect("export restored screening snapshot");
        let mut expected_records = vec![quarantine_outcome.record, pass_outcome.record];
        expected_records.sort_by_key(|record| record.record_id);
        assert_eq!(
            restored_snapshot,
            ModerationScreeningSnapshot {
                screening_records: expected_records,
                quarantine_records: vec![quarantine_outcome.quarantine.expect("quarantine")],
                authenticated_admissions: Vec::new(),
            }
        );
    }

    #[test]
    fn moderation_quarantine_review_and_release_updates_checkpoint() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let source = NodeHandle::new(cfg.clone());
        let outcome = source
            .record_moderation_screening_result(moderation_screening_input_fixture(
                "cid:bafy-review-release",
                ModerationScreeningVerdict::Quarantine,
            ))
            .expect("record quarantine result");
        let quarantine_id = outcome
            .quarantine
            .as_ref()
            .expect("quarantine record")
            .quarantine_id;

        let reviewed = source
            .review_moderation_quarantine_record(moderation_quarantine_review_input(quarantine_id))
            .expect("review quarantine record");
        assert_eq!(reviewed.state, ModerationQuarantineState::Reviewed);
        assert_eq!(reviewed.reviewed_at_unix, Some(1_800_000_060));
        assert_eq!(reviewed.reviewed_by.as_deref(), Some("operator@moderation"));
        assert!(reviewed.released_at_unix.is_none());

        let released = source
            .release_moderation_quarantine_record(moderation_quarantine_release_input(
                quarantine_id,
            ))
            .expect("release quarantine record");
        assert_eq!(released.state, ModerationQuarantineState::Released);
        assert_eq!(released.reviewed_at_unix, Some(1_800_000_060));
        assert_eq!(released.released_at_unix, Some(1_800_000_070));
        assert_eq!(
            released.release_authority.as_deref(),
            Some("release-authority@moderation")
        );

        drop(source);
        let restored = NodeHandle::new(cfg);
        let snapshot = restored
            .export_moderation_screening_snapshot()
            .expect("export restored screening snapshot");
        assert_eq!(snapshot.screening_records, vec![outcome.record]);
        assert_eq!(snapshot.quarantine_records, vec![released]);
    }

    #[test]
    fn moderation_quarantine_object_store_persists_encrypted_payload_and_reloads() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let payload = b"quarantine payload bytes retained for operator review".to_vec();
        let mut screening = moderation_screening_input_fixture(
            "cid:bafy-object-store",
            ModerationScreeningVerdict::Quarantine,
        );
        screening.subject_digest = *blake3::hash(&payload).as_bytes();
        let source = node_with_test_quarantine_key_wrapper(cfg.clone());
        let outcome = source
            .record_moderation_screening_result(screening)
            .expect("record quarantine result");
        let quarantine_id = outcome.quarantine.expect("quarantine record").quarantine_id;

        let record = source
            .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                quarantine_id,
                payload: payload.clone(),
                captured_at_unix: 1_800_000_080,
                content_type: Some("application/octet-stream".to_string()),
                notes: None,
            })
            .expect("store quarantine object");
        assert_eq!(record.payload_digest, *blake3::hash(&payload).as_bytes());
        assert_eq!(record.payload_len, payload.len() as u64);
        assert_eq!(record.notes, None);

        let envelope_path =
            moderation_quarantine_object_store_root(cfg.data_dir()).join(&record.envelope_path);
        let envelope_bytes = fs::read(&envelope_path).expect("read encrypted envelope");
        let envelope: crate::moderation::ModerationQuarantineObjectEnvelopeV1 =
            norito::decode_from_bytes(&envelope_bytes).expect("decode encrypted envelope");
        assert_eq!(envelope.wrapping_key_id, "kms:test/quarantine-v1");
        assert!(!envelope.wrapped_dek.is_empty());
        assert!(!envelope.chunks.is_empty());
        assert!(
            !moderation_quarantine_object_store_root(cfg.data_dir())
                .join("local-seal.key")
                .exists(),
            "runtime wrapping keys must never be persisted in the object store"
        );
        assert!(
            !envelope_bytes
                .windows(payload.len())
                .any(|window| window == payload.as_slice()),
            "encrypted object envelope must not contain plaintext payload bytes"
        );

        let decrypted = source
            .read_moderation_quarantine_object(quarantine_id)
            .expect("read quarantine object");
        assert_eq!(decrypted.record, record);
        assert_eq!(decrypted.payload, payload);
        let replay = source
            .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                quarantine_id,
                payload: decrypted.payload.clone(),
                captured_at_unix: 1_800_000_080,
                content_type: Some("application/octet-stream".to_owned()),
                notes: None,
            })
            .expect("idempotently replay quarantine object store");
        assert_eq!(replay, record);

        let index_path = moderation_quarantine_object_index_path(cfg.data_dir());
        let index_bytes = fs::read(&index_path).expect("read object index");
        let index: ModerationQuarantineObjectSnapshot =
            norito::decode_from_bytes(&index_bytes).expect("decode object index");
        assert_eq!(index.objects, vec![record.clone()]);

        drop(source);
        let restored = node_with_test_quarantine_key_wrapper(cfg);
        assert_eq!(
            restored
                .export_moderation_quarantine_object_snapshot()
                .expect("export restored object index")
                .objects,
            vec![record.clone()]
        );
        let restored_payload = restored
            .read_moderation_quarantine_object(quarantine_id)
            .expect("read restored object");
        assert_eq!(restored_payload.record, record);
        assert_eq!(restored_payload.payload, payload);
    }

    #[test]
    fn moderation_quarantine_range_and_dek_rewrap_survive_restart() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let payload =
            (0..(crate::moderation::MODERATION_QUARANTINE_OBJECT_CHUNK_BYTES_V1 as usize + 8_192))
                .map(|index| (index % 251) as u8)
                .collect::<Vec<_>>();
        let mut screening = moderation_screening_input_fixture(
            "cid:bafy-object-range-rewrap",
            ModerationScreeningVerdict::Quarantine,
        );
        screening.subject_digest = *blake3::hash(&payload).as_bytes();
        let old_wrapper: Arc<dyn ModerationQuarantineKeyWrapper> = Arc::new(
            TestQuarantineKeyWrapper::single("kms:test/quarantine-old", 0x31),
        );
        let source =
            NodeHandle::try_new_with_quarantine_key_wrapper(cfg.clone(), Arc::clone(&old_wrapper))
                .expect("initialise with old wrapping key");
        let quarantine_id = source
            .record_moderation_screening_result(screening)
            .expect("record quarantine result")
            .quarantine
            .expect("quarantine record")
            .quarantine_id;
        let record = source
            .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                quarantine_id,
                payload: payload.clone(),
                captured_at_unix: 1_800_000_085,
                content_type: Some("application/octet-stream".to_owned()),
                notes: None,
            })
            .expect("store multi-chunk object");
        let range_start =
            u64::from(crate::moderation::MODERATION_QUARANTINE_OBJECT_CHUNK_BYTES_V1 - 1_024);
        let range_end = range_start + 4_096;
        let range = source
            .read_moderation_quarantine_object_range(quarantine_id, range_start, range_end)
            .expect("read authenticated cross-chunk range");
        assert_eq!(range.record, record);
        assert_eq!(range.start, range_start);
        assert_eq!(range.end, range_end);
        assert_eq!(
            range.payload,
            payload[range_start as usize..range_end as usize]
        );
        assert!(matches!(
            source
                .read_moderation_quarantine_object_range(
                    quarantine_id,
                    range_end,
                    record.payload_len + 1,
                )
                .expect_err("out-of-bounds range rejected"),
            ModerationQuarantineObjectError::InvalidRange { .. }
        ));
        drop(source);

        let rotated_wrapper: Arc<dyn ModerationQuarantineKeyWrapper> =
            Arc::new(TestQuarantineKeyWrapper::rotated(
                "kms:test/quarantine-old",
                0x31,
                "kms:test/quarantine-new",
                0x52,
            ));
        let rotated = NodeHandle::try_new_with_quarantine_key_wrapper(
            cfg.clone(),
            Arc::clone(&rotated_wrapper),
        )
        .expect("restart with rotation-capable wrapper");
        let envelope_path =
            moderation_quarantine_object_store_root(cfg.data_dir()).join(&record.envelope_path);
        let before_bytes = fs::read(&envelope_path).expect("read pre-rotation envelope");
        let before: crate::moderation::ModerationQuarantineObjectEnvelopeV1 =
            norito::decode_from_bytes(&before_bytes).expect("decode pre-rotation envelope");
        assert_eq!(before.wrapping_key_id, "kms:test/quarantine-old");
        assert_eq!(
            rotated
                .rewrap_moderation_quarantine_object_dek(quarantine_id)
                .expect("rewrap object DEK"),
            record
        );
        let after_bytes = fs::read(&envelope_path).expect("read rewrapped envelope");
        let after: crate::moderation::ModerationQuarantineObjectEnvelopeV1 =
            norito::decode_from_bytes(&after_bytes).expect("decode rewrapped envelope");
        assert_ne!(after_bytes, before_bytes);
        assert_eq!(after.wrapping_key_id, "kms:test/quarantine-new");
        assert_eq!(after.object_id, before.object_id);
        assert_eq!(after.ciphertext_digest, before.ciphertext_digest);
        assert_eq!(after.chunks, before.chunks);
        assert_eq!(
            rotated
                .read_moderation_quarantine_object(quarantine_id)
                .expect("read rewrapped object")
                .payload,
            payload
        );
        drop(rotated);

        let new_only_wrapper: Arc<dyn ModerationQuarantineKeyWrapper> = Arc::new(
            TestQuarantineKeyWrapper::single("kms:test/quarantine-new", 0x52),
        );
        let restored = NodeHandle::try_new_with_quarantine_key_wrapper(cfg, new_only_wrapper)
            .expect("restart using only the replacement key");
        assert_eq!(
            restored
                .read_moderation_quarantine_object(quarantine_id)
                .expect("read after rewrap restart")
                .payload,
            payload
        );
    }

    #[test]
    fn moderation_quarantine_startup_recovers_canonical_unindexed_envelope() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let payload = b"crash between envelope rename and index commit".to_vec();
        let mut screening = moderation_screening_input_fixture(
            "cid:bafy-object-crash-orphan",
            ModerationScreeningVerdict::Quarantine,
        );
        screening.subject_digest = *blake3::hash(&payload).as_bytes();
        let source = node_with_test_quarantine_key_wrapper(cfg.clone());
        let quarantine_id = source
            .record_moderation_screening_result(screening)
            .expect("record quarantine result")
            .quarantine
            .expect("quarantine record")
            .quarantine_id;
        let record = source
            .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                quarantine_id,
                payload,
                captured_at_unix: 1_800_000_086,
                content_type: None,
                notes: None,
            })
            .expect("store quarantine object");
        let envelope_path =
            moderation_quarantine_object_store_root(cfg.data_dir()).join(&record.envelope_path);
        source
            .persist_moderation_quarantine_object_index_snapshot(
                &ModerationQuarantineObjectSnapshot::default(),
            )
            .expect("simulate index state before interrupted insertion");
        drop(source);

        let restored = node_with_test_quarantine_key_wrapper(cfg);
        assert!(
            restored
                .moderation_quarantine_object_snapshot()
                .objects
                .is_empty()
        );
        assert!(
            !envelope_path.exists(),
            "startup recovery must durably remove a canonical unindexed envelope"
        );
    }

    #[test]
    fn moderation_quarantine_object_store_rejects_digest_mismatch() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let expected_payload = b"expected quarantined bytes".to_vec();
        let mut screening = moderation_screening_input_fixture(
            "cid:bafy-object-digest",
            ModerationScreeningVerdict::Quarantine,
        );
        screening.subject_digest = *blake3::hash(&expected_payload).as_bytes();
        let handle = node_with_test_quarantine_key_wrapper(cfg);
        let outcome = handle
            .record_moderation_screening_result(screening)
            .expect("record quarantine result");
        let quarantine_id = outcome.quarantine.expect("quarantine record").quarantine_id;

        let err = handle
            .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                quarantine_id,
                payload: b"different bytes".to_vec(),
                captured_at_unix: 1_800_000_081,
                content_type: None,
                notes: None,
            })
            .expect_err("digest mismatch rejected");
        assert!(matches!(
            err,
            ModerationQuarantineObjectError::DigestMismatch { .. }
        ));
        assert!(
            handle
                .moderation_quarantine_object_snapshot()
                .objects
                .is_empty()
        );
    }

    #[test]
    fn moderation_quarantine_object_read_rejects_tampered_envelope() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let payload = b"tamper-detected quarantine payload".to_vec();
        let mut screening = moderation_screening_input_fixture(
            "cid:bafy-object-tamper",
            ModerationScreeningVerdict::Quarantine,
        );
        screening.subject_digest = *blake3::hash(&payload).as_bytes();
        let handle = node_with_test_quarantine_key_wrapper(cfg.clone());
        let outcome = handle
            .record_moderation_screening_result(screening)
            .expect("record quarantine result");
        let quarantine_id = outcome.quarantine.expect("quarantine record").quarantine_id;
        let record = handle
            .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                quarantine_id,
                payload,
                captured_at_unix: 1_800_000_082,
                content_type: None,
                notes: None,
            })
            .expect("store quarantine object");
        let envelope_path =
            moderation_quarantine_object_store_root(cfg.data_dir()).join(&record.envelope_path);
        let envelope_bytes = fs::read(&envelope_path).expect("read envelope");
        let mut envelope: crate::moderation::ModerationQuarantineObjectEnvelopeV1 =
            norito::decode_from_bytes(&envelope_bytes).expect("decode envelope");
        envelope.chunks[0].ciphertext[0] ^= 0x01;
        let tampered_bytes = norito::to_bytes(&envelope).expect("encode tampered envelope");
        fs::write(&envelope_path, tampered_bytes).expect("write tampered envelope");

        let err = handle
            .read_moderation_quarantine_object(quarantine_id)
            .expect_err("tampered envelope rejected");
        assert!(matches!(
            err,
            ModerationQuarantineObjectError::AuthenticationFailed { .. }
        ));
    }

    #[test]
    fn moderation_quarantine_store_rejects_authenticated_envelope_tampering_on_restart() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let payload = b"restart audit must authenticate every quarantine envelope".to_vec();
        let mut screening = moderation_screening_input_fixture(
            "cid:bafy-object-restart-tamper",
            ModerationScreeningVerdict::Quarantine,
        );
        screening.subject_digest = *blake3::hash(&payload).as_bytes();
        let handle = node_with_test_quarantine_key_wrapper(cfg.clone());
        let quarantine_id = handle
            .record_moderation_screening_result(screening)
            .expect("record quarantine result")
            .quarantine
            .expect("quarantine record")
            .quarantine_id;
        let record = handle
            .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                quarantine_id,
                payload,
                captured_at_unix: 1_800_000_083,
                content_type: None,
                notes: None,
            })
            .expect("store quarantine object");
        drop(handle);

        let envelope_path =
            moderation_quarantine_object_store_root(cfg.data_dir()).join(&record.envelope_path);
        let bytes = fs::read(&envelope_path).expect("read envelope");
        let mut envelope: crate::moderation::ModerationQuarantineObjectEnvelopeV1 =
            norito::decode_from_bytes(&bytes).expect("decode envelope");
        envelope.chunks[0].ciphertext[0] ^= 0x80;
        fs::write(
            &envelope_path,
            norito::to_bytes(&envelope).expect("encode canonical tampered envelope"),
        )
        .expect("write tampered envelope");

        assert!(matches!(
            NodeHandle::try_new_with_quarantine_key_wrapper(cfg, test_quarantine_key_wrapper()),
            Err(NodeInitError::Checkpoint {
                component: "moderation quarantine object envelope",
                ..
            })
        ));
    }

    #[test]
    fn moderation_quarantine_store_requires_runtime_wrapper_and_rejects_unknown_orphans() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let payload = b"indexed quarantine object requires its runtime key wrapper".to_vec();
        let mut screening = moderation_screening_input_fixture(
            "cid:bafy-object-missing-wrapper",
            ModerationScreeningVerdict::Quarantine,
        );
        screening.subject_digest = *blake3::hash(&payload).as_bytes();
        let handle = node_with_test_quarantine_key_wrapper(cfg.clone());
        let quarantine_id = handle
            .record_moderation_screening_result(screening)
            .expect("record quarantine result")
            .quarantine
            .expect("quarantine record")
            .quarantine_id;
        handle
            .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                quarantine_id,
                payload,
                captured_at_unix: 1_800_000_084,
                content_type: None,
                notes: None,
            })
            .expect("store quarantine object");
        drop(handle);

        assert!(matches!(
            NodeHandle::try_new(cfg.clone()),
            Err(NodeInitError::Checkpoint {
                component: "moderation quarantine key wrapper",
                ..
            })
        ));

        let orphan = moderation_quarantine_object_store_root(cfg.data_dir()).join("orphan.to");
        fs::write(&orphan, b"not indexed").expect("write orphan object");
        assert!(matches!(
            NodeHandle::try_new_with_quarantine_key_wrapper(cfg, test_quarantine_key_wrapper()),
            Err(NodeInitError::Checkpoint {
                component: "moderation quarantine object store",
                ..
            })
        ));
    }

    #[cfg(unix)]
    #[test]
    fn moderation_quarantine_store_rejects_symlink_entries_on_restart() {
        use std::os::unix::fs::symlink;

        let (cfg, _dir) = storage_config_with_temp_dir();
        drop(NodeHandle::new(cfg.clone()));
        let victim = cfg.data_dir().join("quarantine-symlink-victim");
        fs::write(&victim, b"victim").expect("write symlink victim");
        let link = moderation_quarantine_object_store_root(cfg.data_dir()).join("orphan-link.to");
        symlink(&victim, &link).expect("create object-store symlink");

        assert!(matches!(
            NodeHandle::try_new(cfg),
            Err(NodeInitError::Checkpoint {
                component: "moderation quarantine object store",
                ..
            })
        ));
        assert_eq!(fs::read(victim).expect("victim remains intact"), b"victim");
    }

    #[test]
    fn moderation_snapshot_restore_rejects_dangling_cross_checkpoint_references() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = node_with_test_quarantine_key_wrapper(cfg);
        seed_moderation_evidence_viewer_activity(
            &handle,
            "cid:bafy-cross-checkpoint-refs",
            b"cross-checkpoint reference fixture",
            1_800_000_100_000,
            1_800_000_200_000,
            &[],
        );
        let screening_before = handle.moderation_screening_snapshot();
        let objects_before = handle.moderation_quarantine_object_snapshot();

        let screening_error = handle
            .restore_moderation_screening_snapshot(ModerationScreeningSnapshot::default())
            .expect_err("referenced quarantine cannot be removed");
        assert!(matches!(
            screening_error,
            ModerationScreeningError::InvalidSnapshot { .. }
        ));
        assert_eq!(handle.moderation_screening_snapshot(), screening_before);

        let object_error = handle
            .restore_moderation_quarantine_object_snapshot(
                ModerationQuarantineObjectSnapshot::default(),
            )
            .expect_err("viewer-referenced object cannot be removed");
        assert!(matches!(
            object_error,
            ModerationQuarantineObjectError::InvalidSnapshot { .. }
        ));
        assert_eq!(
            handle.moderation_quarantine_object_snapshot(),
            objects_before
        );
    }

    #[test]
    fn moderation_evidence_viewer_session_access_persists_and_reloads() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let payload = b"payload-free evidence viewer audit fixture".to_vec();
        let mut screening = moderation_screening_input_fixture(
            "cid:bafy-evidence-viewer",
            ModerationScreeningVerdict::Quarantine,
        );
        screening.subject_digest = *blake3::hash(&payload).as_bytes();
        let source = node_with_test_quarantine_key_wrapper(cfg.clone());
        let outcome = source
            .record_moderation_screening_result(screening)
            .expect("record quarantine result");
        let quarantine_id = outcome.quarantine.expect("quarantine record").quarantine_id;
        let object = source
            .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                quarantine_id,
                payload: payload.clone(),
                captured_at_unix: 1_800_000_090,
                content_type: Some("application/octet-stream".to_string()),
                notes: None,
            })
            .expect("store quarantine object");

        let session = source
            .create_moderation_evidence_viewer_session(moderation_evidence_viewer_session_input(
                quarantine_id,
                1_800_000_000_000,
                1_800_000_300_000,
            ))
            .expect("create viewer session");
        assert_eq!(session.quarantine_id, quarantine_id);
        assert_eq!(session.object_id, object.object_id);
        assert_eq!(session.evidence_digest, *blake3::hash(&payload).as_bytes());
        assert_eq!(session.viewer_role, "juror");
        assert_eq!(
            session.session_id.as_slice(),
            &session.session_manifest_digest[..16]
        );

        let access = source
            .record_moderation_evidence_viewer_access(moderation_evidence_viewer_access_input(
                session.session_id,
                ModerationEvidenceViewerAccessKind::Viewed,
                1_800_000_010_000,
            ))
            .expect("record viewer access");
        assert_eq!(access.sequence, 1);
        assert_eq!(access.session_id, session.session_id);
        assert_eq!(access.quarantine_id, quarantine_id);
        assert_eq!(access.kind, ModerationEvidenceViewerAccessKind::Viewed);

        let checkpoint_path = moderation_evidence_viewer_checkpoint_path(cfg.data_dir());
        assert!(
            checkpoint_path.exists(),
            "evidence viewer checkpoint must persist when storage is enabled"
        );
        drop(source);
        let restored = node_with_test_quarantine_key_wrapper(cfg);
        let snapshot = restored
            .export_moderation_evidence_viewer_snapshot()
            .expect("export restored evidence viewer snapshot");
        assert_eq!(snapshot.sessions, vec![session]);
        assert_eq!(snapshot.access_events, vec![access]);
    }

    #[test]
    fn moderation_evidence_viewer_session_rejects_missing_object_and_payload_material() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let payload = b"evidence viewer missing object fixture".to_vec();
        let mut screening = moderation_screening_input_fixture(
            "cid:bafy-evidence-viewer-missing",
            ModerationScreeningVerdict::Quarantine,
        );
        screening.subject_digest = *blake3::hash(&payload).as_bytes();
        let handle = node_with_test_quarantine_key_wrapper(cfg);
        let outcome = handle
            .record_moderation_screening_result(screening)
            .expect("record quarantine result");
        let quarantine_id = outcome.quarantine.expect("quarantine record").quarantine_id;

        let err = handle
            .create_moderation_evidence_viewer_session(moderation_evidence_viewer_session_input(
                quarantine_id,
                1_800_000_000_000,
                1_800_000_300_000,
            ))
            .expect_err("missing object rejected");
        assert!(matches!(
            err,
            ModerationEvidenceViewerError::MissingObject { .. }
        ));

        handle
            .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                quarantine_id,
                payload,
                captured_at_unix: 1_800_000_091,
                content_type: None,
                notes: None,
            })
            .expect("store quarantine object");
        let mut unsafe_input = moderation_evidence_viewer_session_input(
            quarantine_id,
            1_800_000_000_000,
            1_800_000_300_000,
        );
        unsafe_input.raw_evidence_included = true;
        let err = handle
            .create_moderation_evidence_viewer_session(unsafe_input)
            .expect_err("raw evidence marker rejected");
        assert!(matches!(
            err,
            ModerationEvidenceViewerError::PayloadSafetyViolation { .. }
        ));
    }

    #[test]
    fn moderation_evidence_viewer_access_rejects_expiry_and_tampered_snapshot() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let payload = b"evidence viewer expired access fixture".to_vec();
        let mut screening = moderation_screening_input_fixture(
            "cid:bafy-evidence-viewer-expired",
            ModerationScreeningVerdict::Quarantine,
        );
        screening.subject_digest = *blake3::hash(&payload).as_bytes();
        let handle = node_with_test_quarantine_key_wrapper(cfg);
        let outcome = handle
            .record_moderation_screening_result(screening)
            .expect("record quarantine result");
        let quarantine_id = outcome.quarantine.expect("quarantine record").quarantine_id;
        handle
            .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                quarantine_id,
                payload,
                captured_at_unix: 1_800_000_092,
                content_type: None,
                notes: None,
            })
            .expect("store quarantine object");
        let session = handle
            .create_moderation_evidence_viewer_session(moderation_evidence_viewer_session_input(
                quarantine_id,
                1_800_000_000_000,
                1_800_000_300_000,
            ))
            .expect("create viewer session");

        let err = handle
            .record_moderation_evidence_viewer_access(moderation_evidence_viewer_access_input(
                session.session_id,
                ModerationEvidenceViewerAccessKind::Viewed,
                1_800_000_300_000,
            ))
            .expect_err("expired normal access rejected");
        assert!(matches!(
            err,
            ModerationEvidenceViewerError::ExpiredSession { .. }
        ));

        let expiry_event = handle
            .record_moderation_evidence_viewer_access(moderation_evidence_viewer_access_input(
                session.session_id,
                ModerationEvidenceViewerAccessKind::SessionExpired,
                1_800_000_300_000,
            ))
            .expect("record session expiry anomaly");
        assert_eq!(
            expiry_event.kind,
            ModerationEvidenceViewerAccessKind::SessionExpired
        );

        let mut tampered = handle.moderation_evidence_viewer_snapshot();
        tampered.sessions[0].evidence_digest = [0x44; 32];
        let err = handle
            .restore_moderation_evidence_viewer_snapshot(tampered)
            .expect_err("tampered evidence digest rejected");
        assert!(matches!(
            err,
            ModerationEvidenceViewerError::InvalidSnapshot { .. }
        ));
    }

    #[test]
    fn moderation_evidence_viewer_audit_report_records_transparency_source_entry() {
        use iroha_data_model::sorafs::transparency::ModerationLedgerEntryKindV1;

        let (cfg, _dir) = storage_config_with_temp_dir();
        let payload = b"evidence viewer audit report fixture".to_vec();
        let mut screening = moderation_screening_input_fixture(
            "cid:bafy-evidence-viewer-report",
            ModerationScreeningVerdict::Quarantine,
        );
        screening.subject_digest = *blake3::hash(&payload).as_bytes();
        let handle = node_with_test_quarantine_key_wrapper(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        let outcome = handle
            .record_moderation_screening_result(screening)
            .expect("record quarantine result");
        let quarantine_id = outcome.quarantine.expect("quarantine record").quarantine_id;
        handle
            .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                quarantine_id,
                payload,
                captured_at_unix: 1_800_000_093,
                content_type: None,
                notes: None,
            })
            .expect("store quarantine object");
        let session = handle
            .create_moderation_evidence_viewer_session(moderation_evidence_viewer_session_input(
                quarantine_id,
                1_800_000_000_000,
                1_800_000_300_000,
            ))
            .expect("create viewer session");
        let mut seeked = moderation_evidence_viewer_access_input(
            session.session_id,
            ModerationEvidenceViewerAccessKind::Seeked,
            1_800_000_020_000,
        );
        seeked.request_digest = [0xD8; 32];
        handle
            .record_moderation_evidence_viewer_access(moderation_evidence_viewer_access_input(
                session.session_id,
                ModerationEvidenceViewerAccessKind::Viewed,
                1_800_000_010_000,
            ))
            .expect("record viewer access");
        handle
            .record_moderation_evidence_viewer_access(seeked)
            .expect("record seek access");

        let result = handle
            .record_moderation_evidence_viewer_audit_report(
                moderation_evidence_viewer_audit_report_input(
                    1_800_000_000,
                    1_800_086_400,
                    1_800_086_401,
                ),
            )
            .expect("record audit report");
        assert_eq!(result.report.session_count, 1);
        assert_eq!(result.report.logged_session_count, 1);
        assert_eq!(result.report.access_event_count, 2);
        assert_eq!(
            result
                .report
                .access_kind_counts
                .iter()
                .map(|count| (count.kind.as_str(), count.count))
                .collect::<Vec<_>>(),
            vec![("seeked", 1), ("viewed", 1)]
        );
        assert_eq!(
            result.source_entry.kind,
            ModerationLedgerEntryKindV1::EvidenceAccess
        );
        assert_eq!(result.source_entry.occurred_at_unix, 1_800_086_399);
        assert_eq!(handle.transparency_ledger_source_entry_count(), 1);
        assert!(
            result
                .source_entry
                .metadata
                .iter()
                .any(|item| item.key == "viewer_accounts_included" && item.value == "false")
        );
        assert!(
            result
                .source_entry
                .metadata
                .iter()
                .all(|item| !item.value.contains("juror-1@moderation"))
        );

        handle
            .record_moderation_evidence_viewer_audit_report(
                moderation_evidence_viewer_audit_report_input(
                    1_800_000_000,
                    1_800_086_400,
                    1_800_086_401,
                ),
            )
            .expect("duplicate report export is idempotent");
        assert_eq!(handle.transparency_ledger_source_entry_count(), 1);

        let publication = handle
            .publish_transparency_ledger_cycle_from_source_entries(
                *b"cycle-evrpt00001",
                1_800_000_000,
                1_800_086_402,
                1_800_086_403,
                None,
            )
            .expect("publish evidence viewer report source cycle");
        assert_eq!(publication.block.entry_count, 1);
        assert_eq!(
            publication.proofs[0].entry.kind,
            ModerationLedgerEntryKindV1::EvidenceAccess
        );
        let published = publisher.take();
        assert_eq!(published.len(), 1);
    }

    #[test]
    fn moderation_evidence_viewer_audit_report_publish_due_publishes_and_is_idempotent() {
        use iroha_data_model::sorafs::transparency::ModerationLedgerEntryKindV1;

        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        seed_moderation_evidence_viewer_activity(
            &handle,
            "cid:bafy-evidence-viewer-due-report",
            b"evidence viewer due report fixture",
            1_800_000_000_000,
            1_800_000_060_000,
            &[
                (
                    ModerationEvidenceViewerAccessKind::Viewed,
                    1_800_000_010_000,
                ),
                (
                    ModerationEvidenceViewerAccessKind::DownloadAttempted,
                    1_800_000_020_000,
                ),
            ],
        );

        let schedule = PrivacyAggregateScheduleConfig {
            first_cycle_start_unix: 1_800_000_000,
            cycle_seconds: 100,
            publish_delay_seconds: 10,
        };
        let outcome = handle
            .publish_due_moderation_evidence_viewer_audit_report(
                1_800_000_110,
                schedule,
                "local-daily".to_string(),
                Some([0xF7; 32]),
                None,
            )
            .expect("publish due evidence-viewer report");
        let ModerationEvidenceViewerAuditScheduleOutcome::Published {
            window,
            report,
            source_entry,
            publication,
        } = outcome
        else {
            panic!("expected published evidence-viewer audit report");
        };
        assert_eq!(window.cycle_start_unix, 1_800_000_000);
        assert_eq!(window.cycle_end_unix, 1_800_000_100);
        assert_eq!(window.due_at_unix, 1_800_000_110);
        assert_eq!(report.session_count, 1);
        assert_eq!(report.logged_session_count, 1);
        assert_eq!(report.access_event_count, 2);
        assert_eq!(
            source_entry.kind,
            ModerationLedgerEntryKindV1::EvidenceAccess
        );
        assert_eq!(source_entry.occurred_at_unix, 1_800_000_099);
        assert_eq!(publication.block.entry_count, 1);
        assert_eq!(publication.proofs.len(), 1);
        assert_eq!(
            publication.proofs[0].entry.kind,
            ModerationLedgerEntryKindV1::EvidenceAccess
        );
        assert_eq!(handle.transparency_ledger_source_entry_count(), 0);
        assert_eq!(publisher.take().len(), 1);

        let repeat = handle
            .publish_due_moderation_evidence_viewer_audit_report(
                1_800_000_110,
                schedule,
                "local-daily".to_string(),
                Some([0xF7; 32]),
                None,
            )
            .expect("repeat due evidence-viewer report");
        assert!(matches!(
            repeat,
            ModerationEvidenceViewerAuditScheduleOutcome::AlreadyPublished { .. }
        ));
        assert_eq!(handle.transparency_ledger_source_entry_count(), 0);
        assert_eq!(publisher.take().len(), 0);
    }

    #[test]
    fn moderation_evidence_viewer_audit_report_publish_due_configured_uses_storage_config() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let schedule = PrivacyAggregateScheduleConfig {
            first_cycle_start_unix: 1_800_000_000,
            cycle_seconds: 100,
            publish_delay_seconds: 10,
        };
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(root.join("storage"))
            .evidence_viewer_audit_schedule(Some(schedule))
            .build();
        let handle = NodeHandle::new(cfg);
        assert_eq!(
            handle.configured_evidence_viewer_audit_schedule(),
            Some(schedule)
        );
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        seed_moderation_evidence_viewer_activity(
            &handle,
            "cid:bafy-evidence-viewer-configured-due-report",
            b"configured evidence viewer due report fixture",
            1_800_000_000_000,
            1_800_000_060_000,
            &[(
                ModerationEvidenceViewerAccessKind::Viewed,
                1_800_000_010_000,
            )],
        );

        let outcome = handle
            .publish_due_configured_moderation_evidence_viewer_audit_report(
                1_800_000_110,
                "local-daily".to_string(),
                Some([0xF7; 32]),
                None,
            )
            .expect("publish configured due evidence-viewer report");
        let ModerationEvidenceViewerAuditScheduleOutcome::Published {
            window,
            report,
            publication,
            ..
        } = outcome
        else {
            panic!("expected configured published evidence-viewer audit report");
        };
        assert_eq!(window.cycle_start_unix, 1_800_000_000);
        assert_eq!(window.cycle_end_unix, 1_800_000_100);
        assert_eq!(report.session_count, 1);
        assert_eq!(report.access_event_count, 1);
        assert_eq!(publication.block.entry_count, 1);
        assert_eq!(publisher.take().len(), 1);
    }

    #[test]
    fn moderation_evidence_viewer_audit_report_publish_due_configured_skips_when_disabled() {
        let cfg = StorageConfig::builder()
            .enabled(false)
            .evidence_viewer_audit_schedule(None)
            .build();
        let handle = NodeHandle::new(cfg);
        assert_eq!(handle.configured_evidence_viewer_audit_schedule(), None);

        let outcome = handle
            .publish_due_configured_moderation_evidence_viewer_audit_report(
                1_800_000_110,
                "local-daily".to_string(),
                None,
                None,
            )
            .expect("disabled configured evidence-viewer report");
        assert_eq!(
            outcome,
            ModerationEvidenceViewerAuditScheduleOutcome::Disabled
        );
    }

    #[test]
    fn moderation_evidence_viewer_audit_report_publish_due_reports_empty_and_bad_schedules() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let empty = NodeHandle::new(cfg);
        let schedule = PrivacyAggregateScheduleConfig {
            first_cycle_start_unix: 1_800_000_000,
            cycle_seconds: 100,
            publish_delay_seconds: 10,
        };
        let outcome = empty
            .publish_due_moderation_evidence_viewer_audit_report(
                1_800_000_110,
                schedule,
                "local-daily".to_string(),
                None,
                None,
            )
            .expect("empty due check");
        assert!(matches!(
            outcome,
            ModerationEvidenceViewerAuditScheduleOutcome::NoSourceEvents { .. }
        ));

        let err = empty
            .publish_due_moderation_evidence_viewer_audit_report(
                1_800_000_110,
                PrivacyAggregateScheduleConfig {
                    first_cycle_start_unix: 1_800_000_000,
                    cycle_seconds: 0,
                    publish_delay_seconds: 10,
                },
                "local-daily".to_string(),
                None,
                None,
            )
            .expect_err("zero cycle rejected");
        assert!(err.to_string().contains("evidence viewer audit schedule"));

        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        seed_moderation_evidence_viewer_activity(
            &handle,
            "cid:bafy-evidence-viewer-oversized-due-report",
            b"evidence viewer oversized due report fixture",
            1_800_000_000_000,
            1_800_000_060_000,
            &[(
                ModerationEvidenceViewerAccessKind::Viewed,
                1_800_000_010_000,
            )],
        );
        let oversized = PrivacyAggregateScheduleConfig {
            first_cycle_start_unix: 1_799_992_033,
            cycle_seconds: 86_401,
            publish_delay_seconds: 1,
        };
        let due_at_unix = oversized
            .event_window(1_800_000_010)
            .expect("oversized event window")
            .due_at_unix;
        let err = handle
            .publish_due_moderation_evidence_viewer_audit_report(
                due_at_unix,
                oversized,
                "local-daily".to_string(),
                None,
                None,
            )
            .expect_err("oversized report window rejected");
        assert!(
            err.to_string()
                .contains("record evidence viewer audit report")
        );
    }

    #[test]
    fn moderation_evidence_viewer_audit_report_rejects_unsafe_and_tampered_inputs() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let mut unsafe_input = moderation_evidence_viewer_audit_report_input(
            1_800_000_000,
            1_800_086_400,
            1_800_086_401,
        );
        unsafe_input.viewer_accounts_included = true;
        let err = handle
            .build_moderation_evidence_viewer_audit_report(unsafe_input)
            .expect_err("viewer account export rejected");
        assert!(matches!(
            err,
            ModerationEvidenceViewerError::PayloadSafetyViolation { .. }
        ));

        let err = handle
            .build_moderation_evidence_viewer_audit_report(
                moderation_evidence_viewer_audit_report_input(
                    1_800_000_000,
                    1_800_172_801,
                    1_800_172_802,
                ),
            )
            .expect_err("oversized report window rejected");
        assert!(matches!(
            err,
            ModerationEvidenceViewerError::InvalidInput { .. }
        ));

        let mut report = handle
            .build_moderation_evidence_viewer_audit_report(
                moderation_evidence_viewer_audit_report_input(
                    1_800_000_000,
                    1_800_086_400,
                    1_800_086_401,
                ),
            )
            .expect("empty report is valid");
        report.access_event_count = 1;
        assert!(
            moderation_evidence_viewer_audit_report_source_entry(&report)
                .expect_err("tampered report rejected")
                .to_string()
                .contains("access-kind counts do not sum")
        );
    }

    #[test]
    fn moderation_quarantine_release_requires_review() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let outcome = handle
            .record_moderation_screening_result(moderation_screening_input_fixture(
                "cid:bafy-release-before-review",
                ModerationScreeningVerdict::Quarantine,
            ))
            .expect("record quarantine result");
        let quarantine_id = outcome.quarantine.expect("quarantine record").quarantine_id;

        let err = handle
            .release_moderation_quarantine_record(moderation_quarantine_release_input(
                quarantine_id,
            ))
            .expect_err("release before review rejected");
        assert!(matches!(
            err,
            ModerationScreeningError::InvalidTransition { .. }
        ));
        assert_eq!(
            handle
                .moderation_screening_snapshot()
                .quarantine_records
                .first()
                .map(|record| record.state),
            Some(ModerationQuarantineState::PendingReview)
        );
    }

    #[test]
    fn moderation_screening_restore_rejects_tampered_digest() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let outcome = handle
            .record_moderation_screening_result(moderation_screening_input_fixture(
                "cid:bafy-tamper",
                ModerationScreeningVerdict::Quarantine,
            ))
            .expect("record screening result");
        let mut tampered = outcome.record;
        tampered.record_digest[0] ^= 0xFF;
        let err = handle
            .restore_moderation_screening_snapshot(ModerationScreeningSnapshot {
                screening_records: vec![tampered],
                quarantine_records: Vec::new(),
                authenticated_admissions: Vec::new(),
            })
            .expect_err("tampered digest rejected");
        assert!(matches!(
            err,
            ModerationScreeningError::InvalidSnapshot { .. }
        ));
    }

    #[test]
    fn manifest_metadata_resolves_by_digest() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let payload = b"digest-lookup-fixture";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0xAA; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = payload.as_slice();
        let manifest_id = handle
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest manifest");
        let manifest_digest: [u8; 32] = manifest.digest().expect("manifest digest").into();

        let by_id = handle
            .manifest_metadata(&manifest_id)
            .expect("lookup by id");
        let by_digest = handle
            .manifest_metadata_by_digest(&manifest_digest)
            .expect("lookup by digest");

        assert_eq!(by_digest.manifest_id(), manifest_id);
        assert_eq!(by_digest.manifest_digest(), &manifest_digest);
        assert_eq!(by_id.manifest_digest(), by_digest.manifest_digest());
    }

    #[test]
    fn moderation_state_limit_allows_boundary_replays_and_existing_updates() {
        let cfg = StorageConfig::builder()
            .enabled(false)
            .runtime_retention(RuntimeRetentionPolicy::new(1, 1, 1024 * 1024))
            .build();
        let handle = NodeHandle::new(cfg);

        let repro = moderation_repro_manifest_fixture(0x11, 0x31);
        let admitted = handle
            .admit_moderation_repro_manifest(repro.clone())
            .expect("admit repro at boundary");
        assert_eq!(
            handle
                .admit_moderation_repro_manifest(repro)
                .expect("replay repro at capacity"),
            admitted
        );
        assert!(matches!(
            handle
                .admit_moderation_repro_manifest(moderation_repro_manifest_fixture(0x12, 0x32,))
                .expect_err("new repro above capacity must fail"),
            ModerationModelRegistryError::ResourceExhausted {
                resource: "reproducibility_manifests",
                limit: 1
            }
        ));

        let corpus = adversarial_corpus_manifest_fixture();
        let admitted = handle
            .admit_moderation_corpus_manifest(corpus.clone())
            .expect("admit corpus at boundary");
        assert_eq!(
            handle
                .admit_moderation_corpus_manifest(corpus.clone())
                .expect("replay corpus at capacity"),
            admitted
        );
        let mut second_corpus = corpus;
        second_corpus.issued_at_unix += 1;
        assert!(matches!(
            handle
                .admit_moderation_corpus_manifest(second_corpus)
                .expect_err("new corpus above capacity must fail"),
            ModerationModelRegistryError::ResourceExhausted {
                resource: "adversarial_corpora",
                limit: 1
            }
        ));

        let screening = moderation_screening_input_fixture(
            "limit-subject",
            ModerationScreeningVerdict::Quarantine,
        );
        let first = handle
            .record_moderation_screening_result(screening.clone())
            .expect("record screening at boundary");
        assert_eq!(
            handle
                .record_moderation_screening_result(screening)
                .expect("replay screening at capacity")
                .record,
            first.record
        );
        let quarantine_id = first
            .quarantine
            .expect("quarantine at boundary")
            .quarantine_id;
        handle
            .review_moderation_quarantine_record(moderation_quarantine_review_input(quarantine_id))
            .expect("review existing quarantine at capacity");
        handle
            .release_moderation_quarantine_record(moderation_quarantine_release_input(
                quarantine_id,
            ))
            .expect("release existing quarantine at capacity");
        assert!(matches!(
            handle
                .record_moderation_screening_result(moderation_screening_input_fixture(
                    "second-subject",
                    ModerationScreeningVerdict::Pass,
                ))
                .expect_err("new screening above capacity must fail"),
            ModerationScreeningError::ResourceExhausted {
                resource: "screening_records",
                limit: 1
            }
        ));
    }

    #[test]
    fn moderation_object_viewer_limits_and_checkpoints_survive_restart() {
        let (base, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base.data_dir().clone())
            .runtime_retention(RuntimeRetentionPolicy::new(2, 2, 2 * 1024 * 1024))
            .build();
        let handle = node_with_test_quarantine_key_wrapper(cfg.clone());
        let mut sessions = Vec::new();
        let mut session_inputs = Vec::new();
        for index in 0_u8..2 {
            let payload = vec![index.saturating_add(1); 32];
            let mut screening = moderation_screening_input_fixture(
                &format!("restart-viewer-{index}"),
                ModerationScreeningVerdict::Quarantine,
            );
            screening.subject_digest = *blake3::hash(&payload).as_bytes();
            screening.evidence_digest = Some([0xE1_u8.saturating_add(index); 32]);
            let quarantine_id = handle
                .record_moderation_screening_result(screening)
                .expect("record screening at boundary")
                .quarantine
                .expect("quarantine record")
                .quarantine_id;
            handle
                .store_moderation_quarantine_object(ModerationQuarantineObjectInput {
                    quarantine_id,
                    payload,
                    captured_at_unix: 1_800_000_100 + u64::from(index),
                    content_type: None,
                    notes: None,
                })
                .expect("store object at boundary");
            let input = moderation_evidence_viewer_session_input(
                quarantine_id,
                1_800_000_100_000 + u64::from(index) * 1_000,
                1_800_000_200_000 + u64::from(index) * 1_000,
            );
            let session = handle
                .create_moderation_evidence_viewer_session(input.clone())
                .expect("create session at boundary");
            let mut access = moderation_evidence_viewer_access_input(
                session.session_id,
                ModerationEvidenceViewerAccessKind::Viewed,
                input.issued_at_unix_ms + 1,
            );
            access.request_digest = [0xD7_u8.saturating_add(index); 32];
            handle
                .record_moderation_evidence_viewer_access(access)
                .expect("record access at boundary");
            sessions.push(session);
            session_inputs.push(input);
        }

        assert_eq!(
            handle
                .create_moderation_evidence_viewer_session(session_inputs[0].clone())
                .expect("replay session at capacity"),
            sessions[0]
        );
        let mut third_session = session_inputs[0].clone();
        third_session.session_nonce_digest = [0xF1; 32];
        assert!(matches!(
            handle
                .create_moderation_evidence_viewer_session(third_session)
                .expect_err("new session above capacity must fail"),
            ModerationEvidenceViewerError::ResourceExhausted {
                resource: "evidence_viewer_sessions",
                limit: 2
            }
        ));
        assert!(matches!(
            handle
                .record_moderation_evidence_viewer_access(moderation_evidence_viewer_access_input(
                    sessions[0].session_id,
                    ModerationEvidenceViewerAccessKind::Seeked,
                    session_inputs[0].issued_at_unix_ms + 2,
                ),)
                .expect_err("new access event above capacity must fail"),
            ModerationEvidenceViewerError::ResourceExhausted {
                resource: "evidence_viewer_access_events",
                limit: 2
            }
        ));
        assert_eq!(
            handle.moderation_quarantine_object_snapshot().objects.len(),
            2
        );
        assert_eq!(
            handle.moderation_evidence_viewer_snapshot().sessions.len(),
            2
        );
        assert_eq!(
            handle
                .moderation_evidence_viewer_snapshot()
                .access_events
                .len(),
            2
        );
        drop(handle);

        let restored = node_with_test_quarantine_key_wrapper(cfg);
        assert_eq!(
            restored
                .moderation_screening_snapshot()
                .screening_records
                .len(),
            2
        );
        assert_eq!(
            restored
                .moderation_quarantine_object_snapshot()
                .objects
                .len(),
            2
        );
        let viewer = restored.moderation_evidence_viewer_snapshot();
        assert_eq!(viewer.sessions.len(), 2);
        assert_eq!(viewer.access_events.len(), 2);
        assert_eq!(
            restored
                .create_moderation_evidence_viewer_session(session_inputs[0].clone())
                .expect("replay restored session at capacity"),
            sessions[0]
        );
    }

    #[test]
    fn node_handle_registers_and_settles_deal() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let provider_id = ProviderId::new([0xDD; 32]);
        let client_id = ClientId::new([0xCC; 32]);

        handle
            .deposit_provider_bond(provider_id, xor("3"))
            .expect("deposit provider bond");
        handle
            .deposit_client_credit(client_id, xor("1"))
            .expect("deposit client credit");

        let terms = DealTerms {
            storage_price_per_gib_month: quantity("0.2"),
            egress_price_per_gib: quantity("0.05"),
            settlement_window_epochs: 7,
            micropayment_probability_bps: 10_000,
            micropayment_payout: quantity("0.05"),
        };

        let activation_epoch = 1_700_000_000;
        let proposal = DealProposal {
            provider_id,
            client_id,
            storage_class: StorageClass::Hot,
            capacity_gib: 4,
            start_epoch: activation_epoch,
            end_epoch: activation_epoch + 14,
            terms,
            metadata: Metadata::default(),
        };

        let record = handle
            .open_deal(proposal, activation_epoch)
            .expect("open deal");

        let mut tickets = (1_u64..=5)
            .map(|storage_gib_hours| MicropaymentTicket {
                ticket_id: derive_micropayment_ticket_id(
                    record.deal_id,
                    activation_epoch + 1,
                    storage_gib_hours,
                    0,
                ),
                issued_epoch: activation_epoch + 1,
                storage_gib_hours,
                egress_bytes: 0,
            })
            .collect::<Vec<_>>();
        tickets.sort_unstable_by_key(|ticket| ticket.ticket_id);
        let usage = DealUsageReport {
            deal_id: record.deal_id,
            epoch: activation_epoch + 1,
            storage_gib_hours: (4u128 * GIB_HOURS_PER_MONTH) as u64,
            egress_bytes: BYTES_PER_GIB as u64,
            tickets,
        };

        let usage_outcome = handle.record_deal_usage(usage).expect("record usage");
        assert_eq!(usage_outcome.tickets_processed, 5);

        let outcome = handle
            .settle_deal(record.deal_id, activation_epoch + 7)
            .expect("settle deal");
        let settlement = &outcome.record;
        assert_eq!(settlement.provider_id, provider_id);
        assert_eq!(settlement.client_id, client_id);
        assert_eq!(settlement.deal_id, record.deal_id);
        assert_eq!(settlement.settlement_index, 1);
        assert_eq!(settlement.expected_charge, quantity("0.85"));
        assert_eq!(settlement.micropayment_credit, quantity("0.25"));
        assert_eq!(settlement.client_credit_debit, quantity("0.6"));
        assert_eq!(settlement.bond_slash, Quantity::zero());
        assert_eq!(settlement.outstanding, Quantity::zero());

        let governance = &outcome.governance;
        assert_eq!(governance.deal_id, *record.deal_id.as_bytes());
        assert_eq!(governance.status, DealSettlementStatusV1::WindowSettled);
        assert_eq!(governance.settled_at, activation_epoch + 7);
        let ledger = &governance.ledger;
        assert_eq!(ledger.deal_id, *record.deal_id.as_bytes());
        assert_eq!(ledger.provider_id, *provider_id.as_bytes());
        assert_eq!(ledger.client_id, *client_id.as_bytes());
        assert_eq!(ledger.provider_accrual, xor("0.85"));
        assert_eq!(ledger.client_liability, xor("0.85"));
        assert_eq!(ledger.bond_locked, xor("2.4"));
        assert_eq!(ledger.bond_slashed, XorQuantity::zero());
        assert_eq!(ledger.captured_at, activation_epoch + 7);

        let snapshot = handle.deal_snapshot(record.deal_id).expect("snapshot");
        assert!(matches!(snapshot.status, DealStatus::Active(_)));

        let provider_snapshot = handle
            .deal_engine()
            .provider_snapshot(provider_id)
            .expect("provider snapshot");
        assert!(provider_snapshot.bond_locked >= xor("2.4"));
    }

    #[test]
    fn authenticated_deal_funding_and_cancellation_survive_restart() {
        let (base, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base.data_dir().clone())
            .build();
        let provider_id = ProviderId::new([0xD7; 32]);
        let client_id = ClientId::new([0xC7; 32]);
        let activation_epoch = 1_720_000_000;
        let source = NodeHandle::new(cfg.clone());

        source
            .fund_provider_bond_sequenced(provider_id, xor("0.000001"), 1)
            .expect("persist first provider funding sequence");
        source
            .fund_client_credit_sequenced(client_id, xor("0.0000005"), 1)
            .expect("persist first client funding sequence");
        assert!(matches!(
            source.fund_provider_bond_sequenced(provider_id, xor("0.00000001"), 1),
            Err(DealEngineError::FundingSequenceMismatch {
                expected: 2,
                found: 1,
                ..
            })
        ));
        assert!(matches!(
            source.fund_client_credit_sequenced(client_id, xor("0.00000001"), 3),
            Err(DealEngineError::FundingSequenceMismatch {
                expected: 2,
                found: 3,
                ..
            })
        ));

        let record = source
            .open_deal(
                DealProposal {
                    provider_id,
                    client_id,
                    storage_class: StorageClass::Hot,
                    capacity_gib: 1,
                    start_epoch: activation_epoch,
                    end_epoch: activation_epoch + 10,
                    terms: DealTerms {
                        storage_price_per_gib_month: quantity("0.0000001"),
                        egress_price_per_gib: quantity("0.00000001"),
                        settlement_window_epochs: 5,
                        micropayment_probability_bps: 1,
                        micropayment_payout: quantity("0.000000001"),
                    },
                    metadata: Metadata::default(),
                },
                activation_epoch,
            )
            .expect("persist active deal");
        let cancellation = source
            .cancel_deal(
                record.deal_id,
                activation_epoch + 5,
                "operator-approved client termination".to_owned(),
            )
            .expect("persist canonical cancellation");
        assert_eq!(
            cancellation.governance.status,
            DealSettlementStatusV1::Cancelled
        );
        let expected_governance = norito::to_bytes(&cancellation.governance)
            .expect("encode expected cancellation governance");
        assert_eq!(source.pending_governance_publication_count(), 1);
        drop(source);

        let restored = NodeHandle::new(cfg);
        assert_eq!(
            restored.pending_governance_publication_count(),
            1,
            "cancellation governance outbox survives restart"
        );
        let deal = restored
            .deal_snapshot(record.deal_id)
            .expect("cancelled deal restored");
        assert!(
            matches!(deal.status, DealStatus::Cancelled(epoch) if epoch == activation_epoch + 5)
        );
        assert_eq!(deal.locked_bond, XorQuantity::zero());
        let provider = restored
            .deal_engine()
            .provider_snapshot(provider_id)
            .expect("provider restored");
        assert_eq!(provider.funding_sequence, 1);
        assert_eq!(provider.bond_available, xor("0.000001"));
        let client = restored
            .deal_engine()
            .client_snapshot(client_id)
            .expect("client restored");
        assert_eq!(client.funding_sequence, 1);
        assert_eq!(client.credit_balance, xor("0.0000005"));

        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        restored
            .try_set_governance_publisher(trait_publisher)
            .expect("publisher registration replays restored cancellation outbox");
        assert_eq!(publisher.take(), vec![expected_governance]);
        assert_eq!(restored.pending_governance_publication_count(), 0);

        restored
            .fund_provider_bond_sequenced(provider_id, xor("0.00000001"), 2)
            .expect("next sequence accepted after restart");
        assert!(matches!(
            restored.cancel_deal(
                record.deal_id,
                activation_epoch + 10,
                "replay cancellation".to_owned(),
            ),
            Err(DealEngineError::DealInactive(deal_id)) if deal_id == record.deal_id
        ));
    }

    #[test]
    fn deal_runtime_balances_and_ticket_replay_survive_restart() {
        let (base, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base.data_dir().clone())
            .runtime_retention(RuntimeRetentionPolicy::new(4, 8, 2 * 1024 * 1024))
            .build();
        let provider_id = ProviderId::new([0xD1; 32]);
        let client_id = ClientId::new([0xC1; 32]);
        let activation_epoch = 1_700_000_000;
        let source = NodeHandle::new(cfg.clone());
        source
            .deposit_provider_bond(provider_id, xor("3"))
            .expect("persist provider bond");
        source
            .deposit_client_credit(client_id, xor("1"))
            .expect("persist client credit");
        let record = source
            .open_deal(
                DealProposal {
                    provider_id,
                    client_id,
                    storage_class: StorageClass::Hot,
                    capacity_gib: 1,
                    start_epoch: activation_epoch,
                    end_epoch: activation_epoch + 14,
                    terms: DealTerms {
                        storage_price_per_gib_month: quantity("0.2"),
                        egress_price_per_gib: quantity("0.05"),
                        settlement_window_epochs: 7,
                        micropayment_probability_bps: 10_000,
                        micropayment_payout: quantity("0.05"),
                    },
                    metadata: Metadata::default(),
                },
                activation_epoch,
            )
            .expect("persist deal");
        let replay_ticket = MicropaymentTicket {
            ticket_id: derive_micropayment_ticket_id(record.deal_id, activation_epoch + 1, 1, 0),
            issued_epoch: activation_epoch + 1,
            storage_gib_hours: 1,
            egress_bytes: 0,
        };
        source
            .record_deal_usage(DealUsageReport {
                deal_id: record.deal_id,
                epoch: activation_epoch + 1,
                storage_gib_hours: GIB_HOURS_PER_MONTH as u64,
                egress_bytes: 0,
                tickets: vec![replay_ticket.clone()],
            })
            .expect("persist usage");
        drop(source);

        let restored = NodeHandle::new(cfg);
        assert!(restored.deal_snapshot(record.deal_id).is_some());
        assert_eq!(
            restored
                .deal_engine()
                .provider_snapshot(provider_id)
                .expect("restored provider")
                .bond_locked,
            xor("0.6")
        );
        assert_eq!(
            restored
                .deal_engine()
                .client_snapshot(client_id)
                .expect("restored client")
                .credit_balance,
            xor("1")
        );
        let replay = restored
            .record_deal_usage(DealUsageReport {
                deal_id: record.deal_id,
                epoch: activation_epoch + 1,
                storage_gib_hours: GIB_HOURS_PER_MONTH as u64,
                egress_bytes: 0,
                tickets: vec![replay_ticket],
            })
            .expect_err("replayed usage epoch rejected");
        assert!(matches!(
            replay,
            DealEngineError::UsageEpochNotMonotonic { .. }
        ));
    }

    #[test]
    fn settle_deal_publishes_governance_artifact() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);

        let provider_id = ProviderId::new([0xAB; 32]);
        let client_id = ClientId::new([0xBC; 32]);

        handle
            .deposit_provider_bond(provider_id, xor("3"))
            .expect("deposit provider bond");
        handle
            .deposit_client_credit(client_id, xor("1"))
            .expect("deposit client credit");

        let terms = DealTerms {
            storage_price_per_gib_month: quantity("0.2"),
            egress_price_per_gib: quantity("0.05"),
            settlement_window_epochs: 7,
            micropayment_probability_bps: 10_000,
            micropayment_payout: quantity("0.05"),
        };

        let activation_epoch = 1_650_000_000;
        let proposal = DealProposal {
            provider_id,
            client_id,
            storage_class: StorageClass::Hot,
            capacity_gib: 2,
            start_epoch: activation_epoch,
            end_epoch: activation_epoch + 14,
            terms,
            metadata: Metadata::default(),
        };

        let record = handle
            .open_deal(proposal, activation_epoch)
            .expect("open deal");

        let usage = DealUsageReport {
            deal_id: record.deal_id,
            epoch: activation_epoch + 1,
            storage_gib_hours: GIB_HOURS_PER_MONTH as u64,
            egress_bytes: BYTES_PER_GIB as u64,
            tickets: vec![],
        };

        handle
            .record_deal_usage(usage)
            .expect("record usage should succeed");

        let outcome = handle
            .settle_deal(record.deal_id, activation_epoch + 7)
            .expect("settlement succeeds");

        let published = publisher.take();
        assert_eq!(published.len(), 1, "expected one governance publish");
        let decoded: DealSettlementV1 =
            norito::decode_from_bytes(&published[0]).expect("governance payload decodes");
        assert_eq!(decoded.deal_id, *record.deal_id.as_bytes());
        assert_eq!(decoded.ledger.provider_id, *provider_id.as_bytes());
        assert_eq!(decoded.ledger.client_id, *client_id.as_bytes());
        assert_eq!(decoded.status, outcome.governance.status);
        assert_eq!(decoded.settled_at, outcome.governance.settled_at);
    }

    #[test]
    fn publish_reputation_snapshot_updates_cache_and_governance_publisher() {
        let (cfg, _dir) = storage_config_with_reputation_policy();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        let envelope = signed_reputation_snapshot_fixture();
        let snapshot = envelope.snapshot.clone();
        let expected = envelope
            .canonical_bytes()
            .expect("encode signed reputation snapshot");
        let mut event_receiver = handle.subscribe_reputation_events();

        handle
            .publish_signed_reputation_snapshot(envelope.clone())
            .expect("publish signed reputation snapshot");

        let published = publisher.take();
        assert_eq!(published, vec![expected]);
        let cached = handle
            .latest_reputation_snapshot()
            .expect("latest reputation snapshot");
        assert_eq!(cached.snapshot_id, snapshot.snapshot_id);
        assert_eq!(cached.merkle_root, snapshot.merkle_root);
        let historical = handle
            .reputation_snapshot(snapshot.snapshot_id)
            .expect("historical reputation snapshot");
        assert_eq!(historical.snapshot_id, snapshot.snapshot_id);
        assert_eq!(historical.merkle_root, snapshot.merkle_root);
        assert_eq!(
            handle.latest_signed_reputation_snapshot(),
            Some(envelope.clone())
        );
        assert_eq!(
            handle.signed_reputation_snapshot(snapshot.snapshot_id),
            Some(envelope)
        );
        let events = handle.reputation_events_since(None, 10);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].sequence, 1);
        assert_eq!(events[0].snapshot_id, snapshot.snapshot_id);
        assert_eq!(events[0].merkle_root, snapshot.merkle_root);
        assert_eq!(handle.latest_reputation_event_sequence(), Some(1));
        let live_event = event_receiver
            .try_recv()
            .expect("live reputation event broadcast");
        assert_eq!(live_event.sequence, 1);
        assert_eq!(live_event.snapshot_id, snapshot.snapshot_id);
        assert!(handle.reputation_events_since(Some(1), 10).is_empty());
    }

    #[test]
    fn signed_reputation_admission_fails_closed_without_policy_and_on_adversarial_envelopes() {
        let (unconfigured, _dir) = storage_config_with_temp_dir();
        let unconfigured_handle = NodeHandle::new(unconfigured);
        let error = unconfigured_handle
            .publish_signed_reputation_snapshot(signed_reputation_snapshot_fixture())
            .expect_err("missing external policy must fail closed");
        assert!(error.to_string().contains("no external trust policy"));
        assert!(
            unconfigured_handle
                .latest_signed_reputation_snapshot()
                .is_none()
        );

        let (configured, _dir) = storage_config_with_reputation_policy();
        let handle = NodeHandle::new(configured);
        let now = unix_now_secs();
        let mut adversarial = Vec::new();

        let mut bad_signature = signed_reputation_snapshot_fixture_with([0x51; 16], now, None);
        bad_signature.signatures[0].signature[0] ^= 0x80;
        adversarial.push(bad_signature);

        let mut wrong_policy = signed_reputation_snapshot_fixture_with([0x52; 16], now, None);
        wrong_policy.policy_digest[0] ^= 0x40;
        adversarial.push(wrong_policy);

        adversarial.push(signed_reputation_snapshot_fixture_with(
            [0x53; 16],
            now.saturating_sub(601),
            None,
        ));
        adversarial.push(signed_reputation_snapshot_fixture_with(
            [0x54; 16],
            now.saturating_add(60),
            None,
        ));

        let mut tampered_evidence = signed_reputation_snapshot_fixture_with([0x55; 16], now, None);
        tampered_evidence.scoring_evidence.provider_inputs[0]
            .metrics
            .por_success_bps -= 1;
        adversarial.push(tampered_evidence);

        let mut untrusted_signer = signed_reputation_snapshot_fixture_with([0x56; 16], now, None);
        untrusted_signer.signatures[0].signer_id = "attacker".to_owned();
        adversarial.push(untrusted_signer);

        let mut no_quorum = signed_reputation_snapshot_fixture_with([0x57; 16], now, None);
        no_quorum.signatures.clear();
        adversarial.push(no_quorum);

        for envelope in adversarial {
            handle
                .publish_signed_reputation_snapshot(envelope)
                .expect_err("adversarial signed envelope must be rejected");
            assert!(handle.latest_reputation_snapshot().is_none());
            assert_eq!(handle.pending_governance_publication_count(), 0);
        }
    }

    #[test]
    fn reputation_trust_policy_loading_rejects_missing_noncanonical_and_unsafe_files() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let missing = root.join("missing-policy.to");
        let missing_config = StorageConfig::builder()
            .enabled(false)
            .reputation_trust_policy_path(Some(missing))
            .build();
        assert!(matches!(
            NodeHandle::try_new(missing_config),
            Err(NodeInitError::ReputationTrustPolicy { .. })
        ));

        let malformed = root.join("malformed-policy.to");
        write_local_checkpoint_atomic(&malformed, b"not canonical Norito")
            .expect("write malformed policy");
        let malformed_config = StorageConfig::builder()
            .enabled(false)
            .reputation_trust_policy_path(Some(malformed))
            .build();
        assert!(matches!(
            NodeHandle::try_new(malformed_config),
            Err(NodeInitError::ReputationTrustPolicy { .. })
        ));

        let oversized = root.join("oversized-policy.to");
        let oversized_bytes = vec![0_u8; MAX_REPUTATION_TRUST_POLICY_ENCODED_BYTES + 1];
        write_local_checkpoint_atomic(&oversized, &oversized_bytes)
            .expect("write oversized policy");
        let oversized_config = StorageConfig::builder()
            .enabled(false)
            .reputation_trust_policy_path(Some(oversized))
            .build();
        assert!(matches!(
            NodeHandle::try_new(oversized_config),
            Err(NodeInitError::ReputationTrustPolicy { .. })
        ));

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;

            let target = root.join("valid-policy.to");
            write_local_checkpoint_atomic(
                &target,
                &reputation_trust_policy_fixture()
                    .canonical_bytes()
                    .expect("encode valid policy"),
            )
            .expect("write valid policy");

            let symlink_path = root.join("policy-symlink.to");
            symlink(&target, &symlink_path).expect("create policy symlink");
            let symlink_config = StorageConfig::builder()
                .enabled(false)
                .reputation_trust_policy_path(Some(symlink_path))
                .build();
            assert!(matches!(
                NodeHandle::try_new(symlink_config),
                Err(NodeInitError::ReputationTrustPolicy { .. })
            ));

            let writable_path = root.join("writable-policy.to");
            write_local_checkpoint_atomic(
                &writable_path,
                &reputation_trust_policy_fixture()
                    .canonical_bytes()
                    .expect("encode writable policy"),
            )
            .expect("write policy before permission tamper");
            fs::set_permissions(&writable_path, fs::Permissions::from_mode(0o666))
                .expect("make policy writable by other users");
            let writable_config = StorageConfig::builder()
                .enabled(false)
                .reputation_trust_policy_path(Some(writable_path))
                .build();
            assert!(matches!(
                NodeHandle::try_new(writable_config),
                Err(NodeInitError::ReputationTrustPolicy { .. })
            ));

            let hardlink_path = root.join("policy-hardlink.to");
            fs::hard_link(&target, &hardlink_path).expect("create policy hard link");
            let hardlink_config = StorageConfig::builder()
                .enabled(false)
                .reputation_trust_policy_path(Some(hardlink_path))
                .build();
            assert!(matches!(
                NodeHandle::try_new(hardlink_config),
                Err(NodeInitError::ReputationTrustPolicy { .. })
            ));
        }
    }

    #[test]
    fn reputation_snapshot_rejects_conflicting_ids_and_evicts_only_unreferenced_history() {
        let (base, _dir) = storage_config_with_reputation_policy();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base.data_dir().clone())
            .reputation_trust_policy_path(base.reputation_trust_policy_path().cloned())
            .runtime_retention(RuntimeRetentionPolicy::new(1, 1, 1024 * 1024))
            .build();
        let handle = NodeHandle::new(cfg);
        handle.set_governance_publisher(Arc::new(RecordingPublisher::default()));
        let first = signed_reputation_snapshot_fixture();
        handle
            .publish_signed_reputation_snapshot(first.clone())
            .expect("publish first snapshot");

        let conflicting = signed_reputation_snapshot_fixture_with(
            first.snapshot.snapshot_id,
            first.snapshot.generated_at_unix + 1,
            None,
        );
        let conflict_error = handle
            .publish_signed_reputation_snapshot(conflicting)
            .expect_err("conflicting canonical bytes under one id must fail");
        assert!(conflict_error.to_string().contains("conflicts"));

        let broken_head = signed_reputation_snapshot_fixture_with(
            [0x43; 16],
            first.snapshot.generated_at_unix + 1,
            None,
        );
        let head_error = handle
            .publish_signed_reputation_snapshot(broken_head)
            .expect_err("snapshot must extend current head");
        assert!(head_error.to_string().contains("exact retained head"));

        let next = signed_reputation_snapshot_fixture_with(
            [0x44; 16],
            first.snapshot.generated_at_unix + 1,
            Some(first.snapshot.snapshot_id),
        );
        handle
            .publish_signed_reputation_snapshot(next)
            .expect("unreferenced predecessor can be safely evicted");
        assert_eq!(
            handle
                .latest_reputation_snapshot()
                .map(|snapshot| snapshot.snapshot_id),
            Some([0x44; 16])
        );
        assert!(
            handle
                .reputation_snapshot(first.snapshot.snapshot_id)
                .is_none()
        );
        assert_eq!(handle.reputation_events_since(None, 10).len(), 1);
        assert_eq!(
            handle.reputation_events_since(None, 10)[0].snapshot_id,
            [0x44; 16]
        );
    }

    #[test]
    fn reputation_snapshot_publish_failure_keeps_durable_state_for_exact_retry() {
        let (cfg, _dir) = storage_config_with_reputation_policy();
        let handle = NodeHandle::new(cfg.clone());
        let failing = Arc::new(FailingPublisher::default());
        handle.set_governance_publisher(failing.clone());
        let envelope = signed_reputation_snapshot_fixture();
        let snapshot = envelope.snapshot.clone();

        handle
            .publish_signed_reputation_snapshot(envelope.clone())
            .expect_err("external publisher failure is surfaced");
        assert_eq!(failing.attempts(), 1);
        assert_eq!(handle.pending_governance_publication_count(), 1);
        assert_eq!(
            handle.latest_reputation_snapshot(),
            Some(snapshot.clone()),
            "local commit must survive publication failure"
        );
        assert_eq!(handle.reputation_events_since(None, 10).len(), 1);
        drop(handle);

        let restored = NodeHandle::new(cfg);
        assert_eq!(
            restored.latest_reputation_snapshot(),
            Some(snapshot.clone())
        );
        assert_eq!(restored.reputation_events_since(None, 10).len(), 1);
        assert_eq!(
            restored.latest_signed_reputation_snapshot(),
            Some(envelope.clone())
        );
        assert_eq!(restored.pending_governance_publication_count(), 1);
        let recording = Arc::new(RecordingPublisher::default());
        restored
            .try_set_governance_publisher(recording.clone())
            .expect("publisher registration replays durable pending snapshot");
        assert_eq!(
            recording.take(),
            vec![envelope.canonical_bytes().expect("encode signed snapshot")]
        );
        assert_eq!(restored.pending_governance_publication_count(), 0);
        assert_eq!(restored.reputation_events_since(None, 10).len(), 1);
    }

    #[test]
    fn reputation_restart_rejects_missing_or_changed_external_policy() {
        let (cfg, _dir) = storage_config_with_reputation_policy();
        let envelope = signed_reputation_snapshot_fixture();
        let handle = NodeHandle::new(cfg.clone());
        handle
            .publish_signed_reputation_snapshot(envelope)
            .expect("persist signed reputation envelope");
        drop(handle);

        let no_policy_config = StorageConfig::builder()
            .enabled(true)
            .data_dir(cfg.data_dir().clone())
            .build();
        assert!(matches!(
            NodeHandle::try_new(no_policy_config),
            Err(NodeInitError::Checkpoint {
                component: "auxiliary runtime",
                ..
            })
        ));

        let policy_path = cfg
            .reputation_trust_policy_path()
            .expect("configured reputation policy");
        let mut changed_policy = reputation_trust_policy_fixture();
        changed_policy.policy_id[0] ^= 0x01;
        write_local_checkpoint_atomic(
            policy_path,
            &changed_policy
                .canonical_bytes()
                .expect("encode changed policy"),
        )
        .expect("replace reputation policy");
        assert!(matches!(
            NodeHandle::try_new(cfg),
            Err(NodeInitError::Checkpoint {
                component: "auxiliary runtime",
                ..
            })
        ));
    }

    #[test]
    fn reputation_restart_reuses_original_admission_time_for_freshness() {
        let mut short_policy = reputation_trust_policy_fixture();
        short_policy.max_snapshot_age_secs = 1;
        let (cfg, _dir) = storage_config_with_reputation_policy_fixture(&short_policy);
        let handle = NodeHandle::new(cfg.clone());
        let envelope = signed_reputation_snapshot_fixture_for_policy(
            &short_policy,
            [0x61; 16],
            unix_now_secs(),
            None,
        );
        handle
            .publish_signed_reputation_snapshot(envelope.clone())
            .expect("admit fresh signed envelope");
        drop(handle);

        std::thread::sleep(Duration::from_secs(2));
        let restored = NodeHandle::try_new(cfg)
            .expect("restart replays the persisted original admission time");
        assert_eq!(restored.latest_signed_reputation_snapshot(), Some(envelope));
    }

    #[test]
    fn governance_outbox_survives_restart_without_a_publisher_and_replays_on_registration() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let issuance = proof_token_issuance_fixture();
        let expected = to_bytes(&issuance).expect("encode proof-token issuance");
        let handle = NodeHandle::new(cfg.clone());

        handle
            .publish_proof_token_issuance(issuance)
            .expect("durably queue without a publisher");
        assert_eq!(handle.pending_governance_publication_count(), 1);
        drop(handle);

        let restored = NodeHandle::new(cfg.clone());
        assert_eq!(restored.pending_governance_publication_count(), 1);
        let recording = Arc::new(RecordingPublisher::default());
        restored
            .try_set_governance_publisher(recording.clone())
            .expect("replay pending issuance");
        assert_eq!(recording.take(), vec![expected]);
        assert_eq!(restored.pending_governance_publication_count(), 0);
        drop(restored);

        let acknowledged = NodeHandle::new(cfg);
        assert_eq!(acknowledged.pending_governance_publication_count(), 0);
    }

    #[test]
    fn governance_outbox_deduplicates_pending_payloads_and_fails_closed_at_retention_limit() {
        let (base, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base.data_dir().clone())
            .runtime_retention(RuntimeRetentionPolicy::new(1, 1, 1024 * 1024))
            .build();
        let handle = NodeHandle::new(cfg);
        let first = proof_token_issuance_fixture();

        handle
            .publish_proof_token_issuance(first.clone())
            .expect("queue first issuance");
        handle
            .publish_proof_token_issuance(first)
            .expect("exact pending retry is idempotent");
        assert_eq!(handle.pending_governance_publication_count(), 1);

        let mut second = proof_token_issuance_fixture();
        second.token_id = [0x71; 16];
        second.token_blake3 = [0x72; 32];
        let err = handle
            .publish_proof_token_issuance(second)
            .expect_err("outbox retention exhaustion must fail closed");
        assert!(err.to_string().contains("retention exhausted"));
        assert_eq!(handle.pending_governance_publication_count(), 1);
    }

    #[test]
    fn governance_outbox_replays_at_least_once_after_publish_before_ack_crash() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let issuance = proof_token_issuance_fixture();
        let expected = to_bytes(&issuance).expect("encode proof-token issuance");
        let handle = NodeHandle::new(cfg.clone());
        handle
            .publish_proof_token_issuance(issuance)
            .expect("queue issuance");
        let checkpoint_path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        let before_ack = fs::read(&checkpoint_path).expect("read pending checkpoint");

        let recording = Arc::new(RecordingPublisher::default());
        handle
            .try_set_governance_publisher(recording.clone())
            .expect("publish and acknowledge issuance");
        assert_eq!(handle.pending_governance_publication_count(), 0);

        write_local_checkpoint_atomic(&checkpoint_path, &before_ack)
            .expect("simulate crash before acknowledgement became durable");
        drop(handle);
        let restored = NodeHandle::new(cfg);
        restored
            .try_set_governance_publisher(recording.clone())
            .expect("at-least-once replay succeeds");
        assert_eq!(recording.take(), vec![expected.clone(), expected]);
        assert_eq!(restored.pending_governance_publication_count(), 0);
    }

    #[test]
    fn governance_outbox_checkpoint_rejects_digest_kind_and_sequence_tampering() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg.clone());
        handle
            .publish_proof_token_issuance(proof_token_issuance_fixture())
            .expect("queue issuance");
        drop(handle);

        let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        let original = fs::read(&path).expect("read auxiliary checkpoint");
        for tamper in 0..3 {
            let mut checkpoint: AuxiliaryRuntimeCheckpointV2 =
                norito::decode_from_bytes(&original).expect("decode auxiliary checkpoint");
            let entry = checkpoint
                .governance_outbox_entries
                .first_mut()
                .expect("pending outbox entry");
            match tamper {
                0 => entry.payload_digest[0] ^= 0x80,
                1 => entry.kind = GovernanceOutboxKindV1::DealSettlement,
                2 => entry.sequence = checkpoint.governance_outbox_next_sequence,
                _ => unreachable!(),
            }
            write_local_checkpoint_atomic(
                &path,
                &norito::to_bytes(&checkpoint).expect("encode tampered checkpoint"),
            )
            .expect("write tampered checkpoint");
            assert!(matches!(
                NodeHandle::try_new(cfg.clone()),
                Err(NodeInitError::Checkpoint {
                    component: "auxiliary runtime",
                    ..
                })
            ));
        }
        write_local_checkpoint_atomic(&path, &original).expect("restore original checkpoint");
        assert!(NodeHandle::try_new(cfg).is_ok());
    }

    #[test]
    fn governance_outbox_checkpoint_rejects_semantically_tampered_audit_header() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg.clone());
        handle
            .publish_gc_audit_event(GcAuditPayloadV1 {
                version: GC_AUDIT_PAYLOAD_VERSION_V1,
                manifest_digest: [0x81; 32],
                provider_id: [0; 32],
                evicted_at_unix: 1_800_000_000,
                freed_bytes: 0,
                reason: GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1.to_owned(),
                blocked_reason: Some(GC_AUDIT_BLOCKED_REPAIR_ACTIVE_V1.to_owned()),
            })
            .expect("queue blocked GC audit");
        assert_eq!(handle.pending_governance_publication_count(), 1);
        drop(handle);

        let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        let bytes = fs::read(&path).expect("read auxiliary checkpoint");
        let mut checkpoint: AuxiliaryRuntimeCheckpointV2 =
            norito::decode_from_bytes(&bytes).expect("decode auxiliary checkpoint");
        let entry = checkpoint
            .governance_outbox_entries
            .first_mut()
            .expect("pending audit entry");
        let mut audit: GcAuditEventV1 =
            norito::decode_from_bytes(&entry.payload_bytes).expect("decode GC audit event");
        audit.header.signer = "attacker".to_owned();
        entry.payload_bytes = norito::to_bytes(&audit).expect("encode tampered audit event");
        entry.payload_digest = *blake3::hash(&entry.payload_bytes).as_bytes();
        entry.binding_digest = governance_outbox_binding_digest(
            entry.version,
            entry.sequence,
            entry.kind,
            entry.payload_digest,
        );
        write_local_checkpoint_atomic(
            &path,
            &norito::to_bytes(&checkpoint).expect("encode tampered checkpoint"),
        )
        .expect("write tampered checkpoint");

        assert!(matches!(
            NodeHandle::try_new(cfg),
            Err(NodeInitError::Checkpoint {
                component: "auxiliary runtime",
                ..
            })
        ));
    }

    #[test]
    fn reputation_checkpoint_rejects_event_snapshot_metadata_tampering() {
        let (cfg, _dir) = storage_config_with_reputation_policy();
        let handle = NodeHandle::new(cfg.clone());
        handle
            .publish_signed_reputation_snapshot(signed_reputation_snapshot_fixture())
            .expect("publish signed snapshot");
        drop(handle);

        let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        let bytes = fs::read(&path).expect("read auxiliary checkpoint");
        let mut checkpoint: AuxiliaryRuntimeCheckpointV2 =
            norito::decode_from_bytes(&bytes).expect("decode auxiliary checkpoint");
        checkpoint.reputation_events[0].snapshot_id = [0x99; 16];
        write_local_checkpoint_atomic(
            &path,
            &norito::to_bytes(&checkpoint).expect("encode tampered checkpoint"),
        )
        .expect("write tampered checkpoint");

        assert!(matches!(
            NodeHandle::try_new(cfg),
            Err(NodeInitError::Checkpoint {
                component: "auxiliary runtime",
                ..
            })
        ));
    }

    #[test]
    fn reputation_checkpoint_rejects_envelope_admission_and_outbox_tampering() {
        let (cfg, _dir) = storage_config_with_reputation_policy();
        let handle = NodeHandle::new(cfg.clone());
        handle
            .publish_signed_reputation_snapshot(signed_reputation_snapshot_fixture())
            .expect("persist signed reputation envelope");
        drop(handle);

        let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        let original = fs::read(&path).expect("read auxiliary checkpoint");
        for case in 0..7_u8 {
            let mut checkpoint: AuxiliaryRuntimeCheckpointV2 =
                norito::decode_from_bytes(&original).expect("decode auxiliary checkpoint");
            match case {
                0 => {
                    checkpoint.reputation_snapshots[0].version ^= 1;
                }
                1 => checkpoint.reputation_snapshots[0].admitted_at_unix = 0,
                2 => {
                    checkpoint.reputation_snapshots[0].envelope.signatures[0].signature[0] ^= 0x80;
                }
                3 => checkpoint.reputation_snapshots[0].envelope.policy_digest[0] ^= 0x40,
                4 => {
                    checkpoint.reputation_snapshots[0]
                        .envelope
                        .scoring_evidence_digest[0] ^= 0x20;
                }
                5 => checkpoint.reputation_snapshots[0].encoded_len ^= 1,
                6 => {
                    let replacement =
                        signed_reputation_snapshot_fixture_with([0x7A; 16], unix_now_secs(), None);
                    let entry = checkpoint
                        .governance_outbox_entries
                        .first_mut()
                        .expect("pending reputation outbox entry");
                    entry.payload_bytes = replacement
                        .canonical_bytes()
                        .expect("encode replacement signed envelope");
                    entry.payload_digest = *blake3::hash(&entry.payload_bytes).as_bytes();
                    entry.binding_digest = governance_outbox_binding_digest(
                        entry.version,
                        entry.sequence,
                        entry.kind,
                        entry.payload_digest,
                    );
                }
                _ => unreachable!("bounded checkpoint tamper case"),
            }
            write_local_checkpoint_atomic(
                &path,
                &norito::to_bytes(&checkpoint).expect("encode tampered checkpoint"),
            )
            .expect("write tampered checkpoint");
            assert!(matches!(
                NodeHandle::try_new(cfg.clone()),
                Err(NodeInitError::Checkpoint {
                    component: "auxiliary runtime",
                    ..
                })
            ));
        }
    }

    #[test]
    fn publish_appeal_finance_report_writes_governance_publisher() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        let report = appeal_finance_report_fixture();
        let expected = to_bytes(&report).expect("encode appeal finance report");

        handle
            .publish_appeal_finance_report(report.clone())
            .expect("publish appeal finance report");

        let published = publisher.take();
        assert_eq!(published, vec![expected]);
    }

    #[test]
    fn publish_transparency_ledger_publication_writes_governance_publisher() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        let publication = transparency_ledger_publication_fixture();
        let expected = to_bytes(&publication).expect("encode transparency ledger publication");

        handle
            .publish_transparency_ledger_publication(publication.clone())
            .expect("publish transparency ledger publication");

        let published = publisher.take();
        assert_eq!(published, vec![expected]);
        let decoded: ModerationLedgerCyclePublicationV1 = norito::decode_from_bytes(&published[0])
            .expect("decode transparency ledger publication");
        assert_eq!(decoded.block.entry_count, 2);
        decoded.validate().expect("publication validates");
    }

    #[test]
    fn publish_proof_token_issuance_writes_governance_publisher() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        let issuance = proof_token_issuance_fixture();
        let expected = to_bytes(&issuance).expect("encode proof-token issuance");

        handle
            .publish_proof_token_issuance(issuance.clone())
            .expect("publish proof-token issuance");

        let published = publisher.take();
        assert_eq!(published, vec![expected]);
        let decoded: ProofTokenIssuanceV1 =
            norito::decode_from_bytes(&published[0]).expect("decode proof-token issuance");
        assert_eq!(decoded, issuance);
        decoded.validate().expect("issuance validates");
    }

    #[test]
    fn publish_proof_token_base64_issuance_derives_and_writes_governance_publisher() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);

        let issuance = handle
            .publish_proof_token_base64_issuance(
                VALID_PROOF_TOKEN_B64,
                proof_token_signer_key_fixture(),
                Some([0x65; 32]),
                Some([0x66; 32]),
                vec![
                    iroha_data_model::sorafs::transparency::ModerationLedgerMetadataV1 {
                        key: "issuer".to_string(),
                        value: "gateway-a".to_string(),
                    },
                ],
            )
            .expect("publish proof-token issuance from base64");

        assert_eq!(issuance.token_id, [0x61; 16]);
        assert_eq!(issuance.issued_at_unix, 1_800_000_030);
        assert_eq!(issuance.expires_at_unix, Some(1_800_086_430));
        assert_eq!(issuance.moderation_action_code, 2);
        assert_eq!(issuance.signer_key, proof_token_signer_key_fixture());
        assert_eq!(issuance.blinded_digest, [0x64; 32]);
        assert_eq!(
            issuance.entry_ids,
            vec!["denylist/global".to_string(), "gar/policy/42".to_string()]
        );

        let published = publisher.take();
        assert_eq!(published.len(), 1);
        let decoded: ProofTokenIssuanceV1 =
            norito::decode_from_bytes(&published[0]).expect("decode proof-token issuance");
        assert_eq!(decoded, issuance);
    }

    #[test]
    fn record_transparency_ledger_source_entry_is_idempotent_and_rejects_conflicts() {
        use iroha_data_model::sorafs::transparency::ModerationLedgerEntryKindV1;

        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let entry = transparency_ledger_source_entry(
            "gar-1",
            1_800_000_010,
            ModerationLedgerEntryKindV1::GarEnforcementReceipt,
            "gar-receipt-1",
            0x50,
        );

        handle
            .record_transparency_ledger_source_entry(entry.clone())
            .expect("record source entry");
        handle
            .record_transparency_ledger_source_entry(entry.clone())
            .expect("exact duplicate is idempotent");

        let mut conflicting = entry;
        conflicting.payload_digest = [0xA5; 32];
        let err = handle
            .record_transparency_ledger_source_entry(conflicting)
            .expect_err("conflicting source entry rejected");

        assert!(
            err.to_string()
                .contains("conflicts with retained canonical data")
        );
        assert_eq!(handle.transparency_ledger_source_entry_count(), 1);
    }

    #[test]
    fn publish_transparency_ledger_source_entries_builds_and_publishes_publication() {
        use iroha_data_model::sorafs::transparency::ModerationLedgerEntryKindV1;

        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        for entry in [
            transparency_ledger_source_entry(
                "redaction-1",
                1_800_000_030,
                ModerationLedgerEntryKindV1::Redaction,
                "redaction-case-1",
                0x70,
            ),
            transparency_ledger_source_entry(
                "gar-1",
                1_800_000_010,
                ModerationLedgerEntryKindV1::GarEnforcementReceipt,
                "gar-receipt-1",
                0x50,
            ),
            transparency_ledger_source_entry(
                "hold-1",
                1_800_000_030,
                ModerationLedgerEntryKindV1::LegalHold,
                "hold-case-1",
                0x60,
            ),
            transparency_ledger_source_entry(
                "appeal-1",
                1_800_000_005,
                ModerationLedgerEntryKindV1::AppealOutcome,
                "appeal-case-1",
                0x40,
            ),
            transparency_ledger_source_entry(
                "future-1",
                1_800_604_900,
                ModerationLedgerEntryKindV1::EvidenceAccess,
                "evidence-view-1",
                0x80,
            ),
        ] {
            handle
                .record_transparency_ledger_source_entry(entry)
                .expect("record source entry");
        }

        let publication = handle
            .publish_transparency_ledger_cycle_from_source_entries(
                *b"cycle-src-pub001",
                1_800_000_000,
                1_800_604_800,
                1_800_604_801,
                Some([0x44; 32]),
            )
            .expect("publish transparency source cycle");

        publication.validate().expect("publication validates");
        assert_eq!(publication.block.entry_count, 4);
        assert_eq!(publication.block.previous_block_hash, Some([0x44; 32]));
        let subjects = publication
            .proofs
            .iter()
            .map(|proof| proof.entry.subject.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            subjects,
            vec![
                "appeal-case-1",
                "gar-receipt-1",
                "hold-case-1",
                "redaction-case-1"
            ]
        );
        for (index, proof) in publication.proofs.iter().enumerate() {
            assert_eq!(proof.entry.sequence, u64::try_from(index).unwrap() + 1);
            assert_eq!(proof.entry.cycle_id, publication.block.cycle_id);
            assert_ne!(proof.entry.entry_id, [0; 16]);
        }

        let published = publisher.take();
        assert_eq!(published.len(), 1);
        let decoded: ModerationLedgerCyclePublicationV1 = norito::decode_from_bytes(&published[0])
            .expect("decode transparency source publication");
        assert_eq!(decoded, publication);
    }

    #[test]
    fn publish_transparency_ledger_source_entries_rejects_empty_window() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);

        let err = handle
            .publish_transparency_ledger_cycle_from_source_entries(
                *b"cycle-src-pub001",
                1_800_000_000,
                1_800_604_800,
                1_800_604_801,
                None,
            )
            .expect_err("empty source window rejected");

        assert!(err.to_string().contains("no source entries"));
        assert!(publisher.take().is_empty());
    }

    #[test]
    fn record_concrete_transparency_source_entries_builds_publication() {
        use iroha_data_model::sorafs::{
            gar::GarEnforcementActionV1, transparency::ModerationLedgerEntryKindV1,
        };

        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        handle
            .record_gar_enforcement_receipt_transparency_entry(&gar_enforcement_receipt_fixture(
                GarEnforcementActionV1::LegalHold,
            ))
            .expect("record GAR receipt source entry");
        let moderation_event = SoraFsModerationBallotGovernanceEventV1 {
            version: SORAFS_MODERATION_BALLOT_GOVERNANCE_EVENT_VERSION_V1,
            sequence: 7,
            kind: SoraFsModerationBallotGovernanceEventKindV1::BallotTallied,
            generated_at_unix_ms: 1_800_000_020_000,
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
                tallied_at_unix_ms: 1_800_000_020_000,
            }),
            challenge: None,
        };
        handle
            .record_moderation_ballot_governance_transparency_entry(&moderation_event)
            .expect("record moderation governance source entry");
        let report = appeal_finance_report_fixture();
        handle
            .record_appeal_finance_report_transparency_entry(&report)
            .expect("record appeal report source entry");
        let receipt = appeal_finance_settlement_receipt_fixture();
        handle
            .record_appeal_finance_settlement_receipt_transparency_entry(&receipt)
            .expect("record appeal settlement source entry");

        assert_eq!(handle.transparency_ledger_source_entry_count(), 4);
        let publication = handle
            .publish_transparency_ledger_cycle_from_source_entries(
                *b"cycle-src-pub002",
                1_800_000_000,
                1_800_604_800,
                1_800_604_801,
                None,
            )
            .expect("publish concrete source cycle");

        publication.validate().expect("publication validates");
        assert_eq!(publication.block.entry_count, 4);
        let kinds = publication
            .proofs
            .iter()
            .map(|proof| proof.entry.kind.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            vec![
                ModerationLedgerEntryKindV1::LegalHold,
                ModerationLedgerEntryKindV1::ModerationAction,
                ModerationLedgerEntryKindV1::AppealOutcome,
                ModerationLedgerEntryKindV1::AppealOutcome,
            ]
        );
        assert!(
            publication
                .proofs
                .iter()
                .any(|proof| proof.entry.subject == "docs.sora@docs.gateway.sora.net")
        );
        assert!(
            publication
                .proofs
                .iter()
                .any(|proof| proof.entry.subject == "case-42:drawdown_non_refund")
        );
    }

    #[test]
    fn publish_privacy_aggregate_cycle_builds_and_publishes_publication() {
        use iroha_data_model::sorafs::transparency::ModerationLedgerEntryKindV1;

        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        let cycle_id = *b"cycle-2026-wk-03";
        let aggregate_b = privacy_aggregate_fixture("sfm4c-jurisdiction-b", 0xB0);
        let aggregate_a = privacy_aggregate_fixture("sfm4c-jurisdiction-a", 0xA0);

        let publication = handle
            .publish_privacy_aggregate_cycle(
                cycle_id,
                1_800_000_000,
                1_800_604_800,
                1_800_604_801,
                None,
                vec![aggregate_b, aggregate_a],
            )
            .expect("publish privacy aggregate cycle");

        publication.validate().expect("publication validates");
        assert_eq!(publication.block.entry_count, 2);
        let subjects = publication
            .proofs
            .iter()
            .map(|proof| proof.entry.subject.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            subjects,
            vec!["sfm4c-jurisdiction-a", "sfm4c-jurisdiction-b"]
        );
        assert!(
            publication
                .proofs
                .iter()
                .all(|proof| proof.entry.kind == ModerationLedgerEntryKindV1::PrivacyAggregate)
        );

        let published = publisher.take();
        assert_eq!(published.len(), 1);
        let decoded: ModerationLedgerCyclePublicationV1 =
            norito::decode_from_bytes(&published[0]).expect("decode privacy aggregate publication");
        assert_eq!(decoded, publication);
    }

    #[test]
    fn privacy_aggregate_publication_rejects_mixed_cycle_policy_or_randomness() {
        use iroha_data_model::sorafs::transparency::{
            ModerationPrivacyNoiseSourceV1, ModerationPrivacyThresholdPrfCommitmentV1,
        };

        let aggregate_a = privacy_aggregate_fixture("sfm4c-jurisdiction-a", 0xA0);
        let mut aggregate_b = privacy_aggregate_fixture("sfm4c-jurisdiction-b", 0xB0);
        aggregate_b.policy_digest = [0xD0; 32];
        let err = NodeHandle::build_privacy_aggregate_publication(
            *b"cycle-2026-wk-03",
            1_800_000_000,
            1_800_604_800,
            1_800_604_800,
            None,
            vec![aggregate_a.clone(), aggregate_b.clone()],
        )
        .expect_err("mixed policy digests are rejected");
        assert!(err.to_string().contains("mixed policy digests"));

        aggregate_b.policy_digest = aggregate_a.policy_digest;
        aggregate_b.noise_source = ModerationPrivacyNoiseSourceV1::ThresholdPrf(
            ModerationPrivacyThresholdPrfCommitmentV1 {
                commitment: [0xDD; 32],
            },
        );
        let err = NodeHandle::build_privacy_aggregate_publication(
            *b"cycle-2026-wk-03",
            1_800_000_000,
            1_800_604_800,
            1_800_604_800,
            None,
            vec![aggregate_a, aggregate_b],
        )
        .expect_err("mixed privacy randomness commitments are rejected");
        assert!(err.to_string().contains("mixed privacy noise sources"));
    }

    #[test]
    fn publish_privacy_aggregate_cycle_rejects_out_of_window_without_publishing() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        let mut aggregate = privacy_aggregate_fixture("sfm4c-jurisdiction-a", 0xA0);
        aggregate.window_start_unix = 1_799_999_999;

        let err = handle
            .publish_privacy_aggregate_cycle(
                *b"cycle-2026-wk-03",
                1_800_000_000,
                1_800_604_800,
                1_800_604_801,
                None,
                vec![aggregate],
            )
            .expect_err("out-of-window aggregate is rejected");

        assert!(
            err.to_string()
                .contains("window must equal the publication cycle")
        );
        assert!(publisher.take().is_empty());
    }

    #[test]
    fn record_privacy_aggregate_source_event_is_idempotent_and_rejects_equivocation() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let event = privacy_source_event("event-a", "jurisdiction-a", 0xA0, 1_800_000_010);

        assert_eq!(
            handle
                .record_privacy_aggregate_source_event(event.clone())
                .expect("record source event"),
            PrivacySourceEventRecordOutcomeV1::Recorded
        );
        assert_eq!(
            handle
                .record_privacy_aggregate_source_event(event.clone())
                .expect("exact retry is idempotent"),
            PrivacySourceEventRecordOutcomeV1::AlreadyRecorded
        );
        let mut equivocation = event;
        equivocation.occurred_at_unix += 1;
        let err = handle
            .record_privacy_aggregate_source_event(equivocation)
            .expect_err("changed bytes under one event id are rejected");

        assert!(err.to_string().contains("idempotency key equivocation"));
        assert_eq!(handle.privacy_aggregate_source_event_count(), 1);
    }

    #[test]
    fn publish_privacy_aggregate_cycle_from_source_events_suppresses_and_publishes() {
        use iroha_data_model::sorafs::transparency::{
            MODERATION_PRIVACY_RANDOMNESS_COMMITMENT_METADATA_KEY_V1, ModerationLedgerEntryKindV1,
        };

        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        for event in [
            privacy_source_event("alpha-1", "jurisdiction-a", 0xA0, 1_800_000_010),
            privacy_source_event("alpha-2", "jurisdiction-a", 0xA0, 1_800_000_020),
            privacy_source_event("beta-1", "jurisdiction-b", 0xB0, 1_800_000_030),
            privacy_source_event("future-1", "jurisdiction-c", 0xC0, 1_800_604_900),
        ] {
            handle
                .record_privacy_aggregate_source_event(event)
                .expect("record source event");
        }

        let config = privacy_aggregate_cycle_config();
        let cycle_prf_input = privacy_cycle_prf_input(
            &config,
            1_800_000_000,
            1_800_604_800,
            1_800_604_801,
            [0x5A; 32],
        );
        let publication = handle
            .publish_privacy_aggregate_cycle_from_source_events(PrivacyAggregateSourceCycleInput {
                cycle_id: *b"cycle-2026-wk-04",
                cycle_start_unix: 1_800_000_000,
                cycle_end_unix: 1_800_604_800,
                previous_block_hash: None,
                config,
                cycle_prf_input: Some(cycle_prf_input),
            })
            .expect("publish aggregate cycle from source events");

        publication.validate().expect("publication validates");
        assert_eq!(publication.block.entry_count, 2);
        let entry = &publication.proofs[0].entry;
        assert_eq!(entry.kind, ModerationLedgerEntryKindV1::PrivacyAggregate);
        assert!(entry.subject.contains("jurisdiction-a"));
        assert_eq!(entry.evidence_uris.len(), 0);
        assert!(
            entry.metadata.iter().any(|item| {
                item.key == MODERATION_PRIVACY_RANDOMNESS_COMMITMENT_METADATA_KEY_V1
            })
        );
        assert!(
            entry.metadata.iter().all(|item| !matches!(
                item.key.as_str(),
                "source_event_count" | "source_subject_count" | "suppressed_count"
            )),
            "public ledger metadata must not disclose exact private counts"
        );

        let published = publisher.take();
        assert_eq!(published.len(), 1);
        let decoded: ModerationLedgerCyclePublicationV1 =
            norito::decode_from_bytes(&published[0]).expect("decode aggregate publication");
        assert_eq!(decoded, publication);
    }

    #[test]
    fn publish_privacy_aggregate_cycle_from_source_events_requires_cycle_prf_output() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        handle
            .record_privacy_aggregate_source_event(privacy_source_event(
                "alpha-1",
                "jurisdiction-a",
                0xA0,
                1_800_000_010,
            ))
            .expect("record source event");
        handle
            .record_privacy_aggregate_source_event(privacy_source_event(
                "alpha-2",
                "jurisdiction-a",
                0xA0,
                1_800_000_020,
            ))
            .expect("record source event");

        let err = handle
            .publish_privacy_aggregate_cycle_from_source_events(PrivacyAggregateSourceCycleInput {
                cycle_id: *b"cycle-2026-wk-04",
                cycle_start_unix: 1_800_000_000,
                cycle_end_unix: 1_800_604_800,
                previous_block_hash: None,
                config: privacy_aggregate_cycle_config(),
                cycle_prf_input: None,
            })
            .expect_err("missing cycle PRF output rejected");

        assert!(err.to_string().contains("hidden cycle PRF output"));
        assert!(publisher.take().is_empty());
    }

    #[test]
    fn publish_due_privacy_aggregate_cycle_from_source_events_publishes_once() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = node_with_test_privacy_cycle_prf_provider(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        for event in [
            privacy_source_event("alpha-1", "jurisdiction-a", 0xA0, 110),
            privacy_source_event("alpha-2", "jurisdiction-a", 0xA0, 120),
            privacy_source_event("future-1", "jurisdiction-a", 0xA0, 220),
        ] {
            handle
                .record_privacy_aggregate_source_event(event)
                .expect("record source event");
        }

        let outcome = handle
            .publish_due_privacy_aggregate_cycle_from_source_events(
                211,
                privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                "publish-once".to_string(),
                privacy_aggregate_schedule_config(),
                privacy_aggregate_cycle_config(),
                Some(privacy_composition_budget_policy()),
            )
            .expect("publish due aggregate cycle");
        let publication = match outcome {
            PrivacyAggregateScheduleOutcome::Published {
                window,
                publication,
            } => {
                assert_eq!(window.cycle_start_unix, 100);
                assert_eq!(window.cycle_end_unix, 200);
                publication
            }
            other => panic!("expected published outcome, got {other:?}"),
        };
        assert_eq!(publication.block.cycle_start_unix, 100);
        assert_eq!(publication.block.cycle_end_unix, 200);
        assert_eq!(publication.block.generated_at_unix, 200);
        assert_eq!(publication.block.entry_count, 2);

        let repeated = handle
            .publish_due_privacy_aggregate_cycle_from_source_events(
                211,
                privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                "publish-once".to_string(),
                privacy_aggregate_schedule_config(),
                privacy_aggregate_cycle_config(),
                Some(privacy_composition_budget_policy()),
            )
            .expect("repeat due aggregate cycle");
        assert!(matches!(
            repeated,
            PrivacyAggregateScheduleOutcome::Published { .. }
        ));
        assert_eq!(publisher.take().len(), 1);
    }

    #[test]
    fn publish_due_privacy_aggregate_cycle_from_source_events_catches_up_stale_windows() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = node_with_test_privacy_cycle_prf_provider(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        for event in [
            privacy_source_event("alpha-1", "jurisdiction-a", 0xA0, 110),
            privacy_source_event("alpha-2", "jurisdiction-a", 0xA0, 120),
            privacy_source_event("beta-1", "jurisdiction-b", 0xB0, 210),
            privacy_source_event("beta-2", "jurisdiction-b", 0xB0, 220),
        ] {
            handle
                .record_privacy_aggregate_source_event(event)
                .expect("record source event");
        }

        let first = handle
            .publish_due_privacy_aggregate_cycle_from_source_events(
                311,
                privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                "catchup-cycle-1".to_string(),
                privacy_aggregate_schedule_config(),
                privacy_aggregate_cycle_config(),
                Some(privacy_composition_budget_policy()),
            )
            .expect("publish first stale aggregate cycle");
        let first_publication = match first {
            PrivacyAggregateScheduleOutcome::Published {
                window,
                publication,
            } => {
                assert_eq!(window.cycle_start_unix, 100);
                assert_eq!(window.cycle_end_unix, 200);
                assert_eq!(publication.block.cycle_start_unix, 100);
                assert_eq!(publication.block.cycle_end_unix, 200);
                assert_eq!(publication.block.generated_at_unix, 200);
                publication
            }
            other => panic!("expected stale published outcome, got {other:?}"),
        };

        let second = handle
            .publish_due_privacy_aggregate_cycle_from_source_events(
                311,
                privacy_aggregate_cycle_id([0xB0; 32], 200, 300),
                "catchup-cycle-2".to_string(),
                privacy_aggregate_schedule_config(),
                privacy_aggregate_cycle_config(),
                Some(privacy_composition_budget_policy()),
            )
            .expect("publish latest aggregate cycle after catch-up");
        match second {
            PrivacyAggregateScheduleOutcome::Published {
                window,
                publication,
            } => {
                assert_eq!(window.cycle_start_unix, 200);
                assert_eq!(window.cycle_end_unix, 300);
                assert_eq!(publication.block.cycle_start_unix, 200);
                assert_eq!(publication.block.cycle_end_unix, 300);
                assert_eq!(publication.block.generated_at_unix, 300);
            }
            other => panic!("expected latest published outcome, got {other:?}"),
        }

        let replayed = handle
            .publish_due_privacy_aggregate_cycle_from_source_events(
                311,
                privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                "catchup-cycle-1".to_string(),
                privacy_aggregate_schedule_config(),
                privacy_aggregate_cycle_config(),
                Some(privacy_composition_budget_policy()),
            )
            .expect("old exact request replays after the head advances");
        let replayed_publication = match replayed {
            PrivacyAggregateScheduleOutcome::Published { publication, .. } => publication,
            other => panic!("expected replayed publication, got {other:?}"),
        };
        assert_eq!(
            norito::to_bytes(&replayed_publication).expect("encode replayed publication"),
            norito::to_bytes(&first_publication).expect("encode original publication")
        );

        let mut rotated_delay = privacy_aggregate_schedule_config();
        rotated_delay.publish_delay_seconds = rotated_delay.publish_delay_seconds.saturating_add(1);
        let rotated_replay = handle
            .publish_due_privacy_aggregate_cycle_from_source_events(
                311,
                privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                "catchup-cycle-1".to_string(),
                rotated_delay,
                privacy_aggregate_cycle_config(),
                Some(privacy_composition_budget_policy()),
            )
            .expect_err("old exact request cannot replay under a rotated release cadence");
        assert!(
            rotated_replay
                .to_string()
                .contains("cadence does not match the configured query lineage")
        );

        let stale_fresh = handle
            .publish_due_privacy_aggregate_cycle_from_source_events(
                411,
                privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                "catchup-stale-fresh".to_string(),
                privacy_aggregate_schedule_config(),
                privacy_aggregate_cycle_config(),
                Some(privacy_composition_budget_policy()),
            )
            .expect_err("a fresh key cannot target an old terminal release");
        assert!(
            stale_fresh
                .to_string()
                .contains("does not match the direct successor")
        );

        let mismatched_old_key = handle
            .publish_due_privacy_aggregate_cycle_from_source_events(
                411,
                privacy_aggregate_cycle_id([0xB0; 32], 200, 300),
                "catchup-cycle-1".to_string(),
                privacy_aggregate_schedule_config(),
                privacy_aggregate_cycle_config(),
                Some(privacy_composition_budget_policy()),
            )
            .expect_err("an old key cannot be rebound to another cycle");
        assert!(
            mismatched_old_key
                .to_string()
                .contains("idempotency key equivocation")
        );
        assert_eq!(publisher.take().len(), 2);
    }

    #[test]
    fn privacy_cycle_prf_derives_distinct_requests_for_catch_up_windows() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let provider = Arc::new(TestPrivacyCyclePrfProvider::bound());
        let trait_provider: Arc<dyn PrivacyCyclePrfProviderV1> = provider.clone();
        let handle = NodeHandle::try_new_with_runtime_deps(
            cfg,
            NodeRuntimeDeps::default()
                .with_privacy_cycle_prf_provider(trait_provider)
                .with_privacy_release_anchor(test_privacy_release_anchor()),
        )
        .expect("initialise node with recording threshold PRF provider");
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        for event in [
            privacy_source_event("alpha-1", "jurisdiction-a", 0xA0, 110),
            privacy_source_event("beta-1", "jurisdiction-b", 0xB0, 210),
            privacy_source_event("beta-2", "jurisdiction-b", 0xB0, 220),
        ] {
            handle
                .record_privacy_aggregate_source_event(event)
                .expect("record source event");
        }

        let first = handle
            .publish_due_privacy_aggregate_cycle_from_source_events(
                311,
                privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                "prf-cycle-1".to_string(),
                privacy_aggregate_schedule_config(),
                privacy_aggregate_cycle_config(),
                Some(privacy_composition_budget_policy()),
            )
            .expect("publish first due aggregate cycle");
        match first {
            PrivacyAggregateScheduleOutcome::Published {
                window,
                publication,
            } => {
                assert_eq!(window.cycle_start_unix, 100);
                assert_eq!(window.cycle_end_unix, 200);
                assert_eq!(publication.block.entry_count, 2);
            }
            other => panic!("expected first published outcome, got {other:?}"),
        }
        let second = handle
            .publish_due_privacy_aggregate_cycle_from_source_events(
                311,
                privacy_aggregate_cycle_id([0xB0; 32], 200, 300),
                "prf-cycle-2".to_string(),
                privacy_aggregate_schedule_config(),
                privacy_aggregate_cycle_config(),
                Some(privacy_composition_budget_policy()),
            )
            .expect("publish second due aggregate cycle");
        match second {
            PrivacyAggregateScheduleOutcome::Published {
                window,
                publication,
            } => {
                assert_eq!(window.cycle_start_unix, 200);
                assert_eq!(window.cycle_end_unix, 300);
                assert_eq!(publication.block.entry_count, 2);
            }
            other => panic!("expected second published outcome, got {other:?}"),
        }
        assert_eq!(publisher.take().len(), 2);
        let requests = provider.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].policy_digest(), [0xC0; 32]);
        assert_eq!(requests[1].policy_digest(), [0xC0; 32]);
        assert_eq!(
            (requests[0].cycle_start_unix(), requests[0].cycle_end_unix()),
            (100, 200)
        );
        assert_eq!(
            (requests[1].cycle_start_unix(), requests[1].cycle_end_unix()),
            (200, 300)
        );
        assert_ne!(requests[0].cycle_id(), requests[1].cycle_id());
        assert_ne!(requests[0].binding_digest(), requests[1].binding_digest());
    }

    #[test]
    fn privacy_cycle_prf_startup_requires_runtime_provider() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let cfg = privacy_aggregate_storage_config(&root);
        assert!(matches!(
            NodeHandle::try_new(cfg),
            Err(NodeInitError::PrivacyCyclePrfProviderUnavailable)
        ));
    }

    #[test]
    fn differential_privacy_startup_requires_finalized_release_anchor() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let cfg = privacy_aggregate_storage_config(&root);
        assert!(matches!(
            NodeHandle::try_new_with_runtime_deps(
                cfg,
                NodeRuntimeDeps::default()
                    .with_privacy_cycle_prf_provider(test_privacy_cycle_prf_provider()),
            ),
            Err(NodeInitError::PrivacyReleaseAnchorUnavailable)
        ));
    }

    #[test]
    fn privacy_cycle_prf_rejects_zero_output_before_provider_boundary() {
        assert_eq!(
            PrivacyCyclePrfOutputV1::new([0; 32]).expect_err("zero output must fail"),
            PrivacyCyclePrfInputErrorV1::ZeroOutput
        );
    }

    #[test]
    fn privacy_cycle_prf_redacts_failed_provider_diagnostics() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let cfg = privacy_aggregate_storage_config(&root);
        let provider: Arc<dyn PrivacyCyclePrfProviderV1> = Arc::new(
            TestPrivacyCyclePrfProvider::failing(PrivacyCyclePrfProviderErrorV1::Internal),
        );
        let handle = NodeHandle::try_new_with_runtime_deps(
            cfg,
            NodeRuntimeDeps::default()
                .with_privacy_cycle_prf_provider(provider)
                .with_privacy_release_anchor(test_privacy_release_anchor()),
        )
        .expect("initialise node with error-injecting threshold PRF provider");
        for event in [
            privacy_source_event("alpha-1", "jurisdiction-a", 0xA0, 110),
            privacy_source_event("alpha-2", "jurisdiction-a", 0xA0, 120),
        ] {
            handle
                .record_privacy_aggregate_source_event(event)
                .expect("record source event");
        }
        let error = handle
            .publish_due_configured_privacy_aggregate_cycle_from_source_events(
                privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                "failed-provider".to_string(),
            )
            .expect_err("failed provider output must fail closed");
        assert_eq!(
            error.to_string(),
            "runtime threshold PRF provider internal failure"
        );
        assert!(
            !error
                .to_string()
                .contains("TEST-PRF-VENDOR-DIAGNOSTIC-MUST-NOT-LEAK")
        );
    }

    #[test]
    fn privacy_cycle_prf_debug_redacts_runtime_crypto_provider_implementations() {
        let config = StorageConfig::builder().enabled(false).build();
        let privacy_provider: Arc<dyn PrivacyCyclePrfProviderV1> =
            Arc::new(TestPrivacyCyclePrfProvider::bound());
        let quarantine_wrapper = test_quarantine_key_wrapper();
        let node = NodeHandle::try_new_with_runtime_deps(
            config,
            NodeRuntimeDeps::default()
                .with_moderation_quarantine_key_wrapper(quarantine_wrapper)
                .with_privacy_cycle_prf_provider(privacy_provider)
                .with_privacy_release_anchor(test_privacy_release_anchor()),
        )
        .expect("initialise node with runtime crypto providers");
        let debug = format!("{node:?}");
        assert!(debug.contains("ModerationQuarantineKeyWrapper(<runtime-only>)"));
        assert!(debug.contains("PrivacyCyclePrfProviderV1(<runtime-only>)"));
        assert!(debug.contains("PrivacyReleaseAnchorV1(<runtime-only>)"));
        assert!(!debug.contains("kms:test/quarantine-v1"));
        assert!(!debug.contains("TEST-PRF-VENDOR-DIAGNOSTIC-MUST-NOT-LEAK"));
    }

    #[test]
    fn publish_due_configured_privacy_aggregate_cycle_uses_storage_config() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let schedule = privacy_aggregate_schedule_config();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(root.join("storage"))
            .privacy_aggregate_schedule(Some(schedule))
            .privacy_aggregate_policy(Some(privacy_aggregate_policy_config()))
            .build();
        let handle = node_with_test_privacy_cycle_prf_provider(cfg);
        assert_eq!(
            handle.configured_privacy_aggregate_schedule(),
            Some(schedule)
        );
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        for event in [
            privacy_source_event("alpha-1", "jurisdiction-a", 0xA0, 110),
            privacy_source_event("alpha-2", "jurisdiction-a", 0xA0, 120),
        ] {
            handle
                .record_privacy_aggregate_source_event(event)
                .expect("record source event");
        }

        let outcome = handle
            .publish_due_configured_privacy_aggregate_cycle_from_source_events(
                privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                "configured-cycle-1".to_string(),
            )
            .expect("publish configured aggregate cycle");
        let publication = match outcome {
            PrivacyAggregateScheduleOutcome::Published {
                window,
                publication,
            } => {
                assert_eq!(window.cycle_start_unix, 100);
                assert_eq!(window.cycle_end_unix, 200);
                publication
            }
            other => panic!("expected published outcome, got {other:?}"),
        };
        assert_eq!(publication.block.generated_at_unix, 200);
        assert_eq!(publisher.take().len(), 1);
        assert_eq!(handle.privacy_aggregate_source_event_count(), 0);
        let budget = handle
            .privacy_composition_budget_snapshot()
            .expect("privacy composition budget snapshot");
        assert_eq!(budget.chains.len(), 1);
        assert_eq!(budget.chains[0].charges.len(), 1);
        assert_eq!(
            budget.chains[0].charges[0].cycle_id,
            publication.block.cycle_id
        );
        assert_eq!(
            handle
                .record_privacy_aggregate_source_event(privacy_source_event(
                    "alpha-1",
                    "jurisdiction-a",
                    0xA0,
                    110,
                ))
                .expect("processed event retry remains idempotent"),
            PrivacySourceEventRecordOutcomeV1::AlreadyRecorded
        );
        let replay_error = handle
            .record_privacy_aggregate_source_event(privacy_source_event(
                "late-replay",
                "jurisdiction-a",
                0xA0,
                110,
            ))
            .expect_err("a finalized release window must reject later source events");
        assert!(
            replay_error
                .to_string()
                .contains("targets a finalized release window")
        );
        assert_eq!(handle.privacy_aggregate_source_event_count(), 0);
    }

    #[test]
    fn privacy_publication_budget_state_and_outbox_restore_atomically() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let schedule = privacy_aggregate_schedule_config();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(root.join("storage"))
            .privacy_aggregate_schedule(Some(schedule))
            .privacy_aggregate_policy(Some(privacy_aggregate_policy_config()))
            .build();
        let anchor = Arc::new(TestPrivacyReleaseAnchor::default());
        let source = NodeHandle::try_new_with_runtime_deps(
            cfg.clone(),
            NodeRuntimeDeps::default()
                .with_privacy_cycle_prf_provider(test_privacy_cycle_prf_provider())
                .with_privacy_release_anchor(anchor.clone()),
        )
        .expect("initialise source node with shared release anchor");
        for event in [
            privacy_source_event("alpha-1", "jurisdiction-a", 0xA0, 110),
            privacy_source_event("alpha-2", "jurisdiction-a", 0xA0, 120),
        ] {
            source
                .record_privacy_aggregate_source_event(event)
                .expect("record source event");
        }

        let published = source
            .publish_due_configured_privacy_aggregate_cycle_from_source_events(
                privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                "restore-cycle-1".to_string(),
            )
            .expect("commit configured privacy cycle without publisher");
        let cycle_id = match published {
            PrivacyAggregateScheduleOutcome::Published { publication, .. } => {
                publication.block.cycle_id
            }
            other => panic!("expected published outcome, got {other:?}"),
        };
        assert_eq!(source.pending_governance_publication_count(), 1);
        assert_eq!(source.privacy_aggregate_source_event_count(), 0);
        drop(source);

        let restored = NodeHandle::try_new_with_runtime_deps(
            cfg,
            NodeRuntimeDeps::default()
                .with_privacy_cycle_prf_provider(test_privacy_cycle_prf_provider())
                .with_privacy_release_anchor(anchor),
        )
        .expect("restore node with shared release anchor");
        assert_eq!(restored.pending_governance_publication_count(), 1);
        assert_eq!(restored.privacy_aggregate_source_event_count(), 0);
        let budget = restored
            .privacy_composition_budget_snapshot()
            .expect("restored privacy budget");
        assert_eq!(budget.chains.len(), 1);
        assert_eq!(budget.chains[0].charges.len(), 1);
        assert_eq!(budget.chains[0].charges[0].cycle_id, cycle_id);

        let repeated = restored
            .publish_due_configured_privacy_aggregate_cycle_from_source_events(
                privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                "restore-cycle-1".to_string(),
            )
            .expect("repeat restored privacy cycle");
        assert!(matches!(
            repeated,
            PrivacyAggregateScheduleOutcome::Published { .. }
        ));
        assert_eq!(
            restored
                .privacy_composition_budget_snapshot()
                .expect("budget after replay")
                .chains[0]
                .charges
                .len(),
            1
        );

        let publisher = Arc::new(RecordingPublisher::default());
        restored
            .try_set_governance_publisher(publisher.clone())
            .expect("replay durable privacy publication");
        assert_eq!(publisher.take().len(), 1);
        assert_eq!(restored.pending_governance_publication_count(), 0);
    }

    #[test]
    fn privacy_publish_receipt_checkpoint_rejects_pre_due_observation_and_delay_tampering() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let cfg = privacy_aggregate_storage_config(&root);
        let checkpoint_path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        let anchor = Arc::new(TestPrivacyReleaseAnchor::default());
        let source = NodeHandle::try_new_with_runtime_deps(
            cfg.clone(),
            NodeRuntimeDeps::default()
                .with_privacy_cycle_prf_provider(test_privacy_cycle_prf_provider())
                .with_privacy_release_anchor(anchor.clone()),
        )
        .expect("initialise privacy receipt source node");
        assert!(matches!(
            source
                .publish_due_configured_privacy_aggregate_cycle_from_source_events(
                    privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                    "receipt-tamper-cycle-1".to_string(),
                )
                .expect("commit privacy release"),
            PrivacyAggregateScheduleOutcome::Published { .. }
        ));
        drop(source);

        let original = fs::read(&checkpoint_path).expect("read privacy receipt checkpoint");
        for tamper in 0..2 {
            let mut checkpoint: AuxiliaryRuntimeCheckpointV2 =
                norito::decode_from_bytes(&original).expect("decode privacy receipt checkpoint");
            let receipt = checkpoint
                .privacy_publish_request_receipts
                .first_mut()
                .expect("privacy publish receipt");
            match tamper {
                0 => receipt.requested_now_unix = receipt.due_at_unix.saturating_sub(1),
                1 => {
                    receipt.publish_delay_seconds = receipt.publish_delay_seconds.saturating_add(1);
                }
                _ => unreachable!("bounded privacy receipt tamper case"),
            }
            write_local_checkpoint_atomic(
                &checkpoint_path,
                &norito::to_bytes(&checkpoint).expect("encode tampered privacy receipt checkpoint"),
            )
            .expect("write tampered privacy receipt checkpoint");
            let error = NodeHandle::try_new_with_runtime_deps(
                cfg.clone(),
                NodeRuntimeDeps::default()
                    .with_privacy_cycle_prf_provider(test_privacy_cycle_prf_provider())
                    .with_privacy_release_anchor(anchor.clone()),
            )
            .expect_err("tampered privacy publish receipt must fail restart");
            assert!(matches!(
                error,
                NodeInitError::Checkpoint {
                    component: "auxiliary runtime",
                    ..
                }
            ));
        }
        write_local_checkpoint_atomic(&checkpoint_path, &original)
            .expect("restore canonical privacy receipt checkpoint");
        NodeHandle::try_new_with_runtime_deps(
            cfg,
            NodeRuntimeDeps::default()
                .with_privacy_cycle_prf_provider(test_privacy_cycle_prf_provider())
                .with_privacy_release_anchor(anchor),
        )
        .expect("canonical privacy receipt checkpoint restores");
    }

    #[test]
    fn privacy_source_receipt_only_checkpoint_rejects_oversized_event_id() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let cfg = privacy_aggregate_storage_config(&root);
        let checkpoint_path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        let anchor = Arc::new(TestPrivacyReleaseAnchor::default());
        let source = NodeHandle::try_new_with_runtime_deps(
            cfg.clone(),
            NodeRuntimeDeps::default()
                .with_privacy_cycle_prf_provider(test_privacy_cycle_prf_provider())
                .with_privacy_release_anchor(anchor.clone()),
        )
        .expect("initialise privacy source-receipt node");
        source
            .record_privacy_aggregate_source_event(privacy_source_event(
                "receipt-only-event",
                "jurisdiction-a",
                0xA0,
                110,
            ))
            .expect("record privacy source event");
        assert!(matches!(
            source
                .publish_due_configured_privacy_aggregate_cycle_from_source_events(
                    privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                    "receipt-only-cycle-1".to_string(),
                )
                .expect("commit privacy release"),
            PrivacyAggregateScheduleOutcome::Published { .. }
        ));
        assert_eq!(source.privacy_aggregate_source_event_count(), 0);
        drop(source);

        let bytes = fs::read(&checkpoint_path).expect("read receipt-only checkpoint");
        let mut checkpoint: AuxiliaryRuntimeCheckpointV2 =
            norito::decode_from_bytes(&bytes).expect("decode receipt-only checkpoint");
        assert!(checkpoint.privacy_source_events.is_empty());
        checkpoint
            .privacy_source_event_receipts
            .first_mut()
            .expect("retained source-event receipt")
            .event_id = "x".repeat(MODERATION_LEDGER_MAX_PUBLIC_TEXT_BYTES_V1 + 1);
        write_local_checkpoint_atomic(
            &checkpoint_path,
            &norito::to_bytes(&checkpoint).expect("encode oversized receipt-only checkpoint"),
        )
        .expect("write oversized receipt-only checkpoint");

        let error = NodeHandle::try_new_with_runtime_deps(
            cfg,
            NodeRuntimeDeps::default()
                .with_privacy_cycle_prf_provider(test_privacy_cycle_prf_provider())
                .with_privacy_release_anchor(anchor),
        )
        .expect_err("oversized retained source-event receipt must fail restart");
        assert!(matches!(
            error,
            NodeInitError::Checkpoint {
                component: "auxiliary runtime",
                ..
            }
        ));
    }

    #[test]
    fn privacy_release_restart_rejects_cadence_and_delay_rotation() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let cfg = privacy_aggregate_storage_config(&root);
        let anchor = Arc::new(TestPrivacyReleaseAnchor::default());
        let source = NodeHandle::try_new_with_runtime_deps(
            cfg,
            NodeRuntimeDeps::default()
                .with_privacy_cycle_prf_provider(test_privacy_cycle_prf_provider())
                .with_privacy_release_anchor(anchor.clone()),
        )
        .expect("initialise privacy delay-lineage node");
        assert!(matches!(
            source
                .publish_due_configured_privacy_aggregate_cycle_from_source_events(
                    privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                    "delay-lineage-cycle-1".to_string(),
                )
                .expect("commit privacy release"),
            PrivacyAggregateScheduleOutcome::Published { .. }
        ));
        drop(source);

        let mut delay_schedule = privacy_aggregate_schedule_config();
        delay_schedule.publish_delay_seconds =
            delay_schedule.publish_delay_seconds.saturating_add(1);
        let mut first_schedule = privacy_aggregate_schedule_config();
        first_schedule.first_cycle_start_unix = first_schedule
            .first_cycle_start_unix
            .saturating_add(first_schedule.cycle_seconds);
        let mut first_cycle = privacy_aggregate_cycle_config();
        first_cycle.first_cycle_start_unix = first_schedule.first_cycle_start_unix;
        let mut width_schedule = privacy_aggregate_schedule_config();
        width_schedule.cycle_seconds /= 2;
        let mut width_cycle = privacy_aggregate_cycle_config();
        width_cycle.cycle_seconds = width_schedule.cycle_seconds;

        for (case, schedule, policy) in [
            (
                "publish delay",
                delay_schedule,
                privacy_aggregate_policy_config(),
            ),
            (
                "first-cycle activation",
                first_schedule,
                privacy_aggregate_policy_config_for_cycle(first_cycle),
            ),
            (
                "cycle width",
                width_schedule,
                privacy_aggregate_policy_config_for_cycle(width_cycle),
            ),
        ] {
            let rotated_cfg = StorageConfig::builder()
                .enabled(true)
                .data_dir(root.join("storage"))
                .privacy_aggregate_schedule(Some(schedule))
                .privacy_aggregate_policy(Some(policy))
                .build();
            let error = NodeHandle::try_new_with_runtime_deps(
                rotated_cfg,
                NodeRuntimeDeps::default()
                    .with_privacy_cycle_prf_provider(test_privacy_cycle_prf_provider())
                    .with_privacy_release_anchor(anchor.clone()),
            )
            .expect_err("cadence rotation must fail the durable query lineage");
            assert!(
                matches!(
                    &error,
                    NodeInitError::Checkpoint {
                        component: "auxiliary runtime",
                        ..
                    }
                ),
                "{case} rotation returned the wrong startup error: {error}"
            );
            assert!(
                error
                    .to_string()
                    .contains("cadence does not match the configured query lineage"),
                "{case} rotation was not rejected as a lineage conflict: {error}"
            );
        }
    }

    #[test]
    fn privacy_checkpoint_rollback_behind_finalized_release_anchor_fails_closed() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let cfg = privacy_aggregate_storage_config(&root);
        let checkpoint_path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        let anchor = Arc::new(TestPrivacyReleaseAnchor::default());
        let source = NodeHandle::try_new_with_runtime_deps(
            cfg.clone(),
            NodeRuntimeDeps::default()
                .with_privacy_cycle_prf_provider(test_privacy_cycle_prf_provider())
                .with_privacy_release_anchor(anchor.clone()),
        )
        .expect("initialise source node");
        for event in [
            privacy_source_event("alpha-1", "jurisdiction-a", 0xA0, 110),
            privacy_source_event("alpha-2", "jurisdiction-a", 0xA0, 120),
        ] {
            source
                .record_privacy_aggregate_source_event(event)
                .expect("record source event");
        }
        let rolled_back_checkpoint =
            fs::read(&checkpoint_path).expect("capture pre-release checkpoint");
        assert!(matches!(
            source
                .publish_due_configured_privacy_aggregate_cycle_from_source_events(
                    privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                    "rollback-cycle-1".to_string(),
                )
                .expect("commit privacy release"),
            PrivacyAggregateScheduleOutcome::Published { .. }
        ));
        assert_eq!(
            anchor
                .finalized_head([0xB0; 32])
                .expect("read finalized release head")
                .sequence(),
            1
        );
        drop(source);

        fs::write(&checkpoint_path, rolled_back_checkpoint)
            .expect("simulate checkpoint rollback behind finalized head");
        let error = NodeHandle::try_new_with_runtime_deps(
            cfg,
            NodeRuntimeDeps::default()
                .with_privacy_cycle_prf_provider(test_privacy_cycle_prf_provider())
                .with_privacy_release_anchor(anchor),
        )
        .expect_err("rollback behind the finalized release anchor must fail");
        assert!(
            error
                .to_string()
                .contains("behind or equivocates with the finalized anchor")
        );
    }

    #[test]
    fn publish_due_configured_privacy_aggregate_cycle_skips_when_disabled() {
        let cfg = StorageConfig::builder()
            .enabled(false)
            .privacy_aggregate_schedule(None)
            .build();
        let handle = NodeHandle::new(cfg);
        assert_eq!(handle.configured_privacy_aggregate_schedule(), None);

        let outcome = handle
            .publish_due_configured_privacy_aggregate_cycle_from_source_events(
                privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                "disabled-cycle".to_string(),
            )
            .expect("disabled configured aggregate cycle");
        assert_eq!(outcome, PrivacyAggregateScheduleOutcome::Disabled);
    }

    #[test]
    fn publish_due_privacy_aggregate_cycle_from_source_events_emits_fixed_empty_population_set() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = node_with_test_privacy_cycle_prf_provider(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        let empty = handle
            .publish_due_privacy_aggregate_cycle_from_source_events(
                211,
                privacy_aggregate_cycle_id([0xB0; 32], 100, 200),
                "empty-cycle-1".to_string(),
                privacy_aggregate_schedule_config(),
                privacy_aggregate_cycle_config(),
                Some(privacy_composition_budget_policy()),
            )
            .expect("empty due aggregate cycle");
        let publication = match empty {
            PrivacyAggregateScheduleOutcome::Published {
                window,
                publication,
            } => {
                assert_eq!(window.cycle_start_unix, 100);
                assert_eq!(window.cycle_end_unix, 200);
                publication
            }
            other => panic!("expected empty fixed-schema publication, got {other:?}"),
        };
        assert_eq!(publication.block.generated_at_unix, 200);
        assert_eq!(publication.block.entry_count, 2);
        assert_eq!(publisher.take().len(), 1);
    }

    #[test]
    fn governance_publisher_presence_tracks_set_and_clear() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        assert!(!handle.has_governance_publisher());

        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher;
        handle.set_governance_publisher(trait_publisher);
        assert!(handle.has_governance_publisher());

        handle.clear_governance_publisher();
        assert!(!handle.has_governance_publisher());
    }

    #[test]
    fn publish_appeal_finance_weekly_rollup_writes_governance_publisher() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        let rollup = appeal_finance_weekly_rollup_fixture();
        let expected = to_bytes(&rollup).expect("encode appeal finance weekly rollup");

        handle
            .publish_appeal_finance_weekly_rollup(rollup)
            .expect("publish appeal finance weekly rollup");

        let published = publisher.take();
        assert_eq!(published, vec![expected]);
    }

    #[test]
    fn publish_appeal_finance_settlement_receipt_writes_governance_publisher() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        let receipt = appeal_finance_settlement_receipt_fixture();
        let expected = to_bytes(&receipt).expect("encode appeal finance settlement receipt");

        handle
            .publish_appeal_finance_settlement_receipt(receipt)
            .expect("publish appeal finance settlement receipt");

        let published = publisher.take();
        assert_eq!(published, vec![expected]);
    }

    #[test]
    fn settle_deal_writes_filesystem_governance_payloads() {
        let temp = tempfile::tempdir().expect("temp dir");
        let root = temp.path().canonicalize().expect("canonical temp dir");
        let governance_dir = root.join("governance");
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(root.join("storage"))
            .governance_dir(Some(governance_dir.clone()))
            .build();
        let handle = NodeHandle::new(cfg);

        let provider_id = ProviderId::new([0x10; 32]);
        let client_id = ClientId::new([0x20; 32]);

        handle
            .deposit_provider_bond(provider_id, xor("1"))
            .expect("deposit provider bond");
        handle
            .deposit_client_credit(client_id, xor("1"))
            .expect("deposit client credit");

        let terms = DealTerms {
            storage_price_per_gib_month: quantity("0.1"),
            egress_price_per_gib: quantity("0.025"),
            settlement_window_epochs: 5,
            micropayment_probability_bps: 10_000,
            micropayment_payout: quantity("0.001"),
        };

        let activation_epoch = 1_680_000_000;
        let proposal = DealProposal {
            provider_id,
            client_id,
            storage_class: StorageClass::Hot,
            capacity_gib: 1,
            start_epoch: activation_epoch,
            end_epoch: activation_epoch + 10,
            terms,
            metadata: Metadata::default(),
        };

        let record = handle
            .open_deal(proposal, activation_epoch)
            .expect("open deal");

        let usage = DealUsageReport {
            deal_id: record.deal_id,
            epoch: activation_epoch + 1,
            storage_gib_hours: (GIB_HOURS_PER_MONTH / 2) as u64,
            egress_bytes: (BYTES_PER_GIB / 4) as u64,
            tickets: vec![],
        };

        handle
            .record_deal_usage(usage)
            .expect("record usage succeeds");

        let outcome = handle
            .settle_deal(record.deal_id, activation_epoch + 5)
            .expect("settlement");

        let deal_hex = hex::encode(record.deal_id.as_bytes());
        let output_dir = governance_dir.join("settlements").join(deal_hex);
        let entries = std::fs::read_dir(&output_dir)
            .expect("settlement artefacts directory exists")
            .map(|entry| entry.expect("dir entry").path())
            .collect::<Vec<_>>();
        assert!(
            entries
                .iter()
                .any(|path| path.extension().map(|ext| ext == "to").unwrap_or(false)),
            "encoded artefact missing"
        );

        let encoded_path = entries
            .iter()
            .find(|path| path.extension().map(|ext| ext == "to").unwrap_or(false))
            .expect("encoded artefact present");
        let encoded = std::fs::read(encoded_path).expect("read encoded artefact");
        let decoded: DealSettlementV1 =
            norito::decode_from_bytes(&encoded).expect("decode artefact");
        assert_eq!(decoded.deal_id, *record.deal_id.as_bytes());
        assert_eq!(decoded.status, outcome.governance.status);

        let json_path = entries
            .iter()
            .find(|path| path.extension().map(|ext| ext == "json").unwrap_or(false))
            .expect("json artefact present");
        let json_bytes = std::fs::read(json_path).expect("read json artefact");
        let value: norito::json::Value =
            norito::json::from_slice(&json_bytes).expect("json parses");
        let status = value
            .get("metadata")
            .and_then(|meta| meta.get("status"))
            .and_then(norito::json::Value::as_str)
            .expect("status present");
        assert_eq!(status, "window_settled");
    }

    #[test]
    fn settlement_publish_failure_is_best_effort() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let publisher = Arc::new(FailingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);

        let provider_id = ProviderId::new([0xDE; 32]);
        let client_id = ClientId::new([0xEF; 32]);

        handle
            .deposit_provider_bond(provider_id, xor("3"))
            .expect("deposit provider bond");
        handle
            .deposit_client_credit(client_id, xor("1"))
            .expect("deposit client credit");

        let terms = DealTerms {
            storage_price_per_gib_month: quantity("0.2"),
            egress_price_per_gib: quantity("0.05"),
            settlement_window_epochs: 7,
            micropayment_probability_bps: 10_000,
            micropayment_payout: quantity("0.05"),
        };

        let activation_epoch = 1_700_100_000;
        let proposal = DealProposal {
            provider_id,
            client_id,
            storage_class: StorageClass::Hot,
            capacity_gib: 3,
            start_epoch: activation_epoch,
            end_epoch: activation_epoch + 14,
            terms,
            metadata: Metadata::default(),
        };

        let record = handle
            .open_deal(proposal, activation_epoch)
            .expect("open deal");

        let usage = DealUsageReport {
            deal_id: record.deal_id,
            epoch: activation_epoch + 1,
            storage_gib_hours: (2 * GIB_HOURS_PER_MONTH) as u64,
            egress_bytes: (BYTES_PER_GIB / 2) as u64,
            tickets: vec![],
        };

        handle
            .record_deal_usage(usage)
            .expect("record usage should succeed");

        let outcome = handle
            .settle_deal(record.deal_id, activation_epoch + 7)
            .expect("settlement succeeds despite publish failure");
        assert_eq!(publisher.attempts(), 1);
        assert_eq!(handle.pending_governance_publication_count(), 1);
        assert_eq!(outcome.record.deal_id, record.deal_id);
    }

    #[test]
    fn por_ingestion_status_tracks_backlog_and_history() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let challenge = por_sample_challenge();

        handle
            .record_por_challenge(&challenge)
            .expect("record challenge");

        let initial = handle
            .por_ingestion_status(&challenge.manifest_digest)
            .expect("status before verdict");
        assert_eq!(initial.providers.len(), 1);
        assert_eq!(initial.providers[0].pending_challenges, 1);
        assert_eq!(initial.providers[0].last_success_unix, None);

        let proof = por_sample_proof(&challenge);
        handle
            .record_por_proof(&proof, &por_sample_provider_key())
            .expect("record proof succeeds");
        let verdict = por_sample_verdict(&challenge, proof.proof_digest());
        handle
            .record_por_verdict(
                &verdict,
                &por_sample_auditor_keys(),
                1,
                &SuccessfulPorRepairHandoff,
            )
            .expect("record verdict succeeds");

        let after = handle
            .por_ingestion_status(&challenge.manifest_digest)
            .expect("status after verdict");
        assert_eq!(after.providers.len(), 1);
        let provider = &after.providers[0];
        assert_eq!(provider.pending_challenges, 0);
        assert_eq!(provider.last_success_unix, Some(verdict.decided_at));
        assert_eq!(provider.failures_total, 0);
        assert_eq!(provider.consecutive_failures, 0);
    }

    #[test]
    fn por_ingestion_status_tracks_failures() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let challenge = por_sample_challenge();
        handle
            .record_por_challenge(&challenge)
            .expect("record challenge");

        let mut verdict = por_sample_verdict(&challenge, [0; 32]);
        verdict.outcome = AuditOutcomeV1::Failed;
        verdict.failure_reason = Some("timeout".to_string());
        verdict.proof_digest = None;
        resign_por_sample_verdict(&mut verdict);
        handle
            .record_por_verdict(
                &verdict,
                &por_sample_auditor_keys(),
                1,
                &SuccessfulPorRepairHandoff,
            )
            .expect("record failure verdict");

        let status = handle
            .por_ingestion_status(&challenge.manifest_digest)
            .expect("status after failure");
        assert_eq!(status.providers.len(), 1);
        let provider = &status.providers[0];
        assert_eq!(provider.pending_challenges, 0);
        assert_eq!(provider.failures_total, 1);
        assert_eq!(provider.consecutive_failures, 1);
        assert_eq!(provider.last_failure_unix, Some(verdict.decided_at));
        assert!(provider.last_success_unix.is_none());
    }

    #[test]
    fn por_ingestion_overview_reports_pending_and_failures() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let challenge = por_sample_challenge();
        handle
            .record_por_challenge(&challenge)
            .expect("record challenge");

        let overview = handle.por_ingestion_overview();
        assert_eq!(overview.len(), 1);
        assert_eq!(overview[0].pending_challenges, 1);
        assert_eq!(overview[0].failures_total, 0);

        let mut verdict = por_sample_verdict(&challenge, [0; 32]);
        verdict.outcome = AuditOutcomeV1::Failed;
        verdict.failure_reason = Some("missed".to_string());
        verdict.proof_digest = None;
        resign_por_sample_verdict(&mut verdict);
        handle
            .record_por_verdict(
                &verdict,
                &por_sample_auditor_keys(),
                1,
                &SuccessfulPorRepairHandoff,
            )
            .expect("record failure verdict");

        let overview_after = handle.por_ingestion_overview();
        assert_eq!(overview_after.len(), 1);
        assert_eq!(overview_after[0].pending_challenges, 0);
        assert_eq!(overview_after[0].failures_total, 1);
        assert_eq!(
            overview_after[0].last_failure_unix,
            Some(verdict.decided_at)
        );
    }

    #[test]
    fn por_failures_trigger_slash_after_threshold() {
        let (base_cfg, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base_cfg.data_dir().clone())
            .penalty_strike_threshold(2)
            .penalty_bond_bps(5_000)
            .penalty_cooldown_secs(0)
            .build();
        let handle = NodeHandle::new(cfg);
        let challenge = por_sample_challenge();
        let provider = ProviderId::new(challenge.provider_id);

        handle
            .deposit_provider_bond(provider, xor("0.00001"))
            .expect("deposit provider bond");

        let mut verdict = por_sample_verdict(&challenge, [0; 32]);
        verdict.outcome = AuditOutcomeV1::Failed;
        verdict.failure_reason = Some("proof missing".to_string());
        verdict.proof_digest = None;
        resign_por_sample_verdict(&mut verdict);

        handle
            .record_por_challenge(&challenge)
            .expect("record first challenge");
        let first = handle
            .record_por_verdict(
                &verdict,
                &por_sample_auditor_keys(),
                1,
                &SuccessfulPorRepairHandoff,
            )
            .expect("record first failure");
        assert_eq!(first.consecutive_failures, 1);
        assert!(first.slash.is_none());

        let second_challenge = subsequent_por_challenge(&challenge, 10);
        let mut second_verdict = por_sample_verdict(&second_challenge, [0; 32]);
        second_verdict.outcome = AuditOutcomeV1::Failed;
        second_verdict.failure_reason = Some("proof missing".to_owned());
        second_verdict.proof_digest = None;
        resign_por_sample_verdict(&mut second_verdict);
        handle
            .record_por_challenge(&second_challenge)
            .expect("record second challenge");
        let second = handle
            .record_por_verdict(
                &second_verdict,
                &por_sample_auditor_keys(),
                1,
                &SuccessfulPorRepairHandoff,
            )
            .expect("record second failure");

        let slash = second.slash.expect("slash recommendation expected");
        assert_eq!(slash.provider_id, provider);
        assert_eq!(slash.manifest_digest, challenge.manifest_digest);
        assert_eq!(slash.penalty, xor("0.000005"));
        assert_eq!(second.consecutive_failures, 0);
    }

    #[test]
    fn por_slash_respects_cooldown() {
        let (base_cfg, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base_cfg.data_dir().clone())
            .penalty_strike_threshold(1)
            .penalty_bond_bps(10_000)
            .penalty_cooldown_secs(300)
            .build();
        let handle = NodeHandle::new(cfg);
        let challenge = por_sample_challenge();
        let provider = ProviderId::new(challenge.provider_id);
        handle
            .deposit_provider_bond(provider, xor("0.000002"))
            .expect("deposit provider bond");

        let mut verdict = por_sample_verdict(&challenge, [0; 32]);
        verdict.outcome = AuditOutcomeV1::Failed;
        verdict.failure_reason = Some("timeout".to_string());
        verdict.proof_digest = None;
        resign_por_sample_verdict(&mut verdict);

        handle
            .record_por_challenge(&challenge)
            .expect("record challenge");
        let first = handle
            .record_por_verdict(
                &verdict,
                &por_sample_auditor_keys(),
                1,
                &SuccessfulPorRepairHandoff,
            )
            .expect("record verdict");
        assert!(first.slash.is_some());

        // Cooldown prevents an immediate second slash even though the strike threshold is 1.
        let later_challenge = subsequent_por_challenge(&challenge, 120);
        let mut later_verdict = por_sample_verdict(&later_challenge, [0; 32]);
        later_verdict.outcome = AuditOutcomeV1::Failed;
        later_verdict.failure_reason = Some("timeout".to_owned());
        later_verdict.proof_digest = None;
        resign_por_sample_verdict(&mut later_verdict);
        handle
            .record_por_challenge(&later_challenge)
            .expect("record challenge after cooldown start");
        let second = handle
            .record_por_verdict(
                &later_verdict,
                &por_sample_auditor_keys(),
                1,
                &SuccessfulPorRepairHandoff,
            )
            .expect("record verdict during cooldown");
        assert!(second.slash.is_none());
        assert_eq!(second.consecutive_failures, 1);
    }

    #[test]
    fn por_penalty_rejects_bond_sum_overflow() {
        let maximum =
            "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824503042047"
                .parse::<XorQuantity>()
                .expect("maximum canonical XOR quantity");
        let one = "1".parse::<XorQuantity>().expect("canonical XOR quantity");

        assert!(matches!(
            checked_por_penalty(&maximum, &one, 10_000),
            Err(sorafs_manifest::deal::DealAmountError::Overflow)
        ));
    }

    #[test]
    fn node_handle_gc_evicts_expired_manifest_and_publishes_audit() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let gc_actual = iroha_config::parameters::actual::SorafsGc {
            enabled: true,
            retention_grace_secs: 0,
            max_deletions_per_run: 10,
            ..Default::default()
        };
        let handle =
            NodeHandle::new_with_policies(cfg, RepairConfig::default(), GcConfig::from(&gc_actual));

        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);

        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0xAB; 32],
            stake: sorafs_manifest::provider_advert::StakePointer {
                pool_id: [0xAA; 32],
                stake_amount: xor("1"),
            },
            committed_capacity_gib: 100,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 100,
                capability_refs: Vec::new(),
            }],
            lane_commitments: vec![LaneCommitmentV1 {
                lane_id: "default".into(),
                max_gib: 100,
            }],
            pricing: None,
            valid_from: 1,
            valid_until: 2,
            metadata: vec![],
        };
        let payload = to_bytes(&declaration).expect("encode declaration");
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(declaration.provider_id),
            payload,
            declaration.committed_capacity_gib,
            1,
            1,
            2,
            Metadata::default(),
        );
        handle
            .record_capacity_declaration(&record)
            .expect("record declaration");

        let payload = b"gc-expired-payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let retention_epoch = 1_700_000_000;
        let now_unix = retention_epoch + 10;
        let mut policy = PinPolicy::default();
        policy.retention_epoch = retention_epoch;
        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0x11; 8])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(policy)
            .build()
            .expect("manifest");

        let mut reader = payload.as_slice();
        let manifest_id = handle
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest manifest");
        let manifest_digest: [u8; 32] = manifest.digest().expect("digest").into();

        let report = run_test_gc(&handle, now_unix, &empty_finalized_repair_projection());
        assert_eq!(report.evictions.len(), 1);
        assert_eq!(report.freed_bytes, plan.content_length);
        assert!(handle.manifest_metadata(&manifest_id).is_err());

        let payloads = publisher.take();
        let mut gc_events = Vec::new();
        for payload in payloads {
            if let Ok(event) = norito::decode_from_bytes::<GcAuditEventV1>(&payload) {
                gc_events.push(event);
            }
        }
        assert_eq!(gc_events.len(), 1);
        let event = &gc_events[0];
        assert_eq!(event.version, GC_AUDIT_EVENT_VERSION_V1);
        assert_eq!(event.payload.version, GC_AUDIT_PAYLOAD_VERSION_V1);
        assert_eq!(event.payload.manifest_digest, manifest_digest);
        assert_eq!(event.payload.provider_id, declaration.provider_id);
        assert_eq!(event.payload.freed_bytes, plan.content_length);
        assert!(event.payload.blocked_reason.is_none());
    }

    #[test]
    fn gc_eviction_transaction_prepare_checkpoint_failure_prevents_domain_commit() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new_with_policies(
            cfg.clone(),
            RepairConfig::default(),
            enabled_gc_config(1),
        );
        let now_unix = 1_710_000_050;
        let digest = build_manifest_with_retention(
            vec![0x70; 8],
            now_unix - 1,
            b"gc-prepare-checkpoint-failure",
            &handle,
        );
        let manifest_id = hex::encode(digest);
        let checkpoint_path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        let committed = fs::read(&checkpoint_path).expect("read committed auxiliary checkpoint");
        fs::remove_file(&checkpoint_path).expect("remove auxiliary checkpoint");
        fs::create_dir(&checkpoint_path).expect("inject auxiliary checkpoint directory");

        let report = run_test_gc(&handle, now_unix, &empty_finalized_repair_projection());
        assert!(report.evictions.is_empty());
        assert_eq!(report.errors, 1);
        assert!(handle.manifest_metadata(&manifest_id).is_ok());
        assert_eq!(handle.storage.as_ref().unwrap().gc_counters(), (0, 0));
        assert_eq!(handle.pending_governance_publication_count(), 0);
        assert!(
            handle
                .gc_eviction_intents
                .read()
                .expect("intent lock")
                .entries
                .is_empty()
        );
        assert!(handle.durability_failure_reason().is_none());

        fs::remove_dir(&checkpoint_path).expect("remove injected checkpoint directory");
        write_local_checkpoint_atomic(&checkpoint_path, &committed)
            .expect("restore committed checkpoint");
        drop(handle);
        let restored =
            NodeHandle::new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1));
        assert!(restored.manifest_metadata(&manifest_id).is_ok());
        assert_eq!(restored.pending_governance_publication_count(), 0);
    }

    #[test]
    fn gc_eviction_transaction_discards_pre_domain_crash_intent() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new_with_policies(
            cfg.clone(),
            RepairConfig::default(),
            enabled_gc_config(1),
        );
        let now_unix = 1_710_000_100;
        let digest = build_manifest_with_retention(
            vec![0x71; 8],
            now_unix - 1,
            b"gc-pre-domain-crash",
            &handle,
        );
        let manifest_id = hex::encode(digest);
        let storage = handle.storage.as_ref().expect("storage backend");
        let target = storage.manifest(&manifest_id).expect("GC target");
        {
            let gc_guard = handle.gc_mutation_lock.lock().expect("GC mutation lock");
            let drain_guard = handle
                .governance_outbox_drain_lock
                .lock()
                .expect("outbox drain lock");
            let intent = handle
                .prepare_gc_eviction_intent(
                    (&gc_guard, &drain_guard),
                    storage,
                    &target,
                    [0; 32],
                    now_unix,
                    GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1,
                )
                .expect("persist GC eviction intent");
            assert_eq!(intent.reserved_outbox_slots, 1);
        }
        assert_eq!(
            handle
                .gc_eviction_intents
                .read()
                .expect("intent lock")
                .entries
                .len(),
            1
        );
        assert_eq!(handle.pending_governance_publication_count(), 0);
        assert!(storage.manifest(&manifest_id).is_some());
        drop(target);
        drop(handle);

        let restored =
            NodeHandle::new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1));
        assert!(restored.manifest_metadata(&manifest_id).is_ok());
        assert_eq!(restored.storage.as_ref().unwrap().gc_counters(), (0, 0));
        assert_eq!(restored.pending_governance_publication_count(), 0);
        assert!(
            restored
                .gc_eviction_intents
                .read()
                .expect("restored intent lock")
                .entries
                .is_empty()
        );
        assert!(
            restored
                .gc_eviction_audit_links
                .read()
                .expect("restored link lock")
                .is_empty()
        );
    }

    #[test]
    fn gc_eviction_transaction_fail_closes_storage_generation_drift() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new_with_policies(
            cfg.clone(),
            RepairConfig::default(),
            enabled_gc_config(1),
        );
        let now_unix = 1_710_000_150;
        let digest = build_manifest_with_retention(
            vec![0x7B; 8],
            now_unix - 1,
            b"gc-generation-drift-target",
            &handle,
        );
        let manifest_id = hex::encode(digest);
        let storage = handle.storage.as_ref().expect("storage backend");
        let target = storage.manifest(&manifest_id).expect("GC target");
        let gc_guard = handle.gc_mutation_lock.lock().expect("GC mutation lock");
        let drain_guard = handle
            .governance_outbox_drain_lock
            .lock()
            .expect("outbox drain lock");
        let intent = handle
            .prepare_gc_eviction_intent(
                (&gc_guard, &drain_guard),
                storage,
                &target,
                [0; 32],
                now_unix,
                GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1,
            )
            .expect("persist GC eviction intent");
        build_manifest_with_retention(
            vec![0x7C; 8],
            now_unix + 100,
            b"gc-generation-drift-interloper",
            &handle,
        );

        let error = handle
            .settle_gc_eviction_intent_against_storage(
                &gc_guard,
                &drain_guard,
                storage,
                &intent,
                true,
            )
            .expect_err("unexpected storage generation must fail closed");
        assert!(error.to_string().contains("storage generation drift"));
        assert!(handle.durability_failure_reason().is_some());
        assert!(storage.manifest(&manifest_id).is_some());
        assert_eq!(storage.gc_counters(), (0, 0));
        assert_eq!(handle.pending_governance_publication_count(), 0);
        assert_eq!(
            handle
                .gc_eviction_intents
                .read()
                .expect("intent lock")
                .entries
                .len(),
            1
        );
        drop(drain_guard);
        drop(gc_guard);
        drop(target);
        drop(handle);

        let error =
            NodeHandle::try_new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1))
                .expect_err("ambiguous GC generation must also fail startup");
        assert!(error.to_string().contains("storage generation drift"));
    }

    #[test]
    fn gc_eviction_transaction_recovers_post_domain_crash_exactly_once() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new_with_policies(
            cfg.clone(),
            RepairConfig::default(),
            enabled_gc_config(1),
        );
        let now_unix = 1_710_000_200;
        let payload = b"gc-post-domain-crash";
        let digest = build_manifest_with_retention(vec![0x72; 8], now_unix - 1, payload, &handle);
        let manifest_id = hex::encode(digest);
        let storage = handle.storage.as_ref().expect("storage backend");
        let target = storage.manifest(&manifest_id).expect("GC target");
        {
            let gc_guard = handle.gc_mutation_lock.lock().expect("GC mutation lock");
            let drain_guard = handle
                .governance_outbox_drain_lock
                .lock()
                .expect("outbox drain lock");
            handle
                .prepare_gc_eviction_intent(
                    (&gc_guard, &drain_guard),
                    storage,
                    &target,
                    [0; 32],
                    now_unix,
                    GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1,
                )
                .expect("persist GC eviction intent");
            assert_eq!(
                storage
                    .evict_manifest(&manifest_id)
                    .expect("commit storage eviction"),
                u64::try_from(payload.len()).unwrap()
            );
        }
        assert_eq!(storage.gc_counters(), (payload.len() as u64, 1));
        assert_eq!(handle.pending_governance_publication_count(), 0);
        drop(target);
        drop(handle);

        let restored = NodeHandle::new_with_policies(
            cfg.clone(),
            RepairConfig::default(),
            enabled_gc_config(1),
        );
        assert!(restored.manifest_metadata(&manifest_id).is_err());
        assert_eq!(restored.pending_governance_publication_count(), 1);
        assert_eq!(
            restored
                .gc_eviction_audit_links
                .read()
                .expect("link lock")
                .len(),
            1
        );
        assert!(
            restored
                .gc_eviction_intents
                .read()
                .expect("intent lock")
                .entries
                .is_empty()
        );
        let publisher = Arc::new(RecordingPublisher::default());
        restored
            .try_set_governance_publisher(publisher.clone())
            .expect("publish recovered GC audit");
        let published = publisher.take();
        assert_eq!(published.len(), 1);
        let audit: GcAuditEventV1 =
            norito::decode_from_bytes(&published[0]).expect("decode recovered GC audit");
        audit.validate().expect("recovered audit validates");
        assert_eq!(audit.payload.manifest_digest, digest);
        assert_eq!(audit.payload.freed_bytes, payload.len() as u64);
        drop(restored);

        let acknowledged =
            NodeHandle::new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1));
        assert_eq!(acknowledged.pending_governance_publication_count(), 0);
        assert_eq!(
            acknowledged.storage.as_ref().unwrap().gc_counters(),
            (payload.len() as u64, 1)
        );
        assert_eq!(
            acknowledged
                .gc_eviction_audit_links
                .read()
                .expect("acknowledged link lock")
                .len(),
            1
        );
    }

    #[test]
    fn gc_eviction_transaction_finalization_checkpoint_failure_fail_stops_and_recovers() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new_with_policies(
            cfg.clone(),
            RepairConfig::default(),
            enabled_gc_config(1),
        );
        let now_unix = 1_710_000_250;
        let payload = b"gc-finalization-checkpoint-failure";
        let digest = build_manifest_with_retention(vec![0x7A; 8], now_unix - 1, payload, &handle);
        let manifest_id = hex::encode(digest);
        let storage = handle.storage.as_ref().expect("storage backend");
        let target = storage.manifest(&manifest_id).expect("GC target");
        let gc_guard = handle.gc_mutation_lock.lock().expect("GC mutation lock");
        let drain_guard = handle
            .governance_outbox_drain_lock
            .lock()
            .expect("outbox drain lock");
        let intent = handle
            .prepare_gc_eviction_intent(
                (&gc_guard, &drain_guard),
                storage,
                &target,
                [0; 32],
                now_unix,
                GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1,
            )
            .expect("persist GC eviction intent");
        let checkpoint_path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        let prepared = fs::read(&checkpoint_path).expect("read prepared GC checkpoint");
        assert_eq!(
            storage
                .evict_manifest(&manifest_id)
                .expect("commit storage eviction"),
            payload.len() as u64
        );
        fs::remove_file(&checkpoint_path).expect("remove prepared auxiliary checkpoint");
        fs::create_dir(&checkpoint_path).expect("inject auxiliary checkpoint directory");

        let error = handle
            .settle_gc_eviction_intent_against_storage(
                &gc_guard,
                &drain_guard,
                storage,
                &intent,
                true,
            )
            .expect_err("GC publication checkpoint failure must surface");
        assert!(
            error
                .to_string()
                .contains("storage state may have committed without its audit publication")
        );
        assert!(handle.durability_failure_reason().is_some());
        assert_eq!(handle.pending_governance_publication_count(), 0);
        assert_eq!(
            handle
                .gc_eviction_intents
                .read()
                .expect("rolled-back intent lock")
                .entries
                .len(),
            1
        );
        assert!(
            handle
                .gc_eviction_audit_links
                .read()
                .expect("rolled-back link lock")
                .is_empty()
        );
        drop(drain_guard);
        drop(gc_guard);
        let publisher = Arc::new(RecordingPublisher::default());
        assert!(
            handle
                .try_set_governance_publisher(publisher.clone())
                .is_err()
        );
        assert!(publisher.take().is_empty());

        drop(target);
        fs::remove_dir(&checkpoint_path).expect("remove injected checkpoint directory");
        write_local_checkpoint_atomic(&checkpoint_path, &prepared)
            .expect("restore prepared GC checkpoint");
        drop(handle);

        let restored =
            NodeHandle::new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1));
        assert!(restored.manifest_metadata(&manifest_id).is_err());
        assert_eq!(restored.storage.as_ref().unwrap().gc_counters().1, 1);
        assert_eq!(restored.pending_governance_publication_count(), 1);
        assert!(
            restored
                .gc_eviction_intents
                .read()
                .expect("restored intent lock")
                .entries
                .is_empty()
        );
        assert_eq!(
            restored
                .gc_eviction_audit_links
                .read()
                .expect("restored link lock")
                .len(),
            1
        );
    }

    #[test]
    fn gc_eviction_transaction_reservation_survives_full_outbox_restart() {
        let (base, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base.data_dir().clone())
            .runtime_retention(RuntimeRetentionPolicy::new(1, 2, 2 * 1024 * 1024))
            .build();
        let handle = NodeHandle::new_with_policies(
            cfg.clone(),
            RepairConfig::default(),
            enabled_gc_config(1),
        );
        let now_unix = 1_710_000_300;
        let digest = build_manifest_with_retention(
            vec![0x73; 8],
            now_unix - 1,
            b"gc-full-outbox-restart",
            &handle,
        );
        let manifest_id = hex::encode(digest);
        let issuance = proof_token_issuance_fixture();
        handle
            .publish_proof_token_issuance(issuance)
            .expect("occupy first outbox slot");
        let storage = handle.storage.as_ref().expect("storage backend");
        let target = storage.manifest(&manifest_id).expect("GC target");
        {
            let gc_guard = handle.gc_mutation_lock.lock().expect("GC mutation lock");
            let drain_guard = handle
                .governance_outbox_drain_lock
                .lock()
                .expect("outbox drain lock");
            handle
                .prepare_gc_eviction_intent(
                    (&gc_guard, &drain_guard),
                    storage,
                    &target,
                    [0; 32],
                    now_unix,
                    GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1,
                )
                .expect("reserve final outbox slot");

            let mut competing = proof_token_issuance_fixture();
            competing.token_id = [0xD1; 16];
            competing.token_blake3 = [0xD2; 32];
            let error = handle
                .enqueue_governance_outbox(
                    GovernanceOutboxKindV1::ProofTokenIssuance,
                    norito::to_bytes(&competing).expect("encode competing publication"),
                )
                .expect_err("competing publication cannot consume GC reservation");
            assert!(error.to_string().contains("slots are reserved"));
            storage
                .evict_manifest(&manifest_id)
                .expect("commit storage eviction");
        }
        assert_eq!(handle.pending_governance_publication_count(), 1);
        drop(target);
        drop(handle);

        let restored = NodeHandle::new_with_policies(
            cfg.clone(),
            RepairConfig::default(),
            enabled_gc_config(1),
        );
        assert_eq!(restored.pending_governance_publication_count(), 2);
        assert!(
            restored
                .gc_eviction_intents
                .read()
                .expect("restored intent lock")
                .entries
                .is_empty()
        );
        let publisher = Arc::new(RecordingPublisher::default());
        restored
            .try_set_governance_publisher(publisher.clone())
            .expect("drain full recovered outbox");
        assert_eq!(publisher.take().len(), 2);
        drop(restored);

        let acknowledged =
            NodeHandle::new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1));
        assert_eq!(acknowledged.pending_governance_publication_count(), 0);
        assert_eq!(acknowledged.storage.as_ref().unwrap().gc_counters().1, 1);
    }

    #[test]
    fn gc_eviction_transaction_rejects_intent_binding_and_counter_tampering() {
        for tamper_counter in [false, true] {
            let (cfg, _dir) = storage_config_with_temp_dir();
            let handle = NodeHandle::new_with_policies(
                cfg.clone(),
                RepairConfig::default(),
                enabled_gc_config(1),
            );
            let now_unix = if tamper_counter {
                1_710_000_401
            } else {
                1_710_000_400
            };
            let digest = build_manifest_with_retention(
                vec![if tamper_counter { 0x75 } else { 0x74 }; 8],
                now_unix - 1,
                if tamper_counter {
                    b"gc-counter-tamper".as_slice()
                } else {
                    b"gc-binding-tamper".as_slice()
                },
                &handle,
            );
            let manifest_id = hex::encode(digest);
            let storage = handle.storage.as_ref().expect("storage backend");
            let target = storage.manifest(&manifest_id).expect("GC target");
            {
                let gc_guard = handle.gc_mutation_lock.lock().expect("GC mutation lock");
                let drain_guard = handle
                    .governance_outbox_drain_lock
                    .lock()
                    .expect("outbox drain lock");
                handle
                    .prepare_gc_eviction_intent(
                        (&gc_guard, &drain_guard),
                        storage,
                        &target,
                        [0; 32],
                        now_unix,
                        GC_AUDIT_REASON_RETENTION_EXPIRED_PROVIDER_MISSING_V1,
                    )
                    .expect("persist GC intent");
            }
            let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
            let bytes = fs::read(&path).expect("read GC intent checkpoint");
            let mut checkpoint: AuxiliaryRuntimeCheckpointV2 =
                norito::decode_from_bytes(&bytes).expect("decode GC intent checkpoint");
            let intent = checkpoint
                .gc_eviction_intents
                .first_mut()
                .expect("GC intent");
            if tamper_counter {
                intent.storage_after.gc_evictions_total =
                    intent.storage_after.gc_evictions_total.saturating_add(1);
                intent.binding_digest =
                    gc_eviction_intent_binding_digest(intent).expect("rebind forged intent");
            } else {
                intent.binding_digest[0] ^= 0x80;
            }
            write_local_checkpoint_atomic(
                &path,
                &norito::to_bytes(&checkpoint).expect("encode tampered GC checkpoint"),
            )
            .expect("write tampered GC checkpoint");
            drop(target);
            drop(handle);

            let error = NodeHandle::try_new_with_policies(
                cfg,
                RepairConfig::default(),
                enabled_gc_config(1),
            )
            .expect_err("tampered GC intent must fail startup");
            let message = error.to_string();
            assert!(
                message.contains("binding digest mismatch")
                    || message.contains("one exact eviction"),
                "unexpected error: {message}"
            );
        }
    }

    #[test]
    fn gc_eviction_transaction_rejects_acknowledged_link_counter_tampering() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new_with_policies(
            cfg.clone(),
            RepairConfig::default(),
            enabled_gc_config(1),
        );
        let now_unix = 1_710_000_500;
        build_manifest_with_retention(
            vec![0x76; 8],
            now_unix - 1,
            b"gc-link-counter-tamper",
            &handle,
        );
        let publisher = Arc::new(RecordingPublisher::default());
        handle
            .try_set_governance_publisher(publisher)
            .expect("install publisher");
        let report = run_test_gc(&handle, now_unix, &empty_finalized_repair_projection());
        assert_eq!(report.evictions.len(), 1);
        assert_eq!(handle.pending_governance_publication_count(), 0);
        let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        let bytes = fs::read(&path).expect("read linked GC checkpoint");
        let mut checkpoint: AuxiliaryRuntimeCheckpointV2 =
            norito::decode_from_bytes(&bytes).expect("decode linked GC checkpoint");
        let link = checkpoint
            .gc_eviction_audit_links
            .first_mut()
            .expect("GC audit link");
        link.storage_gc_evictions_total = link.storage_gc_evictions_total.saturating_add(1);
        link.binding_digest = gc_eviction_audit_link_binding_digest(link);
        write_local_checkpoint_atomic(
            &path,
            &norito::to_bytes(&checkpoint).expect("encode forged GC link checkpoint"),
        )
        .expect("write forged GC link checkpoint");
        drop(handle);

        let error =
            NodeHandle::try_new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1))
                .expect_err("forged GC storage counter linkage must fail startup");
        assert!(error.to_string().contains("storage counter generation"));
    }

    #[test]
    fn gc_blocks_shared_chunks_with_zero_byte_audit() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle =
            NodeHandle::new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1));
        let now_unix = 1_710_000_600;
        let payload = b"gc-shared-chunk-zero-byte-audit";
        build_manifest_with_retention(vec![0x76; 8], now_unix + 60, payload, &handle);
        build_manifest_with_retention(vec![0x77; 8], now_unix - 1, payload, &handle);

        let report = run_test_gc(&handle, now_unix, &empty_finalized_repair_projection());
        assert_eq!(report.errors, 0);
        assert!(report.evictions.is_empty());
        assert_eq!(report.freed_bytes, 0);
        assert!(
            report
                .skipped
                .iter()
                .any(|skip| skip.reason == GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1)
        );
        assert_eq!(handle.storage.as_ref().unwrap().gc_counters(), (0, 0));
        let outbox = handle.governance_outbox.read().expect("outbox lock");
        let entry = outbox.entries.values().next().expect("GC audit entry");
        let audit: GcAuditEventV1 =
            norito::decode_from_bytes(&entry.payload_bytes).expect("decode zero-byte audit");
        audit.validate().expect("zero-byte GC audit validates");
        assert_eq!(audit.payload.freed_bytes, 0);
        assert_eq!(
            audit.payload.blocked_reason.as_deref(),
            Some(GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1)
        );
    }

    #[test]
    fn gc_eviction_transaction_publisher_failure_retries_durable_audit() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new_with_policies(
            cfg.clone(),
            RepairConfig::default(),
            enabled_gc_config(1),
        );
        let now_unix = 1_710_000_700;
        build_manifest_with_retention(vec![0x78; 8], now_unix - 1, b"gc-publisher-retry", &handle);
        let failing = Arc::new(FailingPublisher::default());
        handle
            .try_set_governance_publisher(failing.clone())
            .expect("install initially idle failing publisher");

        let report = run_test_gc(&handle, now_unix, &empty_finalized_repair_projection());
        assert_eq!(report.evictions.len(), 1);
        assert_eq!(report.errors, 1);
        assert!(failing.attempts() >= 1);
        assert_eq!(handle.pending_governance_publication_count(), 1);
        handle.clear_governance_publisher();
        let recording = Arc::new(RecordingPublisher::default());
        handle
            .try_set_governance_publisher(recording.clone())
            .expect("retry durable GC audit");
        assert_eq!(recording.take().len(), 1);
        assert_eq!(handle.pending_governance_publication_count(), 0);
        drop(handle);

        let restored =
            NodeHandle::new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1));
        assert_eq!(restored.storage.as_ref().unwrap().gc_counters().1, 1);
        assert_eq!(
            restored
                .gc_eviction_audit_links
                .read()
                .expect("restored link lock")
                .len(),
            1
        );
    }

    #[test]
    fn gc_eviction_transaction_serializes_concurrent_sweeps() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle =
            NodeHandle::new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1));
        let now_unix = 1_710_000_800;
        for index in 0_u8..4 {
            let payload = vec![0x80 + index; 32];
            build_manifest_with_retention(vec![0x80 + index; 8], now_unix - 1, &payload, &handle);
        }
        let barrier = Arc::new(Barrier::new(8));
        let workers = (0..8)
            .map(|_| {
                let handle = handle.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    run_test_gc(&handle, now_unix, &empty_finalized_repair_projection())
                })
            })
            .collect::<Vec<_>>();
        let reports = workers
            .into_iter()
            .map(|worker| worker.join().expect("GC worker joins"))
            .collect::<Vec<_>>();
        assert_eq!(
            reports
                .iter()
                .map(|report| report.evictions.len())
                .sum::<usize>(),
            4
        );
        assert_eq!(reports.iter().map(|report| report.errors).sum::<u32>(), 0);
        let storage = handle.storage.as_ref().expect("storage backend");
        assert_eq!(storage.manifest_count(), 0);
        assert_eq!(storage.gc_counters(), (128, 4));
        assert_eq!(handle.pending_governance_publication_count(), 4);
        assert_eq!(
            handle
                .gc_eviction_audit_links
                .read()
                .expect("link lock")
                .len(),
            4
        );
    }

    #[test]
    fn gc_blocked_audit_full_outbox_is_reported_without_eviction() {
        let (base, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base.data_dir().clone())
            .runtime_retention(RuntimeRetentionPolicy::new(1, 1, 2 * 1024 * 1024))
            .build();
        let handle =
            NodeHandle::new_with_policies(cfg, RepairConfig::default(), enabled_gc_config(1));
        handle
            .publish_proof_token_issuance(proof_token_issuance_fixture())
            .expect("fill governance outbox");
        let now_unix = 1_710_000_900;
        let payload = b"gc-blocked-full-outbox";
        let first = build_manifest_with_retention(vec![0x91; 8], now_unix - 1, payload, &handle);
        let second = build_manifest_with_retention(vec![0x92; 8], now_unix - 1, payload, &handle);

        let report = run_test_gc(&handle, now_unix, &empty_finalized_repair_projection());
        assert!(report.evictions.is_empty());
        assert_eq!(report.errors, 1);
        assert!(
            report
                .skipped
                .iter()
                .any(|skip| skip.reason == GC_AUDIT_BLOCKED_SHARED_CHUNKS_V1)
        );
        assert!(handle.manifest_metadata(&hex::encode(first)).is_ok());
        assert!(handle.manifest_metadata(&hex::encode(second)).is_ok());
        assert_eq!(handle.storage.as_ref().unwrap().gc_counters(), (0, 0));
        assert_eq!(handle.pending_governance_publication_count(), 1);
    }

    #[test]
    fn node_handle_reconciliation_emits_report() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle =
            NodeHandle::new_with_policies(cfg, enabled_repair_config(1), GcConfig::default());

        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);

        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x11; 32],
            stake: sorafs_manifest::provider_advert::StakePointer {
                pool_id: [0x22; 32],
                stake_amount: xor("1"),
            },
            committed_capacity_gib: 100,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 100,
                capability_refs: Vec::new(),
            }],
            lane_commitments: vec![LaneCommitmentV1 {
                lane_id: "default".into(),
                max_gib: 100,
            }],
            pricing: None,
            valid_from: 1,
            valid_until: 2,
            metadata: vec![],
        };
        let payload = to_bytes(&declaration).expect("encode declaration");
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(declaration.provider_id),
            payload,
            declaration.committed_capacity_gib,
            1,
            1,
            2,
            Metadata::default(),
        );
        handle
            .record_capacity_declaration(&record)
            .expect("record declaration");

        let payload = b"reconciliation-payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let mut policy = PinPolicy::default();
        policy.retention_epoch = 1_700_000_000;
        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0x33; 8])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(policy)
            .build()
            .expect("manifest");
        let manifest_digest: [u8; 32] = manifest.digest().expect("digest").into();
        let mut reader = payload.as_slice();
        handle
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest manifest");

        let repair_projection = finalized_repair_projection(vec![active_native_repair_task(
            manifest_digest,
            declaration.provider_id,
        )]);
        publisher.take();

        let now_unix = 1_700_000_200;
        let reconciliation = handle
            .run_reconciliation_once(now_unix, &repair_projection)
            .expect("reconciliation report");
        assert_eq!(
            reconciliation.version,
            SORAFS_RECONCILIATION_REPORT_VERSION_V1
        );
        assert_eq!(reconciliation.provider_id, declaration.provider_id);
        assert_eq!(reconciliation.generated_at_unix, now_unix);
        assert_eq!(reconciliation.repair_task_count, 1);
        assert_eq!(reconciliation.retention_manifest_count, 1);
        assert_eq!(reconciliation.gc_evictions_total, 0);
        assert_eq!(reconciliation.gc_freed_bytes_total, 0);
        assert_eq!(reconciliation.divergence_count, 0);
        assert!(reconciliation.appeal_finance.is_none());

        let payloads = publisher.take();
        let decoded = payloads
            .iter()
            .find_map(|payload| {
                norito::decode_from_bytes::<SorafsReconciliationReportV1>(payload).ok()
            })
            .expect("reconciliation payload");
        assert_eq!(decoded, reconciliation);

        let reconciliation_again = handle
            .run_reconciliation_once(now_unix, &repair_projection)
            .expect("reconciliation report");
        assert_eq!(
            reconciliation_again.repair_snapshot_hash,
            reconciliation.repair_snapshot_hash
        );
        assert_eq!(
            reconciliation_again.retention_snapshot_hash,
            reconciliation.retention_snapshot_hash
        );
        assert_eq!(
            reconciliation_again.gc_snapshot_hash,
            reconciliation.gc_snapshot_hash
        );
    }

    #[test]
    fn node_handle_reconciliation_includes_appeal_finance_rollups() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(root.join("storage"))
            .governance_dir(Some(root.join("governance")))
            .build();
        let handle =
            NodeHandle::new_with_policies(cfg, enabled_repair_config(1), GcConfig::default());
        ensure_test_capacity_provider(&handle);
        assert!(handle.has_governance_publisher());

        let rollup = appeal_finance_weekly_rollup_fixture();
        handle
            .publish_appeal_finance_weekly_rollup(rollup.clone())
            .expect("publish appeal finance weekly rollup");

        let reconciliation = handle
            .run_reconciliation_once(1_700_000_300, &empty_finalized_repair_projection())
            .expect("reconciliation report");
        let appeal_finance = reconciliation
            .appeal_finance
            .as_ref()
            .expect("appeal finance reconciliation summary");
        assert_eq!(appeal_finance.rollup_count, 1);
        assert_ne!(appeal_finance.rollup_snapshot_hash, [0u8; 32]);
        assert_eq!(appeal_finance.source_report_count, rollup.report_count);
        assert_eq!(appeal_finance.case_count, rollup.case_count);
        assert_eq!(appeal_finance.total_treasury_xor, rollup.total_treasury_xor);
        assert_eq!(
            appeal_finance.total_rewards_forfeited_treasury_xor,
            rollup.total_rewards_forfeited_treasury_xor
        );
    }

    #[test]
    fn reconciliation_without_provider_binding_fails_without_publication() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle =
            NodeHandle::new_with_policies(cfg, RepairConfig::default(), GcConfig::default());
        let publisher = Arc::new(RecordingPublisher::default());
        handle.set_governance_publisher(publisher.clone());

        let error = handle
            .run_reconciliation_once(1_700_000_301, &empty_finalized_repair_projection())
            .expect_err("unbound reconciliation must fail closed");

        assert!(matches!(
            error,
            ReconciliationError::ProviderBindingUnavailable
        ));
        assert!(publisher.take().is_empty());
        assert_eq!(handle.pending_governance_publication_count(), 0);
    }

    #[test]
    fn appeal_finance_exact_addition_normalizes_scale() {
        let sum = xor("420")
            .checked_add(&xor("80.2500"))
            .expect("exact XOR sum");

        assert_eq!(sum, xor("500.25"));
    }

    #[test]
    fn node_handle_gc_skips_manifest_with_active_repair_task() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let gc_actual = iroha_config::parameters::actual::SorafsGc {
            enabled: true,
            retention_grace_secs: 0,
            max_deletions_per_run: 10,
            ..Default::default()
        };
        let handle =
            NodeHandle::new_with_policies(cfg, RepairConfig::default(), GcConfig::from(&gc_actual));
        let provider_id = ensure_test_capacity_provider(&handle);

        let payload = b"gc-repair-blocked";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let retention_epoch = 1_700_000_000;
        let now_unix = retention_epoch + 10;
        let mut policy = PinPolicy::default();
        policy.retention_epoch = retention_epoch;
        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0x22; 8])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(policy)
            .build()
            .expect("manifest");

        let mut reader = payload.as_slice();
        let manifest_id = handle
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest manifest");
        let manifest_digest: [u8; 32] = manifest.digest().expect("digest").into();

        let repair_projection = finalized_repair_projection(vec![active_native_repair_task(
            manifest_digest,
            provider_id,
        )]);
        let report = run_test_gc(&handle, now_unix, &repair_projection);
        assert!(report.evictions.is_empty());
        assert!(
            report
                .skipped
                .iter()
                .any(|skip| skip.reason == "repair_active")
        );
        assert!(handle.manifest_metadata(&manifest_id).is_ok());
    }

    #[test]
    fn node_handle_gc_blocks_shared_chunks_and_records_metrics() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let gc_actual = iroha_config::parameters::actual::SorafsGc {
            enabled: true,
            retention_grace_secs: 0,
            max_deletions_per_run: 10,
            ..Default::default()
        };
        let handle =
            NodeHandle::new_with_policies(cfg, RepairConfig::default(), GcConfig::from(&gc_actual));

        let payload = b"shared-chunk-payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let retention_epoch = 1_700_000_000;
        let now_unix = retention_epoch + 10;
        let mut policy = PinPolicy::default();
        policy.retention_epoch = retention_epoch;

        let manifest_a = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0x33; 8])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(policy.clone())
            .build()
            .expect("manifest a");
        let manifest_b = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0x44; 8])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(policy)
            .build()
            .expect("manifest b");

        let mut reader = payload.as_slice();
        handle
            .ingest_manifest(&manifest_a, &plan, &mut reader)
            .expect("ingest manifest a");
        let mut reader = payload.as_slice();
        handle
            .ingest_manifest(&manifest_b, &plan, &mut reader)
            .expect("ingest manifest b");

        let metrics = global_or_default();
        let before = metrics
            .torii_sorafs_gc_blocked_total
            .with_label_values(&["shared_chunks"])
            .get();

        let report = run_test_gc(&handle, now_unix, &empty_finalized_repair_projection());
        assert!(report.evictions.is_empty());
        assert!(
            report
                .skipped
                .iter()
                .any(|skip| skip.reason == "shared_chunks")
        );

        let after = metrics
            .torii_sorafs_gc_blocked_total
            .with_label_values(&["shared_chunks"])
            .get();
        assert!(after >= before.saturating_add(1));
    }

    #[test]
    fn node_handle_reflects_config() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg.clone());

        assert!(handle.is_enabled());
        let observed = handle.config();
        assert_eq!(observed.enabled(), cfg.enabled());
        assert_eq!(observed.data_dir(), cfg.data_dir());
        assert_eq!(observed.max_capacity_bytes().0, cfg.max_capacity_bytes().0);
        assert_eq!(observed.max_parallel_fetches(), cfg.max_parallel_fetches());
        assert_eq!(observed.max_pins(), cfg.max_pins());
        assert_eq!(
            observed.por_sample_interval_secs(),
            cfg.por_sample_interval_secs()
        );
        assert_eq!(observed.alias(), cfg.alias());
        assert_eq!(observed.adverts().topics(), cfg.adverts().topics());
        assert!(handle.storage().is_some());
        assert!(handle.pdp_provider_protocol().is_some());
    }

    #[test]
    fn node_handle_is_disabled_when_backend_is_unavailable() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let mut handle = NodeHandle::new(cfg);

        handle.storage = None;

        assert!(!handle.is_enabled());
    }

    #[test]
    fn node_handle_threads_repair_and_gc_config() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let actual_repair = iroha_config::parameters::actual::SorafsRepair {
            enabled: true,
            claim_ttl_secs: 900,
            heartbeat_interval_secs: 45,
            max_attempts: 6,
            worker_concurrency: 9,
            ..Default::default()
        };

        let actual_gc = iroha_config::parameters::actual::SorafsGc {
            enabled: true,
            interval_secs: 300,
            max_deletions_per_run: 2_000,
            retention_grace_secs: 86_400,
            ..Default::default()
        };

        let repair_cfg = RepairConfig::from(&actual_repair);
        let gc_cfg = GcConfig::from(&actual_gc);
        let handle = NodeHandle::new_with_policies(cfg, repair_cfg.clone(), gc_cfg.clone());

        assert!(handle.repair_config().enabled());
        assert_eq!(handle.repair_config().claim_ttl_secs(), 900);
        assert_eq!(handle.repair_config().heartbeat_interval_secs(), 45);
        assert_eq!(handle.repair_config().max_attempts(), 6);
        assert_eq!(handle.repair_config().worker_concurrency(), 9);

        assert!(handle.gc_config().enabled());
        assert_eq!(handle.gc_config().interval_secs(), 300);
        assert_eq!(handle.gc_config().max_deletions_per_run(), 2_000);
        assert_eq!(handle.gc_config().retention_grace_secs(), 86_400);
    }

    #[test]
    fn native_repair_config_fails_startup_outside_consensus_and_resource_bounds() {
        let baseline = iroha_config::parameters::actual::SorafsRepair {
            enabled: true,
            claim_ttl_secs: 2,
            heartbeat_interval_secs: 1,
            max_attempts: 1,
            worker_concurrency: 1,
        };
        let mut invalid = Vec::new();

        let mut lease_too_small = baseline;
        lease_too_small.claim_ttl_secs = 0;
        invalid.push(("claim_ttl_secs", lease_too_small));
        let mut lease_overflow = baseline;
        lease_overflow.claim_ttl_secs = u64::MAX;
        invalid.push(("overflows", lease_overflow));
        let mut renewal_zero = baseline;
        renewal_zero.heartbeat_interval_secs = 0;
        invalid.push(("heartbeat_interval_secs", renewal_zero));
        let mut renewal_not_below = baseline;
        renewal_not_below.heartbeat_interval_secs = renewal_not_below.claim_ttl_secs;
        invalid.push(("strictly below", renewal_not_below));
        let mut attempts_zero = baseline;
        attempts_zero.max_attempts = 0;
        invalid.push(("max_attempts", attempts_zero));
        let mut attempts_large = baseline;
        attempts_large.max_attempts =
            iroha_config::parameters::defaults::sorafs::repair::MAX_ATTEMPTS_LIMIT + 1;
        invalid.push(("max_attempts", attempts_large));
        let mut concurrency_zero = baseline;
        concurrency_zero.worker_concurrency = 0;
        invalid.push(("worker_concurrency", concurrency_zero));
        let mut concurrency_large = baseline;
        concurrency_large.worker_concurrency =
            iroha_config::parameters::defaults::sorafs::repair::WORKER_CONCURRENCY_LIMIT + 1;
        invalid.push(("worker_concurrency", concurrency_large));
        let mut disabled_but_consumed = baseline;
        disabled_but_consumed.enabled = false;
        disabled_but_consumed.max_attempts = 0;
        invalid.push(("max_attempts", disabled_but_consumed));

        let temp = tempfile::tempdir().expect("temp dir");
        for (expected, repair) in invalid {
            let config = StorageConfig::builder()
                .enabled(false)
                .data_dir(temp.path().join(expected))
                .build();
            let error = NodeHandle::try_new_with_policies(
                config,
                RepairConfig::from(repair),
                GcConfig::default(),
            )
            .expect_err("invalid enabled native repair config must fail startup");
            assert!(matches!(error, NodeInitError::NativeRepairConfig { .. }));
            assert!(error.to_string().contains(expected), "{error}");
        }

        let maximum = iroha_config::parameters::actual::SorafsRepair {
            claim_ttl_secs: REPAIR_LEDGER_MAX_LEASE_MS_V1 / 1_000,
            heartbeat_interval_secs: REPAIR_LEDGER_MAX_LEASE_MS_V1 / 1_000 - 1,
            ..baseline
        };
        validate_native_repair_config(&RepairConfig::from(maximum))
            .expect("maximum bounded native lease config is accepted");
    }

    #[test]
    fn node_handle_records_capacity_declaration() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x11; 32],
            stake: sorafs_manifest::provider_advert::StakePointer {
                pool_id: [0xAA; 32],
                stake_amount: xor("1"),
            },
            committed_capacity_gib: 100,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 100,
                capability_refs: Vec::new(),
            }],
            lane_commitments: vec![LaneCommitmentV1 {
                lane_id: "default".into(),
                max_gib: 100,
            }],
            pricing: None,
            valid_from: 1,
            valid_until: 2,
            metadata: vec![],
        };
        let payload = to_bytes(&declaration).expect("encode declaration");
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(declaration.provider_id),
            payload,
            declaration.committed_capacity_gib,
            1,
            1,
            2,
            Metadata::default(),
        );

        handle
            .record_capacity_declaration(&record)
            .expect("record declaration");

        let usage = handle.capacity_usage();
        assert_eq!(usage.provider_id, Some([0x11; 32]));
        assert_eq!(usage.committed_total_gib, 100);
        assert_eq!(usage.available_total_gib, 100);

        let telemetry = handle
            .build_capacity_telemetry()
            .expect("telemetry accumulator present")
            .expect("telemetry payload");
        assert_eq!(telemetry.declared_capacity_gib, 100);
        assert_eq!(telemetry.utilised_capacity_gib, 0);
        assert_eq!(telemetry.successful_replications, 0);
    }

    #[test]
    fn node_handle_completes_replication_order() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x22; 32],
            stake: sorafs_manifest::provider_advert::StakePointer {
                pool_id: [0xAA; 32],
                stake_amount: xor("1"),
            },
            committed_capacity_gib: 200,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 200,
                capability_refs: Vec::new(),
            }],
            lane_commitments: vec![LaneCommitmentV1 {
                lane_id: "default".into(),
                max_gib: 200,
            }],
            pricing: None,
            valid_from: 1,
            valid_until: 100,
            metadata: vec![],
        };
        let payload = norito::to_bytes(&declaration).expect("encode declaration");
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(declaration.provider_id),
            payload,
            declaration.committed_capacity_gib,
            1,
            1,
            100,
            Metadata::default(),
        );

        handle
            .record_capacity_declaration(&record)
            .expect("record declaration");

        let order = ReplicationOrderV1 {
            version: sorafs_manifest::capacity::REPLICATION_ORDER_VERSION_V1,
            order_id: [0x99; 32],
            manifest_cid: vec![0x55; 32],
            manifest_digest: [0x77; 32],
            chunking_profile: "sorafs.sf1@1.0.0".into(),
            target_replicas: 1,
            assignments: vec![sorafs_manifest::capacity::ReplicationAssignmentV1 {
                provider_id: [0x22; 32],
                slice_gib: 50,
                lane: Some("default".into()),
            }],
            issued_at: 10,
            deadline_at: 20,
            sla: sorafs_manifest::capacity::ReplicationOrderSlaV1 {
                ingest_deadline_secs: 600,
                min_availability_percent_milli: 99_000,
                min_por_success_percent_milli: 99_000,
            },
            metadata: Vec::new(),
        };

        let plan = handle
            .schedule_replication_order(&order)
            .expect("schedule order")
            .expect("plan produced");
        assert_eq!(plan.assigned_slice_gib, 50);

        let release = handle
            .complete_replication_order(order.order_id)
            .expect("complete order");
        assert_eq!(release.released_gib, 50);
        assert_eq!(release.remaining_total_gib, 200);
    }

    #[test]
    fn capacity_declaration_reservations_and_meter_survive_restart() {
        let (base, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base.data_dir().clone())
            .runtime_retention(RuntimeRetentionPolicy::new(4, 8, 2 * 1024 * 1024))
            .build();
        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x23; 32],
            stake: sorafs_manifest::provider_advert::StakePointer {
                pool_id: [0xAA; 32],
                stake_amount: xor("1"),
            },
            committed_capacity_gib: 200,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 200,
                capability_refs: Vec::new(),
            }],
            lane_commitments: vec![LaneCommitmentV1 {
                lane_id: "default".into(),
                max_gib: 200,
            }],
            pricing: None,
            valid_from: 1,
            valid_until: 100,
            metadata: vec![],
        };
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(declaration.provider_id),
            norito::to_bytes(&declaration).expect("encode declaration"),
            declaration.committed_capacity_gib,
            1,
            1,
            100,
            Metadata::default(),
        );
        let order = ReplicationOrderV1 {
            version: sorafs_manifest::capacity::REPLICATION_ORDER_VERSION_V1,
            order_id: [0x9A; 32],
            manifest_cid: vec![0x55; 32],
            manifest_digest: [0x78; 32],
            chunking_profile: "sorafs.sf1@1.0.0".into(),
            target_replicas: 1,
            assignments: vec![sorafs_manifest::capacity::ReplicationAssignmentV1 {
                provider_id: declaration.provider_id,
                slice_gib: 50,
                lane: Some("default".into()),
            }],
            issued_at: 10,
            deadline_at: 20,
            sla: sorafs_manifest::capacity::ReplicationOrderSlaV1 {
                ingest_deadline_secs: 600,
                min_availability_percent_milli: 99_000,
                min_por_success_percent_milli: 99_000,
            },
            metadata: Vec::new(),
        };
        let source = NodeHandle::new(cfg.clone());
        source
            .record_capacity_declaration(&record)
            .expect("persist declaration");
        source
            .schedule_replication_order(&order)
            .expect("persist order")
            .expect("targeted plan");
        drop(source);

        let restored = NodeHandle::new(cfg);
        let usage = restored.capacity_usage();
        assert_eq!(usage.provider_id, Some(declaration.provider_id));
        assert_eq!(usage.allocated_total_gib, 50);
        assert_eq!(usage.outstanding_orders.len(), 1);
        assert_eq!(usage.outstanding_orders[0].issued_at, 10);
        let meter = restored.metering_snapshot();
        assert_eq!(meter.declared_gib, 200);
        assert_eq!(meter.orders_issued, 1);
        assert_eq!(meter.outstanding_orders, 1);
        assert_eq!(meter.outstanding_total_gib, 50);
        assert!(restored.build_capacity_telemetry().is_some());
        let release = restored
            .complete_replication_order(order.order_id)
            .expect("complete restored order");
        assert_eq!(release.released_gib, 50);
        assert_eq!(restored.capacity_usage().allocated_total_gib, 0);
    }

    #[test]
    fn node_handle_meter_tracks_replication_flow() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x55; 32],
            stake: sorafs_manifest::provider_advert::StakePointer {
                pool_id: [0xAA; 32],
                stake_amount: xor("1"),
            },
            committed_capacity_gib: 256,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 256,
                capability_refs: Vec::new(),
            }],
            lane_commitments: vec![LaneCommitmentV1 {
                lane_id: "default".into(),
                max_gib: 256,
            }],
            pricing: None,
            valid_from: 1,
            valid_until: 10,
            metadata: vec![],
        };
        let payload = norito::to_bytes(&declaration).expect("encode declaration");
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(declaration.provider_id),
            payload,
            declaration.committed_capacity_gib,
            0,
            1,
            10,
            Metadata::default(),
        );
        handle
            .record_capacity_declaration(&record)
            .expect("record declaration");

        let meter = handle.capacity_meter();
        let snapshot = meter.snapshot();
        assert_eq!(snapshot.declared_gib, 256);
        assert_eq!(snapshot.orders_issued, 0);
        assert_eq!(snapshot.outstanding_orders, 0);

        let order = ReplicationOrderV1 {
            version: REPLICATION_ORDER_VERSION_V1,
            order_id: [0x44; 32],
            manifest_cid: vec![0xDE, 0xAD],
            manifest_digest: [0xCD; 32],
            chunking_profile: "sorafs.sf1@1.0.0".into(),
            target_replicas: 1,
            assignments: vec![ReplicationAssignmentV1 {
                provider_id: declaration.provider_id,
                slice_gib: 64,
                lane: Some("default".into()),
            }],
            issued_at: 100,
            deadline_at: 400,
            sla: ReplicationOrderSlaV1 {
                ingest_deadline_secs: 600,
                min_availability_percent_milli: 99_000,
                min_por_success_percent_milli: 98_000,
            },
            metadata: vec![CapacityMetadataEntry {
                key: "priority".into(),
                value: "standard".into(),
            }],
        };

        let plan = handle
            .schedule_replication_order(&order)
            .expect("schedule ok")
            .expect("plan expected");
        assert_eq!(plan.assigned_slice_gib, 64);

        let snapshot_after_schedule = meter.snapshot();
        assert_eq!(snapshot_after_schedule.orders_issued, 1);
        assert_eq!(snapshot_after_schedule.outstanding_orders, 1);
        assert_eq!(snapshot_after_schedule.outstanding_total_gib, 64);

        handle
            .complete_replication_order(order.order_id)
            .expect("complete order");

        let snapshot_after_complete = meter.snapshot();
        assert_eq!(snapshot_after_complete.orders_completed, 1);
        assert_eq!(snapshot_after_complete.utilised_gib, 64);
        assert_eq!(snapshot_after_complete.outstanding_orders, 0);

        handle.update_telemetry(|acc| {
            acc.record_uptime_sample(540, 600).expect("uptime sample");
            acc.record_por_sample(true);
            acc.record_por_sample(false);
        });

        let telemetry = handle
            .build_capacity_telemetry()
            .expect("telemetry accumulator present")
            .expect("payload");
        assert_eq!(telemetry.successful_replications, 1);
        assert_eq!(telemetry.failed_replications, 0);
        assert_eq!(telemetry.uptime_percent_milli, 90_000);
        assert_eq!(telemetry.por_success_percent_milli, 50_000);
    }

    #[test]
    fn node_handle_storage_ingest_and_fetch_range() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let payload = b"node handle storage fetch test";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0xAA; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = handle
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");

        let bytes = handle
            .read_payload_range(&manifest_id, 5, 6)
            .expect("read range");
        assert_eq!(bytes, b"handle"[..]);
    }

    #[test]
    fn node_handle_storage_sample_por() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let payload = b"SoraFS node handle PoR sampling payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0xBB; 16])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let manifest_id = handle
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");

        let storage = handle.storage().expect("storage backend");
        let stored = storage.manifest(&manifest_id).expect("stored manifest");
        let expected = stored.por_tree().leaf_count().min(3);

        let samples = handle.sample_por(&manifest_id, 3, 99).expect("sample por");
        assert_eq!(samples.len(), expected);
        let root = *stored.por_tree().root();

        for (_idx, proof) in samples {
            assert!(proof.verify(&root));
        }
    }

    #[test]
    fn node_handle_plan_por_challenges_handles_vrf_and_forced() {
        use std::collections::HashMap;

        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x11; 32],
            stake: sorafs_manifest::provider_advert::StakePointer {
                pool_id: [0xAA; 32],
                stake_amount: xor("1"),
            },
            committed_capacity_gib: 128,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 128,
                capability_refs: Vec::new(),
            }],
            lane_commitments: vec![LaneCommitmentV1 {
                lane_id: "default".into(),
                max_gib: 128,
            }],
            pricing: None,
            valid_from: 1,
            valid_until: 2,
            metadata: vec![],
        };
        let payload = to_bytes(&declaration).expect("encode declaration");
        let provider_metadata = {
            let mut metadata = Metadata::default();
            metadata.insert(
                Name::from_str("profile.sample_multiplier").expect("valid metadata key"),
                2u64,
            );
            metadata
        };
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(declaration.provider_id),
            payload,
            declaration.committed_capacity_gib,
            1,
            1,
            2,
            provider_metadata,
        );
        handle
            .record_capacity_declaration(&record)
            .expect("record declaration");

        let payload = vec![0xEE; 128 * 1024];
        let plan = CarBuildPlan::single_file(&payload).expect("plan");
        let manifest = manifest_builder_for_plan(&payload, &plan)
            .root_cid(vec![0xDD; 8])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(&payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        handle
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");

        let randomness = PorRandomness {
            epoch_id: 42,
            issued_at_unix: 1_700_000_000,
            response_window_secs: 900,
            drand_round: 12345,
            drand_randomness: [0x33; 32],
            drand_signature: [0x44; 48],
        };

        let plans = handle
            .plan_por_challenges(randomness.clone(), &HashMap::new())
            .expect("forced challenge");
        assert_eq!(plans.len(), 1);
        let forced = &plans[0].challenge;
        assert!(forced.forced);
        assert!(forced.vrf_output.is_none());
        assert!(forced.sample_count > 0);
        assert_eq!(forced.sample_count, 128);

        let mut inert_randomness = randomness.clone();
        inert_randomness.drand_signature = [0; 48];
        assert!(matches!(
            handle.plan_por_challenges(inert_randomness, &HashMap::new()),
            Err(PorChallengePlannerError::InvalidDrandSignature)
        ));

        let mut vrf_records = HashMap::new();
        vrf_records.insert(
            ManifestVrfKey {
                provider_id: forced.provider_id,
                manifest_digest: forced.manifest_digest,
            },
            ManifestVrfBundle {
                provider_id: forced.provider_id,
                manifest_digest: forced.manifest_digest,
                epoch_id: randomness.epoch_id,
                drand_round: randomness.drand_round,
                output: [0x55; 32],
                proof: iroha_crypto::vrf::VrfProof::SigInG1([0x66; 48]),
            },
        );

        let plans_with_vrf = handle
            .plan_por_challenges(randomness.clone(), &vrf_records)
            .expect("vrf-backed challenge");
        let satisfied = &plans_with_vrf[0].challenge;
        assert!(!satisfied.forced);
        assert_eq!(satisfied.vrf_output, Some([0x55; 32]));
        assert_eq!(satisfied.sample_count, 128);
        assert!(matches!(
            satisfied.vrf_proof,
            Some(iroha_crypto::vrf::VrfProof::SigInG1(_))
        ));

        assert!(matches!(
            handle.plan_por_challenges_with_forced_policy(
                randomness.clone(),
                &HashMap::new(),
                false,
            ),
            Err(PorChallengePlannerError::MissingVrfBeforeDeadline { .. })
        ));
    }

    #[test]
    fn node_handle_plan_por_challenges_skips_expired_manifest() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let gc_actual = iroha_config::parameters::actual::SorafsGc {
            retention_grace_secs: 0,
            ..Default::default()
        };
        let handle =
            NodeHandle::new_with_policies(cfg, RepairConfig::default(), GcConfig::from(&gc_actual));

        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x22; 32],
            stake: sorafs_manifest::provider_advert::StakePointer {
                pool_id: [0xAA; 32],
                stake_amount: xor("1"),
            },
            committed_capacity_gib: 128,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 128,
                capability_refs: Vec::new(),
            }],
            lane_commitments: vec![LaneCommitmentV1 {
                lane_id: "default".into(),
                max_gib: 128,
            }],
            pricing: None,
            valid_from: 1,
            valid_until: 2,
            metadata: vec![],
        };
        let payload = to_bytes(&declaration).expect("encode declaration");
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(declaration.provider_id),
            payload,
            declaration.committed_capacity_gib,
            1,
            1,
            2,
            Metadata::default(),
        );
        handle
            .record_capacity_declaration(&record)
            .expect("record declaration");

        let now_unix = 1_700_000_000;
        let expired_manifest = build_manifest_with_retention(
            vec![0x01; 8],
            now_unix - 10,
            b"expired-por-manifest",
            &handle,
        );
        let active_manifest = build_manifest_with_retention(
            vec![0x02; 8],
            now_unix + 86_400,
            b"active-por-manifest",
            &handle,
        );

        let randomness = PorRandomness {
            epoch_id: 7,
            issued_at_unix: now_unix,
            response_window_secs: 900,
            drand_round: 777,
            drand_randomness: [0x55; 32],
            drand_signature: [0x66; 48],
        };

        let plans = handle
            .plan_por_challenges(randomness, &HashMap::new())
            .expect("plan por");
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].challenge.manifest_digest, active_manifest);
        assert_ne!(plans[0].challenge.manifest_digest, expired_manifest);
    }

    fn build_manifest_with_retention(
        cid: Vec<u8>,
        retention_epoch: u64,
        payload: &[u8],
        handle: &NodeHandle,
    ) -> [u8; 32] {
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let mut policy = PinPolicy::default();
        policy.retention_epoch = retention_epoch;
        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(cid)
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(policy)
            .build()
            .expect("manifest");
        let mut reader = payload;
        handle
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest");
        manifest.digest().expect("digest").into()
    }

    #[test]
    fn finalized_native_repair_rejects_stale_leases_and_deduplicates_after_restart() {
        use crate::{
            native_repair_worker::{NativeRepairExecutionErrorV1, NativeRepairTerminalKindV1},
            repair_transaction_forwarder::{
                RepairOperationV1, RepairTransactionContextV1, RepairTransactionEnqueueResultV1,
            },
        };
        use iroha_data_model::{
            ChainId,
            isi::sorafs::SorafsRepairTaskActionV1,
            sorafs::moderation_ledger::{
                REPAIR_LEDGER_TASK_VERSION_V1, RepairFinalizedCursorV1, RepairFinalizedTaskV1,
                RepairLedgerActionReceiptV1, RepairLedgerLeaseV1, RepairLedgerTaskV1,
                sorafs_repair_task_id_v1,
            },
        };

        let (cfg, _dir) = storage_config_with_temp_dir();
        let repair_actual = iroha_config::parameters::actual::SorafsRepair {
            enabled: true,
            ..Default::default()
        };
        let repair_config = RepairConfig::from(&repair_actual);
        let handle =
            NodeHandle::new_with_policies(cfg.clone(), repair_config.clone(), GcConfig::default());
        let provider_id = [0xC1; 32];
        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id,
            stake: sorafs_manifest::provider_advert::StakePointer {
                pool_id: [0xC2; 32],
                stake_amount: xor("1"),
            },
            committed_capacity_gib: 100,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".into(),
                profile_aliases: None,
                committed_gib: 100,
                capability_refs: Vec::new(),
            }],
            lane_commitments: vec![LaneCommitmentV1 {
                lane_id: "default".into(),
                max_gib: 100,
            }],
            pricing: None,
            valid_from: 1,
            valid_until: 10,
            metadata: Vec::new(),
        };
        let record = CapacityDeclarationRecord::new(
            ProviderId::new(provider_id),
            to_bytes(&declaration).expect("encode capacity declaration"),
            declaration.committed_capacity_gib,
            1,
            declaration.valid_from,
            declaration.valid_until,
            Metadata::default(),
        );
        handle
            .record_capacity_declaration(&record)
            .expect("bind local provider");

        let payload = b"finalized-native-repair-corrupt-chunk";
        let plan = CarBuildPlan::single_file(payload).expect("chunk plan");
        let build_manifest = |root_cid: Vec<u8>| {
            manifest_builder_for_plan(payload, &plan)
                .root_cid(root_cid)
                .dag_codec(DagCodecId(0x71))
                .chunking_from_profile(
                    sorafs_chunker::ChunkProfile::DEFAULT,
                    sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
                )
                .content_length(plan.content_length)
                .car_digest(blake3::hash(payload).into())
                .car_size(plan.content_length)
                .pin_policy(PinPolicy::default())
                .build()
                .expect("manifest")
        };
        let target_manifest = build_manifest(vec![0xC3; 16]);
        let source_manifest = build_manifest(vec![0xC4; 16]);
        let mut reader = payload.as_slice();
        handle
            .ingest_manifest(&target_manifest, &plan, &mut reader)
            .expect("ingest target manifest");
        let mut reader = payload.as_slice();
        handle
            .ingest_manifest(&source_manifest, &plan, &mut reader)
            .expect("ingest source manifest");
        let target_digest: [u8; 32] = target_manifest.digest().expect("target digest").into();
        let source_digest: [u8; 32] = source_manifest.digest().expect("source digest").into();
        let target = handle
            .manifest_metadata_by_digest(&target_digest)
            .expect("target metadata")
            .chunk(0)
            .expect("target chunk")
            .clone();
        let source = handle
            .manifest_metadata_by_digest(&source_digest)
            .expect("source metadata")
            .chunk(0)
            .expect("source chunk")
            .clone();
        assert_eq!(target.digest, source.digest);
        assert_ne!(target.path, source.path);
        let corrupt = vec![0xA5; target.length as usize];
        std::fs::write(&target.path, &corrupt).expect("corrupt target chunk");

        let authority_key =
            KeyPair::try_from_seed(vec![0xC5; 32], Algorithm::Ed25519).expect("authority key");
        let authority = AccountId::new(authority_key.public_key().clone());
        let other_key =
            KeyPair::try_from_seed(vec![0xC6; 32], Algorithm::Ed25519).expect("other key");
        let other = AccountId::new(other_key.public_key().clone());
        let ticket_id = RepairTicketId("REP-NATIVE-FINALIZED-1".to_owned());
        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: ticket_id.clone(),
            auditor_account: authority.to_string(),
            submitted_at_unix: 1,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest: target_digest,
                provider_id,
                por_history_id: None,
                cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                    reason: "corrupt chunk".to_owned(),
                }),
                evidence_json: None,
                notes: None,
            },
            notes: None,
        };
        let source_identity = [0xC7; 32];
        let finalized_cursor = RepairFinalizedCursorV1 {
            height: 7,
            block_hash: [0xC8; 32],
        };
        let finalized_task = RepairFinalizedTaskV1 {
            finalized_cursor,
            task: RepairLedgerTaskV1 {
                version: REPAIR_LEDGER_TASK_VERSION_V1,
                task_id: sorafs_repair_task_id_v1(source_identity),
                source_identity,
                ticket_id: ticket_id.0.clone(),
                canonical_report: to_bytes(&report).expect("encode canonical report"),
                manifest_digest: target_digest,
                provider_id,
                submitted_by: authority.clone(),
                submitted_at_unix_ms: 1_000,
                revision: 2,
                lease: Some(RepairLedgerLeaseV1 {
                    owner: authority.clone(),
                    generation: 1,
                    acquired_at_unix_ms: 1_000,
                    renewed_at_unix_ms: 1_000,
                    expires_at_unix_ms: 60_000,
                }),
                terminal_outcome: None,
                slash: None,
                appeal: None,
                action_receipts: vec![RepairLedgerActionReceiptV1 {
                    idempotency_digest: [0xD1; 32],
                    action_digest: [0xD2; 32],
                    resulting_revision: 2,
                }],
                updated_at_unix_ms: 1_000,
            },
        };
        let context = RepairTransactionContextV1 {
            chain_id: ChainId::from("native-repair-test-chain"),
            finalized_cursor,
        };
        let stale_context = RepairTransactionContextV1 {
            chain_id: context.chain_id.clone(),
            finalized_cursor: RepairFinalizedCursorV1 {
                height: 8,
                block_hash: [0xC9; 32],
            },
        };
        assert!(matches!(
            handle.execute_finalized_native_repair(
                &finalized_task,
                &authority,
                &stale_context,
                2_000,
            ),
            Err(NativeRepairExecutionErrorV1::StaleFinalizedCursor)
        ));
        assert_eq!(
            std::fs::read(&target.path).expect("read corrupt target"),
            corrupt
        );
        assert!(
            handle
                .pending_repair_transactions_after(None, 8)
                .expect("empty forwarder")
                .is_empty()
        );
        assert!(matches!(
            handle.execute_finalized_native_repair(&finalized_task, &other, &context, 2_000,),
            Err(NativeRepairExecutionErrorV1::LeaseOwnerMismatch)
        ));
        assert_eq!(
            std::fs::read(&target.path).expect("read corrupt target"),
            corrupt
        );
        let mut malformed_task = finalized_task.clone();
        malformed_task.task.action_receipts[0].resulting_revision = 3;
        assert!(matches!(
            handle.execute_finalized_native_repair(&malformed_task, &authority, &context, 2_000,),
            Err(NativeRepairExecutionErrorV1::InvalidFinalizedTask)
        ));
        assert_eq!(
            std::fs::read(&target.path).expect("malformed task performs no storage I/O"),
            corrupt
        );

        std::fs::write(&source.path, &corrupt).expect("make every local replica invalid");
        let orchestrator_calls = Arc::new(AtomicUsize::new(0));
        handle.set_repair_orchestrator(Arc::new(FailingRepairOrchestrator {
            calls: Arc::clone(&orchestrator_calls),
        }));
        assert!(matches!(
            handle.execute_finalized_native_repair(&finalized_task, &authority, &context, 2_000,),
            Err(NativeRepairExecutionErrorV1::Orchestrator(_))
        ));
        assert_eq!(orchestrator_calls.load(Ordering::Relaxed), 1);
        assert!(
            handle
                .pending_repair_transactions_after(None, 8)
                .expect("transient orchestrator failure enqueues no terminal action")
                .is_empty()
        );
        assert_eq!(
            std::fs::read(&target.path).expect("orchestrator failure leaves target retryable"),
            corrupt
        );
        handle.clear_repair_orchestrator();
        std::fs::write(&source.path, payload).expect("restore a valid local source replica");

        let first = handle
            .execute_finalized_native_repair(&finalized_task, &authority, &context, 2_000)
            .expect("execute exact finalized native lease");
        assert!(matches!(
            first.enqueue_result,
            RepairTransactionEnqueueResultV1::Inserted { .. }
        ));
        assert!(matches!(
            first.terminal_kind,
            NativeRepairTerminalKindV1::Complete { .. }
        ));
        assert_eq!(first.invalid_chunks_before, 1);
        assert_eq!(first.invalid_chunks_after, 0);
        assert_eq!(
            blake3::hash(&std::fs::read(&target.path).expect("read restored target")).as_bytes(),
            &target.digest
        );

        let replay = handle
            .execute_finalized_native_repair(&finalized_task, &authority, &context, 2_001)
            .expect("deduplicate exact terminal operation");
        assert_eq!(replay.operation_id, first.operation_id);
        assert!(matches!(
            replay.enqueue_result,
            RepairTransactionEnqueueResultV1::Existing { .. }
        ));
        let request = handle
            .repair_transaction_operation_for_reconciliation(first.operation_id)
            .expect("read exact native terminal operation");
        assert_eq!(request.chain_id, context.chain_id);
        assert_eq!(request.authority, authority);
        assert!(matches!(
            request.operation,
            RepairOperationV1::Action(ref instruction)
                if matches!(
                    instruction.action(),
                    SorafsRepairTaskActionV1::Complete(action)
                        if action.lease_generation == 1
                )
        ));
        drop(handle);

        let restored = NodeHandle::new_with_policies(cfg, repair_config, GcConfig::default());
        let pending = restored
            .pending_repair_transactions_after(None, 8)
            .expect("restore durable native terminal operation");
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].operation_id, first.operation_id);
        assert_eq!(pending[0].chain_id, context.chain_id);
    }

    #[test]
    fn node_handle_storage_methods_error_when_disabled() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);

        let payload = b"disabled storage payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let manifest = manifest_builder_for_plan(payload, &plan)
            .root_cid(vec![0xCC; 8])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan.content_length)
            .car_digest(blake3::hash(payload).into())
            .car_size(plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");

        let mut reader = &payload[..];
        let err = handle
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect_err("storage disabled");
        assert!(matches!(err, NodeStorageError::Disabled));
    }

    #[derive(Debug)]
    struct FailingRepairOrchestrator {
        calls: Arc<AtomicUsize>,
    }

    impl RepairOrchestrator for FailingRepairOrchestrator {
        fn rehydrate_missing_chunks(
            &self,
            _context: &native_repair_worker::NativeRepairExecutionContextV1,
            _manifest: &StoredManifest,
            _missing_chunks: &[ChunkFileRecord],
        ) -> Result<Vec<RepairChunkPayload>, RepairOrchestratorError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Err(RepairOrchestratorError::other(
                "simulated transient remote provider outage",
            ))
        }
    }

    #[derive(Debug, Default)]
    struct RecordingPublisher {
        payloads: Mutex<Vec<Vec<u8>>>,
    }

    impl RecordingPublisher {
        fn take(&self) -> Vec<Vec<u8>> {
            let mut guard = self.payloads.lock().expect("publisher lock poisoned");
            guard.drain(..).collect()
        }
    }

    impl GovernancePublisher for RecordingPublisher {
        fn publish_deal_settlement(
            &self,
            _settlement: &DealSettlementV1,
            encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.payloads.lock().expect("publisher lock poisoned");
            guard.push(encoded.to_vec());
            Ok(())
        }

        fn publish_pdp_archive(
            &self,
            _archive: &PdpGovernanceArchiveV1,
            encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.payloads.lock().expect("publisher lock poisoned");
            guard.push(encoded.to_vec());
            Ok(())
        }

        fn publish_gc_audit_event(
            &self,
            _event: &GcAuditEventV1,
            encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.payloads.lock().expect("publisher lock poisoned");
            guard.push(encoded.to_vec());
            Ok(())
        }

        fn publish_reconciliation_report(
            &self,
            _report: &SorafsReconciliationReportV1,
            encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.payloads.lock().expect("publisher lock poisoned");
            guard.push(encoded.to_vec());
            Ok(())
        }

        fn publish_reputation_snapshot(
            &self,
            _snapshot: &SignedReputationSnapshotV1,
            encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.payloads.lock().expect("publisher lock poisoned");
            guard.push(encoded.to_vec());
            Ok(())
        }

        fn publish_moderation_ballot_event(
            &self,
            _event: &SoraFsModerationBallotGovernanceEventV1,
            encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.payloads.lock().expect("publisher lock poisoned");
            guard.push(encoded.to_vec());
            Ok(())
        }

        fn publish_transparency_ledger_publication(
            &self,
            _publication: &ModerationLedgerCyclePublicationV1,
            encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.payloads.lock().expect("publisher lock poisoned");
            guard.push(encoded.to_vec());
            Ok(())
        }

        fn publish_proof_token_issuance(
            &self,
            _issuance: &ProofTokenIssuanceV1,
            encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.payloads.lock().expect("publisher lock poisoned");
            guard.push(encoded.to_vec());
            Ok(())
        }

        fn publish_appeal_finance_report(
            &self,
            _report: &SoraFsAppealFinanceReportV1,
            encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.payloads.lock().expect("publisher lock poisoned");
            guard.push(encoded.to_vec());
            Ok(())
        }

        fn publish_appeal_finance_weekly_rollup(
            &self,
            _rollup: &SoraFsAppealFinanceWeeklyRollupV1,
            encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.payloads.lock().expect("publisher lock poisoned");
            guard.push(encoded.to_vec());
            Ok(())
        }

        fn publish_appeal_finance_settlement_receipt(
            &self,
            _receipt: &SoraFsAppealFinanceSettlementReceiptV1,
            encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.payloads.lock().expect("publisher lock poisoned");
            guard.push(encoded.to_vec());
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    struct FailingPublisher {
        attempts: Mutex<usize>,
    }

    impl FailingPublisher {
        fn attempts(&self) -> usize {
            *self.attempts.lock().expect("publisher lock poisoned")
        }
    }

    impl GovernancePublisher for FailingPublisher {
        fn publish_deal_settlement(
            &self,
            _settlement: &DealSettlementV1,
            _encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.attempts.lock().expect("publisher lock poisoned");
            *guard += 1;
            Err(GovernancePublishError::other("simulated publish failure"))
        }

        fn publish_pdp_archive(
            &self,
            _archive: &PdpGovernanceArchiveV1,
            _encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.attempts.lock().expect("publisher lock poisoned");
            *guard += 1;
            Err(GovernancePublishError::other("simulated publish failure"))
        }

        fn publish_gc_audit_event(
            &self,
            _event: &GcAuditEventV1,
            _encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.attempts.lock().expect("publisher lock poisoned");
            *guard += 1;
            Err(GovernancePublishError::other("simulated publish failure"))
        }

        fn publish_reconciliation_report(
            &self,
            _report: &SorafsReconciliationReportV1,
            _encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.attempts.lock().expect("publisher lock poisoned");
            *guard += 1;
            Err(GovernancePublishError::other("simulated publish failure"))
        }

        fn publish_reputation_snapshot(
            &self,
            _snapshot: &SignedReputationSnapshotV1,
            _encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.attempts.lock().expect("publisher lock poisoned");
            *guard += 1;
            Err(GovernancePublishError::other("simulated publish failure"))
        }

        fn publish_moderation_ballot_event(
            &self,
            _event: &SoraFsModerationBallotGovernanceEventV1,
            _encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.attempts.lock().expect("publisher lock poisoned");
            *guard += 1;
            Err(GovernancePublishError::other("simulated publish failure"))
        }

        fn publish_transparency_ledger_publication(
            &self,
            _publication: &ModerationLedgerCyclePublicationV1,
            _encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.attempts.lock().expect("publisher lock poisoned");
            *guard += 1;
            Err(GovernancePublishError::other("simulated publish failure"))
        }

        fn publish_proof_token_issuance(
            &self,
            _issuance: &ProofTokenIssuanceV1,
            _encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.attempts.lock().expect("publisher lock poisoned");
            *guard += 1;
            Err(GovernancePublishError::other("simulated publish failure"))
        }

        fn publish_appeal_finance_report(
            &self,
            _report: &SoraFsAppealFinanceReportV1,
            _encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.attempts.lock().expect("publisher lock poisoned");
            *guard += 1;
            Err(GovernancePublishError::other("simulated publish failure"))
        }

        fn publish_appeal_finance_weekly_rollup(
            &self,
            _rollup: &SoraFsAppealFinanceWeeklyRollupV1,
            _encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.attempts.lock().expect("publisher lock poisoned");
            *guard += 1;
            Err(GovernancePublishError::other("simulated publish failure"))
        }

        fn publish_appeal_finance_settlement_receipt(
            &self,
            _receipt: &SoraFsAppealFinanceSettlementReceiptV1,
            _encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.attempts.lock().expect("publisher lock poisoned");
            *guard += 1;
            Err(GovernancePublishError::other("simulated publish failure"))
        }
    }
}
