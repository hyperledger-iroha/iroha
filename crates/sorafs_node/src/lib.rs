//! SoraFS node scaffolding.

#![deny(missing_docs)]
#![allow(
    clippy::clone_on_copy,
    clippy::collapsible_if,
    clippy::field_reassign_with_default
)]

pub mod capacity;
pub mod config;
pub mod deal;
pub mod gateway;
mod governance;
pub mod metering;
mod moderation;
mod orderbook;
pub mod por;
pub mod potr;
mod reconciliation;
pub mod repair;
mod reserve;
pub mod scheduler;
pub mod store;
pub mod telemetry;
mod transparency;

pub use deal::{
    ClientSnapshot, DealEngine, DealEngineError, DealSettlementOutcome, DealSnapshot,
    ProviderSnapshot, UsageOutcome,
};
pub use moderation::{
    ModerationAppealDeposit, ModerationBallotAnnouncement, ModerationBallotChallengeDecision,
    ModerationBallotChallengeInput, ModerationBallotChallengeKind, ModerationBallotChallengeRecord,
    ModerationBallotChallengeResolution, ModerationBallotCommitOutcome, ModerationBallotEvent,
    ModerationBallotEventKind, ModerationBallotNoShowPlan, ModerationBallotRecord,
    ModerationBallotRevealOutcome, ModerationBallotRuntimeError, ModerationBallotSnapshot,
    ModerationBallotTally, ModerationCorpusRegistryRecord,
    ModerationEvidenceViewerAccessEventRecord, ModerationEvidenceViewerAccessInput,
    ModerationEvidenceViewerAccessKind, ModerationEvidenceViewerAuditKindCount,
    ModerationEvidenceViewerAuditReport, ModerationEvidenceViewerAuditReportInput,
    ModerationEvidenceViewerError, ModerationEvidenceViewerSessionInput,
    ModerationEvidenceViewerSessionRecord, ModerationEvidenceViewerSnapshot,
    ModerationModelRegistryError, ModerationModelRegistrySnapshot, ModerationQuarantineObjectError,
    ModerationQuarantineObjectInput, ModerationQuarantineObjectPayload,
    ModerationQuarantineObjectRecord, ModerationQuarantineObjectSnapshot,
    ModerationQuarantineRecord, ModerationQuarantineReleaseInput, ModerationQuarantineReviewInput,
    ModerationQuarantineState, ModerationReproRegistryRecord, ModerationScreeningError,
    ModerationScreeningInput, ModerationScreeningOutcome, ModerationScreeningRecord,
    ModerationScreeningSnapshot, ModerationScreeningVerdict, ModerationVoteCounts,
    local_moderation_panel_roster_hash,
};
pub use orderbook::{
    OrderbookBuyerSettlementLedgerEntry, OrderbookCancelOutcome, OrderbookEvent,
    OrderbookEventKind, OrderbookProviderSettlementLedgerEntry, OrderbookReceiptOutcome,
    OrderbookRuntimeError, OrderbookSettlementLedger, OrderbookSnapshot, OrderbookSubmitOutcome,
    local_orderbook_provider_id_for_owner_account,
};
pub use por::{
    ManifestVrfBundle, ManifestVrfKey, PlannedChallenge, PorChallengePlannerError, PorRandomness,
    PorTracker, PorTrackerError, PorVerdictStats, build_por_challenge_for_manifest,
};
pub use reserve::{
    ReserveAppealDecision, ReserveAppealOutcome, ReserveAppealRecord, ReserveAppealRequest,
    ReserveAppealRuntimeError, ReserveAppealSnapshot, ReserveAppealStatus,
    ReserveCreditLineSnapshot, ReserveLifecycleEvent, ReserveLifecyclePolicyOutcome,
    ReserveLifecyclePolicyRecord, ReserveLifecyclePolicySnapshot, ReserveLifecyclePolicyUpdate,
    ReserveLifecycleRuntimeError, ReserveLifecycleSnapshot, ReserveLifecycleUpdate,
    ReserveMovementCustodyStatus, ReserveMovementCustodyUpdate, ReserveMovementKind,
    ReserveMovementOutcome, ReserveMovementRecord, ReserveMovementRequest,
    ReserveMovementRuntimeError, ReserveMovementSnapshot, ReserveProviderBalance,
    ReserveProviderCreditLineState, ReserveProviderLifecycleSummary,
};

/// Outcome returned when recording a PoR verdict.
#[derive(Debug, Clone)]
pub struct PorVerdictOutcome {
    /// Statistics extracted from the verdict.
    pub stats: PorVerdictStats,
    /// Identifier that can be referenced by repair reports (present on failure).
    pub repair_history_id: Option<u64>,
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

#[derive(Debug, Clone, Copy)]
enum GcEvictionPolicy {
    RetentionEpoch,
    LruExpired,
}

const GOVERNANCE_PUBLISH_INDEX_FILE: &str = "publish-index.json";
const GOVERNANCE_PUBLISH_INDEX_SCHEMA: &str = "sorafs.governance_dag.local_publish_index.v1";
const APPEAL_FINANCE_WEEKLY_ROLLUP_KIND: &str = "appeal_finance_weekly_rollup";
const ORDERBOOK_STATE_DIR: &str = "orderbook";
const ORDERBOOK_RUNTIME_SNAPSHOT_FILE: &str = "runtime-snapshot.to";
const ORDERBOOK_RUNTIME_STATE_VERSION_V1: u8 = 1;
const MODERATION_MODEL_REGISTRY_DIR: &str = "moderation-model-registry";
const MODERATION_MODEL_REGISTRY_SNAPSHOT_FILE: &str = "registry-snapshot.to";
const MODERATION_SCREENING_DIR: &str = "moderation-screening";
const MODERATION_SCREENING_SNAPSHOT_FILE: &str = "screening-snapshot.to";
const MODERATION_QUARANTINE_OBJECT_STORE_DIR: &str = "moderation-quarantine-objects";
const MODERATION_QUARANTINE_OBJECT_INDEX_FILE: &str = "object-index.to";
const MODERATION_QUARANTINE_OBJECT_KEY_FILE: &str = "local-seal.key";
const MODERATION_QUARANTINE_OBJECT_STORE_MAX_DEPTH: usize = 4;
const MODERATION_EVIDENCE_VIEWER_DIR: &str = "moderation-evidence-viewer";
const MODERATION_EVIDENCE_VIEWER_SNAPSHOT_FILE: &str = "evidence-viewer-snapshot.to";
const MODERATION_BALLOT_DIR: &str = "moderation-ballots";
const MODERATION_BALLOT_SNAPSHOT_FILE: &str = "ballots-snapshot.to";
const AUX_RUNTIME_STATE_DIR: &str = "runtime-state";
const AUX_RUNTIME_STATE_SNAPSHOT_FILE: &str = "auxiliary-snapshot.to";
const RUNTIME_STATE_INITIALIZATION_FILE: &str = "initialized-v1";
const RUNTIME_STATE_INITIALIZATION_BYTES: &[u8] = b"sorafs.node.runtime-state.initialized.v1\n";
const AUX_RUNTIME_STATE_VERSION_V1: u8 = 1;
const LOCAL_RUNTIME_SNAPSHOT_TMP_EXT: &str = "tmp";
const EVIDENCE_VIEWER_AUDIT_CYCLE_ID_DOMAIN_V1: &[u8] =
    b"sorafs.node.moderation.evidence_viewer_audit.cycle_id.v1";
const PRIVACY_AGGREGATE_ENTRY_ID_DOMAIN_V1: &[u8] =
    b"sorafs.node.transparency.privacy_aggregate.entry_id.v1";

#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque, hash_map::Entry},
    fs,
    io::{self, ErrorKind, Read, Write},
    num::NonZeroU64,
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use capacity::{
    CapacityError, CapacityManager, CapacityUsageSnapshot, DeclarationWindow, ReplicationPlan,
    ReplicationRelease,
};
use config::{GcConfig, OrderbookAdmissionPolicy, RepairConfig, StorageConfig};
use iroha_crypto::{
    Hash,
    numeric::{Numeric, RoundingMode},
};
use iroha_data_model::{
    da::ingest::DaStripeLayout,
    sorafs::{
        capacity::{CapacityDeclarationRecord, ProviderId},
        deal::{ClientId, DealId, DealProposal, DealRecord, DealUsageReport},
        gar::GarEnforcementReceiptV1,
        moderation::{
            AdversarialCorpusManifestV1, ModerationReproManifestV1, SoraFsModerationBallotCommitV1,
            SoraFsModerationBallotRevealV1, SoraFsModerationVoteChoice,
        },
        reserve::ReserveLifecycleStage,
        transparency::{
            ModerationLedgerCyclePublicationV1, ModerationLedgerEntryV1,
            ModerationLedgerMetadataV1, ModerationPrivacyAggregateV1, ProofTokenIssuanceV1,
        },
    },
};
use iroha_telemetry::metrics::{
    MicropaymentCreditSnapshot, MicropaymentTicketCounters, global_or_default,
    global_sorafs_gc_otel, global_sorafs_node_otel, global_sorafs_reconciliation_otel,
};
use norito::codec::Encode;
use norito::derive::{NoritoDeserialize, NoritoSerialize};
use norito::json::Value as JsonValue;
use rand::{rand_core::TryRngCore as _, rngs::OsRng};
pub use repair::{
    RepairManager, RepairSchedulerError, RepairTaskFilters, RepairTaskSnapshot,
    RepairWatchdogReport, RepairWorkerReport,
};
use reserve::ReserveLifecycleRuntime;
use sorafs_car::{CarBuildPlan, PorProof};
use sorafs_manifest::{
    AppealFinanceReconciliationSummaryV1, ManifestV1, OrderCancelV1, OrderRequestV1, OrderSideV1,
    OrderTierV1, OrderbookRuntimeSnapshotV1, REPUTATION_PROVIDER_INPUT_VERSION_V1,
    REPUTATION_PROVIDER_METRICS_VERSION_V1, ReconciliationValidationError,
    ReputationProviderInputV1, ReputationProviderMetricsV1, ReputationReserveStageV1,
    ReputationSnapshotEventV1, ReputationSnapshotV1, ReputationWeightsV1,
    SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1, SORAFS_RECONCILIATION_REPORT_VERSION_V1,
    SettlementChannelStatusV1, SettlementReceiptV1, SoraFsAppealFinanceAccountFlowV1,
    SoraFsAppealFinanceJurorPayoutV1, SoraFsAppealFinanceOutcomeV1, SoraFsAppealFinanceReportV1,
    SoraFsAppealFinanceSettlementReceiptV1, SoraFsAppealFinanceWeeklyRollupV1,
    SoraFsModerationBallotGovernanceEventV1, SorafsReconciliationReportV1,
    capacity::{CapacityTelemetryV1, ReplicationOrderV1},
    deal::{DealSettlementStatusV1, DealSettlementV1, XorQuantity},
    por::{AuditOutcomeV1, AuditVerdictV1, PorChallengeV1, PorProofV1},
    potr::{PotrReceiptV1, PotrReceiptValidationError},
    proof_stream::ProofStreamTier,
    repair::{
        GC_AUDIT_EVENT_VERSION_V1, GC_AUDIT_PAYLOAD_VERSION_V1, GcAuditEventV1, GcAuditPayloadV1,
        REPAIR_AUDIT_EVENT_VERSION_V1, RepairAuditEventV1, RepairReportV1, RepairSlashProposalV1,
        RepairTaskEventV1, RepairTaskRecordV1, RepairTaskStateV1, RepairTicketId,
        SorafsAuditHeaderV1,
    },
    score_provider_reputation,
};
use thiserror::Error;
use tokio::sync::broadcast;
pub use transparency::{
    PrivacyAggregateCycleConfig, PrivacyAggregateCycleWindow, PrivacyAggregateScheduleConfig,
    PrivacyAggregateSourceEvent, PrivacyAggregateSourceMetric, ProofTokenIssuanceIngestError,
    TransparencyLedgerIngestError, TransparencyLedgerSourceEntry,
    TransparencySourceEntryAdapterError, appeal_finance_report_source_entry,
    appeal_finance_settlement_receipt_source_entry, gar_enforcement_receipt_source_entry,
    moderation_ballot_governance_event_source_entry,
    moderation_evidence_viewer_audit_report_source_entry, proof_token_issuance_from_base64,
    proof_token_issuance_from_frame, reserve_appeal_source_entry,
    reserve_lifecycle_event_source_entry, reserve_lifecycle_policy_source_entry,
    reserve_movement_source_entry,
};

use crate::{
    capacity::CapacityRuntimeCheckpointV1,
    deal::DealRuntimeCheckpointV1,
    governance::FilesystemGovernancePublisher,
    metering::{CapacityMeter, MeteringSnapshot, ReplicationUsageSample},
    moderation::{
        ModerationBallotRuntime, ModerationEvidenceViewerRuntime, ModerationModelRegistry,
        ModerationQuarantineObjectEnvelopeV1, ModerationQuarantineObjectRuntime,
        ModerationScreeningRuntime, open_moderation_quarantine_object,
        seal_moderation_quarantine_object, validate_relative_object_path,
    },
    orderbook::OrderbookRuntime,
    potr::PotrTracker,
    reserve::ReserveRuntimeCheckpointV1,
    scheduler::{SchedulerAdmissionError, StorageSchedulerConfig, StorageSchedulersRuntime},
    store::{ChunkFileRecord, ChunkRoleMetadata, StorageBackend, StorageError, StoredManifest},
    telemetry::{TelemetryAccumulator, TelemetryError},
};

/// Stage for a repair slash proposal within the governance pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairSlashStage {
    /// Proposal drafted by the scheduler for review.
    Drafted,
    /// Proposal submitted by an auditor for governance action.
    Submitted,
}

impl RepairSlashStage {
    fn as_str(self) -> &'static str {
        match self {
            Self::Drafted => "drafted",
            Self::Submitted => "submitted",
        }
    }
}

fn repair_idempotency_key(
    action: &str,
    worker_id: &str,
    ticket_id: &RepairTicketId,
    now_unix: u64,
) -> String {
    let raw = format!("{action}:{worker_id}:{ticket_id}:{now_unix}");
    let digest_hex = blake3::hash(raw.as_bytes()).to_hex().to_string();
    format!("{action}-{digest_hex}")
}

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

fn orderbook_side_label(side: OrderSideV1) -> &'static str {
    match side {
        OrderSideV1::Bid => "bid",
        OrderSideV1::Ask => "ask",
    }
}

fn orderbook_tier_label(tier: OrderTierV1) -> &'static str {
    match tier {
        OrderTierV1::Hot => "hot",
        OrderTierV1::Warm => "warm",
        OrderTierV1::Archive => "archive",
    }
}

fn reserve_lifecycle_stage_metric_label(stage: ReserveLifecycleStage) -> &'static str {
    match stage {
        ReserveLifecycleStage::Active => "active",
        ReserveLifecycleStage::Warning => "warning",
        ReserveLifecycleStage::Grace => "grace",
        ReserveLifecycleStage::Delinquent => "delinquent",
        ReserveLifecycleStage::Default => "default",
    }
}

fn reserve_lifecycle_stage_to_reputation(stage: ReserveLifecycleStage) -> ReputationReserveStageV1 {
    match stage {
        ReserveLifecycleStage::Active => ReputationReserveStageV1::Active,
        ReserveLifecycleStage::Warning => ReputationReserveStageV1::Warning,
        ReserveLifecycleStage::Grace => ReputationReserveStageV1::Grace,
        ReserveLifecycleStage::Delinquent => ReputationReserveStageV1::Delinquent,
        ReserveLifecycleStage::Default => ReputationReserveStageV1::Default,
    }
}

const fn reserve_reputation_baseline_metrics() -> ReputationProviderMetricsV1 {
    ReputationProviderMetricsV1 {
        version: REPUTATION_PROVIDER_METRICS_VERSION_V1,
        por_success_bps: 10_000,
        pdp_success_bps: 10_000,
        potr_success_bps: 10_000,
        latency_health_bps: 10_000,
        dispute_rate_bps: 0,
        token_violation_rate_bps: 0,
        repair_breach_rate_bps: 0,
    }
}

fn collect_due_evidence_viewer_audit_window(
    schedule: &PrivacyAggregateScheduleConfig,
    latest_window: PrivacyAggregateCycleWindow,
    published_cycles: &BTreeSet<[u8; 16]>,
    timestamp_unix_ms: u64,
    due_windows: &mut BTreeSet<PrivacyAggregateCycleWindow>,
) -> Result<(), GovernancePublishError> {
    let timestamp_unix = timestamp_unix_ms / 1_000;
    let Some(window) = schedule.event_window(timestamp_unix).map_err(|err| {
        GovernancePublishError::other(format!("evidence viewer audit schedule: {err}"))
    })?
    else {
        return Ok(());
    };
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

fn reserve_movement_kind_metric_label(kind: ReserveMovementKind) -> &'static str {
    match kind {
        ReserveMovementKind::TopUp => "top_up",
        ReserveMovementKind::Withdrawal => "withdrawal",
    }
}

fn validate_orderbook_admission_policy(
    policy: &OrderbookAdmissionPolicy,
    order: &OrderRequestV1,
) -> Result<(), OrderbookRuntimeError> {
    if order.quantity_gib < policy.min_order_gib() {
        return Err(OrderbookRuntimeError::OrderBelowMinimum {
            quantity_gib: order.quantity_gib,
            min_order_gib: policy.min_order_gib(),
        });
    }
    let tick = XorQuantity::from_quantity(policy.price_tick().clone());
    let aligned = order
        .price_per_gib
        .as_quantity()
        .try_div_decimal_exact(tick.as_quantity().as_numeric())
        .is_ok_and(|quotient| quotient.scale() == 0);
    if !aligned {
        return Err(OrderbookRuntimeError::OrderPriceTickMismatch {
            price: order.price_per_gib.clone(),
            tick,
        });
    }
    Ok(())
}

fn validate_orderbook_reserve_lifecycle_admission(
    reserve_lifecycle: &RwLock<ReserveLifecycleRuntime>,
    order: &OrderRequestV1,
) -> Result<(), OrderbookRuntimeError> {
    if order.side != OrderSideV1::Ask {
        return Ok(());
    }
    let provider_id = local_orderbook_provider_id_for_owner_account(&order.owner_account);
    let Some(summary) = reserve_lifecycle
        .read()
        .map_err(|_| OrderbookRuntimeError::StateLockPoisoned)?
        .provider_summary(provider_id)
    else {
        return Ok(());
    };
    if summary.lifecycle.disable_adverts {
        return Err(OrderbookRuntimeError::ReserveLifecycleAdvertDisabled {
            provider_id_hex: hex::encode(provider_id),
            stage: reserve_lifecycle_stage_metric_label(summary.lifecycle.stage).to_owned(),
        });
    }
    Ok(())
}

fn orderbook_runtime_snapshot_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(ORDERBOOK_STATE_DIR)
        .join(ORDERBOOK_RUNTIME_SNAPSHOT_FILE)
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

fn moderation_quarantine_object_key_path(data_dir: &Path) -> PathBuf {
    moderation_quarantine_object_store_root(data_dir).join(MODERATION_QUARANTINE_OBJECT_KEY_FILE)
}

fn moderation_evidence_viewer_checkpoint_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(MODERATION_EVIDENCE_VIEWER_DIR)
        .join(MODERATION_EVIDENCE_VIEWER_SNAPSHOT_FILE)
}

fn moderation_ballot_checkpoint_path(data_dir: &Path) -> PathBuf {
    data_dir
        .join(MODERATION_BALLOT_DIR)
        .join(MODERATION_BALLOT_SNAPSHOT_FILE)
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

fn required_runtime_checkpoint_paths(data_dir: &Path) -> [(&'static str, PathBuf); 7] {
    [
        (
            "orderbook runtime",
            orderbook_runtime_snapshot_path(data_dir),
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
            "moderation ballot",
            moderation_ballot_checkpoint_path(data_dir),
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
                    "marker contents are not canonical for runtime-state v1",
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

    fn kind(&self) -> ErrorKind {
        self.error.kind()
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

const REPAIR_EVENT_CHANNEL_CAPACITY: usize = 128;
const REPUTATION_EVENT_CHANNEL_CAPACITY: usize = 128;
const ORDERBOOK_EVENT_CHANNEL_CAPACITY: usize = 128;
const RESERVE_LIFECYCLE_EVENT_CHANNEL_CAPACITY: usize = 128;
const RESERVE_MOVEMENT_EVENT_CHANNEL_CAPACITY: usize = 128;
const MODERATION_BALLOT_EVENT_CHANNEL_CAPACITY: usize = 128;
const ORDERBOOK_METRIC_CLUSTER_LOCAL: &str = "local";

fn repair_task_terminal(task: &RepairTaskRecordV1) -> bool {
    matches!(
        task.state,
        RepairTaskStateV1::Completed(_) | RepairTaskStateV1::Escalated(_)
    )
}

#[derive(Debug, Default)]
struct RepairRehydrateOutcome {
    missing_before: usize,
    missing_after: usize,
    rehydrated: usize,
    errors: usize,
}

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

fn moderation_appeal_finance_report(
    record: &ModerationBallotRecord,
    tally: &ModerationBallotTally,
) -> Result<Option<SoraFsAppealFinanceReportV1>, String> {
    let Some(deposit) = &record.announcement.appeal_deposit else {
        return Ok(None);
    };
    let outcome = moderation_tally_finance_outcome(tally);
    let (refund, treasury_deposit, held) =
        appeal_finance_deposit_flows(&deposit.deposit_xor, outcome)?;
    let panel_size = u32::try_from(record.announcement.juror_ids.len())
        .map_err(|_| "moderation panel size exceeds report limits".to_string())?;
    if panel_size == 0 {
        return Err("moderation panel size must be non-zero".to_string());
    }

    let attending = record
        .reveals
        .iter()
        .map(|reveal| reveal.juror_id.clone())
        .collect::<BTreeSet<_>>();
    if attending.is_empty() {
        return Err("moderation tally has no attending jurors".to_string());
    }
    let no_show_juror_ids = record
        .announcement
        .juror_ids
        .iter()
        .filter(|juror_id| !attending.contains(*juror_id))
        .cloned()
        .collect::<Vec<_>>();

    let stipend = "25".parse::<XorQuantity>().expect("static XOR stipend");
    let bonus = "10".parse::<XorQuantity>().expect("static XOR bonus");
    let panel_reward_total = stipend
        .checked_mul_u64(u64::from(panel_size))
        .and_then(|value| value.checked_add(&bonus))
        .map_err(|error| format!("panel reward total arithmetic failed: {error}"))?;
    let attending_count = u64::try_from(attending.len())
        .map_err(|_| "attending juror count exceeds report limits".to_string())?;
    let attending_divisor = NonZeroU64::new(attending_count)
        .ok_or_else(|| "moderation tally has no attending jurors".to_string())?;
    // V1 juror payouts settle at six XOR fractional digits; any undistributed
    // remainder is retained by the treasury rather than silently saturated.
    let bonus_share = bonus
        .checked_div_u64_round(attending_divisor, 6, RoundingMode::TowardZero)
        .map_err(|error| format!("juror bonus division failed: {error}"))?;
    let payout_total = stipend
        .checked_add(&bonus_share)
        .map_err(|error| format!("juror payout total arithmetic failed: {error}"))?;
    let rewards_paid_total = payout_total
        .checked_mul_u64(attending_count)
        .map_err(|error| format!("paid reward total arithmetic failed: {error}"))?;
    let rewards_forfeited = panel_reward_total
        .checked_sub(&rewards_paid_total)
        .map_err(|error| format!("forfeited reward arithmetic failed: {error}"))?;
    let treasury_total = treasury_deposit
        .checked_add(&rewards_forfeited)
        .map_err(|error| format!("treasury total arithmetic failed: {error}"))?;

    let report_id = moderation_appeal_finance_report_id(record, tally, deposit, outcome);
    let juror_payouts = attending
        .into_iter()
        .map(|juror_id| SoraFsAppealFinanceJurorPayoutV1 {
            juror_id,
            stipend_xor: stipend.clone(),
            bonus_xor: bonus_share.clone(),
            total_xor: payout_total.clone(),
        })
        .collect::<Vec<_>>();

    let report = SoraFsAppealFinanceReportV1 {
        version: SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
        report_id,
        case_id: tally.case_id.clone(),
        round_id: Some(tally.round_id.clone()),
        generated_at_unix_ms: tally.tallied_at_unix_ms,
        appeal_finance_config_version: record
            .announcement
            .context
            .appeal_finance_config_version
            .clone(),
        evidence_bundle_digest: Some(record.announcement.context.evidence_bundle_digest),
        outcome,
        deposit_xor: deposit.deposit_xor.clone(),
        refund: SoraFsAppealFinanceAccountFlowV1 {
            account_id: deposit.payer_account.clone(),
            amount_xor: refund,
        },
        treasury: SoraFsAppealFinanceAccountFlowV1 {
            account_id: deposit.destination_account.clone(),
            amount_xor: treasury_total,
        },
        held: SoraFsAppealFinanceAccountFlowV1 {
            account_id: deposit.custody_account.clone(),
            amount_xor: held,
        },
        panel_size,
        panel_reward_total_xor: panel_reward_total,
        rewards_paid_total_xor: rewards_paid_total,
        rewards_forfeited_treasury_xor: rewards_forfeited,
        juror_payouts,
        no_show_juror_ids,
    };
    report
        .validate()
        .map_err(|err| format!("moderation-derived appeal finance report is invalid: {err}"))?;
    Ok(Some(report))
}

fn moderation_tally_finance_outcome(tally: &ModerationBallotTally) -> SoraFsAppealFinanceOutcomeV1 {
    match tally.winning_choice {
        Some(SoraFsModerationVoteChoice::Uphold) => SoraFsAppealFinanceOutcomeV1::Uphold,
        Some(SoraFsModerationVoteChoice::Overturn) => SoraFsAppealFinanceOutcomeV1::Overturn,
        Some(SoraFsModerationVoteChoice::Modify) => SoraFsAppealFinanceOutcomeV1::Modify,
        Some(SoraFsModerationVoteChoice::Escalate) | None => {
            SoraFsAppealFinanceOutcomeV1::Escalated
        }
    }
}

fn appeal_finance_deposit_flows(
    deposit: &XorQuantity,
    outcome: SoraFsAppealFinanceOutcomeV1,
) -> Result<(XorQuantity, XorQuantity, XorQuantity), String> {
    let (refund_numerator, treasury_numerator, denominator) = match outcome {
        SoraFsAppealFinanceOutcomeV1::Overturn | SoraFsAppealFinanceOutcomeV1::Modify => {
            (1_u64, 0_u64, 1_u64)
        }
        SoraFsAppealFinanceOutcomeV1::Uphold
        | SoraFsAppealFinanceOutcomeV1::WithdrawnAfterPanel => (0, 1, 1),
        SoraFsAppealFinanceOutcomeV1::WithdrawnBeforePanel => (9, 0, 10),
        SoraFsAppealFinanceOutcomeV1::Frivolous => (1, 1, 2),
        SoraFsAppealFinanceOutcomeV1::Escalated => (0, 0, 1),
    };
    let refund = deposit
        .as_quantity()
        .try_mul_decimal(&Numeric::from(refund_numerator))
        .and_then(|value| value.try_div_decimal_exact(&Numeric::from(denominator)))
        .map(XorQuantity::from_quantity)
        .map_err(|error| format!("refund calculation failed: {error}"))?;
    let treasury = deposit
        .as_quantity()
        .try_mul_decimal(&Numeric::from(treasury_numerator))
        .and_then(|value| value.try_div_decimal_exact(&Numeric::from(denominator)))
        .map(XorQuantity::from_quantity)
        .map_err(|error| format!("treasury calculation failed: {error}"))?;
    let held = deposit
        .checked_sub(&refund)
        .and_then(|remaining| remaining.checked_sub(&treasury))
        .map_err(|error| format!("held deposit calculation failed: {error}"))?;
    Ok((refund, treasury, held))
}

fn moderation_appeal_finance_report_id(
    record: &ModerationBallotRecord,
    tally: &ModerationBallotTally,
    deposit: &ModerationAppealDeposit,
    outcome: SoraFsAppealFinanceOutcomeV1,
) -> [u8; 16] {
    let mut material = String::new();
    material.push_str("sorafs.appeal_finance.moderation_tally_report.v1\n");
    material.push_str("case_id=");
    material.push_str(&tally.case_id);
    material.push('\n');
    material.push_str("round_id=");
    material.push_str(&tally.round_id);
    material.push('\n');
    material.push_str("escrow_id_hex=");
    material.push_str(&deposit.escrow_id_hex);
    material.push('\n');
    material.push_str("outcome=");
    material.push_str(outcome.as_str());
    material.push('\n');
    material.push_str("tallied_at_unix_ms=");
    material.push_str(&tally.tallied_at_unix_ms.to_string());
    material.push('\n');
    material.push_str("counts=");
    material.push_str(&format!(
        "{}/{}/{}/{}",
        tally.counts.uphold, tally.counts.overturn, tally.counts.modify, tally.counts.escalate
    ));
    material.push('\n');
    material.push_str("jurors=");
    for juror_id in &record.announcement.juror_ids {
        material.push_str(juror_id);
        material.push('\n');
    }
    let digest = blake3::hash(material.as_bytes());
    let mut report_id = [0_u8; 16];
    report_id.copy_from_slice(&digest.as_bytes()[..16]);
    report_id
}

/// Interface for emitting settlement artefacts to the governance DAG.
pub trait GovernancePublisher: Send + Sync + std::fmt::Debug {
    /// Persist the supplied settlement NORITO payload to the governance pipeline.
    fn publish_deal_settlement(
        &self,
        settlement: &DealSettlementV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist a repair audit event to the governance pipeline.
    fn publish_repair_audit_event(
        &self,
        event: &sorafs_manifest::repair::RepairAuditEventV1,
        encoded: &[u8],
    ) -> Result<(), GovernancePublishError>;
    /// Persist a repair slash proposal to the governance pipeline.
    fn publish_repair_slash_proposal(
        &self,
        proposal: &RepairSlashProposalV1,
        encoded: &[u8],
        stage: RepairSlashStage,
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
    /// Persist a reputation snapshot to the governance pipeline.
    fn publish_reputation_snapshot(
        &self,
        snapshot: &ReputationSnapshotV1,
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
    /// Persist an orderbook settlement receipt to the governance pipeline.
    fn publish_orderbook_settlement_receipt(
        &self,
        receipt: &SettlementReceiptV1,
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

/// Inputs used to publish a local moderation ballot lifecycle event.
struct ModerationBallotEventInput {
    /// Event kind to publish.
    kind: ModerationBallotEventKind,
    /// Local generation timestamp in milliseconds since the Unix epoch.
    generated_at_unix_ms: u64,
    /// Moderation case identifier.
    case_id: String,
    /// Ballot round identifier.
    round_id: String,
    /// Optional juror involved in the event.
    juror_id: Option<String>,
    /// Optional tally snapshot for finalization events.
    tally: Option<ModerationBallotTally>,
    /// Optional challenge record for challenge lifecycle events.
    challenge: Option<ModerationBallotChallengeRecord>,
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

/// Sequenced local repair event used for replay and live Torii streams.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
pub struct RepairEvent {
    /// Monotonic local stream sequence.
    pub sequence: u64,
    /// Canonical repair task transition payload.
    pub event: RepairTaskEventV1,
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
    /// Fetch missing chunks from remote sources for the supplied repair task.
    ///
    /// Implementations must return payloads whose digests match `missing_chunks`.
    fn rehydrate_missing_chunks(
        &self,
        task: &RepairTaskRecordV1,
        manifest: &StoredManifest,
        missing_chunks: &[ChunkFileRecord],
    ) -> Result<Vec<RepairChunkPayload>, RepairOrchestratorError>;
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
    por_history: Arc<RwLock<HashMap<PorHistoryKey, PorHistoryEntry>>>,
    storage: Option<Arc<StorageBackend>>,
    deal_engine: DealEngine,
    repair: RepairManager,
    repair_events: Arc<RwLock<BoundedEventHistory<RepairEvent>>>,
    repair_event_sender: broadcast::Sender<RepairEvent>,
    repair_orchestrator: Arc<RwLock<Option<Arc<dyn RepairOrchestrator>>>>,
    governance_publisher: Arc<RwLock<Option<Arc<dyn GovernancePublisher>>>>,
    runtime_mutation_lock: Arc<Mutex<()>>,
    auxiliary_checkpoint_lock: Arc<Mutex<()>>,
    durability_failure: Arc<Mutex<Option<String>>>,
    auxiliary_runtime_checkpoint_path: Option<PathBuf>,
    latest_reputation_snapshot: Arc<RwLock<Option<ReputationSnapshotV1>>>,
    reputation_snapshots: Arc<RwLock<BTreeMap<[u8; 16], ReputationSnapshotV1>>>,
    reputation_events: Arc<RwLock<BoundedEventHistory<ReputationSnapshotEventV1>>>,
    reputation_event_sender: broadcast::Sender<ReputationSnapshotEventV1>,
    orderbook: Arc<RwLock<OrderbookRuntime>>,
    orderbook_checkpoint_path: Option<PathBuf>,
    orderbook_events: Arc<RwLock<BoundedEventHistory<OrderbookEvent>>>,
    orderbook_event_sender: broadcast::Sender<OrderbookEvent>,
    reserve_lifecycle: Arc<RwLock<ReserveLifecycleRuntime>>,
    reserve_lifecycle_event_sender: broadcast::Sender<ReserveLifecycleEvent>,
    reserve_movement_event_sender: broadcast::Sender<ReserveMovementRecord>,
    moderation_model_registry_checkpoint_path: Option<PathBuf>,
    moderation_model_registry: Arc<RwLock<ModerationModelRegistry>>,
    moderation_screening_checkpoint_path: Option<PathBuf>,
    moderation_screening: Arc<RwLock<ModerationScreeningRuntime>>,
    moderation_quarantine_object_root: Option<PathBuf>,
    moderation_quarantine_object_index_path: Option<PathBuf>,
    moderation_quarantine_object_key_path: Option<PathBuf>,
    moderation_quarantine_objects: Arc<RwLock<ModerationQuarantineObjectRuntime>>,
    moderation_evidence_viewer_checkpoint_path: Option<PathBuf>,
    moderation_evidence_viewer: Arc<RwLock<ModerationEvidenceViewerRuntime>>,
    moderation_checkpoint_path: Option<PathBuf>,
    moderation: Arc<RwLock<ModerationBallotRuntime>>,
    moderation_events: Arc<RwLock<BoundedEventHistory<ModerationBallotEvent>>>,
    moderation_event_sender: broadcast::Sender<ModerationBallotEvent>,
    transparency_ledger_source_entries:
        Arc<RwLock<BTreeMap<String, TransparencyLedgerSourceEntry>>>,
    privacy_aggregate_source_events: Arc<RwLock<BTreeMap<String, PrivacyAggregateSourceEvent>>>,
    published_privacy_aggregate_cycles: Arc<RwLock<BTreeSet<[u8; 16]>>>,
    published_evidence_viewer_audit_cycles: Arc<RwLock<BTreeSet<[u8; 16]>>>,
}

type PorHistoryKey = ([u8; 32], [u8; 32]);
static MODERATION_KEY_CREATE_LOCK: Mutex<()> = Mutex::new(());

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

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct AuxiliaryRuntimeCheckpointV1 {
    version: u8,
    capacity_runtime: CapacityRuntimeCheckpointV1,
    deal_runtime: DealRuntimeCheckpointV1,
    por_tracker: por::PorTrackerCheckpointV1,
    por_history: Vec<PorHistoryCheckpointEntryV1>,
    reserve_runtime: ReserveRuntimeCheckpointV1,
    repair_events: Vec<RepairEvent>,
    reputation_snapshots: Vec<ReputationSnapshotV1>,
    latest_reputation_snapshot_id: Option<[u8; 16]>,
    reputation_events: Vec<ReputationSnapshotEventV1>,
    transparency_source_entries: Vec<TransparencyLedgerSourceEntry>,
    privacy_source_events: Vec<PrivacyAggregateSourceEvent>,
    published_privacy_aggregate_cycles: Vec<[u8; 16]>,
    published_evidence_viewer_audit_cycles: Vec<[u8; 16]>,
}

#[derive(Debug, Clone, NoritoSerialize, NoritoDeserialize)]
struct OrderbookRuntimeCheckpointV1 {
    version: u8,
    runtime: OrderbookRuntimeSnapshotV1,
    events: Vec<OrderbookEvent>,
}

#[derive(Debug)]
struct OrderbookEventInput {
    kind: OrderbookEventKind,
    order_id: Option<[u8; 32]>,
    trade_ids: Vec<[u8; 32]>,
    settlement_channel_ids: Vec<[u8; 32]>,
    receipt_id: Option<[u8; 32]>,
    expired_order_ids: Vec<[u8; 32]>,
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

/// Errors that prevent a SoraFS node handle from starting with trustworthy state.
#[derive(Debug, Error)]
pub enum NodeInitError {
    /// The configured storage backend could not be opened or validated.
    #[error("failed to initialise SoraFS storage backend: {0}")]
    Storage(#[from] StorageError),
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

/// Errors raised while computing reconciliation summaries.
#[derive(Debug, Error)]
pub enum ReconciliationError {
    /// Timestamp must be non-zero.
    #[error("reconciliation timestamp must be non-zero")]
    InvalidTimestamp,
    /// Storage backend is required for reconciliation.
    #[error("SoraFS storage backend is disabled")]
    StorageDisabled,
    /// Failed to encode reconciliation snapshot data.
    #[error(transparent)]
    Norito(#[from] norito::Error),
    /// Local appeal-finance Governance DAG data could not be reconciled.
    #[error("appeal finance reconciliation failed: {0}")]
    AppealFinance(String),
    /// Durable repair state could not be read for reconciliation.
    #[error("repair reconciliation state unavailable: {0}")]
    RepairStore(String),
    /// Reconciliation report failed validation.
    #[error(transparent)]
    Validation(#[from] ReconciliationValidationError),
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
        let repair_config = repair_config.with_default_state_dir(config.data_dir());
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

        let smoothing = config.smoothing_config();
        let event_history_limit = config.runtime_retention().event_history_limit();
        let state_entry_limit = config.runtime_retention().state_entry_limit();
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
        let orderbook_checkpoint_path = storage
            .as_ref()
            .map(|_| orderbook_runtime_snapshot_path(config.data_dir()));
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
        let moderation_quarantine_object_key_path = storage
            .as_ref()
            .map(|_| moderation_quarantine_object_key_path(config.data_dir()));
        let moderation_evidence_viewer_checkpoint_path = storage
            .as_ref()
            .map(|_| moderation_evidence_viewer_checkpoint_path(config.data_dir()));
        let moderation_checkpoint_path = storage
            .as_ref()
            .map(|_| moderation_ballot_checkpoint_path(config.data_dir()));
        let auxiliary_runtime_checkpoint_path = storage
            .as_ref()
            .map(|_| auxiliary_runtime_checkpoint_path(config.data_dir()));
        let (repair_event_sender, _) = broadcast::channel(REPAIR_EVENT_CHANNEL_CAPACITY);
        let (reputation_event_sender, _) = broadcast::channel(REPUTATION_EVENT_CHANNEL_CAPACITY);
        let (orderbook_event_sender, _) = broadcast::channel(ORDERBOOK_EVENT_CHANNEL_CAPACITY);
        let (reserve_lifecycle_event_sender, _) =
            broadcast::channel(RESERVE_LIFECYCLE_EVENT_CHANNEL_CAPACITY);
        let (reserve_movement_event_sender, _) =
            broadcast::channel(RESERVE_MOVEMENT_EVENT_CHANNEL_CAPACITY);
        let (moderation_event_sender, _) =
            broadcast::channel(MODERATION_BALLOT_EVENT_CHANNEL_CAPACITY);

        let repair_checkpoint_path = repair::repair_store_checkpoint_path(&repair_config)
            .unwrap_or_else(|| PathBuf::from("<unconfigured-repair-state>"));
        let repair = RepairManager::try_new_with_config_policy_and_limits(
            repair_config.clone(),
            repair_config.escalation_policy().clone(),
            state_entry_limit,
            config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|err| NodeInitError::checkpoint("repair", &repair_checkpoint_path, err))?;
        let node = Self {
            config,
            repair_config,
            gc_config,
            capacity: Arc::new(CapacityManager::with_entry_limit(state_entry_limit)),
            meter: CapacityMeter::with_smoothing(smoothing),
            telemetry: Arc::new(RwLock::new(None)),
            schedulers,
            por: PorTracker::with_entry_limit(state_entry_limit),
            potr: PotrTracker::default(),
            por_history: Arc::new(RwLock::new(HashMap::new())),
            storage,
            deal_engine,
            repair,
            repair_events: Arc::new(RwLock::new(BoundedEventHistory::new(event_history_limit))),
            repair_event_sender,
            repair_orchestrator: Arc::new(RwLock::new(None)),
            governance_publisher: Arc::new(RwLock::new(None)),
            runtime_mutation_lock: Arc::new(Mutex::new(())),
            auxiliary_checkpoint_lock: Arc::new(Mutex::new(())),
            durability_failure: Arc::new(Mutex::new(None)),
            auxiliary_runtime_checkpoint_path,
            latest_reputation_snapshot: Arc::new(RwLock::new(None)),
            reputation_snapshots: Arc::new(RwLock::new(BTreeMap::new())),
            reputation_events: Arc::new(RwLock::new(BoundedEventHistory::new(event_history_limit))),
            reputation_event_sender,
            orderbook: Arc::new(RwLock::new(OrderbookRuntime::with_entry_limit(
                state_entry_limit,
            ))),
            orderbook_checkpoint_path,
            orderbook_events: Arc::new(RwLock::new(BoundedEventHistory::new(event_history_limit))),
            orderbook_event_sender,
            reserve_lifecycle: Arc::new(RwLock::new(ReserveLifecycleRuntime::with_limits(
                state_entry_limit,
                event_history_limit,
            ))),
            reserve_lifecycle_event_sender,
            reserve_movement_event_sender,
            moderation_model_registry_checkpoint_path,
            moderation_model_registry: Arc::new(RwLock::new(
                ModerationModelRegistry::with_entry_limit(state_entry_limit),
            )),
            moderation_screening_checkpoint_path,
            moderation_screening: Arc::new(RwLock::new(
                ModerationScreeningRuntime::with_entry_limit(state_entry_limit),
            )),
            moderation_quarantine_object_root,
            moderation_quarantine_object_index_path,
            moderation_quarantine_object_key_path,
            moderation_quarantine_objects: Arc::new(RwLock::new(
                ModerationQuarantineObjectRuntime::with_entry_limit(state_entry_limit),
            )),
            moderation_evidence_viewer_checkpoint_path,
            moderation_evidence_viewer: Arc::new(RwLock::new(
                ModerationEvidenceViewerRuntime::with_entry_limit(state_entry_limit),
            )),
            moderation_checkpoint_path,
            moderation: Arc::new(RwLock::new(ModerationBallotRuntime::with_entry_limit(
                state_entry_limit,
            ))),
            moderation_events: Arc::new(RwLock::new(BoundedEventHistory::new(event_history_limit))),
            moderation_event_sender,
            transparency_ledger_source_entries: Arc::new(RwLock::new(BTreeMap::new())),
            privacy_aggregate_source_events: Arc::new(RwLock::new(BTreeMap::new())),
            published_privacy_aggregate_cycles: Arc::new(RwLock::new(BTreeSet::new())),
            published_evidence_viewer_audit_cycles: Arc::new(RwLock::new(BTreeSet::new())),
        };

        if node.storage.is_some() {
            match runtime_checkpoint_initialization {
                Some(RuntimeCheckpointInitialization::Fresh) => {
                    node.initialize_runtime_checkpoints()?;
                }
                Some(RuntimeCheckpointInitialization::Initialized) => {
                    node.load_orderbook_checkpoint()?;
                    node.load_moderation_model_registry_checkpoint()?;
                    node.load_moderation_screening_checkpoint()?;
                    node.load_moderation_quarantine_object_index_checkpoint()?;
                    node.audit_moderation_quarantine_object_store()?;
                    node.load_moderation_evidence_viewer_checkpoint()?;
                    node.load_moderation_ballot_checkpoint()?;
                    node.load_auxiliary_runtime_checkpoint()?;
                }
                None => unreachable!("storage-backed node must inspect runtime checkpoints"),
            }
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
                node.set_governance_publisher(Arc::new(publisher));
            }
        } else if governance_dir.is_some() {
            iroha_logger::warn!(
                "skipping governance publisher initialisation: storage backend disabled"
            );
        }

        Ok(node)
    }

    /// Returns a reference to the storage configuration.
    #[must_use]
    pub fn config(&self) -> &StorageConfig {
        &self.config
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
        let orderbook_path = self
            .orderbook_checkpoint_path
            .as_ref()
            .expect("storage-backed node has an orderbook checkpoint path");
        let orderbook = self
            .orderbook
            .read()
            .map_err(|_| {
                NodeInitError::checkpoint(
                    "orderbook runtime",
                    orderbook_path,
                    "state lock poisoned",
                )
            })?
            .runtime_snapshot(1);
        let orderbook_checkpoint = OrderbookRuntimeCheckpointV1 {
            version: ORDERBOOK_RUNTIME_STATE_VERSION_V1,
            runtime: orderbook,
            events: Vec::new(),
        };
        self.persist_orderbook_checkpoint(&orderbook_checkpoint)
            .map_err(|err| NodeInitError::checkpoint("orderbook runtime", orderbook_path, err))?;

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

        let ballot_path = self
            .moderation_checkpoint_path
            .as_ref()
            .expect("storage-backed node has a ballot checkpoint path");
        self.persist_moderation_ballot_snapshot(&ModerationBallotSnapshot::default())
            .map_err(|err| NodeInitError::checkpoint("moderation ballot", ballot_path, err))?;

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

    /// Deposit exact XOR-denominated provider bond collateral.
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

    /// Deposit an exact XOR-denominated client credit balance.
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

    /// Open a deal using the supplied proposal and activation epoch.
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
        if let Ok(mut guard) = self.governance_publisher.write() {
            *guard = Some(publisher);
        }
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

    /// Persist and cache the latest SoraFS reputation snapshot.
    ///
    /// The local snapshot, head linkage, and replay event are committed before
    /// optional external publication. Retrying the exact same snapshot id is
    /// idempotent and retries publication; conflicting ids or non-monotonic
    /// heads are rejected.
    pub fn publish_reputation_snapshot(
        &self,
        snapshot: ReputationSnapshotV1,
    ) -> Result<(), GovernancePublishError> {
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        snapshot
            .validate()
            .map_err(|err| GovernancePublishError::other(format!("invalid snapshot: {err}")))?;
        let encoded = norito::to_bytes(&snapshot)
            .map_err(|err| GovernancePublishError::other(format!("encode snapshot: {err}")))?;
        let mut snapshots = self
            .reputation_snapshots
            .write()
            .map_err(|_| GovernancePublishError::other("reputation snapshot index poisoned"))?;
        if let Some(existing) = snapshots.get(&snapshot.snapshot_id) {
            if existing != &snapshot {
                return Err(GovernancePublishError::other(format!(
                    "reputation snapshot id {} conflicts with retained canonical bytes",
                    hex::encode(snapshot.snapshot_id)
                )));
            }
            drop(snapshots);
            if let Some(publisher) = self.governance_publisher() {
                publisher.publish_reputation_snapshot(&snapshot, &encoded)?;
            }
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
        match latest.as_ref() {
            Some(head) => {
                if snapshot.previous_snapshot_id != Some(head.snapshot_id) {
                    return Err(GovernancePublishError::other(format!(
                        "reputation snapshot {} must extend current head {}",
                        hex::encode(snapshot.snapshot_id),
                        hex::encode(head.snapshot_id)
                    )));
                }
                if snapshot.generated_at_unix <= head.generated_at_unix {
                    return Err(GovernancePublishError::other(
                        "reputation snapshot generated_at_unix must advance the current head",
                    ));
                }
            }
            None if snapshot.previous_snapshot_id.is_some() => {
                return Err(GovernancePublishError::other(
                    "first reputation snapshot must not reference a previous snapshot",
                ));
            }
            None => {}
        }
        snapshots.insert(snapshot.snapshot_id, snapshot.clone());
        let next_sequence = events
            .latest_sequence
            .checked_add(1)
            .ok_or_else(|| GovernancePublishError::other("event sequence exhausted"))?;
        let event =
            ReputationSnapshotEventV1::from_snapshot(next_sequence, &snapshot).map_err(|err| {
                GovernancePublishError::other(format!(
                    "validated reputation snapshot could not produce an event: {err}"
                ))
            })?;
        let event = events.append(|sequence| {
            debug_assert_eq!(sequence, next_sequence);
            event
        })?;
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
                .filter(|candidate| !retained_snapshot_ids.contains(&candidate.snapshot_id))
                .min_by_key(|candidate| (candidate.generated_at_unix, candidate.snapshot_id))
                .map(|candidate| candidate.snapshot_id)
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
            return Err(GovernancePublishError::other(err.to_string()));
        }
        let _ = self.reputation_event_sender.send(event);
        if let Some(publisher) = self.governance_publisher() {
            publisher.publish_reputation_snapshot(&snapshot, &encoded)?;
        }
        Ok(())
    }

    /// Publish a reputation snapshot with local reserve lifecycle stages applied.
    ///
    /// Providers already present in the latest reputation snapshot keep their
    /// raw metrics and previous score for deterministic smoothing. Providers
    /// that only exist in local reserve state are added with neutral proof
    /// metrics so the reserve-stage penalty is still visible downstream.
    pub fn publish_reserve_adjusted_reputation_snapshot(
        &self,
        snapshot_id: [u8; 16],
        generated_at_unix: u64,
        weights: ReputationWeightsV1,
    ) -> Result<ReputationSnapshotV1, GovernancePublishError> {
        let reserve_snapshot = self.reserve_lifecycle_snapshot(generated_at_unix);
        if reserve_snapshot.providers.is_empty() {
            return Err(GovernancePublishError::other(
                "reserve-adjusted reputation snapshot requires local reserve provider state",
            ));
        }

        let previous_snapshot = self.latest_reputation_snapshot();
        let previous_by_provider = previous_snapshot
            .as_ref()
            .map(|snapshot| {
                snapshot
                    .providers
                    .iter()
                    .map(|provider| (provider.provider_id.clone(), provider.clone()))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default();
        let previous_snapshot_id = previous_snapshot
            .as_ref()
            .map(|snapshot| snapshot.snapshot_id);

        let mut providers = previous_snapshot
            .as_ref()
            .map_or_else(Vec::new, |snapshot| snapshot.providers.clone());
        let mut provider_positions = providers
            .iter()
            .enumerate()
            .map(|(index, provider)| (provider.provider_id.clone(), index))
            .collect::<HashMap<_, _>>();

        for summary in reserve_snapshot.providers {
            let provider_id = hex::encode(summary.provider_id);
            let previous = previous_by_provider.get(&provider_id);
            let input = ReputationProviderInputV1 {
                version: REPUTATION_PROVIDER_INPUT_VERSION_V1,
                provider_id: provider_id.clone(),
                metrics: previous.map_or_else(reserve_reputation_baseline_metrics, |provider| {
                    provider.raw_metrics
                }),
                reserve_stage: reserve_lifecycle_stage_to_reputation(summary.lifecycle.stage),
                previous_score_bps: previous.map(|provider| provider.score_bps),
                active_dispute: false,
                slashing_event: false,
            };
            let scored = score_provider_reputation(&input, &weights).map_err(|err| {
                GovernancePublishError::other(format!(
                    "score reserve-adjusted reputation provider `{provider_id}`: {err}"
                ))
            })?;
            if let Some(position) = provider_positions.get(&provider_id).copied() {
                providers[position] = scored;
            } else {
                provider_positions.insert(provider_id, providers.len());
                providers.push(scored);
            }
        }

        let snapshot = ReputationSnapshotV1::from_providers(
            snapshot_id,
            generated_at_unix,
            weights,
            providers,
            previous_snapshot_id,
        )
        .map_err(|err| {
            GovernancePublishError::other(format!(
                "build reserve-adjusted reputation snapshot: {err}"
            ))
        })?;
        self.publish_reputation_snapshot(snapshot.clone())?;
        Ok(snapshot)
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
        if let Some(publisher) = self.governance_publisher() {
            publisher.publish_appeal_finance_report(&report, &encoded)?;
        }
        self.record_transparency_source_entry_lossy(
            transparency::appeal_finance_report_source_entry(&report),
            "appeal_finance_report",
            &report.case_id,
        );
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
        if let Some(publisher) = self.governance_publisher() {
            publisher.publish_transparency_ledger_publication(&publication, &encoded)?;
        }
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
        if let Some(publisher) = self.governance_publisher() {
            publisher.publish_proof_token_issuance(&issuance, &encoded)?;
        }
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
        if guard.contains_key(&entry.event_id) {
            return Err(GovernancePublishError::other(format!(
                "duplicate transparency ledger source entry `{}`",
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
    ) -> Result<(), GovernancePublishError> {
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        event.validate().map_err(|err| {
            GovernancePublishError::other(format!("invalid privacy aggregate source event: {err}"))
        })?;
        let mut guard = self
            .privacy_aggregate_source_events
            .write()
            .map_err(|_| GovernancePublishError::other("privacy aggregate event index poisoned"))?;
        if guard.contains_key(&event.event_id) {
            return Err(GovernancePublishError::other(format!(
                "duplicate privacy aggregate source event `{}`",
                event.event_id
            )));
        }
        let limit = self.config.runtime_retention().state_entry_limit();
        if guard.len() >= limit {
            return Err(GovernancePublishError::other(format!(
                "privacy aggregate source-event retention exhausted (limit {limit})"
            )));
        }
        let event_id = event.event_id.clone();
        guard.insert(event_id.clone(), event);
        drop(guard);
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
            return Err(GovernancePublishError::other(err.to_string()));
        }
        Ok(())
    }

    /// Return the number of source events currently retained by the aggregate worker.
    #[must_use]
    pub fn privacy_aggregate_source_event_count(&self) -> usize {
        self.privacy_aggregate_source_events
            .read()
            .map(|guard| guard.len())
            .unwrap_or_default()
    }

    /// Return the config-backed privacy aggregate scheduler, when enabled.
    #[must_use]
    pub fn configured_privacy_aggregate_schedule(&self) -> Option<PrivacyAggregateScheduleConfig> {
        self.config.privacy_aggregate_schedule()
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
    pub fn publish_privacy_aggregate_cycle(
        &self,
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

        let mut seen_aggregate_ids = BTreeSet::new();
        let mut keyed = Vec::with_capacity(aggregates.len());
        for aggregate in aggregates {
            aggregate.validate().map_err(|err| {
                GovernancePublishError::other(format!("invalid privacy aggregate: {err}"))
            })?;
            if aggregate.window_start_unix < cycle_start_unix
                || aggregate.window_end_unix > cycle_end_unix
            {
                return Err(GovernancePublishError::other(format!(
                    "privacy aggregate `{}` window must be contained in the publication cycle",
                    aggregate.aggregate_id
                )));
            }
            if aggregate.generated_at_unix > generated_at_unix {
                return Err(GovernancePublishError::other(format!(
                    "privacy aggregate `{}` generated_at timestamp must not exceed publication generated_at",
                    aggregate.aggregate_id
                )));
            }
            if !seen_aggregate_ids.insert(aggregate.aggregate_id.clone()) {
                return Err(GovernancePublishError::other(format!(
                    "duplicate privacy aggregate id `{}` in cycle",
                    aggregate.aggregate_id
                )));
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
                "build privacy aggregate transparency publication: {err}"
            ))
        })?;
        self.publish_transparency_ledger_publication(publication.clone())?;
        Ok(publication)
    }

    /// Build and publish a privacy aggregate cycle from locally recorded source events.
    ///
    /// The worker filters recorded events to the requested cycle window, applies
    /// suppression/noise policy from `config`, builds aggregate payloads, and
    /// publishes the resulting transparency cycle through
    /// [`Self::publish_privacy_aggregate_cycle`].
    pub fn publish_privacy_aggregate_cycle_from_source_events(
        &self,
        cycle_id: [u8; 16],
        cycle_start_unix: u64,
        cycle_end_unix: u64,
        generated_at_unix: u64,
        previous_block_hash: Option<[u8; 32]>,
        config: PrivacyAggregateCycleConfig,
    ) -> Result<ModerationLedgerCyclePublicationV1, GovernancePublishError> {
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
            generated_at_unix,
            &config,
            &events,
        )
        .map_err(|err| {
            GovernancePublishError::other(format!("build privacy aggregate cycle: {err}"))
        })?;
        self.publish_privacy_aggregate_cycle(
            cycle_id,
            cycle_start_unix,
            cycle_end_unix,
            generated_at_unix,
            previous_block_hash,
            aggregates,
        )
    }

    /// Publish the oldest due privacy aggregate cycle with retained source events.
    ///
    /// This method derives the latest due cycle from `schedule`, then catches up
    /// older unpublished event-backed cycles first so delayed scheduler ticks do
    /// not strand stale source events. Duplicate publication of the same cycle
    /// id is suppressed within the node runtime, and the method returns a
    /// structured skip reason when no cycle is due, the latest due cycle is
    /// empty, already published, or every due event-backed cycle is fully
    /// suppressed by privacy policy.
    pub fn publish_due_privacy_aggregate_cycle_from_source_events(
        &self,
        now_unix: u64,
        schedule: PrivacyAggregateScheduleConfig,
        config: PrivacyAggregateCycleConfig,
        previous_block_hash: Option<[u8; 32]>,
    ) -> Result<PrivacyAggregateScheduleOutcome, GovernancePublishError> {
        let _mutation_guard = self.runtime_mutation_lock.lock().map_err(|_| {
            GovernancePublishError::other("runtime publication transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        let Some(latest_window) = schedule.due_window(now_unix).map_err(|err| {
            GovernancePublishError::other(format!("privacy aggregate schedule: {err}"))
        })?
        else {
            return Ok(PrivacyAggregateScheduleOutcome::NotDue);
        };
        let mut published_cycles = self
            .published_privacy_aggregate_cycles
            .read()
            .map_err(|_| {
                GovernancePublishError::other("privacy aggregate published-cycle index poisoned")
            })?
            .clone();

        let source_events = self
            .privacy_aggregate_source_events
            .read()
            .map_err(|_| GovernancePublishError::other("privacy aggregate event index poisoned"))?
            .values()
            .cloned()
            .collect::<Vec<_>>();

        let mut due_windows = BTreeMap::<PrivacyAggregateCycleWindow, Vec<_>>::new();
        for event in source_events {
            let Some(window) = schedule
                .event_window(event.occurred_at_unix)
                .map_err(|err| {
                    GovernancePublishError::other(format!("privacy aggregate schedule: {err}"))
                })?
            else {
                continue;
            };
            if window.due_at_unix > now_unix || window.cycle_end_unix > latest_window.cycle_end_unix
            {
                continue;
            }
            let cycle_id = transparency::privacy_aggregate_cycle_id(window);
            if published_cycles.contains(&cycle_id) {
                continue;
            }
            due_windows.entry(window).or_default().push(event);
        }

        if due_windows.is_empty() {
            let cycle_id = transparency::privacy_aggregate_cycle_id(latest_window);
            if published_cycles.contains(&cycle_id) {
                return Ok(PrivacyAggregateScheduleOutcome::AlreadyPublished {
                    window: latest_window,
                    cycle_id,
                });
            }
            return Ok(PrivacyAggregateScheduleOutcome::NoSourceEvents {
                window: latest_window,
                cycle_id,
            });
        }

        let mut first_suppressed = None;
        for (window, events) in due_windows {
            let cycle_id = transparency::privacy_aggregate_cycle_id(window);
            let state_limit = self.config.runtime_retention().state_entry_limit();
            if !published_cycles.contains(&cycle_id) && published_cycles.len() >= state_limit {
                return Err(GovernancePublishError::other(format!(
                    "privacy aggregate published-cycle retention exhausted (limit {state_limit})"
                )));
            }
            let aggregates = match transparency::build_privacy_aggregates_from_source_events(
                window.cycle_start_unix,
                window.cycle_end_unix,
                now_unix,
                &config,
                &events,
            ) {
                Ok(aggregates) => aggregates,
                Err(transparency::PrivacyAggregateWorkerError::AllBucketsSuppressed) => {
                    first_suppressed.get_or_insert((window, cycle_id));
                    self.commit_processed_privacy_cycle(cycle_id, window)?;
                    published_cycles.insert(cycle_id);
                    continue;
                }
                Err(err) => {
                    return Err(GovernancePublishError::other(format!(
                        "build scheduled privacy aggregate cycle: {err}"
                    )));
                }
            };
            let publication = self.publish_privacy_aggregate_cycle(
                cycle_id,
                window.cycle_start_unix,
                window.cycle_end_unix,
                now_unix,
                previous_block_hash,
                aggregates,
            )?;
            self.commit_processed_privacy_cycle(cycle_id, window)?;
            return Ok(PrivacyAggregateScheduleOutcome::Published {
                window,
                publication,
            });
        }

        if let Some((window, cycle_id)) = first_suppressed {
            Ok(PrivacyAggregateScheduleOutcome::AllBucketsSuppressed { window, cycle_id })
        } else {
            let cycle_id = transparency::privacy_aggregate_cycle_id(latest_window);
            Ok(PrivacyAggregateScheduleOutcome::NoSourceEvents {
                window: latest_window,
                cycle_id,
            })
        }
    }

    fn commit_processed_privacy_cycle(
        &self,
        cycle_id: [u8; 16],
        window: PrivacyAggregateCycleWindow,
    ) -> Result<(), GovernancePublishError> {
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            GovernancePublishError::other("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_durability_healthy()
            .map_err(GovernancePublishError::other)?;
        let mut published = self
            .published_privacy_aggregate_cycles
            .write()
            .map_err(|_| {
                GovernancePublishError::other("privacy aggregate published-cycle index poisoned")
            })?;
        let mut events = self
            .privacy_aggregate_source_events
            .write()
            .map_err(|_| GovernancePublishError::other("privacy aggregate event index poisoned"))?;
        let previous_published = published.clone();
        let previous_events = events.clone();
        published.insert(cycle_id);
        events.retain(|_, event| {
            event.occurred_at_unix < window.cycle_start_unix
                || event.occurred_at_unix >= window.cycle_end_unix
        });
        drop(published);
        drop(events);
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(GovernancePublishError::other(err.to_string()));
            }
            *self
                .published_privacy_aggregate_cycles
                .write()
                .map_err(|_| {
                    GovernancePublishError::other("privacy cycle rollback lock poisoned")
                })? = previous_published;
            *self.privacy_aggregate_source_events.write().map_err(|_| {
                GovernancePublishError::other("privacy source-event rollback lock poisoned")
            })? = previous_events;
            return Err(GovernancePublishError::other(err.to_string()));
        }
        Ok(())
    }

    /// Publish the next due privacy aggregate cycle using storage configuration.
    ///
    /// Privacy policy and runtime noise seed material remain explicit runtime
    /// inputs; the persisted config only controls whether scheduled publication
    /// is enabled and which cadence is used.
    pub fn publish_due_configured_privacy_aggregate_cycle_from_source_events(
        &self,
        now_unix: u64,
        config: PrivacyAggregateCycleConfig,
        previous_block_hash: Option<[u8; 32]>,
    ) -> Result<PrivacyAggregateScheduleOutcome, GovernancePublishError> {
        let Some(schedule) = self.configured_privacy_aggregate_schedule() else {
            return Ok(PrivacyAggregateScheduleOutcome::Disabled);
        };
        self.publish_due_privacy_aggregate_cycle_from_source_events(
            now_unix,
            schedule,
            config,
            previous_block_hash,
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
        if let Some(publisher) = self.governance_publisher() {
            publisher.publish_appeal_finance_weekly_rollup(&rollup, &encoded)?;
        }
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
        if let Some(publisher) = self.governance_publisher() {
            publisher.publish_appeal_finance_settlement_receipt(&receipt, &encoded)?;
        }
        self.record_transparency_source_entry_lossy(
            transparency::appeal_finance_settlement_receipt_source_entry(&receipt),
            "appeal_finance_settlement_receipt",
            &receipt.case_id,
        );
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
        self.reputation_snapshots
            .read()
            .ok()
            .and_then(|guard| guard.get(&snapshot_id).cloned())
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

    /// Return local repair events after `since_sequence`, capped by `limit`.
    #[must_use]
    pub fn repair_events_since(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
    ) -> Vec<RepairEvent> {
        self.repair_events_replay(since_sequence, limit).events
    }

    /// Return a gap-aware bounded replay of local repair events.
    #[must_use]
    pub fn repair_events_replay(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
    ) -> EventReplay<RepairEvent> {
        self.repair_events.read().map_or_else(
            |_| EventReplay {
                oldest_available_sequence: None,
                latest_sequence: None,
                gap: false,
                events: Vec::new(),
            },
            |guard| guard.replay(since_sequence, limit, |event| event.sequence),
        )
    }

    /// Return the latest local repair event sequence accepted by this node.
    #[must_use]
    pub fn latest_repair_event_sequence(&self) -> Option<u64> {
        self.repair_events
            .read()
            .ok()
            .and_then(|guard| (guard.latest_sequence != 0).then_some(guard.latest_sequence))
    }

    /// Subscribe to live local repair events.
    #[must_use]
    pub fn subscribe_repair_events(&self) -> broadcast::Receiver<RepairEvent> {
        self.repair_event_sender.subscribe()
    }

    /// Return local orderbook events after `since_sequence`, capped by `limit`.
    #[must_use]
    pub fn orderbook_events_since(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
    ) -> Vec<OrderbookEvent> {
        self.orderbook_events_replay(since_sequence, limit).events
    }

    /// Return a gap-aware bounded replay of local orderbook events.
    #[must_use]
    pub fn orderbook_events_replay(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
    ) -> EventReplay<OrderbookEvent> {
        self.orderbook_events.read().map_or_else(
            |_| EventReplay {
                oldest_available_sequence: None,
                latest_sequence: None,
                gap: false,
                events: Vec::new(),
            },
            |guard| guard.replay(since_sequence, limit, |event| event.sequence),
        )
    }

    /// Return the latest local orderbook event sequence accepted by this node.
    #[must_use]
    pub fn latest_orderbook_event_sequence(&self) -> Option<u64> {
        self.orderbook_events
            .read()
            .ok()
            .and_then(|guard| (guard.latest_sequence != 0).then_some(guard.latest_sequence))
    }

    /// Subscribe to live local orderbook events.
    #[must_use]
    pub fn subscribe_orderbook_events(&self) -> broadcast::Receiver<OrderbookEvent> {
        self.orderbook_event_sender.subscribe()
    }

    /// Record a local reserve lifecycle service update for one provider.
    ///
    /// The update is projected using the shared SFM-6 reserve quote logic, then
    /// stored as the provider's latest summary and appended to the local event
    /// history. This local service surface does not submit reserve movements to
    /// chain custody; signed Torii routes remain the production integration
    /// layer.
    pub fn record_reserve_lifecycle_update(
        &self,
        update: ReserveLifecycleUpdate,
    ) -> Result<ReserveLifecycleEvent, ReserveLifecycleRuntimeError> {
        let event = self.mutate_reserve_runtime_durably(
            |runtime| runtime.record_update(update).map(|event| (event, true)),
            || ReserveLifecycleRuntimeError::StateLockPoisoned,
            ReserveLifecycleRuntimeError::Checkpoint,
        )?;
        let _ = self.reserve_lifecycle_event_sender.send(event.clone());
        self.record_transparency_source_entry_lossy(
            transparency::reserve_lifecycle_event_source_entry(&event),
            "reserve_lifecycle_event",
            &hex::encode(event.provider_id),
        );
        global_or_default().record_sorafs_reserve_service_request("lifecycle_update", "accepted");
        self.refresh_reserve_runtime_metrics();
        Ok(event)
    }

    /// Return one provider's latest reserve lifecycle summary.
    #[must_use]
    pub fn reserve_provider_lifecycle_summary(
        &self,
        provider_id: [u8; 32],
    ) -> Option<ReserveProviderLifecycleSummary> {
        self.reserve_lifecycle
            .read()
            .ok()
            .and_then(|runtime| runtime.provider_summary(provider_id))
    }

    /// Return a point-in-time snapshot of the local reserve lifecycle runtime.
    #[must_use]
    pub fn reserve_lifecycle_snapshot(&self, generated_at_unix: u64) -> ReserveLifecycleSnapshot {
        self.reserve_lifecycle.read().map_or_else(
            |_| ReserveLifecycleSnapshot {
                generated_at_unix,
                ..ReserveLifecycleSnapshot::default()
            },
            |runtime| runtime.snapshot(generated_at_unix),
        )
    }

    /// Return one provider's latest local reserve credit-line state.
    #[must_use]
    pub fn reserve_provider_credit_line(
        &self,
        provider_id: [u8; 32],
    ) -> Option<ReserveProviderCreditLineState> {
        self.reserve_lifecycle
            .read()
            .ok()
            .and_then(|runtime| runtime.provider_credit_line(provider_id))
    }

    /// Return a point-in-time snapshot of local reserve credit-line state.
    #[must_use]
    pub fn reserve_credit_line_snapshot(
        &self,
        generated_at_unix: u64,
    ) -> ReserveCreditLineSnapshot {
        self.reserve_lifecycle.read().map_or_else(
            |_| ReserveCreditLineSnapshot {
                generated_at_unix,
                ..ReserveCreditLineSnapshot::default()
            },
            |runtime| runtime.credit_line_snapshot(generated_at_unix),
        )
    }

    /// Advance retained reserve lifecycle summaries to an explicit service timestamp.
    pub fn advance_reserve_lifecycle(
        &self,
        observed_at_unix: u64,
    ) -> Result<Vec<ReserveLifecycleEvent>, ReserveLifecycleRuntimeError> {
        let events = self.mutate_reserve_runtime_durably(
            |runtime| {
                runtime
                    .advance_lifecycle_to(observed_at_unix)
                    .map(|events| {
                        let changed = !events.is_empty();
                        (events, changed)
                    })
            },
            || ReserveLifecycleRuntimeError::StateLockPoisoned,
            ReserveLifecycleRuntimeError::Checkpoint,
        )?;
        for event in &events {
            let _ = self.reserve_lifecycle_event_sender.send(event.clone());
            self.record_transparency_source_entry_lossy(
                transparency::reserve_lifecycle_event_source_entry(event),
                "reserve_lifecycle_time_advance",
                &hex::encode(event.provider_id),
            );
        }
        let outcome = if events.is_empty() {
            "noop"
        } else {
            "advanced"
        };
        global_or_default().record_sorafs_reserve_service_request("lifecycle_advance", outcome);
        if !events.is_empty() {
            self.refresh_reserve_runtime_metrics();
        }
        Ok(events)
    }

    /// Return local reserve lifecycle events after `since_sequence`, capped by `limit`.
    #[must_use]
    pub fn reserve_lifecycle_events_since(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
    ) -> Vec<ReserveLifecycleEvent> {
        self.reserve_lifecycle.read().map_or_else(
            |_| Vec::new(),
            |runtime| runtime.events_since(since_sequence, limit),
        )
    }

    /// Return a gap-aware bounded replay of local reserve lifecycle events.
    #[must_use]
    pub fn reserve_lifecycle_events_replay(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
    ) -> EventReplay<ReserveLifecycleEvent> {
        self.reserve_lifecycle.read().map_or_else(
            |_| EventReplay {
                oldest_available_sequence: None,
                latest_sequence: None,
                gap: false,
                events: Vec::new(),
            },
            |runtime| {
                let (oldest_available_sequence, latest_sequence, gap, events) =
                    runtime.events_replay(since_sequence, limit);
                EventReplay {
                    oldest_available_sequence,
                    latest_sequence,
                    gap,
                    events,
                }
            },
        )
    }

    /// Return the latest local reserve lifecycle event sequence accepted by this node.
    #[must_use]
    pub fn latest_reserve_lifecycle_event_sequence(&self) -> Option<u64> {
        self.reserve_lifecycle
            .read()
            .ok()
            .and_then(|runtime| runtime.latest_event_sequence())
    }

    /// Subscribe to live local reserve lifecycle events.
    #[must_use]
    pub fn subscribe_reserve_lifecycle_events(&self) -> broadcast::Receiver<ReserveLifecycleEvent> {
        self.reserve_lifecycle_event_sender.subscribe()
    }

    /// Record a local reserve custody movement for one provider.
    ///
    /// The local movement ledger tracks provider reserve balances and
    /// idempotent top-up/withdrawal records. It does not submit the movement to
    /// chain custody; callers must still submit the corresponding signed
    /// transaction through the production client path.
    pub fn record_reserve_movement(
        &self,
        request: ReserveMovementRequest,
    ) -> Result<ReserveMovementOutcome, ReserveMovementRuntimeError> {
        let outcome = self.mutate_reserve_runtime_durably(
            |runtime| {
                runtime.record_movement(request).map(|outcome| {
                    let changed = !outcome.duplicate;
                    (outcome, changed)
                })
            },
            || ReserveMovementRuntimeError::StateLockPoisoned,
            ReserveMovementRuntimeError::Checkpoint,
        )?;
        if !outcome.duplicate {
            let _ = self
                .reserve_movement_event_sender
                .send(outcome.record.clone());
            self.record_transparency_source_entry_lossy(
                transparency::reserve_movement_source_entry(&outcome.record),
                "reserve_movement",
                &hex::encode(outcome.record.movement_id),
            );
            global_or_default().record_sorafs_reserve_service_request(
                reserve_movement_kind_metric_label(outcome.record.kind),
                "accepted",
            );
            self.refresh_reserve_runtime_metrics();
        } else {
            global_or_default().record_sorafs_reserve_service_request(
                reserve_movement_kind_metric_label(outcome.record.kind),
                "duplicate",
            );
        }
        Ok(outcome)
    }

    /// Return one locally recorded reserve movement by movement id.
    #[must_use]
    pub fn reserve_movement(&self, movement_id: [u8; 32]) -> Option<ReserveMovementRecord> {
        self.reserve_lifecycle
            .read()
            .ok()
            .and_then(|runtime| runtime.movement(movement_id))
    }

    /// Attach local chain custody evidence to a recorded reserve movement.
    pub fn record_reserve_movement_custody_update(
        &self,
        update: ReserveMovementCustodyUpdate,
    ) -> Result<ReserveMovementRecord, ReserveMovementRuntimeError> {
        let movement_id = update.movement_id;
        let (record, changed) = self.mutate_reserve_runtime_durably(
            |runtime| {
                let previous = runtime.movement(movement_id);
                let record = runtime.record_movement_custody_update(update)?;
                let changed = previous.as_ref() != Some(&record);
                Ok(((record, changed), changed))
            },
            || ReserveMovementRuntimeError::StateLockPoisoned,
            ReserveMovementRuntimeError::Checkpoint,
        )?;
        if changed {
            let _ = self.reserve_movement_event_sender.send(record.clone());
            self.record_transparency_source_entry_lossy(
                transparency::reserve_movement_source_entry(&record),
                "reserve_movement_custody_update",
                &hex::encode(record.movement_id),
            );
            global_or_default()
                .record_sorafs_reserve_service_request("movement_custody_update", "updated");
            self.refresh_reserve_runtime_metrics();
        } else {
            global_or_default()
                .record_sorafs_reserve_service_request("movement_custody_update", "replay");
        }
        Ok(record)
    }

    /// Return one provider's locally recorded reserve balance.
    #[must_use]
    pub fn reserve_provider_balance(
        &self,
        provider_id: [u8; 32],
    ) -> Option<ReserveProviderBalance> {
        self.reserve_lifecycle
            .read()
            .ok()
            .and_then(|runtime| runtime.provider_balance(provider_id))
    }

    /// Return a point-in-time snapshot of the local reserve movement ledger.
    #[must_use]
    pub fn reserve_movement_snapshot(&self, generated_at_unix: u64) -> ReserveMovementSnapshot {
        self.reserve_lifecycle.read().map_or_else(
            |_| ReserveMovementSnapshot {
                generated_at_unix,
                ..ReserveMovementSnapshot::default()
            },
            |runtime| runtime.movement_snapshot(generated_at_unix),
        )
    }

    /// Return local reserve movements after `since_sequence`, capped by `limit`.
    #[must_use]
    pub fn reserve_movements_since(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
    ) -> Vec<ReserveMovementRecord> {
        self.reserve_lifecycle.read().map_or_else(
            |_| Vec::new(),
            |runtime| runtime.movements_since(since_sequence, limit),
        )
    }

    /// Return local reserve movements visible to `account` after `since_sequence`, capped by `limit`.
    #[must_use]
    pub fn reserve_movements_since_visible_to(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
        account: &[u8],
    ) -> Vec<ReserveMovementRecord> {
        self.reserve_lifecycle.read().map_or_else(
            |_| Vec::new(),
            |runtime| runtime.movements_since_visible_to(since_sequence, limit, account),
        )
    }

    /// Return the latest local reserve movement sequence accepted by this node.
    #[must_use]
    pub fn latest_reserve_movement_sequence(&self) -> Option<u64> {
        self.reserve_lifecycle
            .read()
            .ok()
            .and_then(|runtime| runtime.latest_movement_sequence())
    }

    /// Subscribe to live local reserve movement records.
    #[must_use]
    pub fn subscribe_reserve_movement_events(&self) -> broadcast::Receiver<ReserveMovementRecord> {
        self.reserve_movement_event_sender.subscribe()
    }

    /// Record a local reserve appeal submitted by a provider.
    ///
    /// Appeals are local authenticated service records used for rollout and
    /// governance handoff. Accepted decisions with a requested stage are applied
    /// to local lifecycle state by the decision path.
    pub fn record_reserve_appeal(
        &self,
        request: ReserveAppealRequest,
    ) -> Result<ReserveAppealOutcome, ReserveAppealRuntimeError> {
        let outcome = self.mutate_reserve_runtime_durably(
            |runtime| {
                runtime.record_appeal(request).map(|outcome| {
                    let changed = !outcome.duplicate;
                    (outcome, changed)
                })
            },
            || ReserveAppealRuntimeError::StateLockPoisoned,
            ReserveAppealRuntimeError::Checkpoint,
        )?;
        if !outcome.duplicate {
            self.record_transparency_source_entry_lossy(
                transparency::reserve_appeal_source_entry(&outcome.record),
                "reserve_appeal",
                &hex::encode(outcome.record.appeal_id),
            );
            global_or_default().record_sorafs_reserve_service_request("appeal", "accepted");
            self.refresh_reserve_runtime_metrics();
        } else {
            global_or_default().record_sorafs_reserve_service_request("appeal", "duplicate");
        }
        Ok(outcome)
    }

    /// Return one locally recorded reserve appeal by appeal id.
    #[must_use]
    pub fn reserve_appeal(&self, appeal_id: [u8; 32]) -> Option<ReserveAppealRecord> {
        self.reserve_lifecycle
            .read()
            .ok()
            .and_then(|runtime| runtime.appeal(appeal_id))
    }

    /// Record a local reserve appeal decision.
    pub fn record_reserve_appeal_decision(
        &self,
        decision: ReserveAppealDecision,
    ) -> Result<ReserveAppealRecord, ReserveAppealRuntimeError> {
        let appeal_id = decision.appeal_id;
        let (outcome, changed) = self.mutate_reserve_runtime_durably(
            |runtime| {
                let previous = runtime.appeal(appeal_id);
                let outcome = runtime.record_appeal_decision(decision)?;
                let changed = !outcome.duplicate && previous.as_ref() != Some(&outcome.record);
                Ok(((outcome, changed), changed))
            },
            || ReserveAppealRuntimeError::StateLockPoisoned,
            ReserveAppealRuntimeError::Checkpoint,
        )?;
        let record = outcome.record;
        let lifecycle_event = outcome.lifecycle_event;
        if changed {
            self.record_transparency_source_entry_lossy(
                transparency::reserve_appeal_source_entry(&record),
                "reserve_appeal_decision",
                &hex::encode(record.appeal_id),
            );
            global_or_default().record_sorafs_reserve_service_request("appeal_decision", "updated");
        } else {
            global_or_default().record_sorafs_reserve_service_request("appeal_decision", "replay");
        }
        if let Some(event) = lifecycle_event {
            let _ = self.reserve_lifecycle_event_sender.send(event.clone());
            self.record_transparency_source_entry_lossy(
                transparency::reserve_lifecycle_event_source_entry(&event),
                "reserve_lifecycle_appeal_override",
                &hex::encode(event.provider_id),
            );
        }
        if changed {
            self.refresh_reserve_runtime_metrics();
        }
        Ok(record)
    }

    /// Return a point-in-time snapshot of local reserve appeals.
    #[must_use]
    pub fn reserve_appeal_snapshot(&self, generated_at_unix: u64) -> ReserveAppealSnapshot {
        self.reserve_lifecycle.read().map_or_else(
            |_| ReserveAppealSnapshot {
                generated_at_unix,
                ..ReserveAppealSnapshot::default()
            },
            |runtime| runtime.appeal_snapshot(generated_at_unix),
        )
    }

    /// Record a local reserve lifecycle policy-window update.
    ///
    /// The update is stored for service readback and governance handoff. When
    /// the policy is already effective, current provider summaries whose
    /// retained lifecycle observation falls under that policy are reprojected
    /// through new lifecycle events instead of rewriting previous events.
    pub fn record_reserve_lifecycle_policy_update(
        &self,
        update: ReserveLifecyclePolicyUpdate,
    ) -> Result<ReserveLifecyclePolicyOutcome, ReserveAppealRuntimeError> {
        let outcome = self.mutate_reserve_runtime_durably(
            |runtime| {
                runtime
                    .record_lifecycle_policy_update(update)
                    .map(|outcome| {
                        let changed = !outcome.duplicate;
                        (outcome, changed)
                    })
            },
            || ReserveAppealRuntimeError::StateLockPoisoned,
            ReserveAppealRuntimeError::Checkpoint,
        )?;
        if !outcome.duplicate {
            self.record_transparency_source_entry_lossy(
                transparency::reserve_lifecycle_policy_source_entry(&outcome.record),
                "reserve_lifecycle_policy",
                &hex::encode(outcome.record.policy_id),
            );
            for event in &outcome.reprojected_events {
                let _ = self.reserve_lifecycle_event_sender.send(event.clone());
                self.record_transparency_source_entry_lossy(
                    transparency::reserve_lifecycle_event_source_entry(event),
                    "reserve_lifecycle_policy_reprojection",
                    &hex::encode(event.provider_id),
                );
            }
            global_or_default()
                .record_sorafs_reserve_service_request("lifecycle_policy", "accepted");
            if !outcome.reprojected_events.is_empty() {
                self.refresh_reserve_runtime_metrics();
            }
        } else {
            global_or_default()
                .record_sorafs_reserve_service_request("lifecycle_policy", "duplicate");
        }
        Ok(outcome)
    }

    /// Return the latest local reserve lifecycle policy update by sequence.
    #[must_use]
    pub fn latest_reserve_lifecycle_policy(&self) -> Option<ReserveLifecyclePolicyRecord> {
        self.reserve_lifecycle
            .read()
            .ok()
            .and_then(|runtime| runtime.latest_lifecycle_policy())
    }

    /// Return a point-in-time snapshot of local reserve lifecycle policy records.
    #[must_use]
    pub fn reserve_lifecycle_policy_snapshot(
        &self,
        generated_at_unix: u64,
    ) -> ReserveLifecyclePolicySnapshot {
        self.reserve_lifecycle.read().map_or_else(
            |_| ReserveLifecyclePolicySnapshot {
                generated_at_unix,
                ..ReserveLifecyclePolicySnapshot::default()
            },
            |runtime| runtime.lifecycle_policy_snapshot(generated_at_unix),
        )
    }

    fn refresh_reserve_runtime_metrics(&self) {
        let Ok(runtime) = self.reserve_lifecycle.read() else {
            return;
        };
        let lifecycle = runtime.snapshot(0);
        let credit_lines = runtime.credit_line_snapshot(0);
        let appeals = runtime.appeal_snapshot(0);
        let movements = runtime.movement_snapshot(0);
        drop(runtime);

        let mut stage_counts = BTreeMap::<String, u64>::new();
        for provider in &lifecycle.providers {
            let label = reserve_lifecycle_stage_metric_label(provider.lifecycle.stage).to_string();
            *stage_counts.entry(label).or_default() += 1;
        }
        let defaulted_providers = stage_counts.get("default").copied().unwrap_or(0);
        let stage_counts = stage_counts.into_iter().collect::<Vec<_>>();

        let credit_lines = credit_lines
            .credit_lines
            .iter()
            .map(|state| {
                (
                    hex::encode(state.provider_id),
                    state
                        .credit_draw
                        .try_to_micro()
                        .expect("XOR quantity has exact legacy micro representation"),
                    state
                        .credit_shortfall
                        .try_to_micro()
                        .expect("XOR quantity has exact legacy micro representation"),
                    state
                        .accrued_interest
                        .try_to_micro()
                        .expect("XOR quantity has exact legacy micro representation"),
                )
            })
            .collect::<Vec<_>>();

        let open_appeals = appeals
            .appeals
            .iter()
            .filter(|appeal| appeal.status == ReserveAppealStatus::Open)
            .count() as u64;

        let mut custody_counts = BTreeMap::<String, u64>::new();
        let mut chain_reconciled_counts = BTreeMap::<String, u64>::new();
        for movement in &movements.movements {
            let label = movement.custody_status.label().to_string();
            *custody_counts.entry(label.clone()).or_default() += 1;
            if matches!(
                movement.custody_status,
                ReserveMovementCustodyStatus::Confirmed | ReserveMovementCustodyStatus::Rejected
            ) {
                *chain_reconciled_counts.entry(label).or_default() += 1;
            }
        }

        global_or_default().record_sorafs_reserve_runtime_metrics(
            &stage_counts,
            &credit_lines,
            defaulted_providers,
            open_appeals,
            &custody_counts.into_iter().collect::<Vec<_>>(),
            &chain_reconciled_counts.into_iter().collect::<Vec<_>>(),
        );
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

    /// Record one deterministic local moderation screening result.
    ///
    /// `quarantine` and `escalate` verdicts also create a pending quarantine
    /// queue record. Successful updates are persisted to the local checkpoint
    /// when SoraFS storage is enabled.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is invalid, the screening id conflicts
    /// with existing local state, or the runtime lock is poisoned.
    pub fn record_moderation_screening_result(
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
        let key_path = self
            .moderation_quarantine_object_key_path
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::StorageDisabled)?;
        let quarantine = self.moderation_quarantine_record_for_object(&input.quarantine_id)?;
        let payload_digest = *blake3::hash(&input.payload).as_bytes();
        if payload_digest != quarantine.subject_digest {
            return Err(ModerationQuarantineObjectError::DigestMismatch {
                quarantine_id_hex: hex::encode(input.quarantine_id),
                expected_digest_hex: hex::encode(quarantine.subject_digest),
                actual_digest_hex: hex::encode(payload_digest),
            });
        }

        let mut objects = self
            .moderation_quarantine_objects
            .write()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?;
        let previous = objects.snapshot();
        objects.ensure_insert_capacity(&input.quarantine_id)?;
        let local_key = if previous.objects.is_empty() {
            load_or_create_moderation_quarantine_object_key(key_path)?
        } else {
            load_moderation_quarantine_object_key(key_path)?
        };
        let (record, envelope_bytes) = seal_moderation_quarantine_object(input, local_key)?;
        let envelope_path = self.resolve_moderation_quarantine_object_path(root, &record)?;
        if let Some(existing) = objects.get(&record.quarantine_id) {
            if existing != record {
                return Err(ModerationQuarantineObjectError::ConflictingObject {
                    quarantine_id_hex: hex::encode(record.quarantine_id),
                });
            }
            let existing_bytes = read_local_checkpoint_bounded(
                &envelope_path,
                self.config.runtime_retention().checkpoint_max_bytes(),
            )
            .map_err(|err| ModerationQuarantineObjectError::Io {
                path: envelope_path.display().to_string(),
                message: err.to_string(),
            })?
            .ok_or_else(|| ModerationQuarantineObjectError::MissingObject {
                quarantine_id_hex: hex::encode(record.quarantine_id),
            })?;
            let existing_envelope =
                norito::decode_from_bytes::<ModerationQuarantineObjectEnvelopeV1>(&existing_bytes)
                    .map_err(|err| ModerationQuarantineObjectError::Codec {
                        message: err.to_string(),
                    })?;
            if norito::to_bytes(&existing_envelope).map_err(|err| {
                ModerationQuarantineObjectError::Codec {
                    message: err.to_string(),
                }
            })? != existing_bytes
            {
                return Err(ModerationQuarantineObjectError::AuthenticationFailed {
                    quarantine_id_hex: hex::encode(record.quarantine_id),
                });
            }
            let plaintext =
                open_moderation_quarantine_object(existing_envelope, &existing, local_key)?;
            if *blake3::hash(&plaintext).as_bytes() != existing.payload_digest {
                return Err(ModerationQuarantineObjectError::AuthenticationFailed {
                    quarantine_id_hex: hex::encode(record.quarantine_id),
                });
            }
            return Ok(existing);
        }
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
        let key_path = self
            .moderation_quarantine_object_key_path
            .as_ref()
            .ok_or(ModerationQuarantineObjectError::StorageDisabled)?;
        let quarantine = self.moderation_quarantine_record_for_object(&quarantine_id)?;
        let record = self
            .moderation_quarantine_objects
            .read()
            .map_err(|_| ModerationQuarantineObjectError::StateLockPoisoned)?
            .get(&quarantine_id)
            .ok_or_else(|| ModerationQuarantineObjectError::MissingObject {
                quarantine_id_hex: hex::encode(quarantine_id),
            })?;
        let envelope_path = self.resolve_moderation_quarantine_object_path(root, &record)?;
        let envelope_bytes = read_local_checkpoint_bounded(
            &envelope_path,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|err| ModerationQuarantineObjectError::Io {
            path: envelope_path.display().to_string(),
            message: err.to_string(),
        })?
        .ok_or_else(|| ModerationQuarantineObjectError::MissingObject {
            quarantine_id_hex: hex::encode(quarantine_id),
        })?;
        let envelope =
            norito::decode_from_bytes::<ModerationQuarantineObjectEnvelopeV1>(&envelope_bytes)
                .map_err(|err| ModerationQuarantineObjectError::Codec {
                    message: err.to_string(),
                })?;
        let local_key = load_moderation_quarantine_object_key(key_path)?;
        let payload = open_moderation_quarantine_object(envelope, &record, local_key)?;
        if *blake3::hash(&payload).as_bytes() != quarantine.subject_digest {
            return Err(ModerationQuarantineObjectError::AuthenticationFailed {
                quarantine_id_hex: hex::encode(quarantine_id),
            });
        }
        Ok(ModerationQuarantineObjectPayload { record, payload })
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
    /// fails, or the transparency source-entry worker rejects the entry for a
    /// reason other than an idempotent duplicate.
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
        if let Err(err) = self.record_transparency_ledger_source_entry(source_entry.clone()) {
            let message = err.to_string();
            if !message.contains("duplicate transparency ledger source entry") {
                return Err(ModerationEvidenceViewerError::TransparencyExport { message });
            }
        }
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

    /// Return local moderation ballot events after `since_sequence`, capped by `limit`.
    #[must_use]
    pub fn moderation_ballot_events_since(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
    ) -> Vec<ModerationBallotEvent> {
        self.moderation_ballot_events_replay(since_sequence, limit)
            .events
    }

    /// Return a gap-aware bounded replay of local moderation ballot events.
    #[must_use]
    pub fn moderation_ballot_events_replay(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
    ) -> EventReplay<ModerationBallotEvent> {
        self.moderation_events.read().map_or_else(
            |_| EventReplay {
                oldest_available_sequence: None,
                latest_sequence: None,
                gap: false,
                events: Vec::new(),
            },
            |guard| guard.replay(since_sequence, limit, |event| event.sequence),
        )
    }

    /// Return the latest local moderation ballot event sequence accepted by this node.
    #[must_use]
    pub fn latest_moderation_ballot_event_sequence(&self) -> Option<u64> {
        self.moderation_events
            .read()
            .ok()
            .and_then(|guard| (guard.latest_sequence != 0).then_some(guard.latest_sequence))
    }

    /// Subscribe to live local moderation ballot events.
    #[must_use]
    pub fn subscribe_moderation_ballot_events(&self) -> broadcast::Receiver<ModerationBallotEvent> {
        self.moderation_event_sender.subscribe()
    }

    fn mutate_moderation_ballot_durably<T>(
        &self,
        mutate: impl FnOnce(
            &mut ModerationBallotRuntime,
        )
            -> Result<(T, ModerationBallotEventInput), ModerationBallotRuntimeError>,
    ) -> Result<(T, ModerationBallotEvent), ModerationBallotRuntimeError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationBallotRuntimeError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(ModerationBallotRuntimeError::Checkpoint)?;
        let mut runtime = self
            .moderation
            .write()
            .map_err(|_| ModerationBallotRuntimeError::StateLockPoisoned)?;
        let previous_runtime = runtime.snapshot();
        let (outcome, input) = match mutate(&mut runtime) {
            Ok(value) => value,
            Err(err) => {
                if let Err(rollback) = runtime.restore_snapshot(previous_runtime) {
                    let message = self.record_unrecoverable_rollback(
                        "failed to roll back rejected moderation ballot mutation",
                        rollback,
                    );
                    return Err(ModerationBallotRuntimeError::Checkpoint(message));
                }
                return Err(err);
            }
        };
        let record = match runtime.ballot(&input.case_id, &input.round_id) {
            Some(record) => record,
            None => {
                if let Err(rollback) = runtime.restore_snapshot(previous_runtime) {
                    let message = self.record_unrecoverable_rollback(
                        "failed to roll back missing moderation ballot after mutation",
                        rollback,
                    );
                    return Err(ModerationBallotRuntimeError::Checkpoint(message));
                }
                return Err(ModerationBallotRuntimeError::InvalidSnapshot {
                    message: "accepted moderation mutation did not retain its ballot".to_owned(),
                });
            }
        };
        let mut events = match self.moderation_events.write() {
            Ok(events) => events,
            Err(_) => {
                if let Err(rollback) = runtime.restore_snapshot(previous_runtime) {
                    let message = self.record_unrecoverable_rollback(
                        "failed to roll back moderation mutation after event-lock failure",
                        rollback,
                    );
                    return Err(ModerationBallotRuntimeError::Checkpoint(message));
                }
                return Err(ModerationBallotRuntimeError::StateLockPoisoned);
            }
        };
        let previous_events = events.clone();
        let event = match events.append(|sequence| ModerationBallotEvent {
            sequence,
            kind: input.kind,
            generated_at_unix_ms: input.generated_at_unix_ms,
            case_id: input.case_id,
            round_id: input.round_id,
            juror_id: input.juror_id,
            committed_count: record.commits.len() as u64,
            revealed_count: record.reveals.len() as u64,
            challenge_count: record.challenges.len() as u64,
            tally: input.tally,
            challenge: input.challenge,
        }) {
            Ok(event) => event,
            Err(_) => {
                if let Err(rollback) = runtime.restore_snapshot(previous_runtime) {
                    let message = self.record_unrecoverable_rollback(
                        "failed to roll back moderation mutation after event-sequence exhaustion",
                        rollback,
                    );
                    return Err(ModerationBallotRuntimeError::Checkpoint(message));
                }
                return Err(ModerationBallotRuntimeError::EventSequenceOverflow);
            }
        };
        let mut snapshot = runtime.snapshot();
        snapshot.events = events.retained();
        if let Err(err) = self.persist_moderation_ballot_snapshot(&snapshot) {
            if err.committed {
                return Err(ModerationBallotRuntimeError::Checkpoint(err.to_string()));
            }
            let rollback = runtime.restore_snapshot(previous_runtime);
            *events = previous_events;
            if let Err(rollback) = rollback {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation checkpoint failure",
                    rollback,
                );
                return Err(ModerationBallotRuntimeError::Checkpoint(message));
            }
            return Err(ModerationBallotRuntimeError::Checkpoint(err.to_string()));
        }
        drop(events);
        drop(runtime);
        Ok((outcome, event))
    }

    fn publish_committed_moderation_ballot_event(&self, event: ModerationBallotEvent) {
        let _ = self.moderation_event_sender.send(event.clone());
        self.publish_moderation_ballot_governance_event(&event);
    }

    /// Announce a local SoraFS moderation ballot.
    ///
    /// This lifecycle store validates the ballot context, ordered roster hash,
    /// quorum, and commit/challenge/reveal windows, commits ballot plus event
    /// state atomically, then publishes the typed Governance DAG/transparency
    /// event. It does not submit an on-chain transaction.
    pub fn announce_moderation_ballot(
        &self,
        announcement: ModerationBallotAnnouncement,
    ) -> Result<ModerationBallotRecord, ModerationBallotRuntimeError> {
        let case_id = announcement.context.case_id.clone();
        let round_id = announcement.round_id.clone();
        let generated_at_unix_ms = announcement.announced_at_unix_ms;
        let (record, event) = self.mutate_moderation_ballot_durably(|runtime| {
            let record = runtime.announce_ballot(announcement)?;
            Ok((
                record,
                ModerationBallotEventInput {
                    kind: ModerationBallotEventKind::BallotAnnounced,
                    generated_at_unix_ms,
                    case_id,
                    round_id,
                    juror_id: None,
                    tally: None,
                    challenge: None,
                },
            ))
        })?;
        self.publish_committed_moderation_ballot_event(event);
        Ok(record)
    }

    /// Accept a local SoraFS moderation ballot commitment from an eligible juror.
    pub fn submit_moderation_ballot_commit(
        &self,
        commit: SoraFsModerationBallotCommitV1,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotCommitOutcome, ModerationBallotRuntimeError> {
        let case_id = commit.context.case_id.clone();
        let round_id = commit.round_id.clone();
        let juror_id = commit.juror_id.clone();
        let (outcome, event) = self.mutate_moderation_ballot_durably(|runtime| {
            let outcome = runtime.submit_commit(commit, now_unix_ms)?;
            Ok((
                outcome,
                ModerationBallotEventInput {
                    kind: ModerationBallotEventKind::CommitAccepted,
                    generated_at_unix_ms: now_unix_ms,
                    case_id,
                    round_id,
                    juror_id: Some(juror_id),
                    tally: None,
                    challenge: None,
                },
            ))
        })?;
        self.publish_committed_moderation_ballot_event(event);
        Ok(outcome)
    }

    /// Raise a local SoraFS moderation ballot challenge during the challenge window.
    pub fn submit_moderation_ballot_challenge(
        &self,
        input: ModerationBallotChallengeInput,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotChallengeRecord, ModerationBallotRuntimeError> {
        let (record, event) = self.mutate_moderation_ballot_durably(|runtime| {
            let record = runtime.submit_challenge(input, now_unix_ms)?;
            let event = ModerationBallotEventInput {
                kind: ModerationBallotEventKind::ChallengeSubmitted,
                generated_at_unix_ms: now_unix_ms,
                case_id: record.case_id.clone(),
                round_id: record.round_id.clone(),
                juror_id: None,
                tally: None,
                challenge: Some(record.clone()),
            };
            Ok((record, event))
        })?;
        self.publish_committed_moderation_ballot_event(event);
        Ok(record)
    }

    /// Resolve a local SoraFS moderation ballot challenge before reveal progress.
    pub fn resolve_moderation_ballot_challenge(
        &self,
        input: ModerationBallotChallengeResolution,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotChallengeRecord, ModerationBallotRuntimeError> {
        let (record, event) = self.mutate_moderation_ballot_durably(|runtime| {
            let record = runtime.resolve_challenge(input, now_unix_ms)?;
            let event = ModerationBallotEventInput {
                kind: ModerationBallotEventKind::ChallengeResolved,
                generated_at_unix_ms: now_unix_ms,
                case_id: record.case_id.clone(),
                round_id: record.round_id.clone(),
                juror_id: None,
                tally: None,
                challenge: Some(record.clone()),
            };
            Ok((record, event))
        })?;
        self.publish_committed_moderation_ballot_event(event);
        Ok(record)
    }

    /// Accept a local SoraFS moderation ballot reveal after the challenge buffer.
    pub fn submit_moderation_ballot_reveal(
        &self,
        reveal: SoraFsModerationBallotRevealV1,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotRevealOutcome, ModerationBallotRuntimeError> {
        let case_id = reveal.context.case_id.clone();
        let round_id = reveal.round_id.clone();
        let juror_id = reveal.juror_id.clone();
        let (outcome, event) = self.mutate_moderation_ballot_durably(|runtime| {
            let outcome = runtime.submit_reveal(reveal, now_unix_ms)?;
            Ok((
                outcome,
                ModerationBallotEventInput {
                    kind: ModerationBallotEventKind::RevealAccepted,
                    generated_at_unix_ms: now_unix_ms,
                    case_id,
                    round_id,
                    juror_id: Some(juror_id),
                    tally: None,
                    challenge: None,
                },
            ))
        })?;
        self.publish_committed_moderation_ballot_event(event);
        Ok(outcome)
    }

    /// Finalize a local SoraFS moderation ballot tally.
    pub fn tally_moderation_ballot(
        &self,
        case_id: &str,
        round_id: &str,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotTally, ModerationBallotRuntimeError> {
        let case_id_owned = case_id.to_owned();
        let round_id_owned = round_id.to_owned();
        let (tally, event) = self.mutate_moderation_ballot_durably(|runtime| {
            let tally = runtime.tally_ballot(case_id, round_id, now_unix_ms)?;
            Ok((
                tally.clone(),
                ModerationBallotEventInput {
                    kind: ModerationBallotEventKind::BallotTallied,
                    generated_at_unix_ms: now_unix_ms,
                    case_id: case_id_owned,
                    round_id: round_id_owned,
                    juror_id: None,
                    tally: Some(tally),
                    challenge: None,
                },
            ))
        })?;
        self.publish_committed_moderation_ballot_event(event);
        if let Some(record) = self.moderation_ballot(case_id, round_id) {
            self.publish_moderation_appeal_finance_report(&record, &tally);
        }
        Ok(tally)
    }

    /// Return the local payload-free no-show penalty plan for a closed ballot.
    ///
    /// The plan is read-only: it identifies roster jurors without accepted
    /// reveals, separates missing commits from committed no-shows, and binds the
    /// result to a deterministic digest without applying reputation or asset
    /// penalties.
    pub fn moderation_ballot_no_show_plan(
        &self,
        case_id: &str,
        round_id: &str,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotNoShowPlan, ModerationBallotRuntimeError> {
        self.moderation
            .read()
            .map_err(|_| ModerationBallotRuntimeError::StateLockPoisoned)?
            .no_show_plan(case_id, round_id, now_unix_ms)
    }

    /// Return one local SoraFS moderation ballot record.
    #[must_use]
    pub fn moderation_ballot(
        &self,
        case_id: &str,
        round_id: &str,
    ) -> Option<ModerationBallotRecord> {
        self.moderation
            .read()
            .ok()
            .and_then(|moderation| moderation.ballot(case_id, round_id))
    }

    /// Return all local SoraFS moderation ballot records.
    #[must_use]
    pub fn moderation_ballots(&self) -> Vec<ModerationBallotRecord> {
        self.moderation
            .read()
            .map_or_else(|_| Vec::new(), |moderation| moderation.ballots())
    }

    /// Return a deterministic local SoraFS moderation ballot snapshot.
    #[must_use]
    pub fn moderation_ballot_snapshot(&self) -> ModerationBallotSnapshot {
        self.export_moderation_ballot_snapshot()
            .unwrap_or_else(|_| ModerationBallotSnapshot::default())
    }

    /// Export local SoraFS moderation ballot state, reporting lock failures.
    ///
    /// The snapshot includes the validated local ballot records and the
    /// sequenced local event backlog used by Torii readback/replay endpoints.
    ///
    /// # Errors
    ///
    /// Returns an error if the moderation ballot runtime lock is poisoned.
    pub fn export_moderation_ballot_snapshot(
        &self,
    ) -> Result<ModerationBallotSnapshot, ModerationBallotRuntimeError> {
        let mut snapshot = self
            .moderation
            .read()
            .map_err(|_| ModerationBallotRuntimeError::StateLockPoisoned)?
            .snapshot();
        snapshot.events = self
            .moderation_events
            .read()
            .map_err(|_| ModerationBallotRuntimeError::StateLockPoisoned)?
            .retained();
        Ok(snapshot)
    }

    /// Replace local SoraFS moderation ballot state from a validated snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if the snapshot is internally inconsistent or the
    /// moderation ballot runtime lock is poisoned.
    pub fn restore_moderation_ballot_snapshot(
        &self,
        snapshot: ModerationBallotSnapshot,
    ) -> Result<(), ModerationBallotRuntimeError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| ModerationBallotRuntimeError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(ModerationBallotRuntimeError::Checkpoint)?;
        let previous = self.export_moderation_ballot_snapshot()?;
        if let Err(err) = self.restore_moderation_ballot_snapshot_in_memory(snapshot) {
            return Err(err);
        }
        let committed = self.export_moderation_ballot_snapshot()?;
        if let Err(err) = self.persist_moderation_ballot_snapshot(&committed) {
            if err.committed {
                return Err(ModerationBallotRuntimeError::Checkpoint(err.to_string()));
            }
            if let Err(rollback) = self.restore_moderation_ballot_snapshot_in_memory(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back moderation snapshot checkpoint failure",
                    rollback,
                );
                return Err(ModerationBallotRuntimeError::Checkpoint(message));
            }
            return Err(ModerationBallotRuntimeError::Checkpoint(err.to_string()));
        }
        Ok(())
    }

    /// Accept an order into the local SoraFS orderbook mirror and run matching.
    ///
    /// This deterministic local runtime mirror is for SFM-2 rollout testing. It
    /// does not submit an on-chain orderbook transaction or mutate escrow
    /// balances outside the settlement-channel snapshots it returns.
    pub fn submit_orderbook_order(
        &self,
        order: OrderRequestV1,
        now_unix: u64,
    ) -> Result<OrderbookSubmitOutcome, OrderbookRuntimeError> {
        let side = order.side;
        let tier = order.tier;
        let outcome =
            validate_orderbook_admission_policy(self.config.orderbook_admission_policy(), &order)
                .and_then(|()| {
                    self.mutate_orderbook_durably(now_unix, |runtime| {
                        // Reserve lifecycle changes use the same outer mutation lock. Keep this
                        // check inside the transaction so an ask cannot race an advert-disable
                        // transition between admission and the durable orderbook commit.
                        validate_orderbook_reserve_lifecycle_admission(
                            &self.reserve_lifecycle,
                            &order,
                        )?;
                        let outcome = runtime.submit_order(order, now_unix)?;
                        let input = OrderbookEventInput {
                            kind: OrderbookEventKind::OrderAccepted,
                            order_id: Some(outcome.accepted_order.order_id),
                            trade_ids: outcome
                                .fills
                                .iter()
                                .map(|fill| fill.trade.trade_id)
                                .collect(),
                            settlement_channel_ids: outcome
                                .settlement_channels_opened
                                .iter()
                                .map(|channel| channel.channel_id)
                                .collect(),
                            receipt_id: None,
                            expired_order_ids: outcome.expired_order_ids.clone(),
                        };
                        Ok((outcome, input))
                    })
                });
        match &outcome {
            Ok((_, event)) => {
                global_or_default().record_sorafs_orderbook_order(
                    ORDERBOOK_METRIC_CLUSTER_LOCAL,
                    orderbook_tier_label(tier),
                    orderbook_side_label(side),
                    "accepted",
                );
                self.record_orderbook_snapshot_metrics(now_unix);
                let _ = self.orderbook_event_sender.send(event.clone());
            }
            Err(err) => {
                let status = match err {
                    OrderbookRuntimeError::DuplicateOrderId { .. }
                    | OrderbookRuntimeError::StaleOwnerNonce { .. } => "duplicate",
                    OrderbookRuntimeError::Validation(_)
                    | OrderbookRuntimeError::OrderBelowMinimum { .. }
                    | OrderbookRuntimeError::OrderPriceTickMismatch { .. }
                    | OrderbookRuntimeError::ReserveLifecycleAdvertDisabled { .. }
                    | OrderbookRuntimeError::ResourceExhausted { .. } => "rejected",
                    OrderbookRuntimeError::SequenceOverflow
                    | OrderbookRuntimeError::EventSequenceOverflow
                    | OrderbookRuntimeError::MissingMatchedOrder
                    | OrderbookRuntimeError::InvalidMatchedSides
                    | OrderbookRuntimeError::StateLockPoisoned
                    | OrderbookRuntimeError::SettlementChannelNotFound { .. }
                    | OrderbookRuntimeError::DuplicateReceiptId { .. }
                    | OrderbookRuntimeError::ReceiptRangeOverlap { .. }
                    | OrderbookRuntimeError::OrderNotFound { .. }
                    | OrderbookRuntimeError::CancelOwnerMismatch
                    | OrderbookRuntimeError::SettlementLedgerOverflow
                    | OrderbookRuntimeError::InvalidSnapshot(_)
                    | OrderbookRuntimeError::Checkpoint(_) => "error",
                };
                global_or_default().record_sorafs_orderbook_order(
                    ORDERBOOK_METRIC_CLUSTER_LOCAL,
                    orderbook_tier_label(tier),
                    orderbook_side_label(side),
                    status,
                );
            }
        }
        outcome.map(|(outcome, _)| outcome)
    }

    /// Cancel an open order from the local SoraFS orderbook mirror.
    pub fn cancel_orderbook_order(
        &self,
        cancel: OrderCancelV1,
        now_unix: u64,
    ) -> Result<OrderbookCancelOutcome, OrderbookRuntimeError> {
        let (outcome, event) = self.mutate_orderbook_durably(now_unix, |runtime| {
            let outcome = runtime.cancel_order(cancel)?;
            let input = OrderbookEventInput {
                kind: OrderbookEventKind::OrderCancelled,
                order_id: Some(outcome.cancelled_order.order_id),
                trade_ids: Vec::new(),
                settlement_channel_ids: Vec::new(),
                receipt_id: None,
                expired_order_ids: Vec::new(),
            };
            Ok((outcome, input))
        })?;
        global_or_default().record_sorafs_orderbook_order(
            ORDERBOOK_METRIC_CLUSTER_LOCAL,
            "all",
            "all",
            "cancelled",
        );
        self.record_orderbook_snapshot_metrics(now_unix);
        let _ = self.orderbook_event_sender.send(event);
        Ok(outcome)
    }

    /// Apply a streaming-settlement receipt to a local SoraFS orderbook channel.
    ///
    /// The local mirror validates the receipt, rejects replayed receipt ids and
    /// overlapping byte ranges, then updates the in-memory channel snapshot. It
    /// does not yet mutate on-chain escrow balances.
    pub fn submit_orderbook_receipt(
        &self,
        receipt: SettlementReceiptV1,
        now_unix: u64,
    ) -> Result<OrderbookReceiptOutcome, OrderbookRuntimeError> {
        let (outcome, event) = self.mutate_orderbook_durably(now_unix, |runtime| {
            let outcome = runtime.submit_receipt(receipt)?;
            let input = OrderbookEventInput {
                kind: OrderbookEventKind::SettlementReceiptAccepted,
                order_id: None,
                trade_ids: Vec::new(),
                settlement_channel_ids: vec![outcome.updated_channel.channel_id],
                receipt_id: Some(outcome.accepted_receipt.receipt_id),
                expired_order_ids: Vec::new(),
            };
            Ok((outcome, input))
        })?;
        self.record_orderbook_snapshot_metrics(now_unix);
        self.publish_orderbook_settlement_receipt(&outcome.accepted_receipt);
        let _ = self.orderbook_event_sender.send(event);
        Ok(outcome)
    }

    /// Return a point-in-time snapshot of the local SoraFS orderbook mirror.
    pub fn orderbook_snapshot(
        &self,
        generated_at_unix: u64,
    ) -> Result<OrderbookSnapshot, OrderbookRuntimeError> {
        self.orderbook
            .read()
            .map_err(|_| OrderbookRuntimeError::StateLockPoisoned)?
            .snapshot(generated_at_unix)
    }

    /// Export the local orderbook mirror as a canonical Norito replay snapshot.
    ///
    /// The returned payload is a checkpoint for the local mirror only. It does
    /// not replace contract state or escrow custody mutation.
    pub fn export_orderbook_runtime_snapshot(
        &self,
        generated_at_unix: u64,
    ) -> Result<OrderbookRuntimeSnapshotV1, OrderbookRuntimeError> {
        let snapshot = self
            .orderbook
            .read()
            .map_err(|_| OrderbookRuntimeError::StateLockPoisoned)?
            .runtime_snapshot(generated_at_unix);
        snapshot.validate()?;
        Ok(snapshot)
    }

    /// Replace the local orderbook mirror from a validated replay snapshot.
    ///
    /// Event backlog replay is intentionally out of scope for this checkpoint;
    /// live event streams resume from subsequent local events after restore.
    pub fn restore_orderbook_runtime_snapshot(
        &self,
        snapshot: OrderbookRuntimeSnapshotV1,
    ) -> Result<(), OrderbookRuntimeError> {
        let generated_at_unix = snapshot.generated_at_unix;
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| OrderbookRuntimeError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(OrderbookRuntimeError::Checkpoint)?;
        let retention = self.config.runtime_retention();
        let mut restored = OrderbookRuntime::with_entry_limit(retention.state_entry_limit());
        restored.restore_runtime_snapshot(snapshot)?;
        let mut orderbook = self
            .orderbook
            .write()
            .map_err(|_| OrderbookRuntimeError::StateLockPoisoned)?;
        let mut events = self
            .orderbook_events
            .write()
            .map_err(|_| OrderbookRuntimeError::StateLockPoisoned)?;
        let previous_runtime = orderbook.runtime_snapshot(generated_at_unix.max(1));
        let previous_events = events.clone();
        *orderbook = restored;
        *events = BoundedEventHistory::new(retention.event_history_limit());
        let checkpoint = OrderbookRuntimeCheckpointV1 {
            version: ORDERBOOK_RUNTIME_STATE_VERSION_V1,
            runtime: orderbook.runtime_snapshot(generated_at_unix),
            events: Vec::new(),
        };
        if let Err(err) = self.persist_orderbook_checkpoint(&checkpoint) {
            if err.committed {
                return Err(OrderbookRuntimeError::Checkpoint(err.to_string()));
            }
            let rollback = orderbook.restore_runtime_snapshot(previous_runtime);
            *events = previous_events;
            if let Err(rollback) = rollback {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back restored orderbook snapshot after checkpoint failure",
                    rollback,
                );
                return Err(OrderbookRuntimeError::Checkpoint(message));
            }
            return Err(OrderbookRuntimeError::Checkpoint(err.to_string()));
        }
        drop(events);
        drop(orderbook);
        self.record_orderbook_snapshot_metrics(generated_at_unix);
        Ok(())
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
        let snapshot = norito::decode_from_bytes::<ModerationModelRegistrySnapshot>(&bytes)
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
        let snapshot = norito::decode_from_bytes::<ModerationScreeningSnapshot>(&bytes)
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
        let snapshot = norito::decode_from_bytes::<ModerationQuarantineObjectSnapshot>(&bytes)
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
        let key_path = self
            .moderation_quarantine_object_key_path
            .as_ref()
            .expect("storage-backed node has a quarantine key path");
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
        let key = match read_local_checkpoint_bounded(key_path, 32).map_err(|err| {
            NodeInitError::checkpoint("moderation quarantine object key", key_path, err)
        })? {
            Some(bytes) => Some(
                decode_moderation_quarantine_object_key(key_path, bytes).map_err(|err| {
                    NodeInitError::checkpoint("moderation quarantine object key", key_path, err)
                })?,
            ),
            None if snapshot.objects.is_empty() => None,
            None => {
                return Err(NodeInitError::checkpoint(
                    "moderation quarantine object key",
                    key_path,
                    "sealing key is missing while indexed objects exist",
                ));
            }
        };
        let mut expected_files = BTreeSet::from([index_path.clone()]);
        if key.is_some() {
            expected_files.insert(key_path.clone());
        }
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
                envelope,
                record,
                key.expect("non-empty object index requires a loaded sealing key"),
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
        let snapshot = norito::decode_from_bytes::<ModerationEvidenceViewerSnapshot>(&bytes)
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

    fn restore_moderation_ballot_snapshot_in_memory(
        &self,
        snapshot: ModerationBallotSnapshot,
    ) -> Result<(usize, usize), ModerationBallotRuntimeError> {
        let retention = self.config.runtime_retention();
        let mut restored_moderation =
            ModerationBallotRuntime::with_entry_limit(retention.state_entry_limit());
        restored_moderation.ensure_snapshot_capacity(&snapshot)?;
        Self::validate_moderation_ballot_event_backlog(&snapshot)?;
        let ballot_count = snapshot.ballots.len();
        let event_count = snapshot.events.len();
        let events = snapshot.events;
        let ballots = snapshot.ballots;
        restored_moderation.restore_snapshot(ModerationBallotSnapshot {
            ballots,
            events: Vec::new(),
        })?;
        let mut restored_events = BoundedEventHistory::new(retention.event_history_limit());
        restored_events
            .restore(events, |event| event.sequence)
            .map_err(|err| ModerationBallotRuntimeError::InvalidSnapshot {
                message: err.to_string(),
            })?;
        let mut moderation = self
            .moderation
            .write()
            .map_err(|_| ModerationBallotRuntimeError::StateLockPoisoned)?;
        let mut event_backlog = self
            .moderation_events
            .write()
            .map_err(|_| ModerationBallotRuntimeError::StateLockPoisoned)?;
        *moderation = restored_moderation;
        *event_backlog = restored_events;
        Ok((ballot_count, event_count))
    }

    fn export_auxiliary_runtime_checkpoint(
        &self,
    ) -> Result<AuxiliaryRuntimeCheckpointV1, GovernancePublishError> {
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
        let repair_events = self
            .repair_events
            .read()
            .map_err(|_| GovernancePublishError::other("repair event history lock poisoned"))?
            .retained();
        let reserve_runtime = self
            .reserve_lifecycle
            .read()
            .map_err(|_| GovernancePublishError::other("reserve lifecycle lock poisoned"))?
            .checkpoint();
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
        let published_privacy_aggregate_cycles = self
            .published_privacy_aggregate_cycles
            .read()
            .map_err(|_| GovernancePublishError::other("privacy cycle index poisoned"))?
            .iter()
            .copied()
            .collect();
        let published_evidence_viewer_audit_cycles = self
            .published_evidence_viewer_audit_cycles
            .read()
            .map_err(|_| GovernancePublishError::other("evidence cycle index poisoned"))?
            .iter()
            .copied()
            .collect();
        Ok(AuxiliaryRuntimeCheckpointV1 {
            version: AUX_RUNTIME_STATE_VERSION_V1,
            capacity_runtime,
            deal_runtime,
            por_tracker: self.por.checkpoint(),
            por_history,
            reserve_runtime,
            repair_events,
            reputation_snapshots,
            latest_reputation_snapshot_id,
            reputation_events,
            transparency_source_entries,
            privacy_source_events,
            published_privacy_aggregate_cycles,
            published_evidence_viewer_audit_cycles,
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

    fn mutate_reserve_runtime_durably<T, E>(
        &self,
        mutate: impl FnOnce(&mut ReserveLifecycleRuntime) -> Result<(T, bool), E>,
        state_lock_error: impl Fn() -> E,
        checkpoint_error: impl Fn(String) -> E,
    ) -> Result<T, E> {
        // Orderbook ask admission takes this lock before reading reserve state.
        // Reserve transitions must use the same ordering so advert disablement
        // and order admission have one deterministic linearization point.
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| state_lock_error())?;
        let _checkpoint_guard = self
            .auxiliary_checkpoint_lock
            .lock()
            .map_err(|_| state_lock_error())?;
        self.ensure_durability_healthy()
            .map_err(&checkpoint_error)?;
        let mut runtime = self
            .reserve_lifecycle
            .write()
            .map_err(|_| state_lock_error())?;
        let previous = runtime.checkpoint();
        let (outcome, changed) = match mutate(&mut runtime) {
            Ok(outcome) => outcome,
            Err(err) => {
                if let Err(restore_err) = runtime.restore_checkpoint(previous) {
                    let message = self.record_unrecoverable_rollback(
                        "failed to roll back rejected reserve runtime mutation",
                        restore_err,
                    );
                    return Err(checkpoint_error(message));
                }
                return Err(err);
            }
        };
        drop(runtime);
        if !changed {
            return Ok(outcome);
        }
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(checkpoint_error(err.to_string()));
            }
            let mut runtime = match self.reserve_lifecycle.write() {
                Ok(runtime) => runtime,
                Err(_) => {
                    let message = self.record_unrecoverable_rollback(
                        "failed to acquire reserve runtime lock while rolling back checkpoint failure",
                        "state lock poisoned",
                    );
                    return Err(checkpoint_error(message));
                }
            };
            if let Err(restore_err) = runtime.restore_checkpoint(previous) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back reserve checkpoint failure",
                    restore_err,
                );
                return Err(checkpoint_error(message));
            }
            return Err(checkpoint_error(err.to_string()));
        }
        Ok(outcome)
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
        let checkpoint = norito::decode_from_bytes::<AuxiliaryRuntimeCheckpointV1>(&bytes)
            .map_err(|err| NodeInitError::checkpoint("auxiliary runtime", path, err))?;
        self.restore_auxiliary_runtime_checkpoint(checkpoint)
            .map_err(|err| NodeInitError::checkpoint("auxiliary runtime", path, err))?;
        Ok(())
    }

    fn restore_auxiliary_runtime_checkpoint(
        &self,
        checkpoint: AuxiliaryRuntimeCheckpointV1,
    ) -> Result<(), GovernancePublishError> {
        if checkpoint.version != AUX_RUNTIME_STATE_VERSION_V1 {
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
                "published privacy cycles",
                checkpoint.published_privacy_aggregate_cycles.len(),
                state_limit,
            ),
            (
                "published evidence cycles",
                checkpoint.published_evidence_viewer_audit_cycles.len(),
                state_limit,
            ),
            ("repair events", checkpoint.repair_events.len(), event_limit),
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
        for snapshot in checkpoint.reputation_snapshots {
            snapshot.validate().map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid reputation snapshot in auxiliary checkpoint: {err}"
                ))
            })?;
            if snapshots.insert(snapshot.snapshot_id, snapshot).is_some() {
                return Err(GovernancePublishError::other(
                    "duplicate reputation snapshot id in auxiliary checkpoint",
                ));
            }
        }
        let latest_snapshot = match checkpoint.latest_reputation_snapshot_id {
            Some(snapshot_id) => Some(snapshots.get(&snapshot_id).cloned().ok_or_else(|| {
                GovernancePublishError::other(
                    "latest reputation snapshot id is absent from auxiliary checkpoint",
                )
            })?),
            None if snapshots.is_empty() => None,
            None => {
                return Err(GovernancePublishError::other(
                    "non-empty reputation snapshot checkpoint is missing latest id",
                ));
            }
        };
        let mut repair_events = BoundedEventHistory::new(event_limit);
        repair_events.restore(checkpoint.repair_events, |event| event.sequence)?;
        let mut reputation_events = BoundedEventHistory::new(event_limit);
        let mut previous_reputation_event: Option<&ReputationSnapshotEventV1> = None;
        for event in &checkpoint.reputation_events {
            event.validate().map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid reputation event in auxiliary checkpoint: {err}"
                ))
            })?;
            let Some(snapshot) = snapshots.get(&event.snapshot_id) else {
                return Err(GovernancePublishError::other(
                    "reputation event references a missing retained snapshot",
                ));
            };
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
        let mut reserve_runtime = ReserveLifecycleRuntime::with_limits(state_limit, event_limit);
        reserve_runtime
            .restore_checkpoint(checkpoint.reserve_runtime)
            .map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid reserve runtime auxiliary checkpoint: {err}"
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
        let mut privacy_events = BTreeMap::new();
        for event in checkpoint.privacy_source_events {
            event.validate().map_err(|err| {
                GovernancePublishError::other(format!(
                    "invalid privacy source event in auxiliary checkpoint: {err}"
                ))
            })?;
            if privacy_events
                .insert(event.event_id.clone(), event)
                .is_some()
            {
                return Err(GovernancePublishError::other(
                    "duplicate privacy source event in auxiliary checkpoint",
                ));
            }
        }
        let privacy_cycle_count = checkpoint.published_privacy_aggregate_cycles.len();
        let evidence_cycle_count = checkpoint.published_evidence_viewer_audit_cycles.len();
        let privacy_cycles = checkpoint
            .published_privacy_aggregate_cycles
            .into_iter()
            .collect::<BTreeSet<_>>();
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
            .repair_events
            .write()
            .map_err(|_| GovernancePublishError::other("repair event history lock poisoned"))? =
            repair_events;
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
        *self
            .reserve_lifecycle
            .write()
            .map_err(|_| GovernancePublishError::other("reserve lifecycle lock poisoned"))? =
            reserve_runtime;
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
        *self
            .published_privacy_aggregate_cycles
            .write()
            .map_err(|_| GovernancePublishError::other("privacy cycle index poisoned"))? =
            privacy_cycles;
        *self
            .published_evidence_viewer_audit_cycles
            .write()
            .map_err(|_| GovernancePublishError::other("evidence cycle index poisoned"))? =
            evidence_cycles;
        Ok(())
    }

    fn validate_moderation_ballot_event_backlog(
        snapshot: &ModerationBallotSnapshot,
    ) -> Result<(), ModerationBallotRuntimeError> {
        #[derive(Default)]
        struct ReplayState {
            announced: bool,
            commits: BTreeSet<String>,
            reveals: BTreeSet<String>,
            challenges: BTreeMap<String, ModerationBallotChallengeRecord>,
            tallied: bool,
        }

        let invalid = |message: String| ModerationBallotRuntimeError::InvalidSnapshot { message };
        let mut records = BTreeMap::new();
        let mut replay = BTreeMap::new();
        for record in &snapshot.ballots {
            let key = (
                record.announcement.context.case_id.clone(),
                record.announcement.round_id.clone(),
            );
            if records.insert(key.clone(), record).is_some() {
                return Err(invalid(format!(
                    "duplicate moderation ballot `{}` round `{}` in event snapshot",
                    key.0, key.1
                )));
            }
            replay.insert(key, ReplayState::default());
        }

        let first_sequence = snapshot.events.first().map_or(1, |event| event.sequence);
        for (index, event) in snapshot.events.iter().enumerate() {
            let expected_sequence = u64::try_from(index)
                .ok()
                .and_then(|value| first_sequence.checked_add(value))
                .ok_or_else(|| invalid("moderation ballot event sequence overflow".to_owned()))?;
            if event.sequence != expected_sequence {
                return Err(invalid(format!(
                    "moderation ballot event sequence `{}` does not match expected `{expected_sequence}`",
                    event.sequence
                )));
            }
        }

        if first_sequence > 1 {
            #[derive(Default)]
            struct RetainedReplayState {
                last_counts: Option<(u64, u64, u64)>,
                phase: u8,
                commits: BTreeSet<String>,
                reveals: BTreeSet<String>,
                submitted_challenges: BTreeSet<String>,
                resolved_challenges: BTreeSet<String>,
                tallied: bool,
            }

            let mut retained = BTreeMap::<(String, String), RetainedReplayState>::new();
            for event in &snapshot.events {
                let key = (event.case_id.clone(), event.round_id.clone());
                let record = records.get(&key).ok_or_else(|| {
                    invalid(format!(
                        "moderation ballot event `{}` references unknown ballot `{}` round `{}`",
                        event.sequence, event.case_id, event.round_id
                    ))
                })?;
                let state = retained.entry(key).or_default();
                if event.committed_count > record.commits.len() as u64
                    || event.revealed_count > record.reveals.len() as u64
                    || event.challenge_count > record.challenges.len() as u64
                {
                    return Err(invalid(format!(
                        "truncated moderation event `{}` exceeds saved ballot counters",
                        event.sequence
                    )));
                }
                if state.tallied {
                    return Err(invalid(format!(
                        "truncated moderation event `{}` appears after the ballot tally",
                        event.sequence
                    )));
                }
                if let Some((committed, revealed, challenged)) = state.last_counts
                    && (event.committed_count < committed
                        || event.revealed_count < revealed
                        || event.challenge_count < challenged)
                {
                    return Err(invalid(format!(
                        "truncated moderation event `{}` regresses saved counters",
                        event.sequence
                    )));
                }

                let previous_counts = state.last_counts;
                let phase = match event.kind {
                    ModerationBallotEventKind::BallotAnnounced => {
                        if previous_counts.is_some()
                            || event.juror_id.is_some()
                            || event.committed_count != 0
                            || event.revealed_count != 0
                            || event.challenge_count != 0
                            || event.tally.is_some()
                            || event.challenge.is_some()
                            || event.generated_at_unix_ms
                                != record.announcement.announced_at_unix_ms
                        {
                            return Err(invalid(format!(
                                "truncated announcement event `{}` does not match saved ballot state",
                                event.sequence
                            )));
                        }
                        0
                    }
                    ModerationBallotEventKind::CommitAccepted => {
                        let juror_id = event.juror_id.as_ref().ok_or_else(|| {
                            invalid(format!(
                                "truncated commit event `{}` is missing juror id",
                                event.sequence
                            ))
                        })?;
                        let commit = record
                            .commits
                            .iter()
                            .find(|commit| commit.juror_id == *juror_id)
                            .ok_or_else(|| {
                                invalid(format!(
                                    "truncated commit event `{}` references an absent commit",
                                    event.sequence
                                ))
                            })?;
                        if event.tally.is_some()
                            || event.challenge.is_some()
                            || event.generated_at_unix_ms < record.announcement.announced_at_unix_ms
                            || event.generated_at_unix_ms
                                > record.announcement.commit_deadline_unix_ms
                            || event.generated_at_unix_ms != commit.committed_at_unix_ms
                            || commit.context.case_id != record.announcement.context.case_id
                            || commit.round_id != record.announcement.round_id
                            || !state.commits.insert(juror_id.clone())
                        {
                            return Err(invalid(format!(
                                "truncated commit event `{}` does not match saved commit state",
                                event.sequence
                            )));
                        }
                        if let Some((committed, revealed, challenged)) = previous_counts
                            && (event.committed_count != committed.saturating_add(1)
                                || event.revealed_count != revealed
                                || event.challenge_count != challenged)
                        {
                            return Err(invalid(format!(
                                "truncated commit event `{}` has an invalid counter transition",
                                event.sequence
                            )));
                        }
                        if event.committed_count == 0 {
                            return Err(invalid(format!(
                                "truncated commit event `{}` has a zero commit count",
                                event.sequence
                            )));
                        }
                        1
                    }
                    ModerationBallotEventKind::ChallengeSubmitted => {
                        let challenge = event.challenge.as_ref().ok_or_else(|| {
                            invalid(format!(
                                "truncated challenge submission event `{}` is missing challenge data",
                                event.sequence
                            ))
                        })?;
                        let saved = record
                            .challenges
                            .iter()
                            .find(|saved| saved.challenge_id == challenge.challenge_id)
                            .ok_or_else(|| {
                                invalid(format!(
                                    "truncated challenge submission event `{}` references an absent challenge",
                                    event.sequence
                                ))
                            })?;
                        let mut expected = saved.clone();
                        expected.decision = None;
                        expected.resolved_by = None;
                        expected.resolved_at_unix_ms = None;
                        expected.resolution_note = None;
                        if event.juror_id.is_some()
                            || event.tally.is_some()
                            || challenge != &expected
                            || event.generated_at_unix_ms != challenge.raised_at_unix_ms
                            || event.generated_at_unix_ms
                                <= record.announcement.commit_deadline_unix_ms
                            || event.generated_at_unix_ms
                                > record.announcement.challenge_deadline_unix_ms
                            || !state
                                .submitted_challenges
                                .insert(challenge.challenge_id.clone())
                        {
                            return Err(invalid(format!(
                                "truncated challenge submission event `{}` does not match saved challenge state",
                                event.sequence
                            )));
                        }
                        if let Some((committed, revealed, challenged)) = previous_counts
                            && (event.committed_count != committed
                                || event.revealed_count != revealed
                                || event.challenge_count != challenged.saturating_add(1))
                        {
                            return Err(invalid(format!(
                                "truncated challenge submission event `{}` has an invalid counter transition",
                                event.sequence
                            )));
                        }
                        if event.challenge_count == 0 {
                            return Err(invalid(format!(
                                "truncated challenge submission event `{}` has a zero challenge count",
                                event.sequence
                            )));
                        }
                        2
                    }
                    ModerationBallotEventKind::ChallengeResolved => {
                        let challenge = event.challenge.as_ref().ok_or_else(|| {
                            invalid(format!(
                                "truncated challenge resolution event `{}` is missing challenge data",
                                event.sequence
                            ))
                        })?;
                        let saved = record
                            .challenges
                            .iter()
                            .find(|saved| saved.challenge_id == challenge.challenge_id);
                        if event.juror_id.is_some()
                            || event.tally.is_some()
                            || saved != Some(challenge)
                            || challenge.decision.is_none()
                            || challenge
                                .resolved_by
                                .as_deref()
                                .is_none_or(|resolver| resolver.trim().is_empty())
                            || challenge.resolved_at_unix_ms.is_none()
                            || event.generated_at_unix_ms
                                != challenge.resolved_at_unix_ms.unwrap_or(0)
                            || challenge
                                .resolved_at_unix_ms
                                .is_some_and(|resolved| resolved < challenge.raised_at_unix_ms)
                            || !state
                                .resolved_challenges
                                .insert(challenge.challenge_id.clone())
                        {
                            return Err(invalid(format!(
                                "truncated challenge resolution event `{}` does not match saved challenge state",
                                event.sequence
                            )));
                        }
                        if let Some((committed, revealed, challenged)) = previous_counts
                            && (event.committed_count != committed
                                || event.revealed_count != revealed
                                || event.challenge_count != challenged)
                        {
                            return Err(invalid(format!(
                                "truncated challenge resolution event `{}` changes ballot counters",
                                event.sequence
                            )));
                        }
                        if event.challenge_count == 0 {
                            return Err(invalid(format!(
                                "truncated challenge resolution event `{}` has a zero challenge count",
                                event.sequence
                            )));
                        }
                        2
                    }
                    ModerationBallotEventKind::RevealAccepted => {
                        let juror_id = event.juror_id.as_ref().ok_or_else(|| {
                            invalid(format!(
                                "truncated reveal event `{}` is missing juror id",
                                event.sequence
                            ))
                        })?;
                        let reveal = record
                            .reveals
                            .iter()
                            .find(|reveal| reveal.juror_id == *juror_id)
                            .ok_or_else(|| {
                                invalid(format!(
                                    "truncated reveal event `{}` references an absent reveal",
                                    event.sequence
                                ))
                            })?;
                        if event.tally.is_some()
                            || event.challenge.is_some()
                            || event.generated_at_unix_ms
                                <= record.announcement.challenge_deadline_unix_ms
                            || event.generated_at_unix_ms
                                > record.announcement.reveal_deadline_unix_ms
                            || event.generated_at_unix_ms != reveal.revealed_at_unix_ms
                            || reveal.context.case_id != record.announcement.context.case_id
                            || reveal.round_id != record.announcement.round_id
                            || !state.reveals.insert(juror_id.clone())
                        {
                            return Err(invalid(format!(
                                "truncated reveal event `{}` does not match saved reveal state",
                                event.sequence
                            )));
                        }
                        if let Some((committed, revealed, challenged)) = previous_counts
                            && (event.committed_count != committed
                                || event.revealed_count != revealed.saturating_add(1)
                                || event.challenge_count != challenged)
                        {
                            return Err(invalid(format!(
                                "truncated reveal event `{}` has an invalid counter transition",
                                event.sequence
                            )));
                        }
                        if event.revealed_count == 0 {
                            return Err(invalid(format!(
                                "truncated reveal event `{}` has a zero reveal count",
                                event.sequence
                            )));
                        }
                        3
                    }
                    ModerationBallotEventKind::BallotTallied => {
                        let tally = record.tally.as_ref().ok_or_else(|| {
                            invalid(format!(
                                "truncated tally event `{}` references an untallied ballot",
                                event.sequence
                            ))
                        })?;
                        if event.juror_id.is_some()
                            || event.challenge.is_some()
                            || event.tally.as_ref() != Some(tally)
                            || event.generated_at_unix_ms != tally.tallied_at_unix_ms
                            || event.committed_count != record.commits.len() as u64
                            || event.revealed_count != record.reveals.len() as u64
                            || event.challenge_count != record.challenges.len() as u64
                        {
                            return Err(invalid(format!(
                                "truncated tally event `{}` does not match saved tally state",
                                event.sequence
                            )));
                        }
                        state.tallied = true;
                        4
                    }
                };
                if phase < state.phase {
                    return Err(invalid(format!(
                        "truncated moderation event `{}` regresses ballot lifecycle phase",
                        event.sequence
                    )));
                }
                state.phase = phase;
                state.last_counts = Some((
                    event.committed_count,
                    event.revealed_count,
                    event.challenge_count,
                ));
            }

            for (key, state) in retained {
                let record = records.get(&key).ok_or_else(|| {
                    invalid("retained moderation event lost its ballot binding".to_owned())
                })?;
                let expected_counts = (
                    record.commits.len() as u64,
                    record.reveals.len() as u64,
                    record.challenges.len() as u64,
                );
                if state.last_counts != Some(expected_counts)
                    || state.tallied != record.tally.is_some()
                {
                    return Err(invalid(format!(
                        "truncated event suffix does not reach saved ballot `{}` round `{}` state",
                        key.0, key.1
                    )));
                }
                for challenge_id in state.submitted_challenges {
                    let saved = record
                        .challenges
                        .iter()
                        .find(|challenge| challenge.challenge_id == challenge_id)
                        .ok_or_else(|| {
                            invalid(format!(
                                "truncated challenge `{challenge_id}` disappeared from saved ballot state"
                            ))
                        })?;
                    if saved.decision.is_some()
                        && !state.resolved_challenges.contains(&challenge_id)
                    {
                        return Err(invalid(format!(
                            "truncated challenge `{challenge_id}` omits its retained resolution"
                        )));
                    }
                }
            }
            return Ok(());
        }

        for event in &snapshot.events {
            let key = (event.case_id.clone(), event.round_id.clone());
            let record = records.get(&key).ok_or_else(|| {
                invalid(format!(
                    "moderation ballot event `{}` references unknown ballot `{}` round `{}`",
                    event.sequence, event.case_id, event.round_id
                ))
            })?;
            let state = replay.get_mut(&key).ok_or_else(|| {
                invalid(format!(
                    "moderation ballot replay state is missing for `{}` round `{}`",
                    key.0, key.1
                ))
            })?;
            match event.kind {
                ModerationBallotEventKind::BallotAnnounced => {
                    if state.announced {
                        return Err(invalid(format!(
                            "duplicate announcement event for ballot `{}` round `{}`",
                            event.case_id, event.round_id
                        )));
                    }
                    if event.juror_id.is_some()
                        || event.committed_count != 0
                        || event.revealed_count != 0
                        || event.challenge_count != 0
                        || event.tally.is_some()
                        || event.challenge.is_some()
                    {
                        return Err(invalid(format!(
                            "announcement event `{}` carries commit, reveal, challenge, juror, or tally data",
                            event.sequence
                        )));
                    }
                    if event.generated_at_unix_ms != record.announcement.announced_at_unix_ms {
                        return Err(invalid(format!(
                            "announcement event `{}` timestamp does not match ballot announcement",
                            event.sequence
                        )));
                    }
                    state.announced = true;
                }
                ModerationBallotEventKind::CommitAccepted => {
                    if !state.announced || state.tallied {
                        return Err(invalid(format!(
                            "commit event `{}` is outside the announced ballot lifecycle",
                            event.sequence
                        )));
                    }
                    let juror_id = event.juror_id.as_ref().ok_or_else(|| {
                        invalid(format!(
                            "commit event `{}` is missing juror id",
                            event.sequence
                        ))
                    })?;
                    if event.tally.is_some() {
                        return Err(invalid(format!(
                            "commit event `{}` unexpectedly carries tally data",
                            event.sequence
                        )));
                    }
                    if event.challenge.is_some() {
                        return Err(invalid(format!(
                            "commit event `{}` unexpectedly carries challenge data",
                            event.sequence
                        )));
                    }
                    let commit = record
                        .commits
                        .iter()
                        .find(|commit| commit.juror_id == *juror_id)
                        .ok_or_else(|| {
                            invalid(format!(
                                "commit event `{}` references juror `{juror_id}` without a saved commit",
                                event.sequence
                            ))
                        })?;
                    if event.generated_at_unix_ms < record.announcement.announced_at_unix_ms
                        || event.generated_at_unix_ms > record.announcement.commit_deadline_unix_ms
                        || event.generated_at_unix_ms != commit.committed_at_unix_ms
                    {
                        return Err(invalid(format!(
                            "commit event `{}` timestamp is outside the commit window",
                            event.sequence
                        )));
                    }
                    if !state.commits.insert(juror_id.clone()) {
                        return Err(invalid(format!(
                            "duplicate commit event for juror `{juror_id}` in ballot `{}` round `{}`",
                            event.case_id, event.round_id
                        )));
                    }
                    if commit.context.case_id != record.announcement.context.case_id
                        || commit.round_id != record.announcement.round_id
                    {
                        return Err(invalid(format!(
                            "commit event `{}` references a saved commit with mismatched scope",
                            event.sequence
                        )));
                    }
                    if event.committed_count != state.commits.len() as u64
                        || event.revealed_count != state.reveals.len() as u64
                        || event.challenge_count != state.challenges.len() as u64
                    {
                        return Err(invalid(format!(
                            "commit event `{}` count fields do not match replayed state",
                            event.sequence
                        )));
                    }
                }
                ModerationBallotEventKind::ChallengeSubmitted => {
                    if !state.announced || state.tallied || !state.reveals.is_empty() {
                        return Err(invalid(format!(
                            "challenge submission event `{}` is outside the announced challenge lifecycle",
                            event.sequence
                        )));
                    }
                    if event.juror_id.is_some() || event.tally.is_some() {
                        return Err(invalid(format!(
                            "challenge submission event `{}` unexpectedly carries juror or tally data",
                            event.sequence
                        )));
                    }
                    let challenge = event.challenge.as_ref().ok_or_else(|| {
                        invalid(format!(
                            "challenge submission event `{}` is missing challenge data",
                            event.sequence
                        ))
                    })?;
                    if challenge.decision.is_some()
                        || challenge.resolved_by.is_some()
                        || challenge.resolved_at_unix_ms.is_some()
                        || challenge.resolution_note.is_some()
                    {
                        return Err(invalid(format!(
                            "challenge submission event `{}` unexpectedly carries resolution data",
                            event.sequence
                        )));
                    }
                    let saved = record
                        .challenges
                        .iter()
                        .find(|saved| saved.challenge_id == challenge.challenge_id)
                        .ok_or_else(|| {
                            invalid(format!(
                                "challenge submission event `{}` references challenge `{}` without a saved record",
                                event.sequence, challenge.challenge_id
                            ))
                        })?;
                    let mut expected = saved.clone();
                    expected.decision = None;
                    expected.resolved_by = None;
                    expected.resolved_at_unix_ms = None;
                    expected.resolution_note = None;
                    if challenge != &expected {
                        return Err(invalid(format!(
                            "challenge submission event `{}` challenge payload does not match saved challenge start state",
                            event.sequence
                        )));
                    }
                    if event.generated_at_unix_ms != challenge.raised_at_unix_ms
                        || event.generated_at_unix_ms <= record.announcement.commit_deadline_unix_ms
                        || event.generated_at_unix_ms
                            > record.announcement.challenge_deadline_unix_ms
                    {
                        return Err(invalid(format!(
                            "challenge submission event `{}` timestamp is outside the challenge window",
                            event.sequence
                        )));
                    }
                    if state
                        .challenges
                        .insert(challenge.challenge_id.clone(), challenge.clone())
                        .is_some()
                    {
                        return Err(invalid(format!(
                            "duplicate challenge submission event for challenge `{}` in ballot `{}` round `{}`",
                            challenge.challenge_id, event.case_id, event.round_id
                        )));
                    }
                    if event.committed_count != state.commits.len() as u64
                        || event.revealed_count != state.reveals.len() as u64
                        || event.challenge_count != state.challenges.len() as u64
                    {
                        return Err(invalid(format!(
                            "challenge submission event `{}` count fields do not match replayed state",
                            event.sequence
                        )));
                    }
                }
                ModerationBallotEventKind::ChallengeResolved => {
                    if !state.announced || state.tallied || !state.reveals.is_empty() {
                        return Err(invalid(format!(
                            "challenge resolution event `{}` is outside the announced challenge lifecycle",
                            event.sequence
                        )));
                    }
                    if event.juror_id.is_some() || event.tally.is_some() {
                        return Err(invalid(format!(
                            "challenge resolution event `{}` unexpectedly carries juror or tally data",
                            event.sequence
                        )));
                    }
                    let challenge = event.challenge.as_ref().ok_or_else(|| {
                        invalid(format!(
                            "challenge resolution event `{}` is missing challenge data",
                            event.sequence
                        ))
                    })?;
                    let missing_resolver = match challenge.resolved_by.as_deref() {
                        Some(resolved_by) => resolved_by.trim().is_empty(),
                        None => true,
                    };
                    if challenge.decision.is_none()
                        || missing_resolver
                        || challenge.resolved_at_unix_ms.is_none()
                    {
                        return Err(invalid(format!(
                            "challenge resolution event `{}` is missing resolution data",
                            event.sequence
                        )));
                    }
                    if challenge
                        .resolution_note
                        .as_deref()
                        .is_some_and(|note| note.trim().is_empty())
                    {
                        return Err(invalid(format!(
                            "challenge resolution event `{}` carries a blank resolution note",
                            event.sequence
                        )));
                    }
                    let Some(previous) = state.challenges.get(&challenge.challenge_id) else {
                        return Err(invalid(format!(
                            "challenge resolution event `{}` appears before challenge `{}` was submitted",
                            event.sequence, challenge.challenge_id
                        )));
                    };
                    if previous.decision.is_some() {
                        return Err(invalid(format!(
                            "duplicate challenge resolution event for challenge `{}` in ballot `{}` round `{}`",
                            challenge.challenge_id, event.case_id, event.round_id
                        )));
                    }
                    if previous.case_id != challenge.case_id
                        || previous.round_id != challenge.round_id
                        || previous.challenger_id != challenge.challenger_id
                        || previous.kind != challenge.kind
                        || previous.target_juror_id != challenge.target_juror_id
                        || previous.evidence_digest != challenge.evidence_digest
                        || previous.reason != challenge.reason
                        || previous.raised_at_unix_ms != challenge.raised_at_unix_ms
                    {
                        return Err(invalid(format!(
                            "challenge resolution event `{}` mutates challenge submission fields",
                            event.sequence
                        )));
                    }
                    let saved = record
                        .challenges
                        .iter()
                        .find(|saved| saved.challenge_id == challenge.challenge_id)
                        .ok_or_else(|| {
                            invalid(format!(
                                "challenge resolution event `{}` references challenge `{}` without a saved record",
                                event.sequence, challenge.challenge_id
                            ))
                        })?;
                    if challenge != saved {
                        return Err(invalid(format!(
                            "challenge resolution event `{}` challenge payload does not match saved challenge resolution",
                            event.sequence
                        )));
                    }
                    if event.generated_at_unix_ms != challenge.resolved_at_unix_ms.unwrap_or(0)
                        || challenge
                            .resolved_at_unix_ms
                            .is_some_and(|resolved_at| resolved_at < challenge.raised_at_unix_ms)
                    {
                        return Err(invalid(format!(
                            "challenge resolution event `{}` timestamp does not match saved challenge resolution",
                            event.sequence
                        )));
                    }
                    state
                        .challenges
                        .insert(challenge.challenge_id.clone(), challenge.clone());
                    if event.committed_count != state.commits.len() as u64
                        || event.revealed_count != state.reveals.len() as u64
                        || event.challenge_count != state.challenges.len() as u64
                    {
                        return Err(invalid(format!(
                            "challenge resolution event `{}` count fields do not match replayed state",
                            event.sequence
                        )));
                    }
                }
                ModerationBallotEventKind::RevealAccepted => {
                    if !state.announced || state.tallied {
                        return Err(invalid(format!(
                            "reveal event `{}` is outside the announced ballot lifecycle",
                            event.sequence
                        )));
                    }
                    let juror_id = event.juror_id.as_ref().ok_or_else(|| {
                        invalid(format!(
                            "reveal event `{}` is missing juror id",
                            event.sequence
                        ))
                    })?;
                    if event.tally.is_some() {
                        return Err(invalid(format!(
                            "reveal event `{}` unexpectedly carries tally data",
                            event.sequence
                        )));
                    }
                    if event.challenge.is_some() {
                        return Err(invalid(format!(
                            "reveal event `{}` unexpectedly carries challenge data",
                            event.sequence
                        )));
                    }
                    if let Some(blocking) = state.challenges.values().find(|challenge| {
                        !matches!(
                            challenge.decision,
                            Some(ModerationBallotChallengeDecision::Rejected)
                        )
                    }) {
                        return Err(invalid(format!(
                            "reveal event `{}` appears while challenge `{}` is pending or accepted",
                            event.sequence, blocking.challenge_id
                        )));
                    }
                    if !state.commits.contains(juror_id) {
                        return Err(invalid(format!(
                            "reveal event `{}` appears before the juror `{juror_id}` commit event",
                            event.sequence
                        )));
                    }
                    let reveal = record
                        .reveals
                        .iter()
                        .find(|reveal| reveal.juror_id == *juror_id)
                        .ok_or_else(|| {
                            invalid(format!(
                                "reveal event `{}` references juror `{juror_id}` without a saved reveal",
                                event.sequence
                            ))
                        })?;
                    if event.generated_at_unix_ms <= record.announcement.challenge_deadline_unix_ms
                        || event.generated_at_unix_ms > record.announcement.reveal_deadline_unix_ms
                        || event.generated_at_unix_ms != reveal.revealed_at_unix_ms
                    {
                        return Err(invalid(format!(
                            "reveal event `{}` timestamp is outside the reveal window",
                            event.sequence
                        )));
                    }
                    if !state.reveals.insert(juror_id.clone()) {
                        return Err(invalid(format!(
                            "duplicate reveal event for juror `{juror_id}` in ballot `{}` round `{}`",
                            event.case_id, event.round_id
                        )));
                    }
                    if reveal.context.case_id != record.announcement.context.case_id
                        || reveal.round_id != record.announcement.round_id
                    {
                        return Err(invalid(format!(
                            "reveal event `{}` references a saved reveal with mismatched scope",
                            event.sequence
                        )));
                    }
                    if event.committed_count != state.commits.len() as u64
                        || event.revealed_count != state.reveals.len() as u64
                        || event.challenge_count != state.challenges.len() as u64
                    {
                        return Err(invalid(format!(
                            "reveal event `{}` count fields do not match replayed state",
                            event.sequence
                        )));
                    }
                }
                ModerationBallotEventKind::BallotTallied => {
                    if !state.announced || state.tallied {
                        return Err(invalid(format!(
                            "tally event `{}` is outside the announced ballot lifecycle",
                            event.sequence
                        )));
                    }
                    if event.juror_id.is_some() {
                        return Err(invalid(format!(
                            "tally event `{}` unexpectedly carries juror data",
                            event.sequence
                        )));
                    }
                    if event.challenge.is_some() {
                        return Err(invalid(format!(
                            "tally event `{}` unexpectedly carries challenge data",
                            event.sequence
                        )));
                    }
                    if let Some(blocking) = state.challenges.values().find(|challenge| {
                        !matches!(
                            challenge.decision,
                            Some(ModerationBallotChallengeDecision::Rejected)
                        )
                    }) {
                        return Err(invalid(format!(
                            "tally event `{}` appears while challenge `{}` is pending or accepted",
                            event.sequence, blocking.challenge_id
                        )));
                    }
                    let tally = record.tally.as_ref().ok_or_else(|| {
                        invalid(format!(
                            "tally event `{}` references a ballot without a saved tally",
                            event.sequence
                        ))
                    })?;
                    if event.tally.as_ref() != Some(tally) {
                        return Err(invalid(format!(
                            "tally event `{}` tally payload does not match saved ballot tally",
                            event.sequence
                        )));
                    }
                    if event.generated_at_unix_ms != tally.tallied_at_unix_ms {
                        return Err(invalid(format!(
                            "tally event `{}` timestamp does not match saved tally",
                            event.sequence
                        )));
                    }
                    if event.committed_count != state.commits.len() as u64
                        || event.revealed_count != state.reveals.len() as u64
                        || event.challenge_count != state.challenges.len() as u64
                        || state.reveals.len() != record.reveals.len()
                    {
                        return Err(invalid(format!(
                            "tally event `{}` count fields do not match replayed state",
                            event.sequence
                        )));
                    }
                    state.tallied = true;
                }
            }
        }

        for (key, record) in &records {
            let state = replay.get(key).ok_or_else(|| {
                invalid(format!(
                    "moderation ballot replay state is missing for `{}` round `{}`",
                    key.0, key.1
                ))
            })?;
            if !state.announced {
                return Err(invalid(format!(
                    "ballot `{}` round `{}` is missing its announcement event",
                    key.0, key.1
                )));
            }
            if state.commits.len() != record.commits.len()
                || state.reveals.len() != record.reveals.len()
                || state.challenges.values().collect::<Vec<_>>()
                    != record.challenges.iter().collect::<Vec<_>>()
                || state.tallied != record.tally.is_some()
            {
                return Err(invalid(format!(
                    "event backlog does not replay to saved ballot `{}` round `{}`",
                    key.0, key.1
                )));
            }
        }

        Ok(())
    }

    fn load_moderation_ballot_checkpoint(&self) -> Result<(), NodeInitError> {
        let Some(path) = self.moderation_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let Some(bytes) = read_local_checkpoint_bounded(
            path,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|err| NodeInitError::checkpoint("moderation ballot", path, err))?
        else {
            return Ok(());
        };
        let snapshot = norito::decode_from_bytes::<ModerationBallotSnapshot>(&bytes)
            .map_err(|err| NodeInitError::checkpoint("moderation ballot", path, err))?;
        let (ballot_count, event_count) = self
            .restore_moderation_ballot_snapshot_in_memory(snapshot)
            .map_err(|err| NodeInitError::checkpoint("moderation ballot", path, err))?;
        iroha_logger::info!(
            path = %path.display(),
            ballot_count,
            event_count,
            "restored SoraFS moderation ballot checkpoint"
        );
        Ok(())
    }

    fn persist_moderation_ballot_snapshot(
        &self,
        snapshot: &ModerationBallotSnapshot,
    ) -> Result<(), RuntimeCheckpointPersistError> {
        let Some(path) = self.moderation_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let bytes = norito::to_bytes(snapshot).map_err(|err| {
            RuntimeCheckpointPersistError::precommit(format!(
                "encode moderation ballot checkpoint `{}`: {err}",
                path.display()
            ))
        })?;
        self.finish_local_checkpoint_write(
            "moderation ballot",
            path,
            write_local_checkpoint_atomic_bounded(
                path,
                &bytes,
                self.config.runtime_retention().checkpoint_max_bytes(),
            ),
        )
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

    fn resolve_moderation_quarantine_object_path(
        &self,
        root: &Path,
        record: &ModerationQuarantineObjectRecord,
    ) -> Result<PathBuf, ModerationQuarantineObjectError> {
        validate_relative_object_path(&record.envelope_path)
            .map_err(|message| ModerationQuarantineObjectError::InvalidSnapshot { message })?;
        Ok(root.join(&record.envelope_path))
    }

    fn validate_orderbook_event_checkpoint(
        runtime: &OrderbookRuntimeSnapshotV1,
        events: &[OrderbookEvent],
    ) -> Result<(), OrderbookRuntimeError> {
        runtime.validate()?;
        let trade_ids = runtime
            .trades
            .iter()
            .map(|trade| trade.trade_id)
            .collect::<BTreeSet<_>>();
        let channel_ids = runtime
            .settlement_channels
            .iter()
            .map(|channel| channel.channel_id)
            .collect::<BTreeSet<_>>();
        let receipt_ids = runtime
            .settlement_receipts
            .iter()
            .map(|receipt| receipt.receipt_id)
            .collect::<BTreeSet<_>>();
        let expired_ids = runtime
            .expired_order_ids
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let mut retained_trade_ids = BTreeSet::new();
        let mut retained_receipt_ids = BTreeSet::new();
        let mut retained_expired_ids = BTreeSet::new();
        for event in events {
            if event.generated_at_unix == 0 {
                return Err(OrderbookRuntimeError::InvalidSnapshot(format!(
                    "orderbook event {} has a zero timestamp",
                    event.sequence
                )));
            }
            if event.order_id.is_some_and(|order_id| order_id == [0; 32])
                || event
                    .receipt_id
                    .is_some_and(|receipt_id| receipt_id == [0; 32])
            {
                return Err(OrderbookRuntimeError::InvalidSnapshot(format!(
                    "orderbook event {} contains an all-zero identifier",
                    event.sequence
                )));
            }
            match event.kind {
                OrderbookEventKind::OrderAccepted => {
                    if event.order_id.is_none() || event.receipt_id.is_some() {
                        return Err(OrderbookRuntimeError::InvalidSnapshot(format!(
                            "accepted-order event {} has an invalid payload shape",
                            event.sequence
                        )));
                    }
                }
                OrderbookEventKind::OrderCancelled => {
                    if event.order_id.is_none()
                        || event.receipt_id.is_some()
                        || !event.trade_ids.is_empty()
                        || !event.settlement_channel_ids.is_empty()
                        || !event.expired_order_ids.is_empty()
                    {
                        return Err(OrderbookRuntimeError::InvalidSnapshot(format!(
                            "cancelled-order event {} has an invalid payload shape",
                            event.sequence
                        )));
                    }
                }
                OrderbookEventKind::SettlementReceiptAccepted => {
                    if event.order_id.is_some()
                        || event.receipt_id.is_none()
                        || !event.trade_ids.is_empty()
                        || event.settlement_channel_ids.len() != 1
                        || !event.expired_order_ids.is_empty()
                    {
                        return Err(OrderbookRuntimeError::InvalidSnapshot(format!(
                            "settlement-receipt event {} has an invalid payload shape",
                            event.sequence
                        )));
                    }
                }
            }
            for trade_id in &event.trade_ids {
                if !trade_ids.contains(trade_id) || !retained_trade_ids.insert(*trade_id) {
                    return Err(OrderbookRuntimeError::InvalidSnapshot(format!(
                        "orderbook event {} references an absent or duplicate trade",
                        event.sequence
                    )));
                }
            }
            for channel_id in &event.settlement_channel_ids {
                if !channel_ids.contains(channel_id) {
                    return Err(OrderbookRuntimeError::InvalidSnapshot(format!(
                        "orderbook event {} references an absent settlement channel",
                        event.sequence
                    )));
                }
            }
            if let Some(receipt_id) = event.receipt_id
                && (!receipt_ids.contains(&receipt_id) || !retained_receipt_ids.insert(receipt_id))
            {
                return Err(OrderbookRuntimeError::InvalidSnapshot(format!(
                    "orderbook event {} references an absent or duplicate receipt",
                    event.sequence
                )));
            }
            for expired_id in &event.expired_order_ids {
                if !expired_ids.contains(expired_id) || !retained_expired_ids.insert(*expired_id) {
                    return Err(OrderbookRuntimeError::InvalidSnapshot(format!(
                        "orderbook event {} references an absent or duplicate expired order",
                        event.sequence
                    )));
                }
            }
        }
        if let Some(last) = events.last() {
            let open_channel_count = runtime
                .settlement_channels
                .iter()
                .filter(|channel| matches!(channel.status, SettlementChannelStatusV1::Open))
                .count() as u64;
            if last.open_order_count != runtime.open_orders.len() as u64
                || last.open_settlement_channel_count != open_channel_count
                || last.settlement_receipt_count != runtime.settlement_receipts.len() as u64
            {
                return Err(OrderbookRuntimeError::InvalidSnapshot(
                    "retained orderbook event suffix does not reach saved runtime counters"
                        .to_owned(),
                ));
            }
        }
        Ok(())
    }

    fn mutate_orderbook_durably<T>(
        &self,
        generated_at_unix: u64,
        mutate: impl FnOnce(
            &mut OrderbookRuntime,
        ) -> Result<(T, OrderbookEventInput), OrderbookRuntimeError>,
    ) -> Result<(T, OrderbookEvent), OrderbookRuntimeError> {
        let _mutation_guard = self
            .runtime_mutation_lock
            .lock()
            .map_err(|_| OrderbookRuntimeError::StateLockPoisoned)?;
        self.ensure_durability_healthy()
            .map_err(OrderbookRuntimeError::Checkpoint)?;
        let mut runtime = self
            .orderbook
            .write()
            .map_err(|_| OrderbookRuntimeError::StateLockPoisoned)?;
        let previous_runtime = runtime.runtime_snapshot(generated_at_unix.max(1));
        let (outcome, input) = match mutate(&mut runtime) {
            Ok(value) => value,
            Err(err) => {
                if let Err(rollback) = runtime.restore_runtime_snapshot(previous_runtime) {
                    let message = self.record_unrecoverable_rollback(
                        "failed to roll back rejected orderbook mutation",
                        rollback,
                    );
                    return Err(OrderbookRuntimeError::Checkpoint(message));
                }
                return Err(err);
            }
        };
        let committed_runtime = runtime.runtime_snapshot(generated_at_unix);
        if let Err(err) = committed_runtime.validate() {
            if let Err(rollback) = runtime.restore_runtime_snapshot(previous_runtime) {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back invalid committed orderbook snapshot",
                    rollback,
                );
                return Err(OrderbookRuntimeError::Checkpoint(message));
            }
            return Err(OrderbookRuntimeError::Validation(err));
        }
        let open_settlement_channel_count = committed_runtime
            .settlement_channels
            .iter()
            .filter(|channel| matches!(channel.status, SettlementChannelStatusV1::Open))
            .count() as u64;
        let mut events = match self.orderbook_events.write() {
            Ok(events) => events,
            Err(_) => {
                if let Err(rollback) = runtime.restore_runtime_snapshot(previous_runtime) {
                    let message = self.record_unrecoverable_rollback(
                        "failed to roll back orderbook mutation after event-lock failure",
                        rollback,
                    );
                    return Err(OrderbookRuntimeError::Checkpoint(message));
                }
                return Err(OrderbookRuntimeError::StateLockPoisoned);
            }
        };
        let previous_events = events.clone();
        let event = match events.append(|sequence| OrderbookEvent {
            sequence,
            kind: input.kind,
            generated_at_unix,
            order_id: input.order_id,
            trade_ids: input.trade_ids,
            settlement_channel_ids: input.settlement_channel_ids,
            receipt_id: input.receipt_id,
            expired_order_ids: input.expired_order_ids,
            open_order_count: committed_runtime.open_orders.len() as u64,
            open_settlement_channel_count,
            settlement_receipt_count: committed_runtime.settlement_receipts.len() as u64,
        }) {
            Ok(event) => event,
            Err(_) => {
                if let Err(rollback) = runtime.restore_runtime_snapshot(previous_runtime) {
                    let message = self.record_unrecoverable_rollback(
                        "failed to roll back orderbook event-sequence exhaustion",
                        rollback,
                    );
                    return Err(OrderbookRuntimeError::Checkpoint(message));
                }
                return Err(OrderbookRuntimeError::EventSequenceOverflow);
            }
        };
        let checkpoint = OrderbookRuntimeCheckpointV1 {
            version: ORDERBOOK_RUNTIME_STATE_VERSION_V1,
            runtime: committed_runtime,
            events: events.retained(),
        };
        if let Err(err) =
            Self::validate_orderbook_event_checkpoint(&checkpoint.runtime, &checkpoint.events)
        {
            let rollback = runtime.restore_runtime_snapshot(previous_runtime);
            *events = previous_events;
            if let Err(rollback) = rollback {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back invalid orderbook event checkpoint",
                    rollback,
                );
                return Err(OrderbookRuntimeError::Checkpoint(message));
            }
            return Err(err);
        }
        if let Err(err) = self.persist_orderbook_checkpoint(&checkpoint) {
            if err.committed {
                return Err(OrderbookRuntimeError::Checkpoint(err.to_string()));
            }
            let rollback = runtime.restore_runtime_snapshot(previous_runtime);
            *events = previous_events;
            if let Err(rollback) = rollback {
                let message = self.record_unrecoverable_rollback(
                    "failed to roll back orderbook checkpoint failure",
                    rollback,
                );
                return Err(OrderbookRuntimeError::Checkpoint(message));
            }
            return Err(OrderbookRuntimeError::Checkpoint(err.to_string()));
        }
        drop(events);
        drop(runtime);
        Ok((outcome, event))
    }

    fn load_orderbook_checkpoint(&self) -> Result<(), NodeInitError> {
        let Some(path) = self.orderbook_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let Some(bytes) = read_local_checkpoint_bounded(
            path,
            self.config.runtime_retention().checkpoint_max_bytes(),
        )
        .map_err(|err| NodeInitError::checkpoint("orderbook runtime", path, err))?
        else {
            return Ok(());
        };
        let checkpoint = norito::decode_from_bytes::<OrderbookRuntimeCheckpointV1>(&bytes)
            .map_err(|err| NodeInitError::checkpoint("orderbook runtime", path, err))?;
        if checkpoint.version != ORDERBOOK_RUNTIME_STATE_VERSION_V1 {
            return Err(NodeInitError::checkpoint(
                "orderbook runtime",
                path,
                format!("unsupported checkpoint version {}", checkpoint.version),
            ));
        }
        let retention = self.config.runtime_retention();
        if checkpoint.events.len() > retention.event_history_limit() {
            return Err(NodeInitError::checkpoint(
                "orderbook runtime",
                path,
                format!(
                    "event count {} exceeds configured limit {}",
                    checkpoint.events.len(),
                    retention.event_history_limit()
                ),
            ));
        }
        Self::validate_orderbook_event_checkpoint(&checkpoint.runtime, &checkpoint.events)
            .map_err(|err| NodeInitError::checkpoint("orderbook runtime", path, err))?;
        let generated_at_unix = checkpoint.runtime.generated_at_unix;
        let mut restored_orderbook =
            OrderbookRuntime::with_entry_limit(retention.state_entry_limit());
        restored_orderbook
            .restore_runtime_snapshot(checkpoint.runtime)
            .map_err(|err| NodeInitError::checkpoint("orderbook runtime", path, err))?;
        let mut restored_events = BoundedEventHistory::new(retention.event_history_limit());
        restored_events
            .restore(checkpoint.events, |event| event.sequence)
            .map_err(|err| NodeInitError::checkpoint("orderbook runtime", path, err))?;
        let mut orderbook = self.orderbook.write().map_err(|_| {
            NodeInitError::checkpoint("orderbook runtime", path, "state lock poisoned")
        })?;
        let mut events = self.orderbook_events.write().map_err(|_| {
            NodeInitError::checkpoint("orderbook runtime", path, "event lock poisoned")
        })?;
        *orderbook = restored_orderbook;
        *events = restored_events;
        drop(events);
        drop(orderbook);
        self.record_orderbook_snapshot_metrics(generated_at_unix);
        iroha_logger::info!(
            path = %path.display(),
            "restored SoraFS orderbook runtime checkpoint"
        );
        Ok(())
    }

    fn persist_orderbook_checkpoint(
        &self,
        checkpoint: &OrderbookRuntimeCheckpointV1,
    ) -> Result<(), RuntimeCheckpointPersistError> {
        let Some(path) = self.orderbook_checkpoint_path.as_ref() else {
            return Ok(());
        };
        let bytes = norito::to_bytes(checkpoint).map_err(|err| {
            RuntimeCheckpointPersistError::precommit(format!(
                "encode orderbook checkpoint `{}`: {err}",
                path.display()
            ))
        })?;
        self.finish_local_checkpoint_write(
            "orderbook runtime",
            path,
            write_local_checkpoint_atomic_bounded(
                path,
                &bytes,
                self.config.runtime_retention().checkpoint_max_bytes(),
            ),
        )
    }

    fn record_orderbook_snapshot_metrics(&self, now_unix: u64) {
        let snapshot = match self.orderbook_snapshot(now_unix) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                iroha_logger::warn!(
                    %error,
                    "failed to build SoraFS orderbook snapshot metrics"
                );
                return;
            }
        };
        let mut hot_bid = 0u64;
        let mut hot_ask = 0u64;
        let mut warm_bid = 0u64;
        let mut warm_ask = 0u64;
        let mut archive_bid = 0u64;
        let mut archive_ask = 0u64;
        for entry in &snapshot.open_orders {
            match (entry.order.tier, entry.order.side) {
                (OrderTierV1::Hot, OrderSideV1::Bid) => {
                    hot_bid = hot_bid.saturating_add(entry.order.remaining_gib);
                }
                (OrderTierV1::Hot, OrderSideV1::Ask) => {
                    hot_ask = hot_ask.saturating_add(entry.order.remaining_gib);
                }
                (OrderTierV1::Warm, OrderSideV1::Bid) => {
                    warm_bid = warm_bid.saturating_add(entry.order.remaining_gib);
                }
                (OrderTierV1::Warm, OrderSideV1::Ask) => {
                    warm_ask = warm_ask.saturating_add(entry.order.remaining_gib);
                }
                (OrderTierV1::Archive, OrderSideV1::Bid) => {
                    archive_bid = archive_bid.saturating_add(entry.order.remaining_gib);
                }
                (OrderTierV1::Archive, OrderSideV1::Ask) => {
                    archive_ask = archive_ask.saturating_add(entry.order.remaining_gib);
                }
            }
        }
        let metrics = global_or_default();
        for (tier, side, depth) in [
            ("hot", "bid", hot_bid),
            ("hot", "ask", hot_ask),
            ("warm", "bid", warm_bid),
            ("warm", "ask", warm_ask),
            ("archive", "bid", archive_bid),
            ("archive", "ask", archive_ask),
        ] {
            metrics.set_sorafs_orderbook_depth_gib(
                ORDERBOOK_METRIC_CLUSTER_LOCAL,
                tier,
                side,
                depth as f64,
            );
            metrics.set_sorafs_orderbook_match_lag_seconds(
                ORDERBOOK_METRIC_CLUSTER_LOCAL,
                tier,
                0.0,
            );
        }

        let open_channels = snapshot
            .settlement_channels
            .iter()
            .filter(|channel| matches!(channel.status, SettlementChannelStatusV1::Open))
            .collect::<Vec<_>>();
        let oldest_age = open_channels
            .iter()
            .filter_map(|channel| now_unix.checked_sub(channel.opened_at_unix))
            .max()
            .unwrap_or(0);
        metrics.set_sorafs_orderbook_settlement_backlog(
            ORDERBOOK_METRIC_CLUSTER_LOCAL,
            open_channels.len() as u64,
            oldest_age,
        );
        match orderbook_provider_escrow_runways(&snapshot, now_unix) {
            Ok(runways) => {
                for (provider, seconds) in runways {
                    metrics.set_sorafs_orderbook_escrow_runway_seconds(&provider, seconds);
                }
            }
            Err(error) => {
                iroha_logger::warn!(
                    %error,
                    "failed to derive exact SoraFS orderbook escrow runway metrics"
                );
            }
        }
        metrics
            .set_sorafs_orderbook_contract_mirror_divergence(ORDERBOOK_METRIC_CLUSTER_LOCAL, false);
    }

    fn publish_orderbook_settlement_receipt(&self, receipt: &SettlementReceiptV1) {
        let Some(publisher) = self.governance_publisher() else {
            return;
        };
        let encoded = match norito::to_bytes(receipt) {
            Ok(encoded) => encoded,
            Err(err) => {
                iroha_logger::error!(
                    %err,
                    receipt_id = %hex::encode(receipt.receipt_id),
                    channel_id = %hex::encode(receipt.channel_id),
                    "failed to encode SoraFS orderbook settlement receipt"
                );
                return;
            }
        };
        if let Err(err) = publisher.publish_orderbook_settlement_receipt(receipt, &encoded) {
            iroha_logger::error!(
                %err,
                receipt_id = %hex::encode(receipt.receipt_id),
                channel_id = %hex::encode(receipt.channel_id),
                trade_id = %hex::encode(receipt.trade_id),
                "failed to publish SoraFS orderbook settlement receipt to governance DAG"
            );
        }
    }

    fn publish_moderation_ballot_governance_event(&self, event: &ModerationBallotEvent) {
        let governance_event = event.to_governance_event_v1();
        self.record_transparency_source_entry_lossy(
            transparency::moderation_ballot_governance_event_source_entry(&governance_event),
            "moderation_ballot_governance_event",
            &event.case_id,
        );
        let Some(publisher) = self.governance_publisher() else {
            return;
        };
        let encoded = match norito::to_bytes(&governance_event) {
            Ok(encoded) => encoded,
            Err(err) => {
                iroha_logger::error!(
                    %err,
                    case_id = %event.case_id,
                    round_id = %event.round_id,
                    sequence = event.sequence,
                    "failed to encode SoraFS moderation ballot governance event"
                );
                return;
            }
        };
        if let Err(err) = publisher.publish_moderation_ballot_event(&governance_event, &encoded) {
            iroha_logger::error!(
                %err,
                case_id = %event.case_id,
                round_id = %event.round_id,
                sequence = event.sequence,
                "failed to publish SoraFS moderation ballot event to governance DAG"
            );
        }
    }

    fn record_transparency_source_entry_lossy(
        &self,
        entry: Result<TransparencyLedgerSourceEntry, TransparencySourceEntryAdapterError>,
        source_kind: &'static str,
        source_id: &str,
    ) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                iroha_logger::warn!(
                    %err,
                    source_kind,
                    source_id,
                    "failed to derive SoraFS transparency source entry"
                );
                return;
            }
        };
        if let Err(err) = self.record_transparency_ledger_source_entry(entry) {
            iroha_logger::warn!(
                %err,
                source_kind,
                source_id,
                "failed to record SoraFS transparency source entry"
            );
        }
    }

    fn publish_moderation_appeal_finance_report(
        &self,
        record: &ModerationBallotRecord,
        tally: &ModerationBallotTally,
    ) {
        let report = match moderation_appeal_finance_report(record, tally) {
            Ok(Some(report)) => report,
            Ok(None) => return,
            Err(err) => {
                iroha_logger::error!(
                    %err,
                    case_id = %tally.case_id,
                    round_id = %tally.round_id,
                    "failed to derive SoraFS appeal finance report from moderation tally"
                );
                return;
            }
        };
        if let Err(err) = self.publish_appeal_finance_report(report) {
            iroha_logger::error!(
                %err,
                case_id = %tally.case_id,
                round_id = %tally.round_id,
                "failed to publish SoraFS moderation tally appeal finance report"
            );
        }
    }

    /// Finalise a deal settlement for the supplied epoch.
    pub fn settle_deal(
        &self,
        deal_id: DealId,
        settlement_epoch: u64,
    ) -> Result<DealSettlementOutcome, DealEngineError> {
        let outcome = self.mutate_deal_engine_durably(|engine| {
            engine
                .settle(deal_id, settlement_epoch)
                .map(|outcome| (outcome, true))
        })?;
        let provider_hex = hex::encode(outcome.record.provider_id.as_bytes());
        let status_label = match outcome.governance.status {
            DealSettlementStatusV1::Completed => "completed",
            DealSettlementStatusV1::Cancelled => "cancelled",
            DealSettlementStatusV1::Slashed => "slashed",
        };
        global_sorafs_node_otel().record_deal_settlement(
            &provider_hex,
            status_label,
            &outcome.record.expected_charge,
            &outcome.record.client_credit_debit,
            &outcome.record.bond_slash,
            &outcome.record.outstanding,
        );
        let publisher = self.governance_publisher();
        if let Some(publisher) = publisher {
            let encoded = outcome.governance.encode();
            match publisher.publish_deal_settlement(&outcome.governance, &encoded) {
                Ok(()) => {
                    global_sorafs_node_otel().record_settlement_publish(&provider_hex, "success");
                }
                Err(err) => {
                    global_sorafs_node_otel().record_settlement_publish(&provider_hex, "failure");
                    let deal_hex = hex::encode(outcome.record.deal_id.as_bytes());
                    iroha_logger::error!(
                        %deal_hex,
                        %provider_hex,
                        error = %err,
                        "failed to publish SoraFS settlement artefact to governance DAG"
                    );
                }
            }
        }
        Ok(outcome)
    }

    fn repair_durability_error(message: impl Into<String>) -> RepairSchedulerError {
        RepairSchedulerError::Store(repair::RepairStoreError::Other(message.into()))
    }

    fn ensure_repair_durability_healthy(&self) -> Result<(), RepairSchedulerError> {
        self.ensure_durability_healthy()
            .map_err(Self::repair_durability_error)
    }

    fn record_repair_event(&self, event: RepairTaskEventV1) -> Result<(), RepairSchedulerError> {
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            Self::repair_durability_error("auxiliary checkpoint transaction lock poisoned")
        })?;
        self.ensure_repair_durability_healthy()?;
        let mut events = self
            .repair_events
            .write()
            .map_err(|_| Self::repair_durability_error("repair event history lock poisoned"))?;
        let previous_events = events.clone();
        let event = events
            .append(|sequence| RepairEvent { sequence, event })
            .map_err(|err| Self::repair_durability_error(err.to_string()))?;
        drop(events);
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(Self::repair_durability_error(err.to_string()));
            }
            match self.repair_events.write() {
                Ok(mut events) => *events = previous_events,
                Err(_) => {
                    self.mark_durability_unhealthy(
                        "repair event checkpoint failed and rollback lock was poisoned".to_owned(),
                    );
                    return Err(Self::repair_durability_error(
                        "repair event checkpoint failed and rollback lock was poisoned",
                    ));
                }
            }
            let message =
                format!("repair task state committed without its global event checkpoint: {err}");
            self.mark_durability_unhealthy(message.clone());
            return Err(Self::repair_durability_error(message));
        }
        let _ = self.repair_event_sender.send(event);
        Ok(())
    }

    fn publish_repair_audit_event(&self, event: RepairTaskEventV1) {
        let Some(publisher) = self.governance_publisher() else {
            return;
        };
        let payload_bytes = event.encode();
        let payload_digest = Hash::new(payload_bytes);
        let sequence = match self.repair.next_audit_sequence() {
            Ok(sequence) => sequence,
            Err(err) => {
                iroha_logger::error!(
                    %err,
                    ticket = %event.ticket_id,
                    "failed to durably reserve repair audit event sequence"
                );
                return;
            }
        };
        let header = SorafsAuditHeaderV1 {
            sequence,
            occurred_at_unix: event.occurred_at_unix,
            signer: event
                .actor
                .clone()
                .unwrap_or_else(|| "sorafs-repair".to_string()),
            payload_digest: *payload_digest.as_ref(),
        };
        let audit_event = RepairAuditEventV1 {
            version: REPAIR_AUDIT_EVENT_VERSION_V1,
            header,
            payload: event,
        };
        let encoded = match norito::to_bytes(&audit_event) {
            Ok(encoded) => encoded,
            Err(err) => {
                iroha_logger::error!(
                    %err,
                    ticket = %audit_event.payload.ticket_id,
                    "failed to encode repair audit event"
                );
                return;
            }
        };
        if let Err(err) = publisher.publish_repair_audit_event(&audit_event, &encoded) {
            iroha_logger::error!(
                %err,
                ticket = %audit_event.payload.ticket_id,
                status = ?audit_event.payload.status,
                "failed to publish repair audit event to governance DAG"
            );
        }
    }

    fn publish_gc_audit_event(&self, payload: GcAuditPayloadV1) {
        let Some(publisher) = self.governance_publisher() else {
            return;
        };
        let payload_bytes = payload.encode();
        let payload_digest = Hash::new(payload_bytes);
        let sequence = match self.repair.next_audit_sequence() {
            Ok(sequence) => sequence,
            Err(err) => {
                iroha_logger::error!(%err, "failed to durably reserve GC audit event sequence");
                return;
            }
        };
        let header = SorafsAuditHeaderV1 {
            sequence,
            occurred_at_unix: payload.evicted_at_unix,
            signer: "sorafs-gc".to_string(),
            payload_digest: *payload_digest.as_ref(),
        };
        let audit_event = GcAuditEventV1 {
            version: GC_AUDIT_EVENT_VERSION_V1,
            header,
            payload,
        };
        let encoded = match norito::to_bytes(&audit_event) {
            Ok(encoded) => encoded,
            Err(err) => {
                iroha_logger::error!(%err, "failed to encode GC audit event");
                return;
            }
        };
        if let Err(err) = publisher.publish_gc_audit_event(&audit_event, &encoded) {
            iroha_logger::error!(
                %err,
                "failed to publish GC audit event to governance DAG"
            );
        }
    }

    fn publish_reconciliation_report(&self, report: &SorafsReconciliationReportV1) {
        let Some(publisher) = self.governance_publisher() else {
            return;
        };
        let encoded = match norito::to_bytes(report) {
            Ok(encoded) => encoded,
            Err(err) => {
                iroha_logger::error!(%err, "failed to encode reconciliation report");
                return;
            }
        };
        if let Err(err) = publisher.publish_reconciliation_report(report, &encoded) {
            iroha_logger::error!(
                %err,
                "failed to publish reconciliation report to governance DAG"
            );
        }
    }

    fn publish_repair_slash_proposal(
        &self,
        proposal: &RepairSlashProposalV1,
        stage: RepairSlashStage,
    ) {
        let Some(publisher) = self.governance_publisher() else {
            return;
        };
        let encoded = match norito::to_bytes(proposal) {
            Ok(encoded) => encoded,
            Err(err) => {
                iroha_logger::error!(
                    %err,
                    ticket = %proposal.ticket_id,
                    stage = stage.as_str(),
                    "failed to encode repair slash proposal"
                );
                return;
            }
        };
        if let Err(err) = publisher.publish_repair_slash_proposal(proposal, &encoded, stage) {
            iroha_logger::error!(
                %err,
                ticket = %proposal.ticket_id,
                stage = stage.as_str(),
                "failed to publish repair slash proposal to governance DAG"
            );
        }
    }

    fn publish_repair_update(
        &self,
        update: &repair::RepairTaskUpdate,
        slash_stage: Option<RepairSlashStage>,
    ) -> Result<(), RepairSchedulerError> {
        if let Some(event) = update.event.clone() {
            self.record_repair_event(event.clone())?;
            self.publish_repair_audit_event(event);
        }
        if let Some(proposal) = update.slash_proposal.as_ref() {
            if let Some(stage) = slash_stage {
                self.publish_repair_slash_proposal(proposal, stage);
            } else {
                iroha_logger::warn!(
                    ticket = %proposal.ticket_id,
                    "repair slash proposal missing publish stage"
                );
            }
        }
        Ok(())
    }

    /// Capture a snapshot of the deal ledger.
    #[must_use]
    pub fn deal_snapshot(&self, deal_id: DealId) -> Option<DealSnapshot> {
        self.deal_engine.deal_snapshot(deal_id)
    }

    #[cfg(test)]
    fn repair_manager(&self) -> RepairManager {
        self.repair.clone()
    }

    /// Enqueue a repair report submitted by an auditor.
    pub fn enqueue_repair_report(
        &self,
        report: &RepairReportV1,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.ensure_repair_durability_healthy()?;
        let update = self.repair.enqueue_report_with_event(report.clone())?;
        self.publish_repair_update(&update, None)?;
        Ok(update.record)
    }

    /// Record a signed repair auditor request nonce for replay protection.
    pub fn record_repair_auditor_nonce(
        &self,
        auditor_account: &str,
        nonce: u64,
    ) -> Result<(), RepairSchedulerError> {
        self.ensure_repair_durability_healthy()?;
        self.repair.record_auditor_nonce(auditor_account, nonce)
    }

    /// Fetch repair tasks with optional filters applied.
    ///
    /// # Errors
    ///
    /// Returns an error when the durable repair store is unavailable or cannot
    /// prove that its checkpoint state is trustworthy.
    pub fn repair_tasks(
        &self,
        filters: RepairTaskFilters,
    ) -> Result<Vec<RepairTaskRecordV1>, RepairSchedulerError> {
        self.ensure_repair_durability_healthy()?;
        self.repair.list_tasks(filters).map_err(Into::into)
    }

    /// Fetch repair task snapshots with optional filters applied.
    ///
    /// # Errors
    ///
    /// Returns an error when the durable repair store is unavailable or cannot
    /// prove that its checkpoint state is trustworthy.
    pub fn repair_task_snapshots(
        &self,
        filters: RepairTaskFilters,
    ) -> Result<Vec<RepairTaskSnapshot>, RepairSchedulerError> {
        self.ensure_repair_durability_healthy()?;
        self.repair.list_task_snapshots(filters).map_err(Into::into)
    }

    /// Fetch repair tasks associated with the supplied manifest digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the durable repair store is unavailable or cannot
    /// prove that its checkpoint state is trustworthy.
    pub fn repair_tasks_for_manifest(
        &self,
        manifest_digest: &[u8; 32],
    ) -> Result<Vec<RepairTaskRecordV1>, RepairSchedulerError> {
        self.repair_tasks(RepairTaskFilters::for_manifest(*manifest_digest))
    }

    /// Fetch repair task snapshots associated with the supplied manifest digest.
    ///
    /// # Errors
    ///
    /// Returns an error when the durable repair store is unavailable or cannot
    /// prove that its checkpoint state is trustworthy.
    pub fn repair_task_snapshots_for_manifest(
        &self,
        manifest_digest: &[u8; 32],
    ) -> Result<Vec<RepairTaskSnapshot>, RepairSchedulerError> {
        self.repair_task_snapshots(RepairTaskFilters::for_manifest(*manifest_digest))
    }

    /// Fetch a repair task record by ticket id.
    ///
    /// # Errors
    ///
    /// Returns an error when the durable repair store is unavailable or cannot
    /// prove that its checkpoint state is trustworthy.
    pub fn repair_task_record(
        &self,
        ticket_id: &RepairTicketId,
    ) -> Result<Option<RepairTaskRecordV1>, RepairSchedulerError> {
        self.ensure_repair_durability_healthy()?;
        self.repair.task_record(ticket_id).map_err(Into::into)
    }

    /// Fetch a repair task snapshot by ticket id.
    ///
    /// # Errors
    ///
    /// Returns an error when the durable repair store is unavailable or cannot
    /// prove that its checkpoint state is trustworthy.
    pub fn repair_task_snapshot(
        &self,
        ticket_id: &RepairTicketId,
    ) -> Result<Option<RepairTaskSnapshot>, RepairSchedulerError> {
        self.ensure_repair_durability_healthy()?;
        self.repair.task_snapshot(ticket_id).map_err(Into::into)
    }

    /// Submit a slash proposal tied to an escalated repair ticket.
    pub fn submit_repair_slash_proposal(
        &self,
        proposal: &RepairSlashProposalV1,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.ensure_repair_durability_healthy()?;
        let update = self
            .repair
            .submit_slash_proposal_with_event(proposal.clone())?;
        self.publish_repair_update(&update, Some(RepairSlashStage::Submitted))?;
        Ok(update.record)
    }

    /// Mark the specified repair ticket as in progress.
    pub fn mark_repair_in_progress(
        &self,
        ticket_id: &RepairTicketId,
        started_at_unix: u64,
        repair_agent: Option<String>,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.ensure_repair_durability_healthy()?;
        let update =
            self.repair
                .mark_in_progress_with_event(ticket_id, started_at_unix, repair_agent)?;
        self.publish_repair_update(&update, None)?;
        Ok(update.record)
    }

    /// Mark the specified repair ticket as completed.
    pub fn mark_repair_completed(
        &self,
        ticket_id: &RepairTicketId,
        completed_at_unix: u64,
        resolution_notes: Option<String>,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.ensure_repair_durability_healthy()?;
        let update = self.repair.mark_completed_with_event(
            ticket_id,
            completed_at_unix,
            resolution_notes,
        )?;
        self.publish_repair_update(&update, None)?;
        Ok(update.record)
    }

    /// Mark the specified repair ticket as failed.
    pub fn mark_repair_failed(
        &self,
        ticket_id: &RepairTicketId,
        failed_at_unix: u64,
        reason: String,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.ensure_repair_durability_healthy()?;
        let update = self
            .repair
            .mark_failed_with_event(ticket_id, failed_at_unix, reason)?;
        self.publish_repair_update(&update, Some(RepairSlashStage::Drafted))?;
        Ok(update.record)
    }

    /// Claim a repair ticket for a worker.
    pub fn claim_repair_ticket(
        &self,
        ticket_id: &RepairTicketId,
        worker_id: &str,
        claimed_at_unix: u64,
        idempotency_key: &str,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.ensure_repair_durability_healthy()?;
        let update = self.repair.claim_ticket_with_event(
            ticket_id,
            worker_id,
            claimed_at_unix,
            idempotency_key,
        )?;
        self.publish_repair_update(&update, None)?;
        Ok(update.record)
    }

    /// Record a heartbeat for an active repair lease.
    pub fn heartbeat_repair_ticket(
        &self,
        ticket_id: &RepairTicketId,
        worker_id: &str,
        heartbeat_at_unix: u64,
        idempotency_key: &str,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.ensure_repair_durability_healthy()?;
        self.repair
            .heartbeat_ticket(ticket_id, worker_id, heartbeat_at_unix, idempotency_key)
    }

    /// Mark a claimed repair ticket as completed.
    pub fn complete_repair_ticket(
        &self,
        ticket_id: &RepairTicketId,
        worker_id: &str,
        completed_at_unix: u64,
        resolution_notes: Option<String>,
        idempotency_key: &str,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.ensure_repair_durability_healthy()?;
        let update = self.repair.complete_ticket_with_event(
            ticket_id,
            worker_id,
            completed_at_unix,
            resolution_notes,
            idempotency_key,
        )?;
        self.publish_repair_update(&update, None)?;
        Ok(update.record)
    }

    /// Mark a claimed repair ticket as failed.
    pub fn fail_repair_ticket(
        &self,
        ticket_id: &RepairTicketId,
        worker_id: &str,
        failed_at_unix: u64,
        reason: String,
        idempotency_key: &str,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        self.ensure_repair_durability_healthy()?;
        let update = self.repair.fail_ticket_with_event(
            ticket_id,
            worker_id,
            failed_at_unix,
            reason,
            idempotency_key,
        )?;
        self.publish_repair_update(&update, Some(RepairSlashStage::Drafted))?;
        Ok(update.record)
    }

    /// Run a repair watchdog sweep to requeue leases and escalate SLA breaches.
    pub fn run_repair_watchdog_once(
        &self,
        now_unix: u64,
    ) -> Result<RepairWatchdogReport, RepairSchedulerError> {
        if !self.repair_config.enabled() || now_unix == 0 {
            return Ok(RepairWatchdogReport::default());
        }
        self.ensure_repair_durability_healthy()?;
        let report = self.repair.run_watchdog(now_unix)?;
        for event in &report.events {
            self.record_repair_event(event.clone())?;
            self.publish_repair_audit_event(event.clone());
        }
        for proposal in &report.escalated {
            self.publish_repair_slash_proposal(proposal, RepairSlashStage::Drafted);
        }
        Ok(report)
    }

    fn missing_chunk_records(manifest: &StoredManifest) -> Vec<ChunkFileRecord> {
        let total = manifest.chunk_count();
        let mut missing = Vec::new();
        for idx in 0..total {
            let Some(chunk) = manifest.chunk(idx) else {
                continue;
            };
            if !chunk.path.exists() {
                missing.push(chunk.clone());
            }
        }
        missing
    }

    fn rehydrate_missing_chunks_from_local_replicas(
        &self,
        storage: &StorageBackend,
        missing_chunks: &[ChunkFileRecord],
    ) -> RepairRehydrateOutcome {
        let mut outcome = RepairRehydrateOutcome {
            missing_before: missing_chunks.len(),
            ..RepairRehydrateOutcome::default()
        };
        if missing_chunks.is_empty() {
            return outcome;
        }

        let missing_digests: HashSet<[u8; 32]> =
            missing_chunks.iter().map(|chunk| chunk.digest).collect();
        let root_dir = storage.root_dir();
        let mut sources: HashMap<[u8; 32], (String, ChunkFileRecord)> = HashMap::new();

        for manifest in storage.manifests() {
            for idx in 0..manifest.chunk_count() {
                let Some(chunk) = manifest.chunk(idx) else {
                    continue;
                };
                if !missing_digests.contains(&chunk.digest) {
                    continue;
                }
                if !chunk.path.exists() {
                    continue;
                }
                let key = chunk
                    .path
                    .strip_prefix(root_dir)
                    .unwrap_or(&chunk.path)
                    .to_string_lossy()
                    .to_string();
                match sources.entry(chunk.digest) {
                    Entry::Vacant(entry) => {
                        entry.insert((key, chunk.clone()));
                    }
                    Entry::Occupied(mut entry) => {
                        if key < entry.get().0 {
                            entry.insert((key, chunk.clone()));
                        }
                    }
                }
            }
        }

        for chunk in missing_chunks {
            if chunk.path.exists() {
                continue;
            }
            let Some((_, source)) = sources.get(&chunk.digest) else {
                continue;
            };
            if source.length != chunk.length {
                outcome.errors = outcome.errors.saturating_add(1);
                iroha_logger::warn!(
                    digest = %hex::encode(chunk.digest),
                    expected = chunk.length,
                    actual = source.length,
                    "rehydration source length mismatch"
                );
                continue;
            }
            let bytes = match fs::read(&source.path) {
                Ok(bytes) => bytes,
                Err(err) => {
                    outcome.errors = outcome.errors.saturating_add(1);
                    iroha_logger::warn!(
                        %err,
                        digest = %hex::encode(chunk.digest),
                        path = %source.path.display(),
                        "failed to read rehydration source chunk"
                    );
                    continue;
                }
            };
            if bytes.len() != chunk.length as usize {
                outcome.errors = outcome.errors.saturating_add(1);
                iroha_logger::warn!(
                    digest = %hex::encode(chunk.digest),
                    expected = chunk.length,
                    actual = bytes.len(),
                    path = %source.path.display(),
                    "rehydration source length mismatch"
                );
                continue;
            }
            let digest = blake3::hash(&bytes);
            if digest.as_bytes() != &chunk.digest {
                outcome.errors = outcome.errors.saturating_add(1);
                iroha_logger::warn!(
                    digest = %hex::encode(chunk.digest),
                    actual = %digest.to_hex(),
                    path = %source.path.display(),
                    "rehydration source digest mismatch"
                );
                continue;
            }
            if let Err(err) = crate::store::write_atomic(&chunk.path, &bytes) {
                outcome.errors = outcome.errors.saturating_add(1);
                iroha_logger::warn!(
                    %err,
                    digest = %hex::encode(chunk.digest),
                    path = %chunk.path.display(),
                    "failed to write rehydrated chunk"
                );
                continue;
            }
            outcome.rehydrated = outcome.rehydrated.saturating_add(1);
        }

        outcome.missing_after = missing_chunks
            .iter()
            .filter(|chunk| !chunk.path.exists())
            .count();
        outcome
    }

    fn rehydrate_missing_chunks_from_orchestrator(
        &self,
        task: &RepairTaskRecordV1,
        manifest: &StoredManifest,
        missing_chunks: &[ChunkFileRecord],
    ) -> RepairRehydrateOutcome {
        let remaining = missing_chunks
            .iter()
            .filter(|chunk| !chunk.path.exists())
            .cloned()
            .collect::<Vec<_>>();
        let mut outcome = RepairRehydrateOutcome {
            missing_before: remaining.len(),
            ..RepairRehydrateOutcome::default()
        };
        if remaining.is_empty() {
            return outcome;
        }

        let Some(orchestrator) = self.repair_orchestrator() else {
            outcome.missing_after = outcome.missing_before;
            return outcome;
        };

        let payloads = match orchestrator.rehydrate_missing_chunks(task, manifest, &remaining) {
            Ok(payloads) => payloads,
            Err(err) => {
                outcome.errors = outcome.errors.saturating_add(1);
                iroha_logger::warn!(
                    %err,
                    ticket = %task.ticket_id,
                    manifest = %hex::encode(task.manifest_digest),
                    provider = %hex::encode(task.provider_id),
                    "repair orchestrator rehydration failed"
                );
                outcome.missing_after = outcome.missing_before;
                return outcome;
            }
        };

        let mut missing_by_digest: HashMap<[u8; 32], Vec<&ChunkFileRecord>> = HashMap::new();
        for chunk in &remaining {
            missing_by_digest
                .entry(chunk.digest)
                .or_default()
                .push(chunk);
        }

        for payload in payloads {
            let source = payload.source.as_deref();
            let payload_len = match u32::try_from(payload.bytes.len()) {
                Ok(length) => length,
                Err(_) => {
                    outcome.errors = outcome.errors.saturating_add(1);
                    iroha_logger::warn!(
                        digest = %hex::encode(payload.digest),
                        actual = payload.bytes.len(),
                        source,
                        "orchestrator chunk length exceeds supported size"
                    );
                    continue;
                }
            };
            let digest = blake3::hash(&payload.bytes);
            if digest.as_bytes() != &payload.digest {
                outcome.errors = outcome.errors.saturating_add(1);
                iroha_logger::warn!(
                    digest = %hex::encode(payload.digest),
                    actual = %digest.to_hex(),
                    source,
                    "orchestrator chunk digest mismatch"
                );
                continue;
            }
            let Some(targets) = missing_by_digest.get(&payload.digest) else {
                outcome.errors = outcome.errors.saturating_add(1);
                iroha_logger::warn!(
                    digest = %hex::encode(payload.digest),
                    source,
                    "orchestrator returned unexpected chunk digest"
                );
                continue;
            };
            for &chunk in targets {
                if chunk.path.exists() {
                    continue;
                }
                if payload_len != chunk.length {
                    outcome.errors = outcome.errors.saturating_add(1);
                    iroha_logger::warn!(
                        digest = %hex::encode(chunk.digest),
                        expected = chunk.length,
                        actual = payload_len,
                        source,
                        "orchestrator chunk length mismatch"
                    );
                    continue;
                }
                if let Err(err) = crate::store::write_atomic(&chunk.path, &payload.bytes) {
                    outcome.errors = outcome.errors.saturating_add(1);
                    iroha_logger::warn!(
                        %err,
                        digest = %hex::encode(chunk.digest),
                        path = %chunk.path.display(),
                        source,
                        "failed to write rehydrated chunk from orchestrator"
                    );
                    continue;
                }
                outcome.rehydrated = outcome.rehydrated.saturating_add(1);
            }
        }

        outcome.missing_after = remaining
            .iter()
            .filter(|chunk| !chunk.path.exists())
            .count();
        outcome
    }

    /// Run a single repair worker tick for the supplied worker identifier.
    pub fn run_repair_worker_once(&self, worker_id: &str, now_unix: u64) -> RepairWorkerReport {
        let mut report = RepairWorkerReport::default();
        if !self.repair_config.enabled() || now_unix == 0 {
            return report;
        }
        let Some(storage) = self.storage.as_ref() else {
            report.record_error();
            iroha_logger::warn!("repair worker skipped: storage backend disabled");
            return report;
        };
        if let Err(err) = storage.ensure_durability_healthy() {
            report.record_error();
            iroha_logger::error!(%err, "repair worker skipped: storage backend fail-stopped");
            return report;
        }
        if let Err(err) = self.ensure_repair_durability_healthy() {
            report.record_error();
            iroha_logger::error!(%err, "repair worker skipped: node runtime fail-stopped");
            return report;
        }

        let candidates = match self.repair.claimable_tasks(now_unix) {
            Ok(candidates) => candidates,
            Err(err) => {
                report.record_error();
                iroha_logger::error!(%err, "repair worker skipped: task store unavailable");
                return report;
            }
        };
        if candidates.is_empty() {
            report.record_skipped();
            return report;
        }

        for task in candidates {
            let ticket_id = task.ticket_id.clone();
            let claim_key = repair_idempotency_key("claim", worker_id, &ticket_id, now_unix);
            let update = match self
                .repair
                .claim_ticket_with_event(&ticket_id, worker_id, now_unix, &claim_key)
            {
                Ok(update) => update,
                Err(RepairSchedulerError::LeaseHeld { .. })
                | Err(RepairSchedulerError::BackoffActive { .. })
                | Err(RepairSchedulerError::StoreConflict { .. }) => {
                    report.record_skipped();
                    continue;
                }
                Err(err) => {
                    report.record_error();
                    iroha_logger::warn!(
                        %err,
                        ticket = %ticket_id,
                        "repair worker claim failed"
                    );
                    continue;
                }
            };
            report.record_claim();
            if let Err(err) = self.publish_repair_update(&update, None) {
                report.record_error();
                iroha_logger::error!(
                    %err,
                    ticket = %ticket_id,
                    "repair worker fail-stopped after claim event persistence failure"
                );
                break;
            }

            let manifest = storage.manifest_by_digest(&task.manifest_digest);
            let (update, stage) = match manifest {
                Some(manifest) => {
                    let total = manifest.chunk_count();
                    let missing_chunks = match self
                        .schedulers
                        .try_with_pin(|| Self::missing_chunk_records(&manifest))
                    {
                        Ok(chunks) => chunks,
                        Err(err) => {
                            report.record_skipped();
                            iroha_logger::warn!(
                                %err,
                                ticket = %ticket_id,
                                "repair worker deferred: storage scheduler saturated"
                            );
                            break;
                        }
                    };
                    let missing = missing_chunks.len();
                    if missing == 0 {
                        let complete_key =
                            repair_idempotency_key("complete", worker_id, &ticket_id, now_unix);
                        let update = self
                            .repair
                            .complete_ticket_with_event(
                                &ticket_id,
                                worker_id,
                                now_unix,
                                Some("verified local chunks".into()),
                                &complete_key,
                            )
                            .map_err(|err| {
                                iroha_logger::warn!(
                                    %err,
                                    ticket = %ticket_id,
                                    "repair completion failed"
                                );
                                err
                            });
                        match update {
                            Ok(update) => (update, None),
                            Err(_) => {
                                report.record_error();
                                break;
                            }
                        }
                    } else {
                        let mut outcome = match self.schedulers.try_with_pin(|| {
                            self.rehydrate_missing_chunks_from_local_replicas(
                                storage,
                                &missing_chunks,
                            )
                        }) {
                            Ok(outcome) => outcome,
                            Err(err) => {
                                report.record_skipped();
                                iroha_logger::warn!(
                                    %err,
                                    ticket = %ticket_id,
                                    "repair worker deferred: storage scheduler saturated"
                                );
                                break;
                            }
                        };
                        if outcome.missing_after > 0 {
                            let orchestrator_outcome = self
                                .rehydrate_missing_chunks_from_orchestrator(
                                    &task,
                                    &manifest,
                                    &missing_chunks,
                                );
                            outcome.rehydrated = outcome
                                .rehydrated
                                .saturating_add(orchestrator_outcome.rehydrated);
                            outcome.errors =
                                outcome.errors.saturating_add(orchestrator_outcome.errors);
                            outcome.missing_after = orchestrator_outcome.missing_after;
                        }
                        if outcome.errors > 0 {
                            iroha_logger::warn!(
                                ticket = %ticket_id,
                                missing_before = outcome.missing_before,
                                missing_after = outcome.missing_after,
                                errors = outcome.errors,
                                "repair rehydration encountered errors"
                            );
                        }
                        if outcome.missing_after == 0 {
                            let mut resolution = if outcome.rehydrated > 0 {
                                format!(
                                    "rehydrated {} missing chunks from local replicas",
                                    outcome.rehydrated
                                )
                            } else {
                                "verified local chunks after rehydration attempt".to_string()
                            };
                            if outcome.errors > 0 {
                                resolution
                                    .push_str(&format!("; {} rehydration errors", outcome.errors));
                            }
                            let complete_key =
                                repair_idempotency_key("complete", worker_id, &ticket_id, now_unix);
                            let update = self
                                .repair
                                .complete_ticket_with_event(
                                    &ticket_id,
                                    worker_id,
                                    now_unix,
                                    Some(resolution),
                                    &complete_key,
                                )
                                .map_err(|err| {
                                    iroha_logger::warn!(
                                        %err,
                                        ticket = %ticket_id,
                                        "repair completion failed"
                                    );
                                    err
                                });
                            match update {
                                Ok(update) => (update, None),
                                Err(_) => {
                                    report.record_error();
                                    break;
                                }
                            }
                        } else {
                            let reason = format!(
                                "missing {} of {} chunks after rehydrating {}",
                                outcome.missing_after, total, outcome.rehydrated
                            );
                            let fail_key =
                                repair_idempotency_key("fail", worker_id, &ticket_id, now_unix);
                            let update = self
                                .repair
                                .fail_ticket_with_event(
                                    &ticket_id, worker_id, now_unix, reason, &fail_key,
                                )
                                .map_err(|err| {
                                    iroha_logger::warn!(
                                        %err,
                                        ticket = %ticket_id,
                                        "repair failure update rejected"
                                    );
                                    err
                                });
                            match update {
                                Ok(update) => (update, Some(RepairSlashStage::Drafted)),
                                Err(_) => {
                                    report.record_error();
                                    break;
                                }
                            }
                        }
                    }
                }
                None => {
                    let fail_key = repair_idempotency_key("fail", worker_id, &ticket_id, now_unix);
                    let update = self
                        .repair
                        .fail_ticket_with_event(
                            &ticket_id,
                            worker_id,
                            now_unix,
                            "manifest missing from local storage".into(),
                            &fail_key,
                        )
                        .map_err(|err| {
                            iroha_logger::warn!(
                                %err,
                                ticket = %ticket_id,
                                "repair failure update rejected"
                            );
                            err
                        });
                    match update {
                        Ok(update) => (update, Some(RepairSlashStage::Drafted)),
                        Err(_) => {
                            report.record_error();
                            break;
                        }
                    }
                }
            };

            report.record_state(&update.record.state);
            if let Err(err) = self.publish_repair_update(&update, stage) {
                report.record_error();
                iroha_logger::error!(
                    %err,
                    ticket = %ticket_id,
                    "repair worker fail-stopped after update event persistence failure"
                );
            }
            break;
        }

        report
    }

    /// Run a GC sweep against expired manifests.
    pub fn run_gc_once(&self, now_unix: u64) -> GcSweepReport {
        self.run_gc_with_policy(now_unix, GcEvictionPolicy::RetentionEpoch)
    }

    /// Run a GC sweep using LRU ordering for capacity pressure.
    pub fn run_gc_for_capacity(&self, now_unix: u64, _required_bytes: u64) -> GcSweepReport {
        self.run_gc_with_policy(now_unix, GcEvictionPolicy::LruExpired)
    }

    fn run_gc_with_policy(&self, now_unix: u64, policy: GcEvictionPolicy) -> GcSweepReport {
        const REASON_RETENTION_EXPIRED: &str = "retention_expired";
        const REASON_RETENTION_EXPIRED_NO_PROVIDER: &str = "retention_expired_provider_missing";
        const REASON_REPAIR_ACTIVE: &str = "repair_active";
        const REASON_DEAL_ACTIVE: &str = "deal_active";
        const REASON_SHARED_CHUNKS: &str = "shared_chunks";
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
        let repair_tasks = match self.repair.list_tasks(RepairTaskFilters::default()) {
            Ok(tasks) => tasks,
            Err(err) => {
                report.errors = report.errors.saturating_add(1);
                iroha_logger::error!(%err, "GC sweep skipped: repair state unavailable");
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

        let usage = self.capacity_usage();
        let provider_id = usage.provider_id.unwrap_or([0_u8; 32]);
        let provider_known = usage.provider_id.is_some();

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

        match policy {
            GcEvictionPolicy::RetentionEpoch => {
                expired.sort_by(|left, right| {
                    left.retention_epoch()
                        .cmp(&right.retention_epoch())
                        .then(left.manifest_id().cmp(right.manifest_id()))
                });
            }
            GcEvictionPolicy::LruExpired => {
                expired.sort_by(|left, right| {
                    left.last_access()
                        .cmp(&right.last_access())
                        .then(left.manifest_id().cmp(right.manifest_id()))
                });
            }
        }

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
            if repair_tasks
                .iter()
                .any(|task| task.manifest_digest == digest && !repair_task_terminal(task))
            {
                report.skipped.push(GcSkip {
                    manifest_id: manifest.manifest_id().to_owned(),
                    reason: REASON_REPAIR_ACTIVE.to_string(),
                });
                iroha_logger::warn!(
                    manifest_id = %manifest.manifest_id(),
                    "GC retention blocked by active repair tasks"
                );
                global_or_default().inc_sorafs_gc_blocked(REASON_REPAIR_ACTIVE);
                global_sorafs_gc_otel().record_blocked(REASON_REPAIR_ACTIVE);
                let payload = GcAuditPayloadV1 {
                    version: GC_AUDIT_PAYLOAD_VERSION_V1,
                    manifest_digest: digest,
                    provider_id,
                    evicted_at_unix: now_unix,
                    freed_bytes: 0,
                    reason: if provider_known {
                        REASON_RETENTION_EXPIRED.to_string()
                    } else {
                        REASON_RETENTION_EXPIRED_NO_PROVIDER.to_string()
                    },
                    blocked_reason: Some(REASON_REPAIR_ACTIVE.to_string()),
                };
                self.publish_gc_audit_event(payload);
                continue;
            }

            if manifest
                .retention_source()
                .and_then(|source| source.deal_end_epoch)
                .is_some_and(|deal_end| deal_end > now_unix)
            {
                report.skipped.push(GcSkip {
                    manifest_id: manifest.manifest_id().to_owned(),
                    reason: REASON_DEAL_ACTIVE.to_string(),
                });
                iroha_logger::warn!(
                    manifest_id = %manifest.manifest_id(),
                    "GC retention blocked by active deal window"
                );
                global_or_default().inc_sorafs_gc_blocked(REASON_DEAL_ACTIVE);
                global_sorafs_gc_otel().record_blocked(REASON_DEAL_ACTIVE);
                let payload = GcAuditPayloadV1 {
                    version: GC_AUDIT_PAYLOAD_VERSION_V1,
                    manifest_digest: digest,
                    provider_id,
                    evicted_at_unix: now_unix,
                    freed_bytes: 0,
                    reason: if provider_known {
                        REASON_RETENTION_EXPIRED.to_string()
                    } else {
                        REASON_RETENTION_EXPIRED_NO_PROVIDER.to_string()
                    },
                    blocked_reason: Some(REASON_DEAL_ACTIVE.to_string()),
                };
                self.publish_gc_audit_event(payload);
                continue;
            }

            match storage.manifest_has_shared_chunks(manifest.manifest_id()) {
                Ok(true) => {
                    report.skipped.push(GcSkip {
                        manifest_id: manifest.manifest_id().to_owned(),
                        reason: REASON_SHARED_CHUNKS.to_string(),
                    });
                    iroha_logger::warn!(
                        manifest_id = %manifest.manifest_id(),
                        "GC retention blocked by shared chunks"
                    );
                    global_or_default().inc_sorafs_gc_blocked(REASON_SHARED_CHUNKS);
                    global_sorafs_gc_otel().record_blocked(REASON_SHARED_CHUNKS);
                    let payload = GcAuditPayloadV1 {
                        version: GC_AUDIT_PAYLOAD_VERSION_V1,
                        manifest_digest: digest,
                        provider_id,
                        evicted_at_unix: now_unix,
                        freed_bytes: 0,
                        reason: if provider_known {
                            REASON_RETENTION_EXPIRED.to_string()
                        } else {
                            REASON_RETENTION_EXPIRED_NO_PROVIDER.to_string()
                        },
                        blocked_reason: Some(REASON_SHARED_CHUNKS.to_string()),
                    };
                    self.publish_gc_audit_event(payload);
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

            let freed_bytes = match storage.evict_manifest(manifest.manifest_id()) {
                Ok(bytes) => bytes,
                Err(err) => {
                    report.errors = report.errors.saturating_add(1);
                    iroha_logger::warn!(
                        %err,
                        manifest_id = %manifest.manifest_id(),
                        "GC eviction failed"
                    );
                    continue;
                }
            };

            evicted_count += 1;
            report.freed_bytes = report.freed_bytes.saturating_add(freed_bytes);
            let reason = if provider_known {
                REASON_RETENTION_EXPIRED
            } else {
                REASON_RETENTION_EXPIRED_NO_PROVIDER
            };
            report.evictions.push(GcEviction {
                manifest_id: manifest.manifest_id().to_owned(),
                manifest_digest: digest,
                retention_epoch: manifest.retention_epoch(),
                freed_bytes,
                reason: reason.to_string(),
            });

            let payload = GcAuditPayloadV1 {
                version: GC_AUDIT_PAYLOAD_VERSION_V1,
                manifest_digest: digest,
                provider_id,
                evicted_at_unix: now_unix,
                freed_bytes,
                reason: reason.to_string(),
                blocked_reason: None,
            };
            self.publish_gc_audit_event(payload);
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
    ) -> Result<SorafsReconciliationReportV1, ReconciliationError> {
        let result = self.compute_reconciliation_report(now_unix);
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
    ) -> Result<SorafsReconciliationReportV1, ReconciliationError> {
        if now_unix == 0 {
            return Err(ReconciliationError::InvalidTimestamp);
        }
        let Some(storage) = self.storage.as_ref() else {
            return Err(ReconciliationError::StorageDisabled);
        };

        let provider_id = match self.capacity_usage().provider_id {
            Some(provider_id) => provider_id,
            None => {
                iroha_logger::warn!("reconciliation report provider_id missing; using zeros");
                [0_u8; 32]
            }
        };

        let repair_records = self
            .repair
            .list_tasks(RepairTaskFilters::default())
            .map_err(|err| ReconciliationError::RepairStore(err.to_string()))?;
        let repair_entries = repair_records
            .iter()
            .map(|record| reconciliation::RepairReconciliationEntry {
                ticket_id: record.ticket_id.0.clone(),
                manifest_digest: record.manifest_digest,
                provider_id: record.provider_id,
                state: record.state.clone(),
            })
            .collect::<Vec<_>>();
        let repair_snapshot = reconciliation::RepairReconciliationSnapshot {
            version: reconciliation::RECONCILIATION_SNAPSHOT_VERSION_V1,
            tasks: repair_entries,
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
            repair_task_count: u32::try_from(repair_records.len()).unwrap_or(u32::MAX),
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
            rollup_count: u32::try_from(entries.len()).map_err(|_| {
                ReconciliationError::AppealFinance("rollup count exceeds u32".to_string())
            })?,
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

    /// Record an audit verdict and update telemetry counters accordingly.
    pub fn record_por_verdict(
        &self,
        verdict: &AuditVerdictV1,
        trusted_auditor_keys: &[Vec<u8>],
        auditor_threshold: usize,
    ) -> Result<PorVerdictOutcome, PorTrackerError> {
        let _checkpoint_guard = self.auxiliary_checkpoint_lock.lock().map_err(|_| {
            PorTrackerError::RepairStore(repair::RepairStoreError::Other(
                "auxiliary checkpoint transaction lock poisoned".to_owned(),
            ))
        })?;
        self.ensure_durability_healthy()
            .map_err(PorTrackerError::RuntimeCheckpoint)?;
        {
            let history = self.por_history.read().map_err(|_| {
                PorTrackerError::RepairStore(repair::RepairStoreError::Other(
                    "PoR history lock poisoned".to_owned(),
                ))
            })?;
            let key = (verdict.manifest_digest, verdict.provider_id);
            let limit = self.config.runtime_retention().state_entry_limit();
            if !history.contains_key(&key) && history.len() >= limit {
                return Err(PorTrackerError::RepairStore(
                    repair::RepairStoreError::Other(format!(
                        "PoR history retention exhausted (limit {limit})"
                    )),
                ));
            }
        }
        let previous_tracker = self.por.checkpoint();
        let (stats, repair_history_id) = self.por.record_verdict_with(
            verdict,
            trusted_auditor_keys,
            auditor_threshold,
            |stats| {
                if self.repair_config.enabled() {
                    self.repair
                        .register_por_verdict(verdict, stats.failed_samples)
                } else {
                    Ok(None)
                }
            },
        )?;
        let previous_history = self
            .por_history
            .read()
            .map_err(|_| {
                PorTrackerError::RuntimeCheckpoint("PoR history lock poisoned".to_owned())
            })?
            .clone();
        let consecutive_failures = self.update_por_history_entry(verdict)?;
        let slash = self.evaluate_por_penalty(verdict, &stats, consecutive_failures)?;
        if let Err(err) = self.persist_auxiliary_runtime_checkpoint_unlocked() {
            if err.committed {
                return Err(PorTrackerError::RuntimeCheckpoint(err.to_string()));
            }
            let history_rollback = self.por_history.write().map(|mut history| {
                *history = previous_history;
            });
            let tracker_rollback = self.por.restore_checkpoint(previous_tracker);
            if history_rollback.is_err() || tracker_rollback.is_err() {
                let rollback = match (history_rollback, tracker_rollback) {
                    (Err(_), Err(tracker)) => format!(
                        "PoR history rollback lock poisoned; tracker rollback failed: {tracker}"
                    ),
                    (Err(_), Ok(())) => "PoR history rollback lock poisoned".to_owned(),
                    (Ok(()), Err(tracker)) => {
                        format!("PoR tracker rollback failed: {tracker}")
                    }
                    (Ok(()), Ok(())) => unreachable!(),
                };
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
            repair_history_id,
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
    ) -> Result<(), PotrReceiptValidationError> {
        receipt.validate()?;
        self.potr.record_receipt(receipt);
        Ok(())
    }

    /// Retrieve PoTR receipts matching the manifest/provider filters.
    #[must_use]
    pub fn potr_receipts(
        &self,
        manifest_digest: &[u8; 32],
        provider_id: &[u8; 32],
        tier: Option<ProofStreamTier>,
    ) -> Vec<PotrReceiptV1> {
        self.potr.receipts_for(manifest_digest, provider_id, tier)
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
        let chunk_roles_retry = chunk_roles.clone();
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
            Err(StorageError::CapacityExceeded { .. })
                if self.gc_config.enabled() && self.gc_config.pre_admission_sweep() =>
            {
                let gc_report = self.run_gc_for_capacity(unix_now_secs(), plan.content_length);
                if gc_report.errors > 0 {
                    iroha_logger::warn!(
                        errors = gc_report.errors,
                        "GC pre-admission sweep reported errors"
                    );
                }
                let retry = self.schedulers.try_with_pin(|| {
                    storage.ingest_manifest_with_layout(
                        manifest,
                        plan,
                        reader,
                        stripe_layout,
                        chunk_roles_retry,
                    )
                })?;
                match retry {
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

fn load_or_create_moderation_quarantine_object_key(
    path: &Path,
) -> Result<[u8; 32], ModerationQuarantineObjectError> {
    let _guard =
        MODERATION_KEY_CREATE_LOCK
            .lock()
            .map_err(|_| ModerationQuarantineObjectError::Io {
                path: path.display().to_string(),
                message: "sealing-key creation lock poisoned".to_owned(),
            })?;
    match read_local_checkpoint_bounded(path, 32) {
        Ok(Some(bytes)) => decode_moderation_quarantine_object_key(path, bytes),
        Ok(None) => create_moderation_quarantine_object_key(path),
        Err(err) => Err(ModerationQuarantineObjectError::Io {
            path: path.display().to_string(),
            message: err.to_string(),
        }),
    }
}

fn load_moderation_quarantine_object_key(
    path: &Path,
) -> Result<[u8; 32], ModerationQuarantineObjectError> {
    let bytes = read_local_checkpoint_bounded(path, 32)
        .map_err(|err| ModerationQuarantineObjectError::Io {
            path: path.display().to_string(),
            message: err.to_string(),
        })?
        .ok_or_else(|| ModerationQuarantineObjectError::Io {
            path: path.display().to_string(),
            message: "sealing key does not exist".to_owned(),
        })?;
    decode_moderation_quarantine_object_key(path, bytes)
}

fn create_moderation_quarantine_object_key(
    path: &Path,
) -> Result<[u8; 32], ModerationQuarantineObjectError> {
    let mut key = [0u8; 32];
    let mut rng = OsRng;
    rng.try_fill_bytes(&mut key)
        .map_err(|err| ModerationQuarantineObjectError::Io {
            path: path.display().to_string(),
            message: format!("failed to generate local sealing key: {err}"),
        })?;
    match write_local_private_checkpoint_atomic(path, &key) {
        Ok(()) => Ok(key),
        // The key is already visible. A later successful object-index commit
        // synchronizes this same directory, closing the durability window.
        Err(err) if err.committed => Ok(key),
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            load_moderation_quarantine_object_key(path)
        }
        Err(err) => Err(ModerationQuarantineObjectError::Io {
            path: path.display().to_string(),
            message: err.to_string(),
        }),
    }
}

fn decode_moderation_quarantine_object_key(
    path: &Path,
    bytes: Vec<u8>,
) -> Result<[u8; 32], ModerationQuarantineObjectError> {
    let key: [u8; 32] = bytes.try_into().map_err(|bytes: Vec<u8>| {
        ModerationQuarantineObjectError::InvalidInput {
            message: format!(
                "local quarantine object key `{}` must be 32 bytes, found {}",
                path.display(),
                bytes.len()
            ),
        }
    })?;
    Ok(key)
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
        ModerationQuarantineObjectError::Io { path, message } => {
            ModerationEvidenceViewerError::Io { path, message }
        }
        ModerationQuarantineObjectError::StateLockPoisoned => {
            ModerationEvidenceViewerError::StateLockPoisoned
        }
    }
}

fn orderbook_provider_escrow_runways(
    snapshot: &OrderbookSnapshot,
    now_unix: u64,
) -> Result<BTreeMap<String, u64>, String> {
    let mut debit_by_channel = BTreeMap::<[u8; 32], XorQuantity>::new();
    for receipt in &snapshot.settlement_receipts {
        let total = debit_by_channel
            .entry(receipt.channel_id)
            .or_insert_with(XorQuantity::zero);
        *total = total.checked_add(&receipt.xor_debited).map_err(|error| {
            format!(
                "channel {} debit accumulation failed: {error}",
                hex::encode(receipt.channel_id)
            )
        })?;
    }

    let mut runways_by_provider = BTreeMap::<[u8; 32], Option<u64>>::new();
    for channel in &snapshot.settlement_channels {
        let provider_runway = runways_by_provider.entry(channel.provider_id).or_default();
        if !matches!(channel.status, SettlementChannelStatusV1::Open) {
            continue;
        }

        let Some(elapsed) = now_unix.checked_sub(channel.opened_at_unix) else {
            continue;
        };
        let debited = debit_by_channel
            .get(&channel.channel_id)
            .cloned()
            .unwrap_or_default();
        let escrow_remaining = &channel.xor_locked;
        if elapsed == 0 || debited.is_zero() || escrow_remaining.is_zero() {
            continue;
        }

        let scaled_remaining = escrow_remaining
            .as_quantity()
            .try_mul_decimal(&Numeric::from(elapsed))
            .map_err(|error| format!("escrow runway multiplication failed: {error}"))?;
        let runway_numeric = scaled_remaining
            .try_ratio_round(debited.as_quantity(), 0, RoundingMode::TowardZero)
            .map_err(|error| format!("escrow runway division failed: {error}"))?;
        let runway = runway_numeric
            .try_mantissa_u128()
            .and_then(|value| u64::try_from(value).ok())
            .ok_or_else(|| "escrow runway exceeds u64 seconds".to_string())?;
        *provider_runway = Some(provider_runway.map_or(runway, |current| current.min(runway)));
    }

    Ok(runways_by_provider
        .into_iter()
        .map(|(provider, runway)| (hex::encode(provider), runway.unwrap_or(0)))
        .collect())
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
    };

    use iroha_crypto::{Algorithm, Signature as IrohaSignature, SignatureOf};
    use iroha_data_model::{
        metadata::Metadata,
        name::Name,
        sorafs::{
            capacity::{CapacityDeclarationRecord, ProviderId},
            deal::{
                BYTES_PER_GIB, ClientId, DealProposal, DealStatus, DealTerms, DealUsageReport,
                GIB_HOURS_PER_MONTH, MicropaymentTicket, TicketId,
            },
            moderation::{
                ADVERSARIAL_CORPUS_VERSION_V1, AdversarialCorpusManifestV1,
                AdversarialPerceptualFamilyV1, AdversarialPerceptualVariantV1,
                MODERATION_REPRO_MANIFEST_VERSION_V1, ModerationModelFingerprintV1,
                ModerationReproBodyV1, ModerationReproManifestV1, ModerationReproSignatureV1,
                ModerationSeedMaterialV1, ModerationThresholdsV1,
                SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1,
                SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
                SORAFS_MODERATION_BALLOT_REVEAL_VERSION_V1, SoraFsModerationBallotCommitV1,
                SoraFsModerationBallotContextV1, SoraFsModerationBallotError,
                SoraFsModerationBallotRevealV1, SoraFsModerationVoteChoice,
            },
            pin_registry::StorageClass,
            reserve::{ReserveDuration, ReserveLifecycleStage, ReservePolicyV1, ReserveTier},
        },
    };
    use iroha_telemetry::metrics::global_or_default;
    use norito::{codec::Decode, to_bytes};
    use sorafs_car::CarBuildPlan;
    use sorafs_manifest::PorReportIsoWeek;
    use sorafs_manifest::{
        BYTES_PER_GIB as ORDERBOOK_BYTES_PER_GIB, ByteRangeV1, DagCodecId, ManifestBuilder,
        ORDERBOOK_CANCEL_VERSION_V1, ORDERBOOK_ORDER_VERSION_V1, OrderCancelReasonV1,
        OrderCancelV1, OrderRequestV1, OrderSideV1, OrderTierV1, OrderbookSignatureV1, PinPolicy,
        REPUTATION_PROVIDER_INPUT_VERSION_V1, REPUTATION_PROVIDER_METRICS_VERSION_V1,
        ReputationDegradationFlagV1, ReputationProviderInputV1, ReputationProviderMetricsV1,
        ReputationReserveStageV1, ReputationWeightsV1, SETTLEMENT_RECEIPT_VERSION_V1,
        SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1,
        SORAFS_APPEAL_FINANCE_SETTLEMENT_RECEIPT_VERSION_V1,
        SORAFS_RECONCILIATION_REPORT_VERSION_V1, SettlementChannelV1, SettlementReceiptV1,
        SoraFsAppealFinanceAccountFlowV1, SoraFsAppealFinanceJurorPayoutV1,
        SoraFsAppealFinanceOutcomeV1, SoraFsAppealFinanceReportV1,
        SoraFsAppealFinanceSettlementReceiptV1, SoraFsAppealFinanceWeeklyRollupV1,
        SoraFsModerationBallotGovernanceEventKindV1, SoraFsModerationBallotGovernanceEventV1,
        SoraFsModerationVoteChoiceV1, SorafsReconciliationReportV1, build_reputation_snapshot,
        capacity::{
            CAPACITY_DECLARATION_VERSION_V1, CapacityDeclarationV1, CapacityMetadataEntry,
            ChunkerCommitmentV1, LaneCommitmentV1, REPLICATION_ORDER_VERSION_V1,
            ReplicationAssignmentV1, ReplicationOrderSlaV1, ReplicationOrderV1,
        },
        deal::{DealSettlementStatusV1, DealSettlementV1, XorQuantity},
        derive_orderbook_order_id_v1, order_cancel_signature_digest_v1,
        order_request_signature_digest_v1,
        provider_advert::SignatureAlgorithm,
        repair::{
            CompletedRepairStateV1, EscalatedRepairStateV1, FailedRepairStateV1,
            GC_AUDIT_EVENT_VERSION_V1, GC_AUDIT_PAYLOAD_VERSION_V1, GcAuditEventV1,
            InProgressRepairStateV1, QueuedRepairStateV1, REPAIR_EVIDENCE_VERSION_V1,
            REPAIR_REPORT_VERSION_V1, REPAIR_SLASH_PROPOSAL_VERSION_V1, RepairAuditEventV1,
            RepairCauseV1, RepairEvidenceV1, RepairManualCauseV1, RepairReportV1,
            RepairSlashProposalV1, RepairTaskStateV1, RepairTaskStatusV1, RepairTicketId,
        },
        settlement_receipt_signature_digest_v1,
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
        let path = temp.path().join("runtime").join("checkpoint.to");
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
    fn local_checkpoint_distinguishes_precommit_and_visible_uncertain_failures() {
        fn fail_parent_sync(_: &Path) -> io::Result<()> {
            Err(io::Error::other("injected parent sync failure"))
        }

        let temp = tempfile::tempdir().expect("temp dir");
        let path = temp.path().join("checkpoint.to");
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
        let path = Arc::new(temp.path().join("checkpoint.to"));
        let barrier = Arc::new(Barrier::new(8));
        let payloads = (0_u8..8).map(|byte| vec![byte; 4_096]).collect::<Vec<_>>();
        let workers = payloads
            .iter()
            .cloned()
            .map(|payload| {
                let path = Arc::clone(&path);
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    write_local_checkpoint_atomic_bounded(&path, &payload, 8_192)
                })
            })
            .collect::<Vec<_>>();
        for worker in workers {
            worker.join().expect("checkpoint writer joins").unwrap();
        }
        let bytes = fs::read(&*path).expect("read final checkpoint");
        assert!(payloads.contains(&bytes));
        let leftovers = fs::read_dir(temp.path())
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
        let victim = temp.path().join("victim");
        fs::write(&victim, b"unchanged").expect("write victim");
        let target = temp.path().join("checkpoint.to");
        symlink(&victim, &target).expect("symlink target");
        assert!(write_local_checkpoint_atomic(&target, b"replacement").is_err());
        assert!(read_local_checkpoint_bounded(&target, 128).is_err());
        assert_eq!(fs::read(&victim).unwrap(), b"unchanged");

        let real_parent = temp.path().join("real-parent");
        fs::create_dir(&real_parent).expect("create real parent");
        let linked_parent = temp.path().join("linked-parent");
        symlink(&real_parent, &linked_parent).expect("symlink parent");
        assert!(write_local_checkpoint_atomic(&linked_parent.join("state.to"), b"state").is_err());
        assert!(!real_parent.join("state.to").exists());

        let nested_target = linked_parent.join("nested").join("state.to");
        assert!(write_local_checkpoint_atomic(&nested_target, b"state").is_err());
        assert!(!real_parent.join("nested").exists());

        let hardlink_target = temp.path().join("hardlink-target.to");
        write_local_checkpoint_atomic(&hardlink_target, b"state").expect("write hardlink target");
        let hardlink_alias = temp.path().join("hardlink-alias.to");
        fs::hard_link(&hardlink_target, &hardlink_alias).expect("create hardlink alias");
        assert!(read_local_checkpoint_bounded(&hardlink_target, 128).is_err());
        assert!(write_local_checkpoint_atomic(&hardlink_target, b"replacement").is_err());

        let permissive_parent = temp.path().join("permissive-parent");
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
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base.data_dir().clone())
            .runtime_retention(RuntimeRetentionPolicy::new(4, 8, 64))
            .build();
        drop(NodeHandle::new(cfg.clone()));
        let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"not-norito").unwrap();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| NodeHandle::new(
                cfg.clone()
            )))
            .is_err()
        );

        fs::write(&path, vec![0xAA; 65]).unwrap();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| NodeHandle::new(cfg)))
                .is_err()
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
        symlink(&victim, &path).unwrap();
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| NodeHandle::new(cfg)))
                .is_err()
        );
    }

    fn reserve_lifecycle_update(
        provider_byte: u8,
        days_past_due: u16,
        reserve_balance: XorQuantity,
        observed_at_unix: u64,
    ) -> ReserveLifecycleUpdate {
        reserve_lifecycle_update_for_provider(
            [provider_byte; 32],
            vec![provider_byte; 4],
            days_past_due,
            reserve_balance,
            observed_at_unix,
        )
    }

    fn reserve_lifecycle_update_for_provider(
        provider_id: [u8; 32],
        provider_account: Vec<u8>,
        days_past_due: u16,
        reserve_balance: XorQuantity,
        observed_at_unix: u64,
    ) -> ReserveLifecycleUpdate {
        let policy = ReservePolicyV1::default();
        let quote = policy
            .quote(
                StorageClass::Hot,
                10,
                ReserveDuration::Monthly,
                ReserveTier::TierA,
                reserve_balance,
            )
            .expect("reserve quote");
        ReserveLifecycleUpdate {
            provider_id,
            provider_account,
            quote,
            days_past_due,
            grace_period_days: 7,
            default_after_days: 30,
            observed_at_unix,
        }
    }

    fn reserve_movement_request(
        movement_byte: u8,
        provider_byte: u8,
        kind: ReserveMovementKind,
        amount: XorQuantity,
    ) -> ReserveMovementRequest {
        ReserveMovementRequest {
            movement_id: [movement_byte; 32],
            provider_id: [provider_byte; 32],
            provider_account: vec![provider_byte; 4],
            reserve_account: b"reserve-account".to_vec(),
            asset_definition_id: b"xor#sora".to_vec(),
            kind,
            amount,
            idempotency_key: format!("movement-{movement_byte}"),
            observed_at_unix: 1_800_000_000 + u64::from(movement_byte),
        }
    }

    fn reserve_movement_custody_update(
        movement_byte: u8,
        status: ReserveMovementCustodyStatus,
        tx_byte: u8,
    ) -> ReserveMovementCustodyUpdate {
        ReserveMovementCustodyUpdate {
            movement_id: [movement_byte; 32],
            status,
            tx_hash_hex: hex::encode([tx_byte; 32]),
            observed_at_unix: 1_900_000_000 + u64::from(tx_byte),
        }
    }

    fn reserve_appeal_request(appeal_byte: u8, provider_byte: u8) -> ReserveAppealRequest {
        reserve_appeal_request_for_provider(
            appeal_byte,
            [provider_byte; 32],
            vec![provider_byte; 4],
        )
    }

    fn reserve_appeal_request_for_provider(
        appeal_byte: u8,
        provider_id: [u8; 32],
        provider_account: Vec<u8>,
    ) -> ReserveAppealRequest {
        ReserveAppealRequest {
            appeal_id: [appeal_byte; 32],
            provider_id,
            provider_account,
            requested_stage: Some(ReserveLifecycleStage::Grace),
            reason: format!("appeal reason {appeal_byte}"),
            evidence_digest_hex: Some(hex::encode([0xA0 | appeal_byte; 32])),
            idempotency_key: format!("appeal-{appeal_byte}"),
            observed_at_unix: 2_100_000_000 + u64::from(appeal_byte),
        }
    }

    fn reserve_appeal_decision(
        appeal_byte: u8,
        status: ReserveAppealStatus,
    ) -> ReserveAppealDecision {
        ReserveAppealDecision {
            appeal_id: [appeal_byte; 32],
            status,
            decision_account: b"reserve-authority".to_vec(),
            rationale: format!("appeal decision {appeal_byte}"),
            decided_at_unix: 2_200_000_000 + u64::from(appeal_byte),
        }
    }

    fn reserve_lifecycle_policy_update(policy_byte: u8) -> ReserveLifecyclePolicyUpdate {
        ReserveLifecyclePolicyUpdate {
            policy_id: [policy_byte; 32],
            authority_account: b"reserve-authority".to_vec(),
            grace_period_days: 7,
            default_after_days: 30,
            effective_at_unix: 2_300_000_000 + u64::from(policy_byte),
            reason: format!("policy update {policy_byte}"),
            idempotency_key: format!("policy-{policy_byte}"),
            observed_at_unix: 2_400_000_000 + u64::from(policy_byte),
        }
    }

    #[test]
    fn reserve_runtime_is_restart_durable_and_reports_truncated_event_history() {
        let (base, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base.data_dir().clone())
            .runtime_retention(RuntimeRetentionPolicy::new(2, 8, 2 * 1024 * 1024))
            .build();
        let source = NodeHandle::new(cfg.clone());
        source
            .record_reserve_lifecycle_policy_update(reserve_lifecycle_policy_update(0x80))
            .expect("persist lifecycle policy");
        for days_past_due in 0..3 {
            source
                .record_reserve_lifecycle_update(reserve_lifecycle_update(
                    0x81,
                    days_past_due,
                    XorQuantity::zero(),
                    1_800_000_000 + u64::from(days_past_due),
                ))
                .expect("persist lifecycle update");
        }
        source
            .record_reserve_movement(reserve_movement_request(
                0x82,
                0x81,
                ReserveMovementKind::TopUp,
                XorQuantity::try_from_micro(100).expect("legacy micro-XOR value is representable"),
            ))
            .expect("persist movement");
        source
            .record_reserve_movement_custody_update(reserve_movement_custody_update(
                0x82,
                ReserveMovementCustodyStatus::Submitted,
                0x83,
            ))
            .expect("persist custody update");
        source
            .record_reserve_appeal(reserve_appeal_request(0x84, 0x81))
            .expect("persist appeal");
        source
            .record_reserve_appeal_decision(reserve_appeal_decision(
                0x84,
                ReserveAppealStatus::Accepted,
            ))
            .expect("persist appeal decision");
        drop(source);

        let restored = NodeHandle::new(cfg);
        assert_eq!(
            restored
                .reserve_provider_balance([0x81; 32])
                .expect("restored balance")
                .balance
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            100
        );
        assert_eq!(
            restored
                .reserve_movement([0x82; 32])
                .expect("restored movement")
                .custody_status,
            ReserveMovementCustodyStatus::Submitted
        );
        assert_eq!(
            restored
                .reserve_appeal([0x84; 32])
                .expect("restored appeal")
                .status,
            ReserveAppealStatus::Accepted
        );
        assert!(restored.latest_reserve_lifecycle_policy().is_some());
        let replay = restored.reserve_lifecycle_events_replay(Some(0), 10);
        assert_eq!(replay.oldest_available_sequence, Some(2));
        assert_eq!(replay.latest_sequence, Some(3));
        assert!(replay.gap);
        assert_eq!(replay.events.len(), 2);
    }

    fn moderation_repro_manifest_fixture(
        manifest_id_byte: u8,
        manifest_digest_byte: u8,
        runner_hash_byte: u8,
    ) -> ModerationReproManifestV1 {
        let body = ModerationReproBodyV1 {
            schema_version: MODERATION_REPRO_MANIFEST_VERSION_V1,
            manifest_id: [manifest_id_byte; 16],
            manifest_digest: [manifest_digest_byte; 32],
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
                artifact_digest: [0x66; 32],
                weights_digest: [0x77; 32],
                opset: 17,
                weight: Some(10_000),
            }],
            notes: Some("registry fixture".to_string()),
        };
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

    fn reputation_snapshot_fixture_with(
        snapshot_id: [u8; 16],
        generated_at_unix: u64,
        previous_snapshot_id: Option<[u8; 16]>,
    ) -> ReputationSnapshotV1 {
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
        let input = ReputationProviderInputV1 {
            version: REPUTATION_PROVIDER_INPUT_VERSION_V1,
            provider_id: "provider-a".to_string(),
            metrics,
            reserve_stage: ReputationReserveStageV1::Active,
            previous_score_bps: None,
            active_dispute: false,
            slashing_event: false,
        };
        build_reputation_snapshot(
            snapshot_id,
            generated_at_unix,
            ReputationWeightsV1::default(),
            &[input],
            previous_snapshot_id,
        )
        .expect("reputation snapshot fixture")
    }

    fn reputation_snapshot_fixture() -> ReputationSnapshotV1 {
        reputation_snapshot_fixture_with([0x42; 16], 1_800_000_000, None)
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
            ModerationPrivacyModeV1, ModerationPrivacyParametersV1,
        };

        ModerationPrivacyAggregateV1 {
            version: MODERATION_PRIVACY_AGGREGATE_VERSION_V1,
            aggregate_id: aggregate_id.to_string(),
            window_start_unix: 1_800_000_000,
            window_end_unix: 1_800_604_800,
            generated_at_unix: 1_800_604_801,
            population_label: format!("{aggregate_id}-population"),
            population_digest: [seed; 32],
            privacy: ModerationPrivacyParametersV1 {
                version: MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
                mode: ModerationPrivacyModeV1::DifferentialPrivacyWithSuppression,
                epsilon_micros: Some(800_000),
                delta_ppb: Some(10),
                noise_scale_micros: Some(1_000_000),
                suppression_threshold: Some(25),
                suppressed_count: 2,
            },
            source_event_count: 128,
            source_payload_digest: [seed.wrapping_add(1); 32],
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
            policy_digest: Some([seed.wrapping_add(2); 32]),
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
            population_digest: Some([seed; 32]),
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
            policy_digest: Some([0xC0; 32]),
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

    fn privacy_aggregate_cycle_config(noise_seed: Option<[u8; 32]>) -> PrivacyAggregateCycleConfig {
        use iroha_data_model::sorafs::transparency::{
            MODERATION_PRIVACY_PARAMETERS_VERSION_V1, ModerationLedgerMetadataV1,
            ModerationPrivacyModeV1, ModerationPrivacyParametersV1,
        };

        PrivacyAggregateCycleConfig {
            aggregate_id_prefix: "sfm4c-weekly".to_string(),
            privacy: ModerationPrivacyParametersV1 {
                version: MODERATION_PRIVACY_PARAMETERS_VERSION_V1,
                mode: ModerationPrivacyModeV1::DifferentialPrivacyWithSuppression,
                epsilon_micros: Some(800_000),
                delta_ppb: Some(10),
                noise_scale_micros: Some(1_000_000),
                suppression_threshold: Some(2),
                suppressed_count: 0,
            },
            noise_seed,
            policy_digest: Some([0xC0; 32]),
            metadata: vec![ModerationLedgerMetadataV1 {
                key: "publisher".to_string(),
                value: "sfm4c-worker".to_string(),
            }],
        }
    }

    fn privacy_aggregate_schedule_config() -> PrivacyAggregateScheduleConfig {
        PrivacyAggregateScheduleConfig {
            cycle_seconds: 100,
            publish_delay_seconds: 10,
        }
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
            amount_xor: "420".to_string(),
            tx_hash_hex: "22".repeat(32),
            reconciliation_digest_hex: "33".repeat(32),
            reconciliation_status: "pending_client_submission".to_string(),
            observed_lifecycle_status: "locked".to_string(),
            observed_remaining_xor: "420".to_string(),
            deposit_xor: "420".to_string(),
            refund_xor: "0".to_string(),
            treasury_xor: "210".to_string(),
            held_xor: "210".to_string(),
            panel_size: 7,
            configured_signer_count: 1,
        }
    }

    #[test]
    fn moderation_model_registry_admits_repro_manifest_and_rejects_conflict() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let manifest = moderation_repro_manifest_fixture(0x10, 0x20, 0x30);

        let record = handle
            .admit_moderation_repro_manifest(manifest.clone())
            .expect("admit repro manifest");
        assert_eq!(record.manifest_id, [0x10; 16]);
        assert_eq!(record.manifest_digest, [0x20; 32]);
        assert_eq!(record.runner_hash, [0x30; 32]);
        assert_eq!(record.model_count, 1);
        assert_eq!(record.signer_count, 1);

        let repeated = handle
            .admit_moderation_repro_manifest(manifest)
            .expect("re-admit matching repro manifest");
        assert_eq!(repeated, record);

        let err = handle
            .admit_moderation_repro_manifest(moderation_repro_manifest_fixture(0x10, 0x21, 0x31))
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
            .admit_moderation_repro_manifest(moderation_repro_manifest_fixture(0x12, 0x22, 0x32))
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
        let source = NodeHandle::new(cfg.clone());
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
                notes: Some(" sealed locally ".to_string()),
            })
            .expect("store quarantine object");
        assert_eq!(record.payload_digest, *blake3::hash(&payload).as_bytes());
        assert_eq!(record.payload_len, payload.len() as u64);
        assert_eq!(record.notes.as_deref(), Some("sealed locally"));

        #[cfg(unix)]
        {
            let key_path = moderation_quarantine_object_key_path(cfg.data_dir());
            let mode = fs::metadata(&key_path)
                .expect("read local seal key metadata")
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, 0o600);
        }

        let envelope_path =
            moderation_quarantine_object_store_root(cfg.data_dir()).join(&record.envelope_path);
        let envelope_bytes = fs::read(&envelope_path).expect("read encrypted envelope");
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

        let index_path = moderation_quarantine_object_index_path(cfg.data_dir());
        let index_bytes = fs::read(&index_path).expect("read object index");
        let index: ModerationQuarantineObjectSnapshot =
            norito::decode_from_bytes(&index_bytes).expect("decode object index");
        assert_eq!(index.objects, vec![record.clone()]);

        drop(source);
        let restored = NodeHandle::new(cfg);
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
    fn moderation_quarantine_object_store_rejects_digest_mismatch() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let expected_payload = b"expected quarantined bytes".to_vec();
        let mut screening = moderation_screening_input_fixture(
            "cid:bafy-object-digest",
            ModerationScreeningVerdict::Quarantine,
        );
        screening.subject_digest = *blake3::hash(&expected_payload).as_bytes();
        let handle = NodeHandle::new(cfg);
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
        let handle = NodeHandle::new(cfg.clone());
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
        envelope.ciphertext[0] ^= 0x01;
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
        let handle = NodeHandle::new(cfg.clone());
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
        envelope.ciphertext[0] ^= 0x80;
        fs::write(
            &envelope_path,
            norito::to_bytes(&envelope).expect("encode canonical tampered envelope"),
        )
        .expect("write tampered envelope");

        assert!(matches!(
            NodeHandle::try_new(cfg),
            Err(NodeInitError::Checkpoint {
                component: "moderation quarantine object envelope",
                ..
            })
        ));
    }

    #[test]
    fn moderation_quarantine_store_rejects_missing_key_and_orphan_files_on_restart() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let payload = b"indexed quarantine object requires its original sealing key".to_vec();
        let mut screening = moderation_screening_input_fixture(
            "cid:bafy-object-missing-key",
            ModerationScreeningVerdict::Quarantine,
        );
        screening.subject_digest = *blake3::hash(&payload).as_bytes();
        let handle = NodeHandle::new(cfg.clone());
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

        let key_path = moderation_quarantine_object_key_path(cfg.data_dir());
        let key_bytes = fs::read(&key_path).expect("read sealing key");
        fs::remove_file(&key_path).expect("remove sealing key");
        assert!(matches!(
            NodeHandle::try_new(cfg.clone()),
            Err(NodeInitError::Checkpoint {
                component: "moderation quarantine object key",
                ..
            })
        ));

        write_local_private_checkpoint_atomic(&key_path, &key_bytes).expect("restore sealing key");
        let orphan = moderation_quarantine_object_store_root(cfg.data_dir()).join("orphan.to");
        fs::write(&orphan, b"not indexed").expect("write orphan object");
        assert!(matches!(
            NodeHandle::try_new(cfg),
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
        let handle = NodeHandle::new(cfg);
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
        let source = NodeHandle::new(cfg.clone());
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
                notes: Some("viewer object".to_string()),
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
        let restored = NodeHandle::new(cfg);
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
        let handle = NodeHandle::new(cfg);
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
        let handle = NodeHandle::new(cfg);
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
        let handle = NodeHandle::new(cfg);
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
        assert_eq!(handle.transparency_ledger_source_entry_count(), 1);
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
        assert_eq!(handle.transparency_ledger_source_entry_count(), 1);
        assert_eq!(publisher.take().len(), 0);
    }

    #[test]
    fn moderation_evidence_viewer_audit_report_publish_due_configured_uses_storage_config() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonical temp dir");
        let schedule = PrivacyAggregateScheduleConfig {
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
            cycle_seconds: 86_401,
            publish_delay_seconds: 1,
        };
        let due_at_unix = oversized
            .event_window(1_800_000_010)
            .expect("oversized event window")
            .expect("non-zero oversized window")
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
        let manifest = ManifestBuilder::new()
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

    fn orderbook_signature() -> OrderbookSignatureV1 {
        OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: Vec::new(),
            signature: Vec::new(),
        }
    }

    fn orderbook_keypair(seed: u8) -> iroha_crypto::KeyPair {
        iroha_crypto::KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("orderbook fixture seed must derive keypair")
    }

    fn orderbook_signature_for_digest(
        keypair: &iroha_crypto::KeyPair,
        digest: &[u8; 32],
    ) -> OrderbookSignatureV1 {
        let (algorithm, public_key) = keypair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must expose bytes");
        assert_eq!(algorithm, Algorithm::Ed25519);
        let signature = IrohaSignature::try_new(keypair.private_key(), digest)
            .expect("fixture signature must be produced");
        OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: public_key.to_vec(),
            signature: signature.payload().to_vec(),
        }
    }

    fn sign_orderbook_order(mut order: OrderRequestV1, seed: u8) -> OrderRequestV1 {
        let keypair = orderbook_keypair(seed);
        let (algorithm, public_key) = keypair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must expose bytes");
        assert_eq!(algorithm, Algorithm::Ed25519);
        order.signature = OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: public_key.to_vec(),
            signature: Vec::new(),
        };
        let digest = order_request_signature_digest_v1(&order).expect("order digest");
        order.signature = orderbook_signature_for_digest(&keypair, &digest);
        order
    }

    fn sign_orderbook_cancel(mut cancel: OrderCancelV1, seed: u8) -> OrderCancelV1 {
        let keypair = orderbook_keypair(seed);
        let (algorithm, public_key) = keypair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must expose bytes");
        assert_eq!(algorithm, Algorithm::Ed25519);
        cancel.signature = OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: public_key.to_vec(),
            signature: Vec::new(),
        };
        let digest = order_cancel_signature_digest_v1(&cancel).expect("cancel digest");
        cancel.signature = orderbook_signature_for_digest(&keypair, &digest);
        cancel
    }

    fn sign_orderbook_receipt(mut receipt: SettlementReceiptV1, seed: u8) -> SettlementReceiptV1 {
        let keypair = orderbook_keypair(seed);
        let (algorithm, public_key) = keypair
            .public_key()
            .try_to_bytes()
            .expect("fixture public key must expose bytes");
        assert_eq!(algorithm, Algorithm::Ed25519);
        receipt.settlement_signature = OrderbookSignatureV1 {
            algorithm: SignatureAlgorithm::Ed25519,
            public_key: public_key.to_vec(),
            signature: Vec::new(),
        };
        let digest =
            settlement_receipt_signature_digest_v1(&receipt).expect("settlement receipt digest");
        receipt.settlement_signature = orderbook_signature_for_digest(&keypair, &digest);
        receipt
    }

    fn orderbook_order(
        id: u8,
        side: OrderSideV1,
        price_micro: u128,
        owner: &[u8],
    ) -> OrderRequestV1 {
        let owner_account = owner.to_vec();
        let nonce = u64::from(id);
        sign_orderbook_order(
            OrderRequestV1 {
                version: ORDERBOOK_ORDER_VERSION_V1,
                order_id: derive_orderbook_order_id_v1(&owner_account, nonce),
                side,
                tier: OrderTierV1::Hot,
                price_per_gib: XorQuantity::try_from_micro(price_micro)
                    .expect("legacy micro-XOR value is representable"),
                quantity_gib: 4,
                remaining_gib: 4,
                owner_account,
                expiry_unix: 1_800_000_100,
                nonce,
                maker_fee_bps: 10,
                taker_fee_bps: 20,
                signature: orderbook_signature(),
            },
            id.saturating_add(0x20),
        )
    }

    fn orderbook_cancel(id: u8, order_owner: &[u8], cancel_owner: &[u8]) -> OrderCancelV1 {
        sign_orderbook_cancel(
            OrderCancelV1 {
                version: ORDERBOOK_CANCEL_VERSION_V1,
                order_id: derive_orderbook_order_id_v1(order_owner, u64::from(id)),
                owner_account: cancel_owner.to_vec(),
                reason: OrderCancelReasonV1::OwnerRequested,
                nonce: u64::from(id).saturating_add(100),
                signature: orderbook_signature(),
            },
            id.saturating_add(0x40),
        )
    }

    fn orderbook_receipt(
        id: u8,
        channel: &SettlementChannelV1,
        start: u64,
        end: u64,
        issued_at_unix: u64,
        debited_micro: u128,
    ) -> SettlementReceiptV1 {
        sign_orderbook_receipt(
            SettlementReceiptV1 {
                version: SETTLEMENT_RECEIPT_VERSION_V1,
                receipt_id: [id; 32],
                channel_id: channel.channel_id,
                trade_id: channel.trade_id,
                range: ByteRangeV1 { start, end },
                chunk_hash: [id.saturating_add(70); 32],
                bytes_delivered: end - start,
                xor_debited: XorQuantity::try_from_micro(debited_micro)
                    .expect("legacy micro-XOR value is representable"),
                provider_credit: XorQuantity::try_from_micro(
                    debited_micro
                        .checked_sub(10)
                        .expect("fixture debit covers its fee"),
                )
                    .expect("legacy micro-XOR value is representable"),
                fee_amount: XorQuantity::try_from_micro(10)
                    .expect("legacy micro-XOR value is representable"),
                issued_at_unix,
                settlement_signature: orderbook_signature(),
            },
            id.saturating_add(0x60),
        )
    }

    fn moderation_jurors() -> Vec<String> {
        ["juror-a", "juror-b", "juror-c"]
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    fn moderation_context(
        case_id: &str,
        juror_ids: &[String],
        quorum: u16,
    ) -> SoraFsModerationBallotContextV1 {
        SoraFsModerationBallotContextV1 {
            version: SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
            case_id: case_id.to_owned(),
            evidence_bundle_digest: [0xAB; 32],
            appeal_finance_config_version: "appeal-finance-v1".to_owned(),
            panel_roster_hash: local_moderation_panel_roster_hash(juror_ids, quorum),
            policy_reference: "policy://sorafs/moderation/v1".to_owned(),
            evidence_uri: Some("dag://evidence/case".to_owned()),
        }
    }

    fn moderation_announcement(
        case_id: &str,
        juror_ids: Vec<String>,
        quorum: u16,
    ) -> ModerationBallotAnnouncement {
        ModerationBallotAnnouncement {
            context: moderation_context(case_id, &juror_ids, quorum),
            appeal_deposit_escrow_id_hex: None,
            appeal_deposit: None,
            round_id: "round-1".to_owned(),
            juror_ids,
            quorum,
            announced_at_unix_ms: 1_800_000_000_000,
            commit_deadline_unix_ms: 1_800_000_010_000,
            challenge_deadline_unix_ms: 1_800_000_020_000,
            reveal_deadline_unix_ms: 1_800_000_030_000,
        }
    }

    fn moderation_appeal_deposit() -> ModerationAppealDeposit {
        ModerationAppealDeposit {
            escrow_id_hex: "42".repeat(32),
            payer_account: "appeal-payer".to_owned(),
            destination_account: "appeal-treasury".to_owned(),
            release_authority_account: Some("appeal-authority".to_owned()),
            asset_definition_id: "xor#wonderland".to_owned(),
            custody_account: "asset-lock-custody".to_owned(),
            deposit_xor: "420".to_owned(),
            expires_at_ms: Some(1_800_100_000_000),
            idempotency_key: "case-appeal-round-1".to_owned(),
            evidence_hashes_hex: vec![blake3::hash(b"case-appeal-round-1").to_string()],
        }
    }

    fn moderation_reveal(
        context: &SoraFsModerationBallotContextV1,
        juror_id: &str,
        choice: SoraFsModerationVoteChoice,
        nonce_seed: u8,
        revealed_at_unix_ms: u64,
    ) -> SoraFsModerationBallotRevealV1 {
        SoraFsModerationBallotRevealV1 {
            version: SORAFS_MODERATION_BALLOT_REVEAL_VERSION_V1,
            context: context.clone(),
            round_id: "round-1".to_owned(),
            juror_id: juror_id.to_owned(),
            choice,
            nonce: vec![nonce_seed; 16],
            revealed_at_unix_ms,
        }
    }

    fn moderation_commit_from_reveal(
        reveal: &SoraFsModerationBallotRevealV1,
        committed_at_unix_ms: u64,
    ) -> SoraFsModerationBallotCommitV1 {
        SoraFsModerationBallotCommitV1 {
            version: SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1,
            context: reveal.context.clone(),
            round_id: reveal.round_id.clone(),
            juror_id: reveal.juror_id.clone(),
            commitment_blake2b_256: reveal.compute_commitment(),
            committed_at_unix_ms,
        }
    }

    fn moderation_challenge_input(
        case_id: &str,
        challenge_id: &str,
        kind: ModerationBallotChallengeKind,
    ) -> ModerationBallotChallengeInput {
        ModerationBallotChallengeInput {
            challenge_id: challenge_id.to_owned(),
            case_id: case_id.to_owned(),
            round_id: "round-1".to_owned(),
            challenger_id: "moderation-provider".to_owned(),
            kind,
            target_juror_id: matches!(
                kind,
                ModerationBallotChallengeKind::DuplicateCommit
                    | ModerationBallotChallengeKind::PayloadMismatch
                    | ModerationBallotChallengeKind::JurorEligibility
            )
            .then(|| "juror-a".to_owned()),
            evidence_digest: [0xA7; 32],
            reason: "signed-evidence-digest".to_owned(),
        }
    }

    #[test]
    fn moderation_state_limit_allows_boundary_replays_and_existing_updates() {
        let cfg = StorageConfig::builder()
            .enabled(false)
            .runtime_retention(RuntimeRetentionPolicy::new(1, 1, 1024 * 1024))
            .build();
        let handle = NodeHandle::new(cfg);

        let repro = moderation_repro_manifest_fixture(0x11, 0x21, 0x31);
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
                .admit_moderation_repro_manifest(moderation_repro_manifest_fixture(
                    0x12, 0x22, 0x32,
                ))
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

        let jurors = vec!["juror-a".to_owned()];
        let announcement = moderation_announcement("limit-case", jurors.clone(), 1);
        let context = announcement.context.clone();
        let mut events = handle.subscribe_moderation_ballot_events();
        handle
            .announce_moderation_ballot(announcement.clone())
            .expect("announce ballot at boundary");
        events.try_recv().expect("committed announcement event");
        assert!(matches!(
            handle
                .announce_moderation_ballot(announcement)
                .expect_err("duplicate ballot remains duplicate at capacity"),
            ModerationBallotRuntimeError::DuplicateBallot { .. }
        ));
        assert!(matches!(
            handle
                .announce_moderation_ballot(moderation_announcement("second-case", jurors, 1))
                .expect_err("new ballot above capacity must fail"),
            ModerationBallotRuntimeError::ResourceExhausted {
                resource: "ballots",
                limit: 1
            }
        ));
        assert!(matches!(
            events.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
        assert_eq!(handle.latest_moderation_ballot_event_sequence(), Some(1));

        let reveal = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Uphold,
            0xA1,
            1_800_000_020_001,
        );
        let commit = moderation_commit_from_reveal(&reveal, 1_800_000_005_000);
        handle
            .submit_moderation_ballot_commit(commit.clone(), 1_800_000_005_000)
            .expect("commit existing ballot at capacity");
        assert!(matches!(
            handle
                .submit_moderation_ballot_commit(commit, 1_800_000_005_000)
                .expect_err("duplicate commit remains duplicate at capacity"),
            ModerationBallotRuntimeError::DuplicateCommit { .. }
        ));
        handle
            .submit_moderation_ballot_challenge(
                moderation_challenge_input(
                    "limit-case",
                    "challenge-1",
                    ModerationBallotChallengeKind::Other,
                ),
                1_800_000_011_000,
            )
            .expect("challenge at boundary");
        assert!(matches!(
            handle
                .submit_moderation_ballot_challenge(
                    moderation_challenge_input(
                        "limit-case",
                        "challenge-2",
                        ModerationBallotChallengeKind::Other,
                    ),
                    1_800_000_011_001,
                )
                .expect_err("new challenge above capacity must fail"),
            ModerationBallotRuntimeError::ResourceExhausted {
                resource: "ballot_challenges",
                limit: 1
            }
        ));
        handle
            .resolve_moderation_ballot_challenge(
                ModerationBallotChallengeResolution {
                    case_id: "limit-case".to_owned(),
                    round_id: "round-1".to_owned(),
                    challenge_id: "challenge-1".to_owned(),
                    resolved_by: "moderation-operator".to_owned(),
                    decision: ModerationBallotChallengeDecision::Rejected,
                    note: Some("valid challenge resolution".to_owned()),
                },
                1_800_000_012_000,
            )
            .expect("resolve existing challenge at capacity");
        handle
            .submit_moderation_ballot_reveal(reveal.clone(), 1_800_000_020_001)
            .expect("reveal at boundary");
        assert!(matches!(
            handle
                .submit_moderation_ballot_reveal(reveal, 1_800_000_020_001)
                .expect_err("duplicate reveal remains duplicate at capacity"),
            ModerationBallotRuntimeError::DuplicateReveal { .. }
        ));
        handle
            .tally_moderation_ballot("limit-case", "round-1", 1_800_000_020_002)
            .expect("tally existing ballot at capacity");
        assert_eq!(handle.latest_moderation_ballot_event_sequence(), Some(6));
    }

    #[test]
    fn concurrent_moderation_ballot_admission_stops_exactly_at_state_limit() {
        let cfg = StorageConfig::builder()
            .enabled(false)
            .runtime_retention(RuntimeRetentionPolicy::new(4, 4, 1024 * 1024))
            .build();
        let handle = NodeHandle::new(cfg);
        let barrier = Arc::new(Barrier::new(12));
        let workers = (0_u8..12)
            .map(|index| {
                let handle = handle.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    let case_id = format!("concurrent-limit-{index}");
                    barrier.wait();
                    handle.announce_moderation_ballot(moderation_announcement(
                        &case_id,
                        vec![format!("juror-{index}")],
                        1,
                    ))
                })
            })
            .collect::<Vec<_>>();
        let outcomes = workers
            .into_iter()
            .map(|worker| worker.join().expect("moderation admission worker joins"))
            .collect::<Vec<_>>();
        assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 4);
        assert_eq!(
            outcomes
                .iter()
                .filter(|outcome| {
                    matches!(
                        outcome,
                        Err(ModerationBallotRuntimeError::ResourceExhausted {
                            resource: "ballots",
                            limit: 4
                        })
                    )
                })
                .count(),
            8
        );
        let snapshot = handle.moderation_ballot_snapshot();
        assert_eq!(snapshot.ballots.len(), 4);
        assert_eq!(snapshot.events.len(), 4);
        assert_eq!(handle.latest_moderation_ballot_event_sequence(), Some(4));
    }

    #[test]
    fn moderation_object_viewer_limits_and_checkpoints_survive_restart() {
        let (base, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base.data_dir().clone())
            .runtime_retention(RuntimeRetentionPolicy::new(2, 2, 2 * 1024 * 1024))
            .build();
        let handle = NodeHandle::new(cfg.clone());
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

        let restored = NodeHandle::new(cfg);
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
    fn node_handle_orderbook_matches_crossing_orders_and_records_snapshot() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let now = 1_800_000_000_u64;

        let before = global_or_default()
            .torii_sorafs_orderbook_orders_total
            .with_label_values(&["local", "hot", "bid", "accepted"])
            .get();
        let ask = orderbook_order(1, OrderSideV1::Ask, 1_500_000, b"provider");
        let bid = orderbook_order(2, OrderSideV1::Bid, 1_600_000, b"buyer");

        let first = handle.submit_orderbook_order(ask, now).expect("accept ask");
        assert!(first.fills.is_empty());
        assert_eq!(first.open_order_count, 1);

        let second = handle
            .submit_orderbook_order(bid, now)
            .expect("accept bid and match");
        assert_eq!(second.fills.len(), 1);
        assert_eq!(second.settlement_channels_opened.len(), 1);
        assert_eq!(second.open_order_count, 0);

        let snapshot = handle.orderbook_snapshot(now);
        assert!(snapshot.open_orders.is_empty());
        assert_eq!(snapshot.trades.len(), 1);
        assert_eq!(snapshot.settlement_channels.len(), 1);
        assert_eq!(
            snapshot.settlement_channels[0].buyer_account,
            b"buyer".to_vec()
        );
        assert!(
            global_or_default()
                .torii_sorafs_orderbook_orders_total
                .with_label_values(&["local", "hot", "bid", "accepted"])
                .get()
                >= before.saturating_add(1)
        );
        assert_eq!(
            global_or_default()
                .torii_sorafs_orderbook_settlement_backlog
                .with_label_values(&["local"])
                .get(),
            1
        );
    }

    #[test]
    fn node_handle_orderbook_rejects_orders_below_configured_minimum() {
        let cfg = StorageConfig::builder()
            .enabled(false)
            .orderbook_admission_policy(OrderbookAdmissionPolicy::new(8, 1_000))
            .build();
        let handle = NodeHandle::new(cfg);
        let now = 1_800_000_000;

        let err = handle
            .submit_orderbook_order(
                orderbook_order(61, OrderSideV1::Bid, 1_600_000, b"buyer"),
                now,
            )
            .expect_err("below-minimum order should be rejected");

        assert_eq!(
            err,
            OrderbookRuntimeError::OrderBelowMinimum {
                quantity_gib: 4,
                min_order_gib: 8,
            }
        );
        assert!(handle.orderbook_snapshot(now).open_orders.is_empty());
    }

    #[test]
    fn node_handle_orderbook_rejects_prices_outside_configured_tick() {
        let cfg = StorageConfig::builder()
            .enabled(false)
            .orderbook_admission_policy(OrderbookAdmissionPolicy::new(1, 10_000))
            .build();
        let handle = NodeHandle::new(cfg);
        let now = 1_800_000_000;

        let err = handle
            .submit_orderbook_order(
                orderbook_order(62, OrderSideV1::Ask, 1_605_000, b"provider"),
                now,
            )
            .expect_err("off-tick order should be rejected");

        assert_eq!(
            err,
            OrderbookRuntimeError::OrderPriceTickMismatch {
                price_micro_xor: 1_605_000,
                tick_micro_xor: 10_000,
            }
        );
        assert!(handle.orderbook_snapshot(now).open_orders.is_empty());
    }

    #[test]
    fn node_handle_orderbook_rejects_ask_when_reserve_lifecycle_disables_adverts() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let now = 1_800_000_000_u64;
        let provider_account = b"provider".to_vec();
        let provider_id = local_orderbook_provider_id_for_owner_account(&provider_account);
        handle
            .record_reserve_lifecycle_update(reserve_lifecycle_update_for_provider(
                provider_id,
                provider_account.clone(),
                31,
                XorQuantity::zero(),
                now.saturating_sub(1),
            ))
            .expect("record defaulted reserve lifecycle");

        let err = handle
            .submit_orderbook_order(
                orderbook_order(63, OrderSideV1::Ask, 1_500_000, &provider_account),
                now,
            )
            .expect_err("defaulted provider ask should be rejected");

        assert_eq!(
            err,
            OrderbookRuntimeError::ReserveLifecycleAdvertDisabled {
                provider_id_hex: hex::encode(provider_id),
                stage: "default".to_owned(),
            }
        );
        assert!(handle.orderbook_snapshot(now).open_orders.is_empty());
    }

    #[test]
    fn node_handle_orderbook_accepts_ask_after_reserve_appeal_reenables_adverts() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let now = 1_800_000_000_u64;
        let provider_account = b"provider".to_vec();
        let provider_id = local_orderbook_provider_id_for_owner_account(&provider_account);
        handle
            .record_reserve_lifecycle_update(reserve_lifecycle_update_for_provider(
                provider_id,
                provider_account.clone(),
                31,
                XorQuantity::zero(),
                now.saturating_sub(1),
            ))
            .expect("record defaulted reserve lifecycle");
        handle
            .record_reserve_appeal(reserve_appeal_request_for_provider(
                0x49,
                provider_id,
                provider_account.clone(),
            ))
            .expect("record reserve appeal");
        handle
            .record_reserve_appeal_decision(reserve_appeal_decision(
                0x49,
                ReserveAppealStatus::Accepted,
            ))
            .expect("accept reserve appeal");

        let outcome = handle
            .submit_orderbook_order(
                orderbook_order(64, OrderSideV1::Ask, 1_500_000, &provider_account),
                now,
            )
            .expect("appeal override should restore ask admission");

        assert_eq!(outcome.open_order_count, 1);
        let summary = handle
            .reserve_provider_lifecycle_summary(provider_id)
            .expect("reserve summary");
        assert_eq!(summary.lifecycle.stage, ReserveLifecycleStage::Grace);
        assert!(!summary.lifecycle.disable_adverts);
    }

    #[test]
    fn node_handle_orderbook_cancels_owner_order_only() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let now = 1_800_000_000;
        let ask = orderbook_order(3, OrderSideV1::Ask, 1_500_000, b"provider");
        handle.submit_orderbook_order(ask, now).expect("accept ask");

        assert!(matches!(
            handle.cancel_orderbook_order(orderbook_cancel(3, b"provider", b"other"), now),
            Err(OrderbookRuntimeError::CancelOwnerMismatch)
        ));

        let outcome = handle
            .cancel_orderbook_order(orderbook_cancel(3, b"provider", b"provider"), now)
            .expect("cancel provider order");
        assert_eq!(outcome.open_order_count, 0);
        assert_eq!(handle.orderbook_snapshot(now).open_orders.len(), 0);
    }

    #[test]
    fn node_handle_orderbook_receipts_update_channel_and_metrics() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let now = 1_800_000_000;
        let ask = orderbook_order(4, OrderSideV1::Ask, 1_500_000, b"provider");
        let bid = orderbook_order(5, OrderSideV1::Bid, 1_600_000, b"buyer");
        handle.submit_orderbook_order(ask, now).expect("accept ask");
        handle.submit_orderbook_order(bid, now).expect("match bid");
        let channel = handle.orderbook_snapshot(now).settlement_channels[0].clone();
        let receipt = orderbook_receipt(
            9,
            &channel,
            0,
            channel.total_bytes,
            now.saturating_add(10),
            channel
                .xor_locked
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
        );

        let outcome = handle
            .submit_orderbook_receipt(receipt, now.saturating_add(10))
            .expect("apply receipt");

        assert_eq!(outcome.updated_channel.remaining_bytes, 0);
        assert_eq!(outcome.open_settlement_channel_count, 0);
        let snapshot = handle.orderbook_snapshot(now.saturating_add(10));
        assert_eq!(snapshot.settlement_receipts.len(), 1);
        assert_eq!(
            snapshot.settlement_ledger.total_buyer_debited_micro_xor,
            channel
                .xor_locked
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation")
        );
        assert_eq!(snapshot.settlement_ledger.total_fee_retained_micro_xor, 10);
        assert_eq!(
            snapshot.settlement_ledger.total_remaining_locked_micro_xor,
            0
        );
        assert_eq!(snapshot.settlement_ledger.buyers.len(), 1);
        assert_eq!(
            snapshot.settlement_ledger.buyers[0].buyer_account,
            b"buyer".to_vec()
        );
        assert_eq!(snapshot.settlement_ledger.providers.len(), 1);
        assert_eq!(
            snapshot.settlement_ledger.providers[0].provider_id,
            channel.provider_id
        );
        assert_eq!(
            global_or_default()
                .torii_sorafs_orderbook_settlement_backlog
                .with_label_values(&["local"])
                .get(),
            0
        );
        assert_eq!(handle.latest_orderbook_event_sequence(), Some(3));
        let events = handle.orderbook_events_since(Some(2), 10);
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].kind,
            OrderbookEventKind::SettlementReceiptAccepted
        );
        assert_eq!(
            events[0].receipt_id,
            handle
                .orderbook_snapshot(now.saturating_add(10))
                .settlement_receipts
                .first()
                .map(|receipt| receipt.receipt_id)
        );
    }

    #[test]
    fn node_handle_orderbook_receipts_publish_governance_receipt() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        let now = 1_800_000_000;
        let ask = orderbook_order(16, OrderSideV1::Ask, 1_500_000, b"provider");
        let bid = orderbook_order(17, OrderSideV1::Bid, 1_600_000, b"buyer");
        handle.submit_orderbook_order(ask, now).expect("accept ask");
        handle.submit_orderbook_order(bid, now).expect("match bid");
        let channel = handle.orderbook_snapshot(now).settlement_channels[0].clone();
        let receipt = orderbook_receipt(
            18,
            &channel,
            0,
            channel.total_bytes,
            now.saturating_add(10),
            channel
                .xor_locked
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
        );

        handle
            .submit_orderbook_receipt(receipt.clone(), now.saturating_add(10))
            .expect("apply receipt");

        let payloads = publisher.take();
        assert_eq!(payloads.len(), 1);
        let decoded =
            norito::decode_from_bytes::<SettlementReceiptV1>(&payloads[0]).expect("decode receipt");
        assert_eq!(decoded.receipt_id, receipt.receipt_id);
        assert_eq!(decoded.channel_id, channel.channel_id);
        assert_eq!(decoded.trade_id, channel.trade_id);
        assert_eq!(decoded.bytes_delivered, channel.total_bytes);
    }

    #[test]
    fn node_handle_orderbook_runtime_snapshot_round_trips_local_state() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let source = NodeHandle::new(cfg.clone());
        let now = 1_800_000_000;
        source
            .submit_orderbook_order(
                orderbook_order(19, OrderSideV1::Ask, 1_500_000, b"provider"),
                now,
            )
            .expect("accept ask");
        source
            .submit_orderbook_order(
                orderbook_order(20, OrderSideV1::Bid, 1_600_000, b"buyer"),
                now,
            )
            .expect("match bid");
        let channel = source.orderbook_snapshot(now).settlement_channels[0].clone();
        source
            .submit_orderbook_receipt(
                orderbook_receipt(
                    21,
                    &channel,
                    0,
                    channel.total_bytes,
                    now.saturating_add(10),
                    channel
                        .xor_locked
                        .try_to_micro()
                        .expect("XOR quantity has exact legacy micro representation"),
                ),
                now.saturating_add(10),
            )
            .expect("apply receipt");
        source
            .submit_orderbook_order(
                orderbook_order(22, OrderSideV1::Ask, 1_700_000, b"provider-next"),
                now,
            )
            .expect("leave open ask");

        let exported = source
            .export_orderbook_runtime_snapshot(now.saturating_add(20))
            .expect("export orderbook snapshot");
        let encoded = norito::to_bytes(&exported).expect("encode orderbook snapshot");
        let decoded: OrderbookRuntimeSnapshotV1 =
            norito::decode_from_bytes(&encoded).expect("decode orderbook snapshot");
        let restored = NodeHandle::new(cfg);
        restored
            .restore_orderbook_runtime_snapshot(decoded)
            .expect("restore orderbook snapshot");

        let source_snapshot = source.orderbook_snapshot(now.saturating_add(20));
        let restored_snapshot = restored.orderbook_snapshot(now.saturating_add(20));
        assert_eq!(
            restored_snapshot.next_sequence,
            source_snapshot.next_sequence
        );
        assert_eq!(restored_snapshot.open_orders, source_snapshot.open_orders);
        assert_eq!(restored_snapshot.trades, source_snapshot.trades);
        assert_eq!(
            restored_snapshot.settlement_channels,
            source_snapshot.settlement_channels
        );
        assert_eq!(
            restored_snapshot.settlement_receipts,
            source_snapshot.settlement_receipts
        );
        assert_eq!(
            restored_snapshot.settlement_ledger,
            source_snapshot.settlement_ledger
        );
        assert_eq!(
            restored_snapshot.expired_order_ids,
            source_snapshot.expired_order_ids
        );
    }

    #[test]
    fn node_handle_orderbook_checkpoint_persists_and_reloads_local_state() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let source = NodeHandle::new(cfg.clone());
        let now = 1_800_000_000;
        source
            .submit_orderbook_order(
                orderbook_order(23, OrderSideV1::Ask, 1_500_000, b"provider"),
                now,
            )
            .expect("accept ask");
        source
            .submit_orderbook_order(
                orderbook_order(24, OrderSideV1::Bid, 1_600_000, b"buyer"),
                now,
            )
            .expect("match bid");
        let channel = source.orderbook_snapshot(now).settlement_channels[0].clone();
        source
            .submit_orderbook_receipt(
                orderbook_receipt(
                    25,
                    &channel,
                    0,
                    channel.total_bytes,
                    now.saturating_add(10),
                    channel
                        .xor_locked
                        .try_to_micro()
                        .expect("XOR quantity has exact legacy micro representation"),
                ),
                now.saturating_add(10),
            )
            .expect("apply receipt");
        source
            .submit_orderbook_order(
                orderbook_order(26, OrderSideV1::Ask, 1_700_000, b"provider-next"),
                now.saturating_add(20),
            )
            .expect("leave open ask");

        let checkpoint_path = orderbook_runtime_snapshot_path(cfg.data_dir());
        let checkpoint_bytes = fs::read(&checkpoint_path).expect("read orderbook checkpoint");
        let checkpoint: OrderbookRuntimeCheckpointV1 =
            norito::decode_from_bytes(&checkpoint_bytes).expect("decode orderbook checkpoint");
        assert_eq!(checkpoint.version, ORDERBOOK_RUNTIME_STATE_VERSION_V1);
        checkpoint.runtime.validate().expect("checkpoint validates");
        assert_eq!(checkpoint.runtime.generated_at_unix, now.saturating_add(20));
        assert_eq!(checkpoint.events.len(), 4);
        NodeHandle::validate_orderbook_event_checkpoint(&checkpoint.runtime, &checkpoint.events)
            .expect("event checkpoint validates");

        let source_snapshot = source.orderbook_snapshot(now.saturating_add(20));
        let source_events = source.orderbook_events_since(None, usize::MAX);
        drop(source);
        let restored = NodeHandle::new(cfg);
        let restored_snapshot = restored.orderbook_snapshot(now.saturating_add(20));
        assert_eq!(
            restored_snapshot.next_sequence,
            source_snapshot.next_sequence
        );
        assert_eq!(restored_snapshot.open_orders, source_snapshot.open_orders);
        assert_eq!(restored_snapshot.trades, source_snapshot.trades);
        assert_eq!(
            restored_snapshot.settlement_channels,
            source_snapshot.settlement_channels
        );
        assert_eq!(
            restored_snapshot.settlement_receipts,
            source_snapshot.settlement_receipts
        );
        assert_eq!(
            restored_snapshot.settlement_ledger,
            source_snapshot.settlement_ledger
        );
        assert_eq!(
            restored_snapshot.expired_order_ids,
            source_snapshot.expired_order_ids
        );
        assert_eq!(
            restored.orderbook_events_since(None, usize::MAX),
            source_events
        );
    }

    #[test]
    fn orderbook_checkpoint_failure_rolls_back_state_event_and_broadcast() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg.clone());
        let now = 1_800_000_000;
        handle
            .submit_orderbook_order(
                orderbook_order(0x71, OrderSideV1::Ask, 1_500_000, b"provider-a"),
                now,
            )
            .expect("commit first order");
        let checkpoint_path = orderbook_runtime_snapshot_path(cfg.data_dir());
        let committed = fs::read(&checkpoint_path).expect("read committed orderbook checkpoint");
        fs::remove_file(&checkpoint_path).expect("remove orderbook checkpoint");
        fs::create_dir(&checkpoint_path).expect("inject checkpoint directory");
        let mut receiver = handle.subscribe_orderbook_events();

        assert!(matches!(
            handle.submit_orderbook_order(
                orderbook_order(0x72, OrderSideV1::Ask, 1_500_000, b"provider-b"),
                now.saturating_add(1),
            ),
            Err(OrderbookRuntimeError::Checkpoint(_))
        ));
        let snapshot = handle.orderbook_snapshot(now.saturating_add(1));
        assert_eq!(snapshot.open_orders.len(), 1);
        assert_eq!(handle.latest_orderbook_event_sequence(), Some(1));
        assert!(matches!(
            receiver.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));

        fs::remove_dir(&checkpoint_path).expect("remove injected checkpoint directory");
        write_local_checkpoint_atomic(&checkpoint_path, &committed)
            .expect("restore committed checkpoint");
        drop(handle);
        let restored = NodeHandle::try_new(cfg).expect("restart from committed checkpoint");
        assert_eq!(restored.orderbook_snapshot(now).open_orders.len(), 1);
        assert_eq!(restored.latest_orderbook_event_sequence(), Some(1));
    }

    #[test]
    fn orderbook_startup_rejects_forged_event_suffix() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg.clone());
        let now = 1_800_000_000;
        handle
            .submit_orderbook_order(
                orderbook_order(0x73, OrderSideV1::Ask, 1_500_000, b"provider"),
                now,
            )
            .expect("commit order");
        let checkpoint_path = orderbook_runtime_snapshot_path(cfg.data_dir());
        let bytes = fs::read(&checkpoint_path).expect("read orderbook checkpoint");
        let mut checkpoint: OrderbookRuntimeCheckpointV1 =
            norito::decode_from_bytes(&bytes).expect("decode orderbook checkpoint");
        checkpoint
            .events
            .last_mut()
            .expect("retained event")
            .open_order_count = 0;
        write_local_checkpoint_atomic(
            &checkpoint_path,
            &norito::to_bytes(&checkpoint).expect("encode forged checkpoint"),
        )
        .expect("write forged checkpoint");
        drop(handle);

        assert!(matches!(
            NodeHandle::try_new(cfg),
            Err(NodeInitError::Checkpoint {
                component: "orderbook runtime",
                ..
            })
        ));
    }

    #[test]
    fn node_handle_orderbook_receipts_record_escrow_runway_metric() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let now = 1_800_000_000;
        let ask = orderbook_order(12, OrderSideV1::Ask, 1_500_000, b"runway-provider");
        let bid = orderbook_order(13, OrderSideV1::Bid, 1_600_000, b"runway-buyer");
        handle.submit_orderbook_order(ask, now).expect("accept ask");
        handle.submit_orderbook_order(bid, now).expect("match bid");

        let channel = handle.orderbook_snapshot(now).settlement_channels[0].clone();
        let first_end = channel.total_bytes / 2;
        let first_debit = channel
            .xor_locked
            .try_to_micro()
            .expect("XOR quantity has exact legacy micro representation")
            / 2;
        let first_outcome = handle
            .submit_orderbook_receipt(
                orderbook_receipt(
                    14,
                    &channel,
                    0,
                    first_end,
                    now.saturating_add(10),
                    first_debit,
                ),
                now.saturating_add(10),
            )
            .expect("apply partial receipt");
        let provider = hex::encode(channel.provider_id);
        let expected_runway = first_outcome
            .updated_channel
            .xor_locked
            .try_to_micro()
            .expect("XOR quantity has exact legacy micro representation")
            .saturating_mul(10)
            / first_debit;
        assert_eq!(
            global_or_default()
                .torii_sorafs_orderbook_escrow_runway_seconds
                .with_label_values(&[provider.as_str()])
                .get(),
            expected_runway.min(u128::from(u64::MAX)) as u64
        );

        let updated_channel = first_outcome.updated_channel;
        handle
            .submit_orderbook_receipt(
                orderbook_receipt(
                    15,
                    &updated_channel,
                    first_end,
                    channel.total_bytes,
                    now.saturating_add(20),
                    updated_channel
                        .xor_locked
                        .try_to_micro()
                        .expect("XOR quantity has exact legacy micro representation"),
                ),
                now.saturating_add(20),
            )
            .expect("apply final receipt");
        assert_eq!(
            global_or_default()
                .torii_sorafs_orderbook_escrow_runway_seconds
                .with_label_values(&[provider.as_str()])
                .get(),
            0
        );
    }

    #[test]
    fn node_handle_orderbook_rejects_overlapping_receipts() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let now = 1_800_000_000;
        handle
            .submit_orderbook_order(
                orderbook_order(6, OrderSideV1::Ask, 1_500_000, b"provider"),
                now,
            )
            .expect("accept ask");
        handle
            .submit_orderbook_order(
                orderbook_order(7, OrderSideV1::Bid, 1_600_000, b"buyer"),
                now,
            )
            .expect("match bid");
        let channel = handle.orderbook_snapshot(now).settlement_channels[0].clone();
        handle
            .submit_orderbook_receipt(
                orderbook_receipt(10, &channel, 0, ORDERBOOK_BYTES_PER_GIB, now + 10, 100),
                now + 10,
            )
            .expect("apply first receipt");

        assert!(matches!(
            handle.submit_orderbook_receipt(
                orderbook_receipt(
                    11,
                    &channel,
                    ORDERBOOK_BYTES_PER_GIB - 1,
                    ORDERBOOK_BYTES_PER_GIB + 1,
                    now + 11,
                    100,
                ),
                now + 11,
            ),
            Err(OrderbookRuntimeError::ReceiptRangeOverlap { .. })
        ));
    }

    #[test]
    fn node_handle_orderbook_events_replay_and_broadcast() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let mut receiver = handle.subscribe_orderbook_events();
        let now = 1_800_000_000;

        handle
            .submit_orderbook_order(
                orderbook_order(12, OrderSideV1::Ask, 1_500_000, b"provider"),
                now,
            )
            .expect("accept ask");
        let event = receiver.try_recv().expect("live orderbook event");
        assert_eq!(event.sequence, 1);
        assert_eq!(event.kind, OrderbookEventKind::OrderAccepted);
        assert_eq!(
            event.order_id,
            Some(derive_orderbook_order_id_v1(b"provider", 12))
        );
        assert_eq!(event.open_order_count, 1);

        handle
            .submit_orderbook_order(
                orderbook_order(13, OrderSideV1::Bid, 1_600_000, b"buyer"),
                now,
            )
            .expect("match bid");
        let event = receiver.try_recv().expect("live matching event");
        assert_eq!(event.sequence, 2);
        assert_eq!(event.kind, OrderbookEventKind::OrderAccepted);
        assert_eq!(event.trade_ids.len(), 1);
        assert_eq!(event.settlement_channel_ids.len(), 1);
        assert_eq!(event.open_order_count, 0);
        assert_eq!(event.open_settlement_channel_count, 1);

        let replay = handle.orderbook_events_since(Some(1), 10);
        assert_eq!(replay.len(), 1);
        assert_eq!(replay[0].sequence, 2);
        assert_eq!(handle.latest_orderbook_event_sequence(), Some(2));
    }

    #[test]
    fn node_handle_moderation_ballot_lifecycle_tallies_and_records_events() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let mut receiver = handle.subscribe_moderation_ballot_events();
        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-42", jurors.clone(), 2);
        let context = announcement.context.clone();

        let announced = handle
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");
        assert_eq!(announced.announcement.juror_ids, jurors);
        let event = receiver.try_recv().expect("announced event");
        assert_eq!(event.sequence, 1);
        assert_eq!(event.kind, ModerationBallotEventKind::BallotAnnounced);
        assert_eq!(event.committed_count, 0);

        let reveal_a = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Uphold,
            0xA1,
            1_800_000_021_000,
        );
        let reveal_b = moderation_reveal(
            &context,
            "juror-b",
            SoraFsModerationVoteChoice::Uphold,
            0xB2,
            1_800_000_021_500,
        );
        let commit_a = moderation_commit_from_reveal(&reveal_a, 1_800_000_005_000);
        let commit_b = moderation_commit_from_reveal(&reveal_b, 1_800_000_006_000);

        let commit_outcome = handle
            .submit_moderation_ballot_commit(commit_a, 1_800_000_005_000)
            .expect("accept first commit");
        assert_eq!(commit_outcome.committed_count, 1);
        let commit_outcome = handle
            .submit_moderation_ballot_commit(commit_b, 1_800_000_006_000)
            .expect("accept second commit");
        assert_eq!(commit_outcome.committed_count, 2);

        let reveal_outcome = handle
            .submit_moderation_ballot_reveal(reveal_a, 1_800_000_021_000)
            .expect("accept first reveal");
        assert_eq!(reveal_outcome.revealed_count, 1);
        let reveal_outcome = handle
            .submit_moderation_ballot_reveal(reveal_b, 1_800_000_021_500)
            .expect("accept second reveal");
        assert_eq!(reveal_outcome.revealed_count, 2);

        let tally = handle
            .tally_moderation_ballot("case-42", "round-1", 1_800_000_030_000)
            .expect("tally ballot");
        assert_eq!(tally.votes_total, 2);
        assert_eq!(tally.counts.uphold, 2);
        assert_eq!(
            tally.winning_choice,
            Some(SoraFsModerationVoteChoice::Uphold)
        );
        assert!(!tally.contested);

        let record = handle
            .moderation_ballot("case-42", "round-1")
            .expect("ballot record");
        assert_eq!(record.commits.len(), 2);
        assert_eq!(record.reveals.len(), 2);
        assert_eq!(record.tally, Some(tally.clone()));
        assert_eq!(handle.latest_moderation_ballot_event_sequence(), Some(6));

        let replay = handle.moderation_ballot_events_since(Some(4), 10);
        assert_eq!(replay.len(), 2);
        assert_eq!(replay[0].kind, ModerationBallotEventKind::RevealAccepted);
        assert_eq!(replay[1].kind, ModerationBallotEventKind::BallotTallied);
        assert_eq!(replay[1].tally, Some(tally));
    }

    #[test]
    fn node_handle_moderation_ballot_checkpoint_persists_and_reloads_snapshot() {
        let (base, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base.data_dir().clone())
            .runtime_retention(RuntimeRetentionPolicy::new(8, 8, 1024 * 1024))
            .build();
        let source = NodeHandle::new(cfg.clone());
        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-52", jurors, 2);
        let context = announcement.context.clone();
        source
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");

        let reveal_a = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Modify,
            0xF1,
            1_800_000_021_000,
        );
        let reveal_b = moderation_reveal(
            &context,
            "juror-b",
            SoraFsModerationVoteChoice::Modify,
            0xF2,
            1_800_000_021_500,
        );
        source
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal_a, 1_800_000_005_000),
                1_800_000_005_000,
            )
            .expect("accept first commit");
        source
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal_b, 1_800_000_006_000),
                1_800_000_006_000,
            )
            .expect("accept second commit");
        source
            .submit_moderation_ballot_reveal(reveal_a, 1_800_000_021_000)
            .expect("accept first reveal");
        source
            .submit_moderation_ballot_reveal(reveal_b, 1_800_000_021_500)
            .expect("accept second reveal");
        let tally = source
            .tally_moderation_ballot("case-52", "round-1", 1_800_000_030_000)
            .expect("tally ballot");

        let checkpoint_path = moderation_ballot_checkpoint_path(cfg.data_dir());
        let checkpoint_bytes = fs::read(&checkpoint_path).expect("read moderation checkpoint");
        let checkpoint: ModerationBallotSnapshot =
            norito::decode_from_bytes(&checkpoint_bytes).expect("decode moderation checkpoint");
        assert_eq!(checkpoint.ballots.len(), 1);
        assert_eq!(checkpoint.ballots[0].commits.len(), 2);
        assert_eq!(checkpoint.ballots[0].reveals.len(), 2);
        assert_eq!(checkpoint.ballots[0].tally, Some(tally.clone()));
        assert_eq!(checkpoint.events.len(), 6);
        assert_eq!(
            checkpoint.events[0].kind,
            ModerationBallotEventKind::BallotAnnounced
        );
        assert_eq!(
            checkpoint.events[5].kind,
            ModerationBallotEventKind::BallotTallied
        );
        assert_eq!(checkpoint.events[5].tally, Some(tally.clone()));
        let replay = source.moderation_ballot_events_replay(Some(0), 10);
        assert!(!replay.gap);
        assert_eq!(replay.oldest_available_sequence, Some(1));
        assert_eq!(replay.latest_sequence, Some(6));

        let source_snapshot = source
            .export_moderation_ballot_snapshot()
            .expect("export source moderation ballot snapshot");
        let source_events = source.moderation_ballot_events_since(Some(4), 10);
        drop(source);
        let restored = NodeHandle::new(cfg);
        assert_eq!(
            restored
                .export_moderation_ballot_snapshot()
                .expect("export restored moderation ballot snapshot"),
            source_snapshot
        );
        assert_eq!(restored.latest_moderation_ballot_event_sequence(), Some(6));
        assert_eq!(
            restored.moderation_ballot_events_since(Some(4), 10),
            source_events
        );
        assert!(matches!(
            restored.tally_moderation_ballot("case-52", "round-1", 1_800_000_031_000),
            Err(ModerationBallotRuntimeError::AlreadyTallied { .. })
        ));
    }

    #[test]
    fn moderation_ballot_checkpoint_failure_rolls_back_state_event_and_broadcast() {
        let (base, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base.data_dir().clone())
            .runtime_retention(RuntimeRetentionPolicy::new(8, 8, 1024 * 1024))
            .build();
        let handle = NodeHandle::new(cfg.clone());
        handle
            .announce_moderation_ballot(moderation_announcement(
                "checkpoint-first",
                vec!["juror-first".to_owned()],
                1,
            ))
            .expect("commit first ballot");
        let checkpoint_path = moderation_ballot_checkpoint_path(cfg.data_dir());
        let committed_bytes = fs::read(&checkpoint_path).expect("read committed checkpoint");
        fs::remove_file(&checkpoint_path).expect("remove checkpoint for failure injection");
        fs::create_dir(&checkpoint_path).expect("replace checkpoint with directory");
        let mut receiver = handle.subscribe_moderation_ballot_events();

        assert!(matches!(
            handle
                .announce_moderation_ballot(moderation_announcement(
                    "checkpoint-rejected",
                    vec!["juror-rejected".to_owned()],
                    1,
                ))
                .expect_err("checkpoint failure must reject mutation"),
            ModerationBallotRuntimeError::Checkpoint(_)
        ));
        assert!(
            handle
                .moderation_ballot("checkpoint-rejected", "round-1")
                .is_none()
        );
        assert_eq!(handle.moderation_ballots().len(), 1);
        assert_eq!(handle.latest_moderation_ballot_event_sequence(), Some(1));
        assert!(matches!(
            receiver.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));

        fs::remove_dir(&checkpoint_path).expect("remove injected checkpoint directory");
        write_local_checkpoint_atomic(&checkpoint_path, &committed_bytes)
            .expect("restore committed checkpoint bytes");
        drop(handle);
        let restored = NodeHandle::new(cfg);
        assert!(
            restored
                .moderation_ballot("checkpoint-first", "round-1")
                .is_some()
        );
        assert!(
            restored
                .moderation_ballot("checkpoint-rejected", "round-1")
                .is_none()
        );
        assert_eq!(restored.latest_moderation_ballot_event_sequence(), Some(1));
    }

    #[test]
    fn moderation_ballot_event_sequence_exhaustion_rolls_back_without_broadcast() {
        let handle = NodeHandle::new(StorageConfig::builder().enabled(false).build());
        handle
            .moderation_events
            .write()
            .expect("event history lock")
            .latest_sequence = u64::MAX;
        let mut receiver = handle.subscribe_moderation_ballot_events();

        assert_eq!(
            handle
                .announce_moderation_ballot(moderation_announcement(
                    "sequence-exhausted",
                    vec!["juror-a".to_owned()],
                    1,
                ))
                .expect_err("event sequence exhaustion must reject mutation"),
            ModerationBallotRuntimeError::EventSequenceOverflow
        );
        assert!(
            handle
                .moderation_ballot("sequence-exhausted", "round-1")
                .is_none()
        );
        assert!(
            handle
                .moderation_events
                .read()
                .expect("event history lock")
                .events
                .is_empty()
        );
        assert!(matches!(
            receiver.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn poisoned_moderation_event_lock_rolls_back_ballot_without_broadcast() {
        let handle = NodeHandle::new(StorageConfig::builder().enabled(false).build());
        let mut receiver = handle.subscribe_moderation_ballot_events();
        let events = Arc::clone(&handle.moderation_events);
        assert!(
            std::thread::spawn(move || {
                let _guard = events.write().expect("event history lock");
                panic!("poison event history for rollback test");
            })
            .join()
            .is_err()
        );

        assert_eq!(
            handle
                .announce_moderation_ballot(moderation_announcement(
                    "poisoned-events",
                    vec!["juror-a".to_owned()],
                    1,
                ))
                .expect_err("poisoned event lock must reject mutation"),
            ModerationBallotRuntimeError::StateLockPoisoned
        );
        assert!(
            handle
                .moderation_ballot("poisoned-events", "round-1")
                .is_none()
        );
        assert!(matches!(
            receiver.try_recv(),
            Err(tokio::sync::broadcast::error::TryRecvError::Empty)
        ));
    }

    #[test]
    fn node_handle_moderation_ballot_restore_rejects_corrupt_snapshot() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-53", jurors, 2);
        let context = announcement.context.clone();
        let reveal = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Uphold,
            0xF3,
            1_800_000_021_000,
        );
        let commit = moderation_commit_from_reveal(&reveal, 1_800_000_005_000);
        let err = handle
            .restore_moderation_ballot_snapshot(ModerationBallotSnapshot {
                ballots: vec![ModerationBallotRecord {
                    announcement,
                    commits: vec![commit.clone(), commit],
                    reveals: Vec::new(),
                    challenges: Vec::new(),
                    tally: None,
                }],
                events: Vec::new(),
            })
            .expect_err("duplicate commits rejected");
        assert!(matches!(
            err,
            ModerationBallotRuntimeError::InvalidSnapshot { .. }
        ));
        assert_eq!(
            handle.moderation_ballot_snapshot(),
            ModerationBallotSnapshot::default()
        );
    }

    #[test]
    fn node_handle_moderation_ballot_checkpoint_rejects_corrupt_startup_snapshot() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-54", jurors, 2);
        let context = announcement.context.clone();
        let reveal = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Uphold,
            0xF4,
            1_800_000_021_000,
        );
        let mut tally = ModerationBallotTally {
            case_id: "case-54".to_owned(),
            round_id: "round-1".to_owned(),
            counts: ModerationVoteCounts {
                uphold: 2,
                ..ModerationVoteCounts::default()
            },
            votes_total: 1,
            quorum: 2,
            winning_choice: Some(SoraFsModerationVoteChoice::Uphold),
            contested: false,
            tallied_at_unix_ms: 1_800_000_030_000,
        };
        tally.counts.modify = 1;
        let corrupt = ModerationBallotSnapshot {
            ballots: vec![ModerationBallotRecord {
                announcement,
                commits: vec![moderation_commit_from_reveal(&reveal, 1_800_000_005_000)],
                reveals: vec![reveal],
                challenges: Vec::new(),
                tally: Some(tally),
            }],
            events: Vec::new(),
        };
        let checkpoint_path = moderation_ballot_checkpoint_path(cfg.data_dir());
        fs::create_dir_all(checkpoint_path.parent().expect("checkpoint parent"))
            .expect("create checkpoint dir");
        fs::write(
            &checkpoint_path,
            norito::to_bytes(&corrupt).expect("encode corrupt checkpoint"),
        )
        .expect("write corrupt checkpoint");

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            NodeHandle::new(cfg.clone())
        }));
        assert!(result.is_err(), "corrupt durable state must fail startup");
        fs::remove_file(&checkpoint_path).expect("remove rejected corrupt checkpoint");
        let recovered = NodeHandle::new(cfg);
        assert_eq!(
            recovered
                .export_moderation_ballot_snapshot()
                .expect("export recovered moderation snapshot"),
            ModerationBallotSnapshot::default()
        );
    }

    #[test]
    fn node_handle_moderation_ballot_restore_rejects_corrupt_event_backlog() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let source = NodeHandle::new(cfg.clone());
        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-55", jurors, 2);
        let context = announcement.context.clone();
        source
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");
        let reveal_a = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Overturn,
            0xF5,
            1_800_000_021_000,
        );
        let reveal_b = moderation_reveal(
            &context,
            "juror-b",
            SoraFsModerationVoteChoice::Overturn,
            0xF6,
            1_800_000_021_500,
        );
        source
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal_a, 1_800_000_005_000),
                1_800_000_005_000,
            )
            .expect("accept first commit");
        source
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal_b, 1_800_000_006_000),
                1_800_000_006_000,
            )
            .expect("accept second commit");
        source
            .submit_moderation_ballot_reveal(reveal_a, 1_800_000_021_000)
            .expect("accept first reveal");
        source
            .submit_moderation_ballot_reveal(reveal_b, 1_800_000_021_500)
            .expect("accept second reveal");
        source
            .tally_moderation_ballot("case-55", "round-1", 1_800_000_030_000)
            .expect("tally ballot");

        let mut corrupt = source
            .export_moderation_ballot_snapshot()
            .expect("export source moderation ballot snapshot");
        corrupt.events[2].committed_count = 99;
        let restored = NodeHandle::new(cfg);
        let err = restored
            .restore_moderation_ballot_snapshot(corrupt)
            .expect_err("corrupt event count rejected");
        assert!(matches!(
            err,
            ModerationBallotRuntimeError::InvalidSnapshot { .. }
        ));
        assert_eq!(
            restored.moderation_ballot_snapshot(),
            ModerationBallotSnapshot::default()
        );
        assert_eq!(restored.latest_moderation_ballot_event_sequence(), None);

        let full = source
            .export_moderation_ballot_snapshot()
            .expect("export complete moderation snapshot");
        let mut valid_suffix = full.clone();
        valid_suffix.events.drain(..4);
        let suffix_target = NodeHandle::new(StorageConfig::builder().enabled(false).build());
        suffix_target
            .restore_moderation_ballot_snapshot(valid_suffix.clone())
            .expect("canonical retained event suffix is accepted");
        assert_eq!(suffix_target.moderation_ballots(), full.ballots);

        let mut forged_timestamp = valid_suffix;
        forged_timestamp.events[0].generated_at_unix_ms = forged_timestamp.events[0]
            .generated_at_unix_ms
            .saturating_add(1);
        assert!(matches!(
            NodeHandle::new(StorageConfig::builder().enabled(false).build())
                .restore_moderation_ballot_snapshot(forged_timestamp),
            Err(ModerationBallotRuntimeError::InvalidSnapshot { .. })
        ));

        let mut phase_regression = full;
        phase_regression.events.drain(..2);
        phase_regression.events.swap(0, 1);
        phase_regression.events[0].sequence = 3;
        phase_regression.events[1].sequence = 4;
        assert!(matches!(
            NodeHandle::new(StorageConfig::builder().enabled(false).build())
                .restore_moderation_ballot_snapshot(phase_regression),
            Err(ModerationBallotRuntimeError::InvalidSnapshot { .. })
        ));
    }

    #[test]
    fn node_handle_moderation_ballot_restore_rejects_corrupt_challenge_events() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let source = NodeHandle::new(cfg);
        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-61", jurors, 1);
        let context = announcement.context.clone();
        source
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");
        let reveal = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Overturn,
            0x61,
            1_800_000_021_000,
        );
        source
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal, 1_800_000_005_000),
                1_800_000_005_000,
            )
            .expect("accept commit");
        source
            .submit_moderation_ballot_challenge(
                moderation_challenge_input(
                    "case-61",
                    "challenge-restore",
                    ModerationBallotChallengeKind::EvidenceMismatch,
                ),
                1_800_000_011_000,
            )
            .expect("submit challenge");
        source
            .resolve_moderation_ballot_challenge(
                ModerationBallotChallengeResolution {
                    case_id: "case-61".to_owned(),
                    round_id: "round-1".to_owned(),
                    challenge_id: "challenge-restore".to_owned(),
                    resolved_by: "moderation-operator".to_owned(),
                    decision: ModerationBallotChallengeDecision::Rejected,
                    note: None,
                },
                1_800_000_012_000,
            )
            .expect("resolve challenge");
        let snapshot = source
            .export_moderation_ballot_snapshot()
            .expect("export challenge snapshot");
        assert_eq!(snapshot.events.len(), 4);
        assert_eq!(
            snapshot.events[2].kind,
            ModerationBallotEventKind::ChallengeSubmitted
        );
        assert_eq!(
            snapshot.events[3].kind,
            ModerationBallotEventKind::ChallengeResolved
        );

        let mut missing_events = snapshot.clone();
        missing_events.events.truncate(2);
        assert!(matches!(
            NodeHandle::new(StorageConfig::builder().enabled(false).build())
                .restore_moderation_ballot_snapshot(missing_events),
            Err(ModerationBallotRuntimeError::InvalidSnapshot { .. })
        ));

        let mut resolve_before_submit = snapshot.clone();
        let mut resolved = resolve_before_submit.events[3].clone();
        let mut submitted = resolve_before_submit.events[2].clone();
        resolved.sequence = 3;
        submitted.sequence = 4;
        resolve_before_submit.events[2] = resolved;
        resolve_before_submit.events[3] = submitted;
        assert!(matches!(
            NodeHandle::new(StorageConfig::builder().enabled(false).build())
                .restore_moderation_ballot_snapshot(resolve_before_submit),
            Err(ModerationBallotRuntimeError::InvalidSnapshot { .. })
        ));

        let mut mutated_resolution = snapshot;
        mutated_resolution.events[3]
            .challenge
            .as_mut()
            .expect("resolution challenge")
            .evidence_digest = [0xEE; 32];
        assert!(matches!(
            NodeHandle::new(StorageConfig::builder().enabled(false).build())
                .restore_moderation_ballot_snapshot(mutated_resolution),
            Err(ModerationBallotRuntimeError::InvalidSnapshot { .. })
        ));
    }

    #[test]
    fn node_handle_moderation_ballot_publishes_governance_events() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);

        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-47", jurors, 2);
        let context = announcement.context.clone();
        handle
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");

        let payloads = publisher.take();
        assert_eq!(payloads.len(), 1);
        let announced =
            norito::decode_from_bytes::<SoraFsModerationBallotGovernanceEventV1>(&payloads[0])
                .expect("decode announced governance event");
        assert_eq!(
            announced.kind,
            SoraFsModerationBallotGovernanceEventKindV1::BallotAnnounced
        );
        assert_eq!(announced.sequence, 1);
        assert_eq!(announced.case_id, "case-47");
        assert!(announced.tally.is_none());

        let reveal_a = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Overturn,
            0xA4,
            1_800_000_021_000,
        );
        let reveal_b = moderation_reveal(
            &context,
            "juror-b",
            SoraFsModerationVoteChoice::Overturn,
            0xB4,
            1_800_000_021_500,
        );
        let commit_a = moderation_commit_from_reveal(&reveal_a, 1_800_000_005_000);
        let commit_b = moderation_commit_from_reveal(&reveal_b, 1_800_000_006_000);
        handle
            .submit_moderation_ballot_commit(commit_a, 1_800_000_005_000)
            .expect("accept first commit");
        handle
            .submit_moderation_ballot_commit(commit_b, 1_800_000_006_000)
            .expect("accept second commit");
        handle
            .submit_moderation_ballot_reveal(reveal_a, 1_800_000_021_000)
            .expect("accept first reveal");
        handle
            .submit_moderation_ballot_reveal(reveal_b, 1_800_000_021_500)
            .expect("accept second reveal");
        handle
            .tally_moderation_ballot("case-47", "round-1", 1_800_000_030_000)
            .expect("tally ballot");

        let payloads = publisher.take();
        assert_eq!(payloads.len(), 5);
        let events = payloads
            .iter()
            .map(|payload| {
                norito::decode_from_bytes::<SoraFsModerationBallotGovernanceEventV1>(payload)
                    .expect("decode moderation governance event")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            events[0].kind,
            SoraFsModerationBallotGovernanceEventKindV1::CommitAccepted
        );
        assert_eq!(events[0].juror_id.as_deref(), Some("juror-a"));
        assert_eq!(
            events[4].kind,
            SoraFsModerationBallotGovernanceEventKindV1::BallotTallied
        );
        assert_eq!(events[4].sequence, 6);
        let tally = events[4].tally.as_ref().expect("tally payload");
        assert_eq!(tally.votes_total, 2);
        assert_eq!(
            tally.winning_choice,
            Some(SoraFsModerationVoteChoiceV1::Overturn)
        );
        events[4].validate().expect("governance event validates");
    }

    #[test]
    fn node_handle_moderation_ballot_publishes_challenge_governance_events() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);

        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-60", jurors, 1);
        let context = announcement.context.clone();
        handle
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");
        let reveal = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Modify,
            0x60,
            1_800_000_021_000,
        );
        handle
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal, 1_800_000_005_000),
                1_800_000_005_000,
            )
            .expect("accept commit");
        let submitted = handle
            .submit_moderation_ballot_challenge(
                moderation_challenge_input(
                    "case-60",
                    "challenge-governance",
                    ModerationBallotChallengeKind::EvidenceMismatch,
                ),
                1_800_000_011_000,
            )
            .expect("submit challenge");
        let resolved = handle
            .resolve_moderation_ballot_challenge(
                ModerationBallotChallengeResolution {
                    case_id: "case-60".to_owned(),
                    round_id: "round-1".to_owned(),
                    challenge_id: "challenge-governance".to_owned(),
                    resolved_by: "moderation-operator".to_owned(),
                    decision: ModerationBallotChallengeDecision::Rejected,
                    note: Some("governance challenge fixture".to_owned()),
                },
                1_800_000_012_000,
            )
            .expect("resolve challenge");

        let payloads = publisher.take();
        assert_eq!(payloads.len(), 4);
        let events = payloads
            .iter()
            .map(|payload| {
                norito::decode_from_bytes::<SoraFsModerationBallotGovernanceEventV1>(payload)
                    .expect("decode moderation governance event")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            events[2].kind,
            SoraFsModerationBallotGovernanceEventKindV1::ChallengeSubmitted
        );
        assert_eq!(events[2].sequence, 3);
        assert_eq!(events[2].challenge_count, 1);
        let challenge = events[2]
            .challenge
            .as_ref()
            .expect("submitted challenge payload");
        assert_eq!(challenge.challenge_id, submitted.challenge_id);
        assert!(challenge.decision.is_none());
        events[2].validate().expect("submitted challenge validates");

        assert_eq!(
            events[3].kind,
            SoraFsModerationBallotGovernanceEventKindV1::ChallengeResolved
        );
        assert_eq!(events[3].sequence, 4);
        assert_eq!(events[3].challenge_count, 1);
        let challenge = events[3]
            .challenge
            .as_ref()
            .expect("resolved challenge payload");
        assert_eq!(challenge.challenge_id, resolved.challenge_id);
        assert_eq!(
            challenge.decision,
            Some(sorafs_manifest::SoraFsModerationBallotGovernanceChallengeDecisionV1::Rejected)
        );
        events[3].validate().expect("resolved challenge validates");
    }

    #[test]
    fn node_handle_moderation_tally_publishes_appeal_finance_report_for_confirmed_deposit() {
        use iroha_data_model::sorafs::transparency::ModerationLedgerEntryKindV1;

        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);

        let jurors = moderation_jurors();
        let mut announcement = moderation_announcement("case-48", jurors, 2);
        let deposit = moderation_appeal_deposit();
        announcement.appeal_deposit_escrow_id_hex = Some(deposit.escrow_id_hex.clone());
        announcement.appeal_deposit = Some(deposit);
        let context = announcement.context.clone();
        handle
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");
        let _ = publisher.take();

        let reveal_a = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Overturn,
            0xA5,
            1_800_000_021_000,
        );
        let reveal_b = moderation_reveal(
            &context,
            "juror-b",
            SoraFsModerationVoteChoice::Overturn,
            0xB5,
            1_800_000_021_500,
        );
        handle
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal_a, 1_800_000_005_000),
                1_800_000_005_000,
            )
            .expect("accept first commit");
        handle
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal_b, 1_800_000_006_000),
                1_800_000_006_000,
            )
            .expect("accept second commit");
        handle
            .submit_moderation_ballot_reveal(reveal_a, 1_800_000_021_000)
            .expect("accept first reveal");
        handle
            .submit_moderation_ballot_reveal(reveal_b, 1_800_000_021_500)
            .expect("accept second reveal");
        handle
            .tally_moderation_ballot("case-48", "round-1", 1_800_000_030_000)
            .expect("tally ballot");

        let payloads = publisher.take();
        assert_eq!(payloads.len(), 6);
        let tally_event =
            norito::decode_from_bytes::<SoraFsModerationBallotGovernanceEventV1>(&payloads[4])
                .expect("decode tally event");
        assert_eq!(
            tally_event.kind,
            SoraFsModerationBallotGovernanceEventKindV1::BallotTallied
        );
        let report = norito::decode_from_bytes::<SoraFsAppealFinanceReportV1>(&payloads[5])
            .expect("decode appeal finance report");
        report.validate().expect("report validates");
        assert_ne!(report.report_id, [0_u8; 16]);
        assert_eq!(report.case_id, "case-48");
        assert_eq!(report.round_id.as_deref(), Some("round-1"));
        assert_eq!(report.outcome, SoraFsAppealFinanceOutcomeV1::Overturn);
        assert_eq!(report.deposit_xor, "420");
        assert_eq!(report.refund.account_id, "appeal-payer");
        assert_eq!(report.refund.amount_xor, "420");
        assert_eq!(report.treasury.account_id, "appeal-treasury");
        assert_eq!(report.treasury.amount_xor, "25");
        assert_eq!(report.held.account_id, "asset-lock-custody");
        assert_eq!(report.held.amount_xor, "0");
        assert_eq!(report.panel_reward_total_xor, "85");
        assert_eq!(report.rewards_paid_total_xor, "60");
        assert_eq!(report.rewards_forfeited_treasury_xor, "25");
        assert_eq!(report.juror_payouts.len(), 2);
        assert_eq!(report.no_show_juror_ids, vec!["juror-c".to_owned()]);
        assert_eq!(handle.transparency_ledger_source_entry_count(), 7);

        let publication = handle
            .publish_transparency_ledger_cycle_from_source_entries(
                *b"cycle-src-pub003",
                1_800_000_000,
                1_800_604_800,
                1_800_604_801,
                None,
            )
            .expect("publish moderation/appeal source cycle");
        publication.validate().expect("publication validates");
        assert_eq!(publication.block.entry_count, 7);
        assert!(
            publication
                .proofs
                .iter()
                .any(|proof| proof.entry.kind == ModerationLedgerEntryKindV1::ModerationAction)
        );
        assert!(
            publication
                .proofs
                .iter()
                .any(|proof| proof.entry.kind == ModerationLedgerEntryKindV1::AppealOutcome)
        );
    }

    #[test]
    fn node_handle_moderation_ballot_rejects_duplicates_and_mismatched_reveals() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-43", jurors, 1);
        let context = announcement.context.clone();
        handle
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");

        let reveal = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Modify,
            0xA3,
            1_800_000_021_000,
        );
        let commit = moderation_commit_from_reveal(&reveal, 1_800_000_005_000);
        handle
            .submit_moderation_ballot_commit(commit.clone(), 1_800_000_005_000)
            .expect("accept commit");

        assert!(matches!(
            handle.submit_moderation_ballot_commit(commit, 1_800_000_006_000),
            Err(ModerationBallotRuntimeError::DuplicateCommit { .. })
        ));
        assert!(matches!(
            handle.submit_moderation_ballot_reveal(reveal.clone(), 1_800_000_020_000),
            Err(ModerationBallotRuntimeError::RevealWindowNotOpen { .. })
        ));

        let mismatched = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Escalate,
            0xA3,
            1_800_000_021_000,
        );
        assert!(matches!(
            handle.submit_moderation_ballot_reveal(mismatched, 1_800_000_021_000),
            Err(ModerationBallotRuntimeError::Validation(
                SoraFsModerationBallotError::CommitmentMismatch
            ))
        ));

        handle
            .submit_moderation_ballot_reveal(reveal.clone(), 1_800_000_021_000)
            .expect("accept reveal");
        assert!(matches!(
            handle.submit_moderation_ballot_reveal(reveal, 1_800_000_021_500),
            Err(ModerationBallotRuntimeError::DuplicateReveal { .. })
        ));
    }

    #[test]
    fn node_handle_moderation_ballot_challenge_blocks_until_rejected_and_reloads() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let source = NodeHandle::new(cfg.clone());
        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-56", jurors, 2);
        let context = announcement.context.clone();
        source
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");

        let reveal_a = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Modify,
            0x56,
            1_800_000_021_000,
        );
        let reveal_b = moderation_reveal(
            &context,
            "juror-b",
            SoraFsModerationVoteChoice::Modify,
            0x57,
            1_800_000_021_500,
        );
        source
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal_a, 1_800_000_005_000),
                1_800_000_005_000,
            )
            .expect("accept first commit");
        source
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal_b, 1_800_000_006_000),
                1_800_000_006_000,
            )
            .expect("accept second commit");

        let challenge = source
            .submit_moderation_ballot_challenge(
                moderation_challenge_input(
                    "case-56",
                    "challenge-1",
                    ModerationBallotChallengeKind::EvidenceMismatch,
                ),
                1_800_000_011_000,
            )
            .expect("submit challenge");
        assert_eq!(challenge.decision, None);
        assert_eq!(source.latest_moderation_ballot_event_sequence(), Some(4));
        let challenge_event = source
            .moderation_ballot_events_since(Some(3), 10)
            .into_iter()
            .next()
            .expect("challenge submission event");
        assert_eq!(
            challenge_event.kind,
            ModerationBallotEventKind::ChallengeSubmitted
        );
        assert_eq!(challenge_event.challenge_count, 1);
        assert_eq!(challenge_event.challenge, Some(challenge.clone()));

        assert!(matches!(
            source.submit_moderation_ballot_reveal(reveal_a.clone(), 1_800_000_021_000),
            Err(ModerationBallotRuntimeError::ChallengePending { .. })
        ));
        assert!(matches!(
            source.tally_moderation_ballot("case-56", "round-1", 1_800_000_030_001),
            Err(ModerationBallotRuntimeError::ChallengePending { .. })
        ));

        let rejected = source
            .resolve_moderation_ballot_challenge(
                ModerationBallotChallengeResolution {
                    case_id: "case-56".to_owned(),
                    round_id: "round-1".to_owned(),
                    challenge_id: "challenge-1".to_owned(),
                    resolved_by: "moderation-operator".to_owned(),
                    decision: ModerationBallotChallengeDecision::Rejected,
                    note: Some("evidence packet is consistent".to_owned()),
                },
                1_800_000_012_000,
            )
            .expect("reject challenge");
        assert_eq!(
            rejected.decision,
            Some(ModerationBallotChallengeDecision::Rejected)
        );
        assert_eq!(source.latest_moderation_ballot_event_sequence(), Some(5));
        let resolved_event = source
            .moderation_ballot_events_since(Some(4), 10)
            .into_iter()
            .next()
            .expect("challenge resolution event");
        assert_eq!(
            resolved_event.kind,
            ModerationBallotEventKind::ChallengeResolved
        );
        assert_eq!(resolved_event.challenge_count, 1);
        assert_eq!(resolved_event.challenge, Some(rejected.clone()));

        let checkpoint_path = moderation_ballot_checkpoint_path(cfg.data_dir());
        let checkpoint_bytes = fs::read(&checkpoint_path).expect("read moderation checkpoint");
        let checkpoint: ModerationBallotSnapshot =
            norito::decode_from_bytes(&checkpoint_bytes).expect("decode moderation checkpoint");
        assert_eq!(checkpoint.ballots[0].challenges.len(), 1);
        assert_eq!(
            checkpoint.ballots[0].challenges[0].decision,
            Some(ModerationBallotChallengeDecision::Rejected)
        );
        assert_eq!(checkpoint.events.len(), 5);
        assert_eq!(
            checkpoint.events[3].kind,
            ModerationBallotEventKind::ChallengeSubmitted
        );
        assert_eq!(
            checkpoint.events[4].kind,
            ModerationBallotEventKind::ChallengeResolved
        );

        let restored = NodeHandle::new(cfg);
        assert_eq!(
            restored
                .export_moderation_ballot_snapshot()
                .expect("export restored moderation ballot snapshot"),
            source
                .export_moderation_ballot_snapshot()
                .expect("export source moderation ballot snapshot")
        );
        restored
            .submit_moderation_ballot_reveal(reveal_a, 1_800_000_021_000)
            .expect("accepted after rejected challenge");
        restored
            .submit_moderation_ballot_reveal(reveal_b, 1_800_000_021_500)
            .expect("accepted second reveal");
        let tally = restored
            .tally_moderation_ballot("case-56", "round-1", 1_800_000_030_000)
            .expect("tally after rejected challenge");
        assert_eq!(tally.votes_total, 2);
    }

    #[test]
    fn node_handle_moderation_ballot_accepted_challenge_blocks_reveal_and_tally() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-57", jurors, 1);
        let context = announcement.context.clone();
        handle
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");

        let reveal = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Uphold,
            0x58,
            1_800_000_021_000,
        );
        handle
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal, 1_800_000_005_000),
                1_800_000_005_000,
            )
            .expect("accept commit");
        handle
            .submit_moderation_ballot_challenge(
                moderation_challenge_input(
                    "case-57",
                    "challenge-accepted",
                    ModerationBallotChallengeKind::RosterMismatch,
                ),
                1_800_000_011_000,
            )
            .expect("submit challenge");
        handle
            .resolve_moderation_ballot_challenge(
                ModerationBallotChallengeResolution {
                    case_id: "case-57".to_owned(),
                    round_id: "round-1".to_owned(),
                    challenge_id: "challenge-accepted".to_owned(),
                    resolved_by: "moderation-operator".to_owned(),
                    decision: ModerationBallotChallengeDecision::Accepted,
                    note: None,
                },
                1_800_000_012_000,
            )
            .expect("accept challenge");

        assert!(matches!(
            handle.submit_moderation_ballot_reveal(reveal, 1_800_000_021_000),
            Err(ModerationBallotRuntimeError::ChallengeAccepted { .. })
        ));
        assert!(matches!(
            handle.tally_moderation_ballot("case-57", "round-1", 1_800_000_030_001),
            Err(ModerationBallotRuntimeError::ChallengeAccepted { .. })
        ));
        let record = handle
            .moderation_ballot("case-57", "round-1")
            .expect("ballot record");
        assert_eq!(record.challenges.len(), 1);
        assert_eq!(
            record.challenges[0].decision,
            Some(ModerationBallotChallengeDecision::Accepted)
        );
        assert!(record.reveals.is_empty());
    }

    #[test]
    fn node_handle_moderation_ballot_rejects_adversarial_challenges() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg.clone());
        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-58", jurors, 1);
        handle
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");

        assert!(matches!(
            handle.submit_moderation_ballot_challenge(
                moderation_challenge_input(
                    "case-58",
                    "too-early",
                    ModerationBallotChallengeKind::EvidenceMismatch,
                ),
                1_800_000_010_000,
            ),
            Err(ModerationBallotRuntimeError::ChallengeWindowNotOpen { .. })
        ));
        assert!(matches!(
            handle.submit_moderation_ballot_challenge(
                moderation_challenge_input(
                    "case-58",
                    "too-late",
                    ModerationBallotChallengeKind::EvidenceMismatch,
                ),
                1_800_000_020_001,
            ),
            Err(ModerationBallotRuntimeError::ChallengeWindowClosed { .. })
        ));

        let mut missing_target = moderation_challenge_input(
            "case-58",
            "missing-target",
            ModerationBallotChallengeKind::DuplicateCommit,
        );
        missing_target.target_juror_id = None;
        assert!(matches!(
            handle.submit_moderation_ballot_challenge(missing_target, 1_800_000_011_000),
            Err(ModerationBallotRuntimeError::MissingChallengeTarget { .. })
        ));

        let mut zero_digest = moderation_challenge_input(
            "case-58",
            "zero-digest",
            ModerationBallotChallengeKind::EvidenceMismatch,
        );
        zero_digest.evidence_digest = [0; 32];
        assert!(matches!(
            handle.submit_moderation_ballot_challenge(zero_digest, 1_800_000_011_000),
            Err(ModerationBallotRuntimeError::MissingChallengeEvidence)
        ));

        handle
            .submit_moderation_ballot_challenge(
                moderation_challenge_input(
                    "case-58",
                    "duplicate",
                    ModerationBallotChallengeKind::EvidenceMismatch,
                ),
                1_800_000_011_000,
            )
            .expect("submit challenge");
        assert!(matches!(
            handle.submit_moderation_ballot_challenge(
                moderation_challenge_input(
                    "case-58",
                    "duplicate",
                    ModerationBallotChallengeKind::EvidenceMismatch,
                ),
                1_800_000_011_500,
            ),
            Err(ModerationBallotRuntimeError::DuplicateChallenge { .. })
        ));
        assert!(matches!(
            handle.resolve_moderation_ballot_challenge(
                ModerationBallotChallengeResolution {
                    case_id: "case-58".to_owned(),
                    round_id: "round-1".to_owned(),
                    challenge_id: "unknown".to_owned(),
                    resolved_by: "moderation-operator".to_owned(),
                    decision: ModerationBallotChallengeDecision::Rejected,
                    note: None,
                },
                1_800_000_012_000,
            ),
            Err(ModerationBallotRuntimeError::UnknownChallenge { .. })
        ));
        assert!(matches!(
            handle.resolve_moderation_ballot_challenge(
                ModerationBallotChallengeResolution {
                    case_id: "case-58".to_owned(),
                    round_id: "round-1".to_owned(),
                    challenge_id: "duplicate".to_owned(),
                    resolved_by: "moderation-operator".to_owned(),
                    decision: ModerationBallotChallengeDecision::Rejected,
                    note: Some("   ".to_owned()),
                },
                1_800_000_012_000,
            ),
            Err(ModerationBallotRuntimeError::BlankChallengeResolutionNote)
        ));
        assert!(matches!(
            handle.resolve_moderation_ballot_challenge(
                ModerationBallotChallengeResolution {
                    case_id: "case-58".to_owned(),
                    round_id: "round-1".to_owned(),
                    challenge_id: "duplicate".to_owned(),
                    resolved_by: "moderation-operator".to_owned(),
                    decision: ModerationBallotChallengeDecision::Rejected,
                    note: None,
                },
                1_800_000_010_999,
            ),
            Err(ModerationBallotRuntimeError::InvalidChallengeResolutionTimestamp)
        ));
        handle
            .resolve_moderation_ballot_challenge(
                ModerationBallotChallengeResolution {
                    case_id: "case-58".to_owned(),
                    round_id: "round-1".to_owned(),
                    challenge_id: "duplicate".to_owned(),
                    resolved_by: "moderation-operator".to_owned(),
                    decision: ModerationBallotChallengeDecision::Rejected,
                    note: None,
                },
                1_800_000_012_000,
            )
            .expect("resolve challenge");
        assert!(matches!(
            handle.resolve_moderation_ballot_challenge(
                ModerationBallotChallengeResolution {
                    case_id: "case-58".to_owned(),
                    round_id: "round-1".to_owned(),
                    challenge_id: "duplicate".to_owned(),
                    resolved_by: "moderation-operator".to_owned(),
                    decision: ModerationBallotChallengeDecision::Rejected,
                    note: None,
                },
                1_800_000_012_500,
            ),
            Err(ModerationBallotRuntimeError::ChallengeAlreadyResolved { .. })
        ));

        let source = NodeHandle::new(cfg);
        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-59", jurors, 1);
        let context = announcement.context.clone();
        source
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");
        let reveal = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Uphold,
            0x59,
            1_800_000_021_000,
        );
        source
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal, 1_800_000_005_000),
                1_800_000_005_000,
            )
            .expect("accept commit");
        source
            .submit_moderation_ballot_reveal(reveal, 1_800_000_021_000)
            .expect("accept reveal");
        let mut corrupt = source
            .export_moderation_ballot_snapshot()
            .expect("export valid snapshot");
        corrupt.ballots[0]
            .challenges
            .push(ModerationBallotChallengeRecord {
                challenge_id: "pending-with-reveal".to_owned(),
                case_id: "case-59".to_owned(),
                round_id: "round-1".to_owned(),
                challenger_id: "moderation-provider".to_owned(),
                kind: ModerationBallotChallengeKind::EvidenceMismatch,
                target_juror_id: None,
                evidence_digest: [0xC9; 32],
                reason: "should block restored reveal".to_owned(),
                raised_at_unix_ms: 1_800_000_011_000,
                decision: None,
                resolved_by: None,
                resolved_at_unix_ms: None,
                resolution_note: None,
            });
        let restored = NodeHandle::new(StorageConfig::builder().enabled(false).build());
        assert!(matches!(
            restored.restore_moderation_ballot_snapshot(corrupt),
            Err(ModerationBallotRuntimeError::InvalidSnapshot { .. })
        ));
    }

    #[test]
    fn node_handle_moderation_ballot_rejects_invalid_attempts_without_events() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-49", jurors, 2);
        let context = announcement.context.clone();
        handle
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");
        assert_eq!(handle.latest_moderation_ballot_event_sequence(), Some(1));

        let reveal_a = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Uphold,
            0xC1,
            1_800_000_021_000,
        );
        let reveal_b = moderation_reveal(
            &context,
            "juror-b",
            SoraFsModerationVoteChoice::Modify,
            0xC2,
            1_800_000_021_500,
        );
        let reveal_c = moderation_reveal(
            &context,
            "juror-c",
            SoraFsModerationVoteChoice::Escalate,
            0xC3,
            1_800_000_021_500,
        );
        let ineligible_reveal = moderation_reveal(
            &context,
            "juror-x",
            SoraFsModerationVoteChoice::Overturn,
            0xCF,
            1_800_000_021_000,
        );

        assert!(matches!(
            handle.submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal_a, 1_800_000_010_001),
                1_800_000_010_001
            ),
            Err(ModerationBallotRuntimeError::CommitWindowClosed { .. })
        ));
        assert!(matches!(
            handle.submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&ineligible_reveal, 1_800_000_005_000),
                1_800_000_005_000
            ),
            Err(ModerationBallotRuntimeError::IneligibleJuror { .. })
        ));
        let record = handle
            .moderation_ballot("case-49", "round-1")
            .expect("ballot record");
        assert!(record.commits.is_empty());
        assert!(record.reveals.is_empty());
        assert_eq!(handle.latest_moderation_ballot_event_sequence(), Some(1));

        handle
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal_a, 1_800_000_005_000),
                1_800_000_005_000,
            )
            .expect("accept first commit");
        handle
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal_b, 1_800_000_006_000),
                1_800_000_006_000,
            )
            .expect("accept second commit");
        assert_eq!(handle.latest_moderation_ballot_event_sequence(), Some(3));

        assert!(matches!(
            handle.submit_moderation_ballot_reveal(reveal_a.clone(), 1_800_000_020_000),
            Err(ModerationBallotRuntimeError::RevealWindowNotOpen { .. })
        ));
        assert!(matches!(
            handle.submit_moderation_ballot_reveal(reveal_c, 1_800_000_021_000),
            Err(ModerationBallotRuntimeError::MissingCommit { .. })
        ));
        let record = handle
            .moderation_ballot("case-49", "round-1")
            .expect("ballot record");
        assert_eq!(record.commits.len(), 2);
        assert!(record.reveals.is_empty());
        assert_eq!(handle.latest_moderation_ballot_event_sequence(), Some(3));

        handle
            .submit_moderation_ballot_reveal(reveal_a, 1_800_000_021_000)
            .expect("accept first reveal");
        assert!(matches!(
            handle.submit_moderation_ballot_reveal(reveal_b, 1_800_000_030_001),
            Err(ModerationBallotRuntimeError::RevealWindowClosed { .. })
        ));
        let record = handle
            .moderation_ballot("case-49", "round-1")
            .expect("ballot record");
        assert_eq!(record.reveals.len(), 1);
        assert!(record.tally.is_none());
        assert_eq!(handle.latest_moderation_ballot_event_sequence(), Some(4));
    }

    #[test]
    fn node_handle_moderation_ballot_rejects_no_show_quorum_without_tally_event() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-50", jurors, 2);
        let context = announcement.context.clone();
        handle
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");

        let reveal = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Uphold,
            0xD1,
            1_800_000_021_000,
        );
        handle
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal, 1_800_000_005_000),
                1_800_000_005_000,
            )
            .expect("accept commit");
        handle
            .submit_moderation_ballot_reveal(reveal, 1_800_000_021_000)
            .expect("accept reveal");
        assert_eq!(handle.latest_moderation_ballot_event_sequence(), Some(3));

        assert!(matches!(
            handle.tally_moderation_ballot("case-50", "round-1", 1_800_000_025_000),
            Err(ModerationBallotRuntimeError::TallyWindowOpen { .. })
        ));
        assert!(matches!(
            handle.tally_moderation_ballot("case-50", "round-1", 1_800_000_030_001),
            Err(ModerationBallotRuntimeError::QuorumNotMet {
                quorum: 2,
                reveals: 1,
            })
        ));

        let record = handle
            .moderation_ballot("case-50", "round-1")
            .expect("ballot record");
        assert_eq!(record.commits.len(), 1);
        assert_eq!(record.reveals.len(), 1);
        assert!(record.tally.is_none());
        assert_eq!(handle.latest_moderation_ballot_event_sequence(), Some(3));
        assert!(
            handle
                .moderation_ballot_events_since(None, 10)
                .iter()
                .all(|event| event.kind != ModerationBallotEventKind::BallotTallied)
        );
    }

    #[test]
    fn node_handle_moderation_ballot_no_show_plan_tracks_missing_and_unrevealed_jurors() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-62", jurors, 2);
        let context = announcement.context.clone();
        handle
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");

        let reveal_a = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Uphold,
            0x62,
            1_800_000_021_000,
        );
        let reveal_b = moderation_reveal(
            &context,
            "juror-b",
            SoraFsModerationVoteChoice::Modify,
            0x63,
            1_800_000_021_500,
        );
        handle
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal_a, 1_800_000_005_000),
                1_800_000_005_000,
            )
            .expect("accept first commit");
        handle
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal_b, 1_800_000_006_000),
                1_800_000_006_000,
            )
            .expect("accept second commit");
        handle
            .submit_moderation_ballot_reveal(reveal_a, 1_800_000_021_000)
            .expect("accept first reveal");

        assert!(matches!(
            handle.moderation_ballot_no_show_plan("case-62", "round-1", 1_800_000_030_000),
            Err(ModerationBallotRuntimeError::TallyWindowOpen { .. })
        ));
        assert!(matches!(
            handle.tally_moderation_ballot("case-62", "round-1", 1_800_000_030_001),
            Err(ModerationBallotRuntimeError::QuorumNotMet {
                quorum: 2,
                reveals: 1,
            })
        ));

        let plan = handle
            .moderation_ballot_no_show_plan("case-62", "round-1", 1_800_000_030_001)
            .expect("build no-show plan");
        assert_eq!(plan.case_id, "case-62");
        assert_eq!(plan.round_id, "round-1");
        assert_eq!(plan.reveal_deadline_unix_ms, 1_800_000_030_000);
        assert_eq!(plan.quorum, 2);
        assert_eq!(plan.roster_size, 3);
        assert_eq!(plan.committed_count, 2);
        assert_eq!(plan.revealed_count, 1);
        assert_eq!(plan.no_show_count, 2);
        assert!(!plan.quorum_met);
        assert!(!plan.tally_finalized);
        assert!(!plan.contested);
        assert_eq!(plan.missing_commit_juror_ids, vec!["juror-c".to_owned()]);
        assert_eq!(
            plan.unrevealed_committed_juror_ids,
            vec!["juror-b".to_owned()]
        );
        assert_eq!(
            plan.no_show_juror_ids,
            vec!["juror-b".to_owned(), "juror-c".to_owned()]
        );
        assert_ne!(plan.penalty_plan_digest, [0; 32]);
        assert_eq!(handle.latest_moderation_ballot_event_sequence(), Some(4));
        assert!(
            handle
                .moderation_ballot_events_since(None, 10)
                .iter()
                .all(|event| event.kind != ModerationBallotEventKind::BallotTallied)
        );
    }

    #[test]
    fn node_handle_moderation_ballot_no_show_plan_blocks_on_challenges() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-63", jurors, 1);
        let context = announcement.context.clone();
        handle
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");
        let reveal = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Uphold,
            0x64,
            1_800_000_021_000,
        );
        handle
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal, 1_800_000_005_000),
                1_800_000_005_000,
            )
            .expect("accept commit");
        handle
            .submit_moderation_ballot_challenge(
                moderation_challenge_input(
                    "case-63",
                    "challenge-no-show",
                    ModerationBallotChallengeKind::EvidenceMismatch,
                ),
                1_800_000_011_000,
            )
            .expect("submit challenge");

        assert!(matches!(
            handle.moderation_ballot_no_show_plan("case-63", "round-1", 1_800_000_030_001),
            Err(ModerationBallotRuntimeError::ChallengePending { .. })
        ));
        handle
            .resolve_moderation_ballot_challenge(
                ModerationBallotChallengeResolution {
                    case_id: "case-63".to_owned(),
                    round_id: "round-1".to_owned(),
                    challenge_id: "challenge-no-show".to_owned(),
                    resolved_by: "moderation-operator".to_owned(),
                    decision: ModerationBallotChallengeDecision::Accepted,
                    note: Some("challenge accepted".to_owned()),
                },
                1_800_000_012_000,
            )
            .expect("accept challenge");
        assert!(matches!(
            handle.moderation_ballot_no_show_plan("case-63", "round-1", 1_800_000_030_001),
            Err(ModerationBallotRuntimeError::ChallengeAccepted { .. })
        ));
    }

    #[test]
    fn node_handle_moderation_ballot_no_show_plan_is_stable_after_successful_tally() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-64", jurors, 2);
        let context = announcement.context.clone();
        handle
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");

        let reveal_a = moderation_reveal(
            &context,
            "juror-a",
            SoraFsModerationVoteChoice::Overturn,
            0x65,
            1_800_000_021_000,
        );
        let reveal_b = moderation_reveal(
            &context,
            "juror-b",
            SoraFsModerationVoteChoice::Overturn,
            0x66,
            1_800_000_021_500,
        );
        handle
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal_a, 1_800_000_005_000),
                1_800_000_005_000,
            )
            .expect("accept first commit");
        handle
            .submit_moderation_ballot_commit(
                moderation_commit_from_reveal(&reveal_b, 1_800_000_006_000),
                1_800_000_006_000,
            )
            .expect("accept second commit");
        handle
            .submit_moderation_ballot_reveal(reveal_a, 1_800_000_021_000)
            .expect("accept first reveal");
        handle
            .submit_moderation_ballot_reveal(reveal_b, 1_800_000_021_500)
            .expect("accept second reveal");

        let tally = handle
            .tally_moderation_ballot("case-64", "round-1", 1_800_000_030_001)
            .expect("tally with one no-show");
        assert_eq!(tally.votes_total, 2);

        let first = handle
            .moderation_ballot_no_show_plan("case-64", "round-1", 1_800_000_031_000)
            .expect("first no-show plan");
        let second = handle
            .moderation_ballot_no_show_plan("case-64", "round-1", 1_800_000_032_000)
            .expect("second no-show plan");
        assert!(first.quorum_met);
        assert!(first.tally_finalized);
        assert!(!first.contested);
        assert_eq!(first.no_show_count, 1);
        assert_eq!(first.missing_commit_juror_ids, vec!["juror-c".to_owned()]);
        assert!(first.unrevealed_committed_juror_ids.is_empty());
        assert_eq!(first.no_show_juror_ids, vec!["juror-c".to_owned()]);
        assert_eq!(first.penalty_plan_digest, second.penalty_plan_digest);
        assert_ne!(first.generated_at_unix_ms, second.generated_at_unix_ms);
    }

    #[test]
    fn node_handle_moderation_ballot_allows_early_full_panel_tally_once() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let jurors = moderation_jurors();
        let announcement = moderation_announcement("case-51", jurors, 2);
        let context = announcement.context.clone();
        handle
            .announce_moderation_ballot(announcement)
            .expect("announce moderation ballot");

        let reveals = [
            moderation_reveal(
                &context,
                "juror-a",
                SoraFsModerationVoteChoice::Uphold,
                0xE1,
                1_800_000_021_000,
            ),
            moderation_reveal(
                &context,
                "juror-b",
                SoraFsModerationVoteChoice::Uphold,
                0xE2,
                1_800_000_021_500,
            ),
            moderation_reveal(
                &context,
                "juror-c",
                SoraFsModerationVoteChoice::Modify,
                0xE3,
                1_800_000_022_000,
            ),
        ];
        for (idx, reveal) in reveals.iter().enumerate() {
            handle
                .submit_moderation_ballot_commit(
                    moderation_commit_from_reveal(reveal, 1_800_000_005_000 + idx as u64),
                    1_800_000_005_000 + idx as u64,
                )
                .expect("accept full-panel commit");
        }
        for reveal in reveals {
            handle
                .submit_moderation_ballot_reveal(reveal, 1_800_000_021_000)
                .expect("accept full-panel reveal");
        }

        let tally = handle
            .tally_moderation_ballot("case-51", "round-1", 1_800_000_025_000)
            .expect("early tally after full panel reveals");
        assert_eq!(tally.votes_total, 3);
        assert_eq!(tally.quorum, 2);
        assert_eq!(tally.counts.uphold, 2);
        assert_eq!(tally.counts.modify, 1);
        assert_eq!(
            tally.winning_choice,
            Some(SoraFsModerationVoteChoice::Uphold)
        );
        assert!(!tally.contested);
        assert_eq!(handle.latest_moderation_ballot_event_sequence(), Some(8));

        assert!(matches!(
            handle.tally_moderation_ballot("case-51", "round-1", 1_800_000_026_000),
            Err(ModerationBallotRuntimeError::AlreadyTallied { .. })
        ));
        let record = handle
            .moderation_ballot("case-51", "round-1")
            .expect("ballot record");
        assert_eq!(record.tally, Some(tally));
        assert_eq!(handle.latest_moderation_ballot_event_sequence(), Some(8));
    }

    #[test]
    fn node_handle_moderation_ballot_validates_roster_hash_and_quorum() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let jurors = moderation_jurors();

        let mut bad_hash = moderation_announcement("case-44", jurors.clone(), 2);
        bad_hash.context.panel_roster_hash = [0xFF; 32];
        assert!(matches!(
            handle.announce_moderation_ballot(bad_hash),
            Err(ModerationBallotRuntimeError::RosterHashMismatch)
        ));

        let bad_quorum = moderation_announcement("case-45", jurors.clone(), 0);
        assert!(matches!(
            handle.announce_moderation_ballot(bad_quorum),
            Err(ModerationBallotRuntimeError::InvalidQuorum { .. })
        ));

        let duplicate_jurors = vec![
            "juror-a".to_owned(),
            "juror-b".to_owned(),
            "juror-a".to_owned(),
        ];
        let duplicate = moderation_announcement("case-46", duplicate_jurors, 2);
        assert!(matches!(
            handle.announce_moderation_ballot(duplicate),
            Err(ModerationBallotRuntimeError::DuplicateJuror { .. })
        ));
    }

    #[test]
    fn node_handle_registers_and_settles_deal() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let provider_id = ProviderId::new([0xDD; 32]);
        let client_id = ClientId::new([0xCC; 32]);

        handle
            .deposit_provider_bond(provider_id, 3_000_000_000)
            .expect("deposit provider bond");
        handle
            .deposit_client_credit(client_id, 1_000_000_000)
            .expect("deposit client credit");

        let terms = DealTerms {
            storage_price_nano_per_gib_month: 200_000_000,
            egress_price_nano_per_gib: 50_000_000,
            settlement_window_epochs: 7,
            micropayment_probability_bps: 10_000,
            micropayment_payout_nano: 50_000_000,
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

        let usage = DealUsageReport {
            deal_id: record.deal_id,
            epoch: activation_epoch + 1,
            storage_gib_hours: (4u128 * GIB_HOURS_PER_MONTH) as u64,
            egress_bytes: BYTES_PER_GIB as u64,
            tickets: vec![
                MicropaymentTicket {
                    ticket_id: TicketId([1; 32]),
                    issued_epoch: activation_epoch + 1,
                    storage_gib_hours: 0,
                    egress_bytes: 0,
                },
                MicropaymentTicket {
                    ticket_id: TicketId([2; 32]),
                    issued_epoch: activation_epoch + 1,
                    storage_gib_hours: 0,
                    egress_bytes: 0,
                },
                MicropaymentTicket {
                    ticket_id: TicketId([3; 32]),
                    issued_epoch: activation_epoch + 1,
                    storage_gib_hours: 0,
                    egress_bytes: 0,
                },
                MicropaymentTicket {
                    ticket_id: TicketId([4; 32]),
                    issued_epoch: activation_epoch + 1,
                    storage_gib_hours: 0,
                    egress_bytes: 0,
                },
                MicropaymentTicket {
                    ticket_id: TicketId([5; 32]),
                    issued_epoch: activation_epoch + 1,
                    storage_gib_hours: 0,
                    egress_bytes: 0,
                },
            ],
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
        assert_eq!(settlement.expected_charge, 850_000_000);
        assert_eq!(settlement.micropayment_credit, 250_000_000);
        assert_eq!(settlement.client_credit_debit, 600_000_000);
        assert_eq!(settlement.bond_slash, 0);
        assert_eq!(settlement.outstanding, 0);

        let governance = &outcome.governance;
        assert_eq!(governance.deal_id, *record.deal_id.as_bytes());
        assert_eq!(governance.status, DealSettlementStatusV1::Completed);
        assert_eq!(governance.settled_at, activation_epoch + 7);
        let ledger = &governance.ledger;
        assert_eq!(ledger.deal_id, *record.deal_id.as_bytes());
        assert_eq!(ledger.provider_id, *provider_id.as_bytes());
        assert_eq!(ledger.client_id, *client_id.as_bytes());
        assert_eq!(
            ledger
                .provider_accrual
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            850_000
        );
        assert_eq!(
            ledger
                .client_liability
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            850_000
        );
        assert_eq!(
            ledger
                .bond_locked
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            2_400_000
        );
        assert_eq!(
            ledger
                .bond_slashed
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            0
        );
        assert_eq!(ledger.captured_at, activation_epoch + 7);

        let snapshot = handle.deal_snapshot(record.deal_id).expect("snapshot");
        assert!(matches!(snapshot.status, DealStatus::Active(_)));

        let provider_snapshot = handle
            .deal_engine()
            .provider_snapshot(provider_id)
            .expect("provider snapshot");
        assert!(provider_snapshot.bond_locked >= 2_400_000_000);
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
            .deposit_provider_bond(provider_id, 3_000_000_000)
            .expect("persist provider bond");
        source
            .deposit_client_credit(client_id, 1_000_000_000)
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
                        storage_price_nano_per_gib_month: 200_000_000,
                        egress_price_nano_per_gib: 50_000_000,
                        settlement_window_epochs: 7,
                        micropayment_probability_bps: 10_000,
                        micropayment_payout_nano: 50_000_000,
                    },
                    metadata: Metadata::default(),
                },
                activation_epoch,
            )
            .expect("persist deal");
        let replay_ticket = MicropaymentTicket {
            ticket_id: TicketId([0xE1; 32]),
            issued_epoch: activation_epoch + 1,
            storage_gib_hours: 0,
            egress_bytes: 0,
        };
        source
            .record_deal_usage(DealUsageReport {
                deal_id: record.deal_id,
                epoch: activation_epoch + 1,
                storage_gib_hours: 0,
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
            600_000_000
        );
        assert_eq!(
            restored
                .deal_engine()
                .client_snapshot(client_id)
                .expect("restored client")
                .credit_balance,
            1_000_000_000
        );
        let replay = restored
            .record_deal_usage(DealUsageReport {
                deal_id: record.deal_id,
                epoch: activation_epoch + 1,
                storage_gib_hours: 0,
                egress_bytes: 0,
                tickets: vec![replay_ticket],
            })
            .expect("replay retained ticket");
        assert_eq!(replay.tickets_duplicate, 1);
        assert_eq!(replay.tickets_won, 0);
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
            .deposit_provider_bond(provider_id, 3_000_000_000)
            .expect("deposit provider bond");
        handle
            .deposit_client_credit(client_id, 1_000_000_000)
            .expect("deposit client credit");

        let terms = DealTerms {
            storage_price_nano_per_gib_month: 200_000_000,
            egress_price_nano_per_gib: 50_000_000,
            settlement_window_epochs: 7,
            micropayment_probability_bps: 10_000,
            micropayment_payout_nano: 50_000_000,
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
        let mut cursor = &published[0][..];
        let decoded = DealSettlementV1::decode(&mut cursor).expect("governance payload decodes");
        assert_eq!(decoded.deal_id, *record.deal_id.as_bytes());
        assert_eq!(decoded.ledger.provider_id, *provider_id.as_bytes());
        assert_eq!(decoded.ledger.client_id, *client_id.as_bytes());
        assert_eq!(decoded.status, outcome.governance.status);
        assert_eq!(decoded.settled_at, outcome.governance.settled_at);
    }

    #[test]
    fn publish_reputation_snapshot_updates_cache_and_governance_publisher() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        let snapshot = reputation_snapshot_fixture();
        let expected = to_bytes(&snapshot).expect("encode reputation snapshot");
        let mut event_receiver = handle.subscribe_reputation_events();

        handle
            .publish_reputation_snapshot(snapshot.clone())
            .expect("publish reputation snapshot");

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
    fn reputation_snapshot_rejects_conflicting_ids_and_evicts_only_unreferenced_history() {
        let (base, _dir) = storage_config_with_temp_dir();
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(base.data_dir().clone())
            .runtime_retention(RuntimeRetentionPolicy::new(1, 1, 1024 * 1024))
            .build();
        let handle = NodeHandle::new(cfg);
        let first = reputation_snapshot_fixture();
        handle
            .publish_reputation_snapshot(first.clone())
            .expect("publish first snapshot");

        let conflicting =
            reputation_snapshot_fixture_with(first.snapshot_id, first.generated_at_unix + 1, None);
        let conflict_error = handle
            .publish_reputation_snapshot(conflicting)
            .expect_err("conflicting canonical bytes under one id must fail");
        assert!(conflict_error.to_string().contains("conflicts"));

        let broken_head =
            reputation_snapshot_fixture_with([0x43; 16], first.generated_at_unix + 1, None);
        let head_error = handle
            .publish_reputation_snapshot(broken_head)
            .expect_err("snapshot must extend current head");
        assert!(head_error.to_string().contains("must extend current head"));

        let next = reputation_snapshot_fixture_with(
            [0x44; 16],
            first.generated_at_unix + 1,
            Some(first.snapshot_id),
        );
        handle
            .publish_reputation_snapshot(next)
            .expect("unreferenced predecessor can be safely evicted");
        assert_eq!(
            handle
                .latest_reputation_snapshot()
                .map(|snapshot| snapshot.snapshot_id),
            Some([0x44; 16])
        );
        assert!(handle.reputation_snapshot(first.snapshot_id).is_none());
        assert_eq!(handle.reputation_events_since(None, 10).len(), 1);
        assert_eq!(
            handle.reputation_events_since(None, 10)[0].snapshot_id,
            [0x44; 16]
        );
    }

    #[test]
    fn reputation_snapshot_publish_failure_keeps_durable_state_for_exact_retry() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg.clone());
        let failing = Arc::new(FailingPublisher::default());
        handle.set_governance_publisher(failing.clone());
        let snapshot = reputation_snapshot_fixture();

        handle
            .publish_reputation_snapshot(snapshot.clone())
            .expect_err("external publisher failure is surfaced");
        assert_eq!(failing.attempts(), 1);
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
        let recording = Arc::new(RecordingPublisher::default());
        restored.set_governance_publisher(recording.clone());
        restored
            .publish_reputation_snapshot(snapshot.clone())
            .expect("exact retry publishes without appending another event");
        assert_eq!(
            recording.take(),
            vec![to_bytes(&snapshot).expect("encode snapshot")]
        );
        assert_eq!(restored.reputation_events_since(None, 10).len(), 1);
    }

    #[test]
    fn reputation_checkpoint_rejects_event_snapshot_metadata_tampering() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg.clone());
        handle
            .publish_reputation_snapshot(reputation_snapshot_fixture())
            .expect("publish snapshot");
        drop(handle);

        let path = auxiliary_runtime_checkpoint_path(cfg.data_dir());
        let bytes = fs::read(&path).expect("read auxiliary checkpoint");
        let mut checkpoint: AuxiliaryRuntimeCheckpointV1 =
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
    fn publish_reserve_adjusted_reputation_snapshot_penalizes_defaulted_provider() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        handle
            .record_reserve_lifecycle_update(reserve_lifecycle_update(
                0x71,
                31,
                XorQuantity::zero(),
                1_800_000_150,
            ))
            .expect("record defaulted reserve provider");

        let snapshot = handle
            .publish_reserve_adjusted_reputation_snapshot(
                [0x71; 16],
                1_800_000_250,
                ReputationWeightsV1::default(),
            )
            .expect("publish reserve-adjusted reputation snapshot");

        let provider_id = hex::encode([0x71; 32]);
        let provider = snapshot
            .providers
            .iter()
            .find(|provider| provider.provider_id == provider_id)
            .expect("reserve provider reputation");
        assert!(
            provider
                .degradation_flags
                .contains(&ReputationDegradationFlagV1::ReserveDefault)
        );
        assert!(
            provider.score_bps <= 2_000,
            "default reserve stage must materially lower reputation"
        );
        assert_eq!(
            handle
                .latest_reputation_snapshot()
                .expect("latest reputation snapshot")
                .snapshot_id,
            snapshot.snapshot_id
        );
    }

    #[test]
    fn publish_reserve_adjusted_reputation_snapshot_uses_accepted_appeal_override() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let provider_id = [0x72; 32];
        handle
            .record_reserve_lifecycle_update(reserve_lifecycle_update(
                0x72,
                31,
                XorQuantity::zero(),
                1_800_000_150,
            ))
            .expect("record defaulted reserve provider");
        let default_snapshot = handle
            .publish_reserve_adjusted_reputation_snapshot(
                [0x72; 16],
                1_800_000_250,
                ReputationWeightsV1::default(),
            )
            .expect("publish default reserve reputation snapshot");
        let provider_id_hex = hex::encode(provider_id);
        let default_provider = default_snapshot
            .providers
            .iter()
            .find(|provider| provider.provider_id == provider_id_hex)
            .expect("default reserve provider reputation");
        assert!(
            default_provider
                .degradation_flags
                .contains(&ReputationDegradationFlagV1::ReserveDefault)
        );

        handle
            .record_reserve_appeal(reserve_appeal_request(0x73, 0x72))
            .expect("record reserve appeal");
        handle
            .record_reserve_appeal_decision(reserve_appeal_decision(
                0x73,
                ReserveAppealStatus::Accepted,
            ))
            .expect("accept reserve appeal");
        let adjusted_snapshot = handle
            .publish_reserve_adjusted_reputation_snapshot(
                [0x73; 16],
                2_300_000_000,
                ReputationWeightsV1::default(),
            )
            .expect("publish appeal-adjusted reserve reputation snapshot");
        let adjusted_provider = adjusted_snapshot
            .providers
            .iter()
            .find(|provider| provider.provider_id == provider_id_hex)
            .expect("appeal-adjusted reserve provider reputation");

        assert!(
            adjusted_provider
                .degradation_flags
                .contains(&ReputationDegradationFlagV1::ReserveGrace)
        );
        assert!(
            !adjusted_provider
                .degradation_flags
                .contains(&ReputationDegradationFlagV1::ReserveDefault)
        );
        assert_eq!(
            adjusted_snapshot.previous_snapshot_id,
            Some(default_snapshot.snapshot_id)
        );
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
    fn record_transparency_ledger_source_entry_rejects_duplicates() {
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
        let err = handle
            .record_transparency_ledger_source_entry(entry)
            .expect_err("duplicate source entry rejected");

        assert!(
            err.to_string()
                .contains("duplicate transparency ledger source entry")
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
        let moderation_event = ModerationBallotEvent {
            sequence: 7,
            kind: ModerationBallotEventKind::BallotTallied,
            generated_at_unix_ms: 1_800_000_020_000,
            case_id: "case-42".to_string(),
            round_id: "round-1".to_string(),
            juror_id: None,
            committed_count: 3,
            revealed_count: 3,
            challenge_count: 0,
            tally: Some(ModerationBallotTally {
                case_id: "case-42".to_string(),
                round_id: "round-1".to_string(),
                counts: ModerationVoteCounts {
                    uphold: 1,
                    overturn: 2,
                    modify: 0,
                    escalate: 0,
                },
                votes_total: 3,
                quorum: 2,
                winning_choice: Some(SoraFsModerationVoteChoice::Overturn),
                contested: false,
                tallied_at_unix_ms: 1_800_000_020_000,
            }),
            challenge: None,
        }
        .to_governance_event_v1();
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
                .contains("window must be contained in the publication cycle")
        );
        assert!(publisher.take().is_empty());
    }

    #[test]
    fn record_privacy_aggregate_source_event_rejects_duplicates() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let event = privacy_source_event("event-a", "jurisdiction-a", 0xA0, 1_800_000_010);

        handle
            .record_privacy_aggregate_source_event(event.clone())
            .expect("record source event");
        let err = handle
            .record_privacy_aggregate_source_event(event)
            .expect_err("duplicate source event rejected");

        assert!(
            err.to_string()
                .contains("duplicate privacy aggregate source event")
        );
        assert_eq!(handle.privacy_aggregate_source_event_count(), 1);
    }

    #[test]
    fn publish_privacy_aggregate_cycle_from_source_events_suppresses_and_publishes() {
        use iroha_data_model::sorafs::transparency::ModerationLedgerEntryKindV1;

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

        let publication = handle
            .publish_privacy_aggregate_cycle_from_source_events(
                *b"cycle-2026-wk-04",
                1_800_000_000,
                1_800_604_800,
                1_800_604_801,
                None,
                privacy_aggregate_cycle_config(Some([0x5A; 32])),
            )
            .expect("publish aggregate cycle from source events");

        publication.validate().expect("publication validates");
        assert_eq!(publication.block.entry_count, 1);
        let entry = &publication.proofs[0].entry;
        assert_eq!(entry.kind, ModerationLedgerEntryKindV1::PrivacyAggregate);
        assert!(entry.subject.contains("jurisdiction-a"));
        assert_eq!(entry.evidence_uris.len(), 0);
        assert!(
            entry
                .metadata
                .iter()
                .any(|item| item.key == "suppressed_count" && item.value == "1")
        );
        assert!(
            entry
                .metadata
                .iter()
                .any(|item| item.key == "source_event_count" && item.value == "2")
        );

        let published = publisher.take();
        assert_eq!(published.len(), 1);
        let decoded: ModerationLedgerCyclePublicationV1 =
            norito::decode_from_bytes(&published[0]).expect("decode aggregate publication");
        assert_eq!(decoded, publication);
    }

    #[test]
    fn publish_privacy_aggregate_cycle_from_source_events_requires_noise_seed() {
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
            .publish_privacy_aggregate_cycle_from_source_events(
                *b"cycle-2026-wk-04",
                1_800_000_000,
                1_800_604_800,
                1_800_604_801,
                None,
                privacy_aggregate_cycle_config(None),
            )
            .expect_err("missing noise seed rejected");

        assert!(err.to_string().contains("runtime noise seed"));
        assert!(publisher.take().is_empty());
    }

    #[test]
    fn publish_due_privacy_aggregate_cycle_from_source_events_publishes_once() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
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
                privacy_aggregate_schedule_config(),
                privacy_aggregate_cycle_config(Some([0x5A; 32])),
                None,
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
        assert_eq!(publication.block.generated_at_unix, 211);
        assert_eq!(publication.block.entry_count, 1);

        let repeated = handle
            .publish_due_privacy_aggregate_cycle_from_source_events(
                211,
                privacy_aggregate_schedule_config(),
                privacy_aggregate_cycle_config(Some([0x5A; 32])),
                None,
            )
            .expect("repeat due aggregate cycle");
        assert!(matches!(
            repeated,
            PrivacyAggregateScheduleOutcome::AlreadyPublished { .. }
        ));
        assert_eq!(publisher.take().len(), 1);
    }

    #[test]
    fn publish_due_privacy_aggregate_cycle_from_source_events_catches_up_stale_windows() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
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
                privacy_aggregate_schedule_config(),
                privacy_aggregate_cycle_config(Some([0x5A; 32])),
                None,
            )
            .expect("publish first stale aggregate cycle");
        match first {
            PrivacyAggregateScheduleOutcome::Published {
                window,
                publication,
            } => {
                assert_eq!(window.cycle_start_unix, 100);
                assert_eq!(window.cycle_end_unix, 200);
                assert_eq!(publication.block.cycle_start_unix, 100);
                assert_eq!(publication.block.cycle_end_unix, 200);
                assert_eq!(publication.block.generated_at_unix, 311);
            }
            other => panic!("expected stale published outcome, got {other:?}"),
        }

        let second = handle
            .publish_due_privacy_aggregate_cycle_from_source_events(
                311,
                privacy_aggregate_schedule_config(),
                privacy_aggregate_cycle_config(Some([0x5A; 32])),
                None,
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
            }
            other => panic!("expected latest published outcome, got {other:?}"),
        }
        assert_eq!(publisher.take().len(), 2);
    }

    #[test]
    fn publish_due_privacy_aggregate_cycle_from_source_events_skips_stale_suppressed_window() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
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

        let outcome = handle
            .publish_due_privacy_aggregate_cycle_from_source_events(
                311,
                privacy_aggregate_schedule_config(),
                privacy_aggregate_cycle_config(Some([0x5A; 32])),
                None,
            )
            .expect("publish due aggregate cycle with stale suppressed window");
        match outcome {
            PrivacyAggregateScheduleOutcome::Published {
                window,
                publication,
            } => {
                assert_eq!(window.cycle_start_unix, 200);
                assert_eq!(window.cycle_end_unix, 300);
                assert_eq!(publication.block.entry_count, 1);
            }
            other => panic!("expected later published outcome, got {other:?}"),
        }
        assert_eq!(publisher.take().len(), 1);
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
            .build();
        let handle = NodeHandle::new(cfg);
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
                211,
                privacy_aggregate_cycle_config(Some([0x5A; 32])),
                None,
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
        assert_eq!(publication.block.generated_at_unix, 211);
        assert_eq!(publisher.take().len(), 1);
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
                211,
                privacy_aggregate_cycle_config(Some([0x5A; 32])),
                None,
            )
            .expect("disabled configured aggregate cycle");
        assert_eq!(outcome, PrivacyAggregateScheduleOutcome::Disabled);
    }

    #[test]
    fn publish_due_privacy_aggregate_cycle_from_source_events_skips_empty_and_suppressed() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);
        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);
        let empty = handle
            .publish_due_privacy_aggregate_cycle_from_source_events(
                211,
                privacy_aggregate_schedule_config(),
                privacy_aggregate_cycle_config(Some([0x5A; 32])),
                None,
            )
            .expect("empty due aggregate cycle");
        assert!(matches!(
            empty,
            PrivacyAggregateScheduleOutcome::NoSourceEvents { .. }
        ));

        handle
            .record_privacy_aggregate_source_event(privacy_source_event(
                "alpha-1",
                "jurisdiction-a",
                0xA0,
                110,
            ))
            .expect("record source event");
        let suppressed = handle
            .publish_due_privacy_aggregate_cycle_from_source_events(
                211,
                privacy_aggregate_schedule_config(),
                privacy_aggregate_cycle_config(Some([0x5A; 32])),
                None,
            )
            .expect("suppressed due aggregate cycle");
        assert!(matches!(
            suppressed,
            PrivacyAggregateScheduleOutcome::AllBucketsSuppressed { .. }
        ));
        assert!(publisher.take().is_empty());
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
            .deposit_provider_bond(provider_id, 1_000_000_000)
            .expect("deposit provider bond");
        handle
            .deposit_client_credit(client_id, 1_000_000_000)
            .expect("deposit client credit");

        let terms = DealTerms {
            storage_price_nano_per_gib_month: 100_000_000,
            egress_price_nano_per_gib: 25_000_000,
            settlement_window_epochs: 5,
            micropayment_probability_bps: 0,
            micropayment_payout_nano: 0,
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
        let mut cursor = &std::fs::read(encoded_path).expect("read encoded artefact")[..];
        let decoded = DealSettlementV1::decode(&mut cursor).expect("decode artefact");
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
        assert_eq!(status, "completed");
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
            .deposit_provider_bond(provider_id, 3_000_000_000)
            .expect("deposit provider bond");
        handle
            .deposit_client_credit(client_id, 1_000_000_000)
            .expect("deposit client credit");

        let terms = DealTerms {
            storage_price_nano_per_gib_month: 200_000_000,
            egress_price_nano_per_gib: 50_000_000,
            settlement_window_epochs: 7,
            micropayment_probability_bps: 10_000,
            micropayment_payout_nano: 50_000_000,
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
            .record_por_verdict(&verdict, &por_sample_auditor_keys(), 1)
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
            .record_por_verdict(&verdict, &por_sample_auditor_keys(), 1)
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
            .record_por_verdict(&verdict, &por_sample_auditor_keys(), 1)
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
            .deposit_provider_bond(provider, 10_000)
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
            .record_por_verdict(&verdict, &por_sample_auditor_keys(), 1)
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
            .record_por_verdict(&second_verdict, &por_sample_auditor_keys(), 1)
            .expect("record second failure");

        let slash = second.slash.expect("slash recommendation expected");
        assert_eq!(slash.provider_id, provider);
        assert_eq!(slash.manifest_digest, challenge.manifest_digest);
        assert_eq!(
            slash.penalty,
            XorQuantity::try_from_micro(5_000).expect("valid XOR quantity")
        );
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
            .deposit_provider_bond(provider, 2_000)
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
            .record_por_verdict(&verdict, &por_sample_auditor_keys(), 1)
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
            .record_por_verdict(&later_verdict, &por_sample_auditor_keys(), 1)
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
    fn node_handle_manages_repair_queue() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let repair_actual = iroha_config::parameters::actual::SorafsRepair {
            enabled: true,
            ..Default::default()
        };
        let handle = NodeHandle::new_with_policies(
            cfg,
            RepairConfig::from(&repair_actual),
            GcConfig::default(),
        );

        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-351".into()),
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            submitted_at_unix: 1_700_100_000,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest: [0x44; 32],
                provider_id: [0x77; 32],
                por_history_id: None,
                cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                    reason: "operator-verified repair trigger".into(),
                }),
                evidence_json: None,
                notes: Some("PoR sample failed twice".into()),
            },
            notes: Some("auto-generated".into()),
        };

        let queued = handle
            .enqueue_repair_report(&report)
            .expect("queue repair report");
        assert!(matches!(
            queued.state,
            RepairTaskStateV1::Queued(QueuedRepairStateV1 { .. })
        ));

        let in_progress = handle
            .mark_repair_in_progress(
                &report.ticket_id,
                report.submitted_at_unix + 45,
                Some("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D".into()),
            )
            .expect("mark repair in progress");
        assert!(matches!(
            in_progress.state,
            RepairTaskStateV1::InProgress(InProgressRepairStateV1 { .. })
        ));

        let completed = handle
            .mark_repair_completed(
                &report.ticket_id,
                report.submitted_at_unix + 600,
                Some("reseeded manifest".into()),
            )
            .expect("mark repair completed");
        assert!(matches!(
            completed.state,
            RepairTaskStateV1::Completed(CompletedRepairStateV1 { .. })
        ));

        let events = handle.repair_events_since(None, 10);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].sequence, 1);
        assert_eq!(events[0].event.status, RepairTaskStatusV1::Queued);
        assert_eq!(events[1].event.status, RepairTaskStatusV1::InProgress);
        assert_eq!(events[2].event.status, RepairTaskStatusV1::Completed);
        assert_eq!(events[2].event.ticket_id, report.ticket_id);
        assert_eq!(handle.latest_repair_event_sequence(), Some(3));
        let after_first = handle.repair_events_since(Some(1), 10);
        assert_eq!(after_first.len(), 2);

        let tasks = handle
            .repair_tasks_for_manifest(&report.evidence.manifest_digest)
            .expect("repair task store");
        assert_eq!(tasks.len(), 1);
        let provider_tasks = handle
            .repair_tasks(RepairTaskFilters {
                provider_id: Some(report.evidence.provider_id),
                ..RepairTaskFilters::default()
            })
            .expect("repair task store");
        assert_eq!(provider_tasks.len(), 1);

        let mut live_events = handle.subscribe_repair_events();
        let escalated_report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-352".into()),
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            submitted_at_unix: 1_700_200_000,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest: [0x45; 32],
                provider_id: [0x88; 32],
                por_history_id: None,
                cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                    reason: "operator-verified escalation trigger".into(),
                }),
                evidence_json: None,
                notes: None,
            },
            notes: None,
        };
        handle
            .enqueue_repair_report(&escalated_report)
            .expect("queue second report");
        let live_queued = live_events.try_recv().expect("live queued repair event");
        assert_eq!(live_queued.sequence, 4);
        assert_eq!(live_queued.event.ticket_id, escalated_report.ticket_id);
        assert_eq!(live_queued.event.status, RepairTaskStatusV1::Queued);

        let proposal = RepairSlashProposalV1 {
            version: REPAIR_SLASH_PROPOSAL_VERSION_V1,
            ticket_id: escalated_report.ticket_id.clone(),
            provider_id: escalated_report.evidence.provider_id,
            manifest_digest: escalated_report.evidence.manifest_digest,
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            proposed_penalty: "0.5".parse().expect("valid quantity"),
            submitted_at_unix: escalated_report.submitted_at_unix + 1_200,
            rationale: "Repeated PoR failures without acknowledgement".into(),
            approval: None,
        };

        let escalated = handle
            .submit_repair_slash_proposal(&proposal)
            .expect("submit slash proposal");
        assert!(matches!(
            escalated.state,
            RepairTaskStateV1::Escalated(EscalatedRepairStateV1 { .. })
        ));
        let live_escalated = live_events.try_recv().expect("live escalated repair event");
        assert_eq!(live_escalated.sequence, 5);
        assert_eq!(live_escalated.event.ticket_id, escalated_report.ticket_id);
        assert_eq!(live_escalated.event.status, RepairTaskStatusV1::Escalated);
    }

    #[test]
    fn node_handle_tracks_repair_worker_actions() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-451".into()),
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            submitted_at_unix: 1_700_300_000,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest: [0x10; 32],
                provider_id: [0x20; 32],
                por_history_id: None,
                cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                    reason: "manual".into(),
                }),
                evidence_json: None,
                notes: None,
            },
            notes: None,
        };

        handle
            .enqueue_repair_report(&report)
            .expect("queue repair report");
        let claimed = handle
            .claim_repair_ticket(
                &report.ticket_id,
                "worker-1",
                report.submitted_at_unix + 10,
                "claim-1",
            )
            .expect("claim repair ticket");
        assert!(matches!(
            claimed.state,
            RepairTaskStateV1::InProgress(InProgressRepairStateV1 { .. })
        ));

        handle
            .heartbeat_repair_ticket(
                &report.ticket_id,
                "worker-1",
                report.submitted_at_unix + 20,
                "hb-1",
            )
            .expect("heartbeat repair ticket");

        let completed = handle
            .complete_repair_ticket(
                &report.ticket_id,
                "worker-1",
                report.submitted_at_unix + 30,
                Some("repaired".into()),
                "complete-1",
            )
            .expect("complete repair ticket");
        assert!(matches!(
            completed.state,
            RepairTaskStateV1::Completed(CompletedRepairStateV1 { .. })
        ));

        let failed_report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-452".into()),
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            submitted_at_unix: 1_700_400_000,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest: [0x11; 32],
                provider_id: [0x21; 32],
                por_history_id: None,
                cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                    reason: "manual".into(),
                }),
                evidence_json: None,
                notes: None,
            },
            notes: None,
        };
        handle
            .enqueue_repair_report(&failed_report)
            .expect("queue second report");
        handle
            .claim_repair_ticket(
                &failed_report.ticket_id,
                "worker-2",
                failed_report.submitted_at_unix + 5,
                "claim-2",
            )
            .expect("claim second ticket");

        let failed = handle
            .fail_repair_ticket(
                &failed_report.ticket_id,
                "worker-2",
                failed_report.submitted_at_unix + 15,
                "retry later".into(),
                "fail-1",
            )
            .expect("fail repair ticket");
        assert!(matches!(
            failed.state,
            RepairTaskStateV1::Failed(FailedRepairStateV1 { .. })
        ));
    }

    #[test]
    fn node_handle_watchdog_publishes_audit_and_slash() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let repair_actual = iroha_config::parameters::actual::SorafsRepair {
            enabled: true,
            default_slash_penalty: "0.000008".parse().expect("valid quantity"),
            ..Default::default()
        };
        let handle = NodeHandle::new_with_policies(
            cfg,
            RepairConfig::from(&repair_actual),
            GcConfig::default(),
        );

        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);

        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-460".into()),
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            submitted_at_unix: 1_700_500_000,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest: [0x44; 32],
                provider_id: [0x77; 32],
                por_history_id: None,
                cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                    reason: "manual".into(),
                }),
                evidence_json: None,
                notes: None,
            },
            notes: None,
        };
        handle
            .enqueue_repair_report(&report)
            .expect("queue repair report");
        let _ = publisher.take();

        let now_unix = report.submitted_at_unix + 86_400;
        let outcome = handle
            .run_repair_watchdog_once(now_unix)
            .expect("watchdog run");
        assert_eq!(outcome.escalated.len(), 1);

        let payloads = publisher.take();
        let mut audits = Vec::new();
        let mut slashes = Vec::new();
        for payload in payloads {
            if let Ok(event) = norito::decode_from_bytes::<RepairAuditEventV1>(&payload) {
                audits.push(event);
                continue;
            }
            if let Ok(proposal) = norito::decode_from_bytes::<RepairSlashProposalV1>(&payload) {
                slashes.push(proposal);
            }
        }

        assert!(audits.iter().any(|event| {
            event.payload.ticket_id == report.ticket_id
                && event.payload.status == RepairTaskStatusV1::Escalated
        }));
        assert!(
            slashes
                .iter()
                .any(|proposal| proposal.ticket_id == report.ticket_id)
        );
    }

    #[test]
    fn node_handle_repair_worker_completes_and_escalates() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let repair_actual = iroha_config::parameters::actual::SorafsRepair {
            enabled: true,
            max_attempts: 1,
            default_slash_penalty: "0.000009".parse().expect("valid quantity"),
            ..Default::default()
        };
        let handle = NodeHandle::new_with_policies(
            cfg,
            RepairConfig::from(&repair_actual),
            GcConfig::default(),
        );

        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);

        let payload = b"repair-worker-fixture";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xEA; 16])
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
        handle
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest manifest");
        let manifest_digest: [u8; 32] = manifest.digest().expect("digest").into();
        let mut missing_digest = manifest_digest;
        missing_digest[0] ^= 0xFF;

        let report_complete = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-470".into()),
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            submitted_at_unix: 1_700_600_000,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest,
                provider_id: [0x01; 32],
                por_history_id: None,
                cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                    reason: "manual".into(),
                }),
                evidence_json: None,
                notes: None,
            },
            notes: None,
        };
        let report_missing = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-471".into()),
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            submitted_at_unix: 1_700_600_100,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest: missing_digest,
                provider_id: [0x02; 32],
                por_history_id: None,
                cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                    reason: "manual".into(),
                }),
                evidence_json: None,
                notes: None,
            },
            notes: None,
        };

        handle
            .enqueue_repair_report(&report_complete)
            .expect("enqueue repair report");
        handle
            .enqueue_repair_report(&report_missing)
            .expect("enqueue repair report");
        let _ = publisher.take();

        let now_unix = report_complete.submitted_at_unix + 120;
        let report = handle.run_repair_worker_once("worker-1", now_unix);
        assert_eq!(report.claimed, 1);
        let report = handle.run_repair_worker_once("worker-2", now_unix + 60);
        assert_eq!(report.claimed, 1);

        let completed = handle
            .repair_task_record(&report_complete.ticket_id)
            .expect("repair task store")
            .expect("completed task");
        assert!(matches!(
            completed.state,
            RepairTaskStateV1::Completed(CompletedRepairStateV1 { .. })
        ));
        let escalated = handle
            .repair_task_record(&report_missing.ticket_id)
            .expect("repair task store")
            .expect("escalated task");
        assert!(matches!(
            escalated.state,
            RepairTaskStateV1::Escalated(EscalatedRepairStateV1 { .. })
        ));

        let payloads = publisher.take();
        let mut audits = Vec::new();
        let mut slashes = Vec::new();
        for payload in payloads {
            if let Ok(event) = norito::decode_from_bytes::<RepairAuditEventV1>(&payload) {
                audits.push(event);
                continue;
            }
            if let Ok(proposal) = norito::decode_from_bytes::<RepairSlashProposalV1>(&payload) {
                slashes.push(proposal);
            }
        }

        assert!(audits.iter().any(|event| {
            event.payload.ticket_id == report_complete.ticket_id
                && event.payload.status == RepairTaskStatusV1::Completed
        }));
        assert!(audits.iter().any(|event| {
            event.payload.ticket_id == report_missing.ticket_id
                && event.payload.status == RepairTaskStatusV1::Escalated
        }));
        assert!(
            slashes
                .iter()
                .any(|proposal| proposal.ticket_id == report_missing.ticket_id)
        );
    }

    #[test]
    fn node_handle_repair_worker_rehydrates_missing_chunks_from_local_replicas() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let repair_actual = iroha_config::parameters::actual::SorafsRepair {
            enabled: true,
            max_attempts: 1,
            ..Default::default()
        };
        let handle = NodeHandle::new_with_policies(
            cfg,
            RepairConfig::from(&repair_actual),
            GcConfig::default(),
        );

        let payload = b"repair-worker-rehydrate";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let manifest_a = ManifestBuilder::new()
            .root_cid(vec![0xA1; 16])
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
        let manifest_b = ManifestBuilder::new()
            .root_cid(vec![0xB2; 16])
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
        handle
            .ingest_manifest(&manifest_a, &plan, &mut reader)
            .expect("ingest manifest a");
        let mut reader = payload.as_slice();
        handle
            .ingest_manifest(&manifest_b, &plan, &mut reader)
            .expect("ingest manifest b");

        let digest_a: [u8; 32] = manifest_a.digest().expect("digest").into();
        let digest_b: [u8; 32] = manifest_b.digest().expect("digest").into();

        let stored_a = handle
            .manifest_metadata_by_digest(&digest_a)
            .expect("stored manifest a");
        let stored_b = handle
            .manifest_metadata_by_digest(&digest_b)
            .expect("stored manifest b");

        let missing_chunk = stored_a.chunk(0).expect("chunk").clone();
        let source_chunk = stored_b.chunk(0).expect("chunk").clone();
        assert_eq!(missing_chunk.digest, source_chunk.digest);

        std::fs::remove_file(&missing_chunk.path).expect("remove missing chunk");
        assert!(!missing_chunk.path.exists());

        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-472".into()),
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            submitted_at_unix: 1_700_700_000,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest: digest_a,
                provider_id: [0x03; 32],
                por_history_id: None,
                cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                    reason: "local-rehydrate".into(),
                }),
                evidence_json: None,
                notes: None,
            },
            notes: None,
        };

        handle
            .enqueue_repair_report(&report)
            .expect("enqueue repair report");

        let now_unix = report.submitted_at_unix + 120;
        let worker_report = handle.run_repair_worker_once("worker-1", now_unix);
        assert_eq!(worker_report.claimed, 1);

        let task = handle
            .repair_task_record(&report.ticket_id)
            .expect("repair task store")
            .expect("repair task");
        assert!(matches!(
            task.state,
            RepairTaskStateV1::Completed(CompletedRepairStateV1 { .. })
        ));

        let bytes = std::fs::read(&missing_chunk.path).expect("rehydrated bytes");
        assert_eq!(bytes.len(), missing_chunk.length as usize);
        assert_eq!(blake3::hash(&bytes).as_bytes(), &missing_chunk.digest);
    }

    #[test]
    fn node_handle_repair_worker_rehydrates_missing_chunks_from_orchestrator() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let repair_actual = iroha_config::parameters::actual::SorafsRepair {
            enabled: true,
            max_attempts: 1,
            ..Default::default()
        };
        let handle = NodeHandle::new_with_policies(
            cfg,
            RepairConfig::from(&repair_actual),
            GcConfig::default(),
        );

        let payload = b"repair-worker-orchestrator";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let manifest = ManifestBuilder::new()
            .root_cid(vec![0xC3; 16])
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
        handle
            .ingest_manifest(&manifest, &plan, &mut reader)
            .expect("ingest manifest");

        let digest: [u8; 32] = manifest.digest().expect("digest").into();
        let stored = handle
            .manifest_metadata_by_digest(&digest)
            .expect("stored manifest");
        let missing_chunk = stored.chunk(0).expect("chunk").clone();
        std::fs::remove_file(&missing_chunk.path).expect("remove missing chunk");
        assert!(!missing_chunk.path.exists());

        let start = missing_chunk.offset as usize;
        let end = start + missing_chunk.length as usize;
        let chunk_bytes = payload[start..end].to_vec();
        let payloads = vec![RepairChunkPayload {
            digest: missing_chunk.digest,
            bytes: chunk_bytes,
            source: Some("orchestrator#test".into()),
        }];
        let calls = Arc::new(AtomicUsize::new(0));
        let orchestrator = Arc::new(StaticRepairOrchestrator {
            payloads,
            calls: calls.clone(),
        });
        handle.set_repair_orchestrator(orchestrator);

        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-473".into()),
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            submitted_at_unix: 1_700_700_300,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest: digest,
                provider_id: [0x04; 32],
                por_history_id: None,
                cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                    reason: "orchestrator-rehydrate".into(),
                }),
                evidence_json: None,
                notes: None,
            },
            notes: None,
        };

        handle
            .enqueue_repair_report(&report)
            .expect("enqueue repair report");

        let now_unix = report.submitted_at_unix + 120;
        let worker_report = handle.run_repair_worker_once("worker-1", now_unix);
        assert_eq!(worker_report.claimed, 1);
        assert_eq!(calls.load(Ordering::Relaxed), 1);

        let task = handle
            .repair_task_record(&report.ticket_id)
            .expect("repair task store")
            .expect("repair task");
        assert!(matches!(
            task.state,
            RepairTaskStateV1::Completed(CompletedRepairStateV1 { .. })
        ));

        let bytes = std::fs::read(&missing_chunk.path).expect("rehydrated bytes");
        assert_eq!(bytes.len(), missing_chunk.length as usize);
        assert_eq!(blake3::hash(&bytes).as_bytes(), &missing_chunk.digest);
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
                stake_amount: 1,
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
        let manifest = ManifestBuilder::new()
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

        let report = handle.run_gc_once(now_unix);
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
    fn node_handle_reconciliation_emits_report() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

        let publisher = Arc::new(RecordingPublisher::default());
        let trait_publisher: Arc<dyn GovernancePublisher> = publisher.clone();
        handle.set_governance_publisher(trait_publisher);

        let declaration = CapacityDeclarationV1 {
            version: CAPACITY_DECLARATION_VERSION_V1,
            provider_id: [0x11; 32],
            stake: sorafs_manifest::provider_advert::StakePointer {
                pool_id: [0x22; 32],
                stake_amount: 1,
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
        let manifest = ManifestBuilder::new()
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

        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-601".into()),
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            submitted_at_unix: 1_700_000_100,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest,
                provider_id: declaration.provider_id,
                por_history_id: None,
                cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                    reason: "reconciliation".into(),
                }),
                evidence_json: None,
                notes: None,
            },
            notes: None,
        };
        handle
            .enqueue_repair_report(&report)
            .expect("queue repair report");
        publisher.take();

        let now_unix = 1_700_000_200;
        let reconciliation = handle
            .run_reconciliation_once(now_unix)
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
            .run_reconciliation_once(now_unix)
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
        let handle = NodeHandle::new(cfg);
        assert!(handle.has_governance_publisher());

        let rollup = appeal_finance_weekly_rollup_fixture();
        handle
            .publish_appeal_finance_weekly_rollup(rollup.clone())
            .expect("publish appeal finance weekly rollup");

        let reconciliation = handle
            .run_reconciliation_once(1_700_000_300)
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
    fn appeal_finance_decimal_addition_normalizes_scale() {
        let sum = add_appeal_finance_decimal_strings("420", "80.2500").expect("decimal sum");

        assert_eq!(sum, "500.25");
    }

    #[test]
    fn node_handle_gc_skips_manifest_with_active_repair_task() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let repair_actual = iroha_config::parameters::actual::SorafsRepair {
            enabled: true,
            ..Default::default()
        };
        let gc_actual = iroha_config::parameters::actual::SorafsGc {
            enabled: true,
            retention_grace_secs: 0,
            max_deletions_per_run: 10,
            ..Default::default()
        };
        let handle = NodeHandle::new_with_policies(
            cfg,
            RepairConfig::from(&repair_actual),
            GcConfig::from(&gc_actual),
        );

        let payload = b"gc-repair-blocked";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let retention_epoch = 1_700_000_000;
        let now_unix = retention_epoch + 10;
        let mut policy = PinPolicy::default();
        policy.retention_epoch = retention_epoch;
        let manifest = ManifestBuilder::new()
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

        let report = RepairReportV1 {
            version: REPAIR_REPORT_VERSION_V1,
            ticket_id: RepairTicketId("REP-GC-001".into()),
            auditor_account: "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB".into(),
            submitted_at_unix: retention_epoch,
            evidence: RepairEvidenceV1 {
                version: REPAIR_EVIDENCE_VERSION_V1,
                manifest_digest,
                provider_id: [0x44; 32],
                por_history_id: None,
                cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                    reason: "missing shard".into(),
                }),
                evidence_json: None,
                notes: None,
            },
            notes: None,
        };
        handle
            .enqueue_repair_report(&report)
            .expect("enqueue report");

        let report = handle.run_gc_once(now_unix);
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

        let manifest_a = ManifestBuilder::new()
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
        let manifest_b = ManifestBuilder::new()
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

        let report = handle.run_gc_once(now_unix);
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
    fn node_handle_gc_capacity_prefers_least_recently_used_expired() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let gc_actual = iroha_config::parameters::actual::SorafsGc {
            enabled: true,
            retention_grace_secs: 0,
            max_deletions_per_run: 1,
            ..Default::default()
        };
        let handle =
            NodeHandle::new_with_policies(cfg, RepairConfig::default(), GcConfig::from(&gc_actual));

        let retention_epoch = 1_700_000_000;
        let now_unix = retention_epoch + 10;

        let payload_a = b"lru-expired-a";
        let plan_a = CarBuildPlan::single_file(payload_a).expect("plan");
        let mut policy_a = PinPolicy::default();
        policy_a.retention_epoch = retention_epoch;
        let manifest_a = ManifestBuilder::new()
            .root_cid(vec![0x01; 8])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan_a.content_length)
            .car_digest(blake3::hash(payload_a).into())
            .car_size(plan_a.content_length)
            .pin_policy(policy_a)
            .build()
            .expect("manifest");

        let mut reader_a = payload_a.as_slice();
        let manifest_id_a = handle
            .ingest_manifest(&manifest_a, &plan_a, &mut reader_a)
            .expect("ingest a");

        let payload_b = b"lru-expired-b";
        let plan_b = CarBuildPlan::single_file(payload_b).expect("plan");
        let mut policy_b = PinPolicy::default();
        policy_b.retention_epoch = retention_epoch;
        let manifest_b = ManifestBuilder::new()
            .root_cid(vec![0x02; 8])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(plan_b.content_length)
            .car_digest(blake3::hash(payload_b).into())
            .car_size(plan_b.content_length)
            .pin_policy(policy_b)
            .build()
            .expect("manifest");

        let mut reader_b = payload_b.as_slice();
        let manifest_id_b = handle
            .ingest_manifest(&manifest_b, &plan_b, &mut reader_b)
            .expect("ingest b");

        let _ = handle
            .read_payload_range(&manifest_id_a, 0, 4)
            .expect("read a");

        let report = handle.run_gc_for_capacity(now_unix, plan_a.content_length);
        assert_eq!(report.evictions.len(), 1);
        assert_eq!(report.evictions[0].manifest_id, manifest_id_b);
        assert!(handle.manifest_metadata(&manifest_id_a).is_ok());
        assert!(handle.manifest_metadata(&manifest_id_b).is_err());
    }

    #[test]
    fn pre_admission_sweep_allows_ingest() {
        let (mut cfg, _dir) = storage_config_with_temp_dir();
        cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(cfg.data_dir().clone())
            .max_capacity_bytes(iroha_config::base::util::Bytes(32))
            .build();

        let gc_actual = iroha_config::parameters::actual::SorafsGc {
            enabled: true,
            pre_admission_sweep: true,
            retention_grace_secs: 0,
            max_deletions_per_run: 1,
            ..Default::default()
        };
        let handle =
            NodeHandle::new_with_policies(cfg, RepairConfig::default(), GcConfig::from(&gc_actual));

        let expired_payload = vec![0xAA; 16];
        let expired_plan = CarBuildPlan::single_file(&expired_payload).expect("plan");
        let mut expired_policy = PinPolicy::default();
        expired_policy.retention_epoch = unix_now_secs().saturating_sub(10);
        let expired_manifest = ManifestBuilder::new()
            .root_cid(vec![0x33; 8])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(expired_plan.content_length)
            .car_digest(blake3::hash(&expired_payload).into())
            .car_size(expired_plan.content_length)
            .pin_policy(expired_policy)
            .build()
            .expect("manifest");
        let mut expired_reader = expired_payload.as_slice();
        let expired_id = handle
            .ingest_manifest(&expired_manifest, &expired_plan, &mut expired_reader)
            .expect("ingest expired");

        let new_payload = vec![0xBB; 24];
        let new_plan = CarBuildPlan::single_file(&new_payload).expect("plan");
        let new_manifest = ManifestBuilder::new()
            .root_cid(vec![0x44; 8])
            .dag_codec(DagCodecId(0x71))
            .chunking_from_profile(
                sorafs_chunker::ChunkProfile::DEFAULT,
                sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
            )
            .content_length(new_plan.content_length)
            .car_digest(blake3::hash(&new_payload).into())
            .car_size(new_plan.content_length)
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest");
        let mut new_reader = new_payload.as_slice();
        let new_id = handle
            .ingest_manifest(&new_manifest, &new_plan, &mut new_reader)
            .expect("ingest new");

        assert!(handle.manifest_metadata(&expired_id).is_err());
        assert!(handle.manifest_metadata(&new_id).is_ok());
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
            default_slash_penalty: "0.000042".parse().expect("valid quantity"),
            ..Default::default()
        };

        let actual_gc = iroha_config::parameters::actual::SorafsGc {
            enabled: true,
            interval_secs: 300,
            max_deletions_per_run: 2_000,
            retention_grace_secs: 86_400,
            pre_admission_sweep: false,
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
        assert_eq!(
            handle.repair_config().default_slash_penalty(),
            &"0.000042".parse().expect("valid quantity")
        );

        assert!(handle.gc_config().enabled());
        assert_eq!(handle.gc_config().interval_secs(), 300);
        assert_eq!(handle.gc_config().max_deletions_per_run(), 2_000);
        assert_eq!(handle.gc_config().retention_grace_secs(), 86_400);
        assert!(!handle.gc_config().pre_admission_sweep());

        let manager = handle.repair_manager();
        assert_eq!(manager.claim_ttl_secs(), repair_cfg.claim_ttl_secs());
        assert_eq!(
            manager.heartbeat_interval_secs(),
            repair_cfg.heartbeat_interval_secs()
        );
    }

    #[test]
    fn node_handle_records_reserve_lifecycle_summary_and_events() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let mut live_events = handle.subscribe_reserve_lifecycle_events();

        let first = handle
            .record_reserve_lifecycle_update(reserve_lifecycle_update(
                0x31,
                0,
                XorQuantity::zero(),
                1_800_000_100,
            ))
            .expect("record warning");
        assert_eq!(first.sequence, 0);
        assert_eq!(first.previous_stage, None);
        assert_eq!(first.current_stage, ReserveLifecycleStage::Warning);

        let second = handle
            .record_reserve_lifecycle_update(reserve_lifecycle_update(
                0x31,
                3,
                XorQuantity::zero(),
                1_800_000_200,
            ))
            .expect("record grace");
        assert_eq!(second.sequence, 1);
        assert_eq!(second.previous_stage, Some(ReserveLifecycleStage::Warning));
        assert_eq!(second.current_stage, ReserveLifecycleStage::Grace);
        assert_eq!(handle.latest_reserve_lifecycle_event_sequence(), Some(1));

        let summary = handle
            .reserve_provider_lifecycle_summary([0x31; 32])
            .expect("summary");
        assert_eq!(summary.lifecycle.stage, ReserveLifecycleStage::Grace);
        assert_eq!(
            summary
                .ledger
                .rent_due
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            120_000_000
        );
        assert_eq!(summary.updated_at_unix, 1_800_000_200);

        let replay = handle.reserve_lifecycle_events_since(Some(0), 10);
        assert_eq!(replay, vec![second.clone()]);
        assert_eq!(live_events.try_recv().expect("first live event"), first);
        assert_eq!(live_events.try_recv().expect("second live event"), second);
    }

    #[test]
    fn node_handle_advances_reserve_lifecycle_by_time() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let mut live_events = handle.subscribe_reserve_lifecycle_events();
        let initial = handle
            .record_reserve_lifecycle_update(reserve_lifecycle_update(
                0x32,
                29,
                XorQuantity::zero(),
                1_800_000_100,
            ))
            .expect("record lifecycle");
        assert_eq!(initial.current_stage, ReserveLifecycleStage::Delinquent);
        assert_eq!(handle.transparency_ledger_source_entry_count(), 1);

        let advanced = handle
            .advance_reserve_lifecycle(1_800_000_100 + 2 * 86_400)
            .expect("advance lifecycle");

        assert_eq!(advanced.len(), 1);
        assert_eq!(advanced[0].sequence, 1);
        assert_eq!(advanced[0].current_stage, ReserveLifecycleStage::Default);
        assert_eq!(advanced[0].lifecycle.days_past_due, 31);
        assert_eq!(handle.latest_reserve_lifecycle_event_sequence(), Some(1));
        let summary = handle
            .reserve_provider_lifecycle_summary([0x32; 32])
            .expect("summary");
        assert_eq!(summary.lifecycle.stage, ReserveLifecycleStage::Default);
        let credit_line = handle
            .reserve_provider_credit_line([0x32; 32])
            .expect("credit line");
        assert_eq!(credit_line.stage, ReserveLifecycleStage::Default);
        assert_eq!(credit_line.lifecycle_event_sequence, 1);
        assert_eq!(handle.transparency_ledger_source_entry_count(), 2);
        assert_eq!(live_events.try_recv().expect("initial live event"), initial);
        assert_eq!(
            live_events.try_recv().expect("advanced live event"),
            advanced[0]
        );

        let noop = handle
            .advance_reserve_lifecycle(1_800_000_100 + 2 * 86_400 + 1)
            .expect("noop advance");
        assert!(noop.is_empty());
        assert_eq!(handle.transparency_ledger_source_entry_count(), 2);
    }

    #[test]
    fn node_handle_reserve_lifecycle_snapshot_sorts_providers() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        handle
            .record_reserve_lifecycle_update(reserve_lifecycle_update(
                0x42,
                0,
                XorQuantity::zero(),
                1_800_000_100,
            ))
            .expect("record provider b");
        handle
            .record_reserve_lifecycle_update(reserve_lifecycle_update(
                0x11,
                0,
                XorQuantity::zero(),
                1_800_000_101,
            ))
            .expect("record provider a");

        let snapshot = handle.reserve_lifecycle_snapshot(1_800_000_200);
        assert_eq!(snapshot.generated_at_unix, 1_800_000_200);
        assert_eq!(snapshot.next_sequence, 2);
        assert_eq!(snapshot.providers.len(), 2);
        assert_eq!(snapshot.providers[0].provider_id, [0x11; 32]);
        assert_eq!(snapshot.providers[1].provider_id, [0x42; 32]);
        assert_eq!(snapshot.events.len(), 2);
        assert!(handle.reserve_lifecycle_events_since(None, 0).is_empty());
    }

    #[test]
    fn node_handle_records_reserve_credit_line_state() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        handle
            .record_reserve_lifecycle_update(reserve_lifecycle_update(
                0x43,
                10,
                XorQuantity::zero(),
                1_800_000_150,
            ))
            .expect("record delinquent provider");

        let credit_line = handle
            .reserve_provider_credit_line([0x43; 32])
            .expect("credit-line state");
        assert_eq!(credit_line.stage, ReserveLifecycleStage::Delinquent);
        assert_eq!(
            credit_line
                .credit_draw
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            120_000_000
        );
        assert_eq!(
            credit_line
                .accrued_interest
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            29_589
        );
        assert_eq!(credit_line.lifecycle_event_sequence, 0);
        let snapshot = handle.reserve_credit_line_snapshot(1_800_000_250);
        assert_eq!(snapshot.generated_at_unix, 1_800_000_250);
        assert_eq!(snapshot.credit_lines, vec![credit_line]);
    }

    #[test]
    fn node_handle_applies_accepted_reserve_appeal_decision_to_lifecycle_state() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let mut live_events = handle.subscribe_reserve_lifecycle_events();
        handle
            .record_reserve_lifecycle_update(reserve_lifecycle_update(
                0x44,
                31,
                XorQuantity::zero(),
                1_800_000_150,
            ))
            .expect("record defaulted provider");
        handle
            .record_reserve_appeal(reserve_appeal_request(0x47, 0x44))
            .expect("record reserve appeal");

        let decided = handle
            .record_reserve_appeal_decision(reserve_appeal_decision(
                0x47,
                ReserveAppealStatus::Accepted,
            ))
            .expect("accept reserve appeal");
        assert_eq!(decided.status, ReserveAppealStatus::Accepted);

        let initial = live_events.try_recv().expect("initial lifecycle event");
        assert_eq!(initial.current_stage, ReserveLifecycleStage::Default);
        let override_event = live_events
            .try_recv()
            .expect("appeal lifecycle override event");
        assert_eq!(
            override_event.previous_stage,
            Some(ReserveLifecycleStage::Default)
        );
        assert_eq!(override_event.current_stage, ReserveLifecycleStage::Grace);
        assert_eq!(override_event.applied_appeal_id, Some([0x47; 32]));

        let summary = handle
            .reserve_provider_lifecycle_summary([0x44; 32])
            .expect("provider summary");
        assert_eq!(summary.lifecycle.stage, ReserveLifecycleStage::Grace);
        assert_eq!(summary.applied_appeal_id, Some([0x47; 32]));
        assert!(!summary.lifecycle.disable_adverts);

        let credit_line = handle
            .reserve_provider_credit_line([0x44; 32])
            .expect("credit-line state");
        assert_eq!(credit_line.stage, ReserveLifecycleStage::Grace);
        assert_eq!(credit_line.applied_appeal_id, Some([0x47; 32]));
    }

    #[test]
    fn node_handle_records_reserve_appeals_and_decisions() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);

        let appeal = handle
            .record_reserve_appeal(reserve_appeal_request(0x48, 0x33))
            .expect("record reserve appeal");
        assert!(!appeal.duplicate);
        assert_eq!(appeal.record.sequence, 0);
        assert_eq!(appeal.record.status, ReserveAppealStatus::Open);

        let duplicate = handle
            .record_reserve_appeal(reserve_appeal_request(0x48, 0x33))
            .expect("replay reserve appeal");
        assert!(duplicate.duplicate);
        assert_eq!(duplicate.record, appeal.record);

        let decided = handle
            .record_reserve_appeal_decision(reserve_appeal_decision(
                0x48,
                ReserveAppealStatus::Accepted,
            ))
            .expect("decide reserve appeal");
        assert_eq!(decided.status, ReserveAppealStatus::Accepted);
        assert_eq!(handle.reserve_appeal([0x48; 32]), Some(decided.clone()));

        let snapshot = handle.reserve_appeal_snapshot(2_500_000_000);
        assert_eq!(snapshot.generated_at_unix, 2_500_000_000);
        assert_eq!(snapshot.next_sequence, 1);
        assert_eq!(snapshot.appeals, vec![decided]);
    }

    #[test]
    fn node_handle_records_reserve_lifecycle_policy_updates() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);

        let first = handle
            .record_reserve_lifecycle_policy_update(reserve_lifecycle_policy_update(0x49))
            .expect("record lifecycle policy update");
        assert!(!first.duplicate);
        assert_eq!(first.record.sequence, 0);
        assert_eq!(first.record.grace_period_days, 7);
        assert_eq!(first.record.default_after_days, 30);

        let duplicate = handle
            .record_reserve_lifecycle_policy_update(reserve_lifecycle_policy_update(0x49))
            .expect("replay lifecycle policy update");
        assert!(duplicate.duplicate);
        assert_eq!(duplicate.record, first.record);

        let second = handle
            .record_reserve_lifecycle_policy_update(ReserveLifecyclePolicyUpdate {
                grace_period_days: 10,
                default_after_days: 45,
                ..reserve_lifecycle_policy_update(0x4A)
            })
            .expect("record second lifecycle policy update");
        assert_eq!(
            handle.latest_reserve_lifecycle_policy(),
            Some(second.record.clone())
        );

        let snapshot = handle.reserve_lifecycle_policy_snapshot(2_600_000_000);
        assert_eq!(snapshot.generated_at_unix, 2_600_000_000);
        assert_eq!(snapshot.next_sequence, 2);
        assert_eq!(snapshot.latest, Some(second.record));
        assert_eq!(snapshot.policies.len(), 2);
    }

    #[test]
    fn node_handle_records_reserve_governance_source_entries() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);

        handle
            .record_reserve_lifecycle_update(reserve_lifecycle_update(
                0x61,
                3,
                XorQuantity::zero(),
                1_800_000_100,
            ))
            .expect("record lifecycle");
        assert_eq!(handle.transparency_ledger_source_entry_count(), 1);

        let movement = reserve_movement_request(
            0x62,
            0x61,
            ReserveMovementKind::TopUp,
            XorQuantity::try_from_micro(100).expect("legacy micro-XOR value is representable"),
        );
        handle
            .record_reserve_movement(movement.clone())
            .expect("record movement");
        assert_eq!(handle.transparency_ledger_source_entry_count(), 2);
        handle
            .record_reserve_movement(movement)
            .expect("replay movement");
        assert_eq!(handle.transparency_ledger_source_entry_count(), 2);

        let custody =
            reserve_movement_custody_update(0x62, ReserveMovementCustodyStatus::Submitted, 0x63);
        handle
            .record_reserve_movement_custody_update(custody.clone())
            .expect("record custody");
        assert_eq!(handle.transparency_ledger_source_entry_count(), 3);
        handle
            .record_reserve_movement_custody_update(custody)
            .expect("replay custody");
        assert_eq!(handle.transparency_ledger_source_entry_count(), 3);

        let appeal = reserve_appeal_request(0x64, 0x61);
        handle
            .record_reserve_appeal(appeal.clone())
            .expect("record appeal");
        assert_eq!(handle.transparency_ledger_source_entry_count(), 4);
        handle.record_reserve_appeal(appeal).expect("replay appeal");
        assert_eq!(handle.transparency_ledger_source_entry_count(), 4);

        let decision = reserve_appeal_decision(0x64, ReserveAppealStatus::Accepted);
        handle
            .record_reserve_appeal_decision(decision.clone())
            .expect("record appeal decision");
        assert_eq!(handle.transparency_ledger_source_entry_count(), 6);
        handle
            .record_reserve_appeal_decision(decision)
            .expect("replay appeal decision");
        assert_eq!(handle.transparency_ledger_source_entry_count(), 6);

        let policy = reserve_lifecycle_policy_update(0x65);
        handle
            .record_reserve_lifecycle_policy_update(policy.clone())
            .expect("record lifecycle policy");
        assert_eq!(handle.transparency_ledger_source_entry_count(), 7);
        handle
            .record_reserve_lifecycle_policy_update(policy)
            .expect("replay lifecycle policy");
        assert_eq!(handle.transparency_ledger_source_entry_count(), 7);
    }

    #[test]
    fn node_handle_records_reserve_movements_and_balances() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let mut live_events = handle.subscribe_reserve_movement_events();

        let top_up = handle
            .record_reserve_movement(reserve_movement_request(
                0x51,
                0x31,
                ReserveMovementKind::TopUp,
                XorQuantity::try_from_micro(100).expect("legacy micro-XOR value is representable"),
            ))
            .expect("record top-up");
        assert!(!top_up.duplicate);
        assert_eq!(top_up.record.sequence, 0);
        assert_eq!(
            top_up
                .record
                .balance_after
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            100
        );
        assert!(top_up.record.confirmed_balance_after.is_zero());

        let duplicate = handle
            .record_reserve_movement(reserve_movement_request(
                0x51,
                0x31,
                ReserveMovementKind::TopUp,
                XorQuantity::try_from_micro(100).expect("legacy micro-XOR value is representable"),
            ))
            .expect("record duplicate");
        assert!(duplicate.duplicate);
        assert_eq!(duplicate.record, top_up.record);

        let withdrawal = handle
            .record_reserve_movement(reserve_movement_request(
                0x52,
                0x31,
                ReserveMovementKind::Withdrawal,
                XorQuantity::try_from_micro(40).expect("legacy micro-XOR value is representable"),
            ))
            .expect("record withdrawal");
        assert_eq!(withdrawal.record.sequence, 1);
        assert_eq!(
            withdrawal
                .record
                .balance_after
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            60
        );
        assert!(withdrawal.record.confirmed_balance_after.is_zero());
        assert_eq!(handle.latest_reserve_movement_sequence(), Some(1));

        let balance = handle
            .reserve_provider_balance([0x31; 32])
            .expect("provider balance");
        assert_eq!(
            balance
                .balance
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            60
        );
        assert!(balance.confirmed_balance.is_zero());
        assert_eq!(balance.updated_at_unix, 1_800_000_000 + 0x52);

        let replay = handle.reserve_movements_since(Some(0), 10);
        assert_eq!(replay, vec![withdrawal.record.clone()]);
        assert_eq!(
            live_events.try_recv().expect("top-up live event"),
            top_up.record
        );
        assert_eq!(
            live_events.try_recv().expect("withdrawal live event"),
            withdrawal.record
        );

        let snapshot = handle.reserve_movement_snapshot(1_800_000_300);
        assert_eq!(snapshot.generated_at_unix, 1_800_000_300);
        assert_eq!(snapshot.next_sequence, 2);
        assert_eq!(snapshot.provider_balances.len(), 1);
        assert_eq!(snapshot.movements.len(), 2);
        assert!(handle.reserve_movements_since(None, 0).is_empty());
    }

    #[test]
    fn node_handle_filters_visible_reserve_movements_before_limit() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let visible_account = b"visible-provider".to_vec();

        for movement_byte in [0x70, 0x71] {
            handle
                .record_reserve_movement(reserve_movement_request(
                    movement_byte,
                    0x61,
                    ReserveMovementKind::TopUp,
                    XorQuantity::try_from_micro(1)
                        .expect("legacy micro-XOR value is representable"),
                ))
                .expect("record invisible movement");
        }
        let mut visible = reserve_movement_request(
            0x72,
            0x62,
            ReserveMovementKind::TopUp,
            XorQuantity::try_from_micro(1).expect("legacy micro-XOR value is representable"),
        );
        visible.provider_account = visible_account.clone();
        let visible = handle
            .record_reserve_movement(visible)
            .expect("record visible movement");

        let page = handle.reserve_movements_since_visible_to(None, 1, &visible_account);

        assert_eq!(page, vec![visible.record]);
    }

    #[test]
    fn node_handle_records_reserve_movement_custody_updates() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let mut live_events = handle.subscribe_reserve_movement_events();

        let movement = handle
            .record_reserve_movement(reserve_movement_request(
                0x54,
                0x31,
                ReserveMovementKind::TopUp,
                XorQuantity::try_from_micro(100).expect("legacy micro-XOR value is representable"),
            ))
            .expect("record movement");
        assert_eq!(
            live_events.try_recv().expect("movement event"),
            movement.record
        );

        let submitted = handle
            .record_reserve_movement_custody_update(reserve_movement_custody_update(
                0x54,
                ReserveMovementCustodyStatus::Submitted,
                0xAB,
            ))
            .expect("record custody status");
        assert_eq!(
            submitted.custody_status,
            ReserveMovementCustodyStatus::Submitted
        );
        assert!(submitted.confirmed_balance_after.is_zero());
        assert_eq!(handle.reserve_movement([0x54; 32]), Some(submitted.clone()));
        assert_eq!(
            live_events.try_recv().expect("custody event"),
            submitted.clone()
        );

        let confirmed = handle
            .record_reserve_movement_custody_update(reserve_movement_custody_update(
                0x54,
                ReserveMovementCustodyStatus::Confirmed,
                0xAB,
            ))
            .expect("record confirmed custody status");
        assert_eq!(
            confirmed.custody_status,
            ReserveMovementCustodyStatus::Confirmed
        );
        assert_eq!(
            confirmed
                .confirmed_balance_after
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            100
        );
        assert_eq!(
            handle.reserve_movement_snapshot(1).movements[0].custody_status,
            ReserveMovementCustodyStatus::Confirmed
        );
        assert_eq!(
            handle
                .reserve_provider_balance([0x31; 32])
                .expect("provider balance")
                .confirmed_balance
                .try_to_micro()
                .expect("XOR quantity has exact legacy micro representation"),
            100
        );
    }

    #[test]
    fn node_handle_rejects_reserve_withdrawal_underflow() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);

        let err = handle
            .record_reserve_movement(reserve_movement_request(
                0x53,
                0x31,
                ReserveMovementKind::Withdrawal,
                XorQuantity::try_from_micro(1).expect("legacy micro-XOR value is representable"),
            ))
            .expect_err("withdrawal should fail");

        assert!(matches!(
            err,
            ReserveMovementRuntimeError::InsufficientBalance { .. }
        ));
        assert!(handle.reserve_provider_balance([0x31; 32]).is_none());
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
                stake_amount: 1,
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
                stake_amount: 1,
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
                stake_amount: 1,
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
                stake_amount: 1,
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
        let manifest = ManifestBuilder::new()
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
        let manifest = ManifestBuilder::new()
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
                stake_amount: 1,
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
        let manifest = ManifestBuilder::new()
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
                stake_amount: 1,
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
        let manifest = ManifestBuilder::new()
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
    fn node_handle_storage_methods_error_when_disabled() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);

        let payload = b"disabled storage payload";
        let plan = CarBuildPlan::single_file(payload).expect("plan");
        let manifest = ManifestBuilder::new()
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
    struct StaticRepairOrchestrator {
        payloads: Vec<RepairChunkPayload>,
        calls: Arc<AtomicUsize>,
    }

    impl RepairOrchestrator for StaticRepairOrchestrator {
        fn rehydrate_missing_chunks(
            &self,
            _task: &RepairTaskRecordV1,
            _manifest: &StoredManifest,
            _missing_chunks: &[ChunkFileRecord],
        ) -> Result<Vec<RepairChunkPayload>, RepairOrchestratorError> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            Ok(self.payloads.clone())
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

        fn publish_repair_audit_event(
            &self,
            _event: &RepairAuditEventV1,
            encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.payloads.lock().expect("publisher lock poisoned");
            guard.push(encoded.to_vec());
            Ok(())
        }

        fn publish_repair_slash_proposal(
            &self,
            _proposal: &RepairSlashProposalV1,
            encoded: &[u8],
            _stage: RepairSlashStage,
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
            _snapshot: &ReputationSnapshotV1,
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

        fn publish_orderbook_settlement_receipt(
            &self,
            _receipt: &SettlementReceiptV1,
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

        fn publish_repair_audit_event(
            &self,
            _event: &RepairAuditEventV1,
            _encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.attempts.lock().expect("publisher lock poisoned");
            *guard += 1;
            Err(GovernancePublishError::other("simulated publish failure"))
        }

        fn publish_repair_slash_proposal(
            &self,
            _proposal: &RepairSlashProposalV1,
            _encoded: &[u8],
            _stage: RepairSlashStage,
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
            _snapshot: &ReputationSnapshotV1,
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

        fn publish_orderbook_settlement_receipt(
            &self,
            _receipt: &SettlementReceiptV1,
            _encoded: &[u8],
        ) -> Result<(), GovernancePublishError> {
            let mut guard = self.attempts.lock().expect("publisher lock poisoned");
            *guard += 1;
            Err(GovernancePublishError::other("simulated publish failure"))
        }
    }
}
