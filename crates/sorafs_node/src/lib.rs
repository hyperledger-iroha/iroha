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
pub mod scheduler;
pub mod store;
pub mod telemetry;

pub use deal::{
    ClientSnapshot, DealEngine, DealEngineError, DealSettlementOutcome, DealSnapshot,
    ProviderSnapshot, UsageOutcome,
};
pub use moderation::{
    ModerationAppealDeposit, ModerationBallotAnnouncement, ModerationBallotCommitOutcome,
    ModerationBallotEvent, ModerationBallotEventKind, ModerationBallotRecord,
    ModerationBallotRevealOutcome, ModerationBallotRuntimeError, ModerationBallotTally,
    ModerationVoteCounts, local_moderation_panel_roster_hash,
};
pub use orderbook::{
    OrderbookCancelOutcome, OrderbookEvent, OrderbookEventKind, OrderbookReceiptOutcome,
    OrderbookRuntimeError, OrderbookSnapshot, OrderbookSubmitOutcome,
    local_orderbook_provider_id_for_owner_account,
};
pub use por::{
    ManifestVrfBundle, PlannedChallenge, PorChallengePlannerError, PorRandomness, PorTracker,
    PorTrackerError, PorVerdictStats, build_por_challenge_for_manifest,
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
    /// Proposed penalty (nano-XOR) to be debited from the provider bond.
    pub penalty_nano: u128,
    /// Failure streak length that triggered the slash.
    pub strikes: u32,
    /// Reason recorded for the recommendation.
    pub reason: String,
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

#[derive(Debug, Clone, Copy)]
enum GcEvictionPolicy {
    RetentionEpoch,
    LruExpired,
}

const GOVERNANCE_PUBLISH_INDEX_FILE: &str = "publish-index.json";
const GOVERNANCE_PUBLISH_INDEX_SCHEMA: &str = "sorafs.governance_dag.local_publish_index.v1";
const APPEAL_FINANCE_WEEKLY_ROLLUP_KIND: &str = "appeal_finance_weekly_rollup";

use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, hash_map::Entry},
    env, fs,
    io::Read,
    path::{Component, Path},
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use capacity::{
    CapacityError, CapacityManager, CapacityUsageSnapshot, DeclarationWindow, ReplicationPlan,
    ReplicationRelease,
};
use config::{GcConfig, RepairConfig, StorageConfig};
use iroha_crypto::Hash;
use iroha_data_model::{
    da::ingest::DaStripeLayout,
    sorafs::{
        capacity::{CapacityDeclarationRecord, ProviderId},
        deal::{ClientId, DealId, DealProposal, DealRecord, DealUsageReport},
        moderation::{
            SoraFsModerationBallotCommitV1, SoraFsModerationBallotRevealV1,
            SoraFsModerationVoteChoice,
        },
    },
};
use iroha_telemetry::metrics::{
    global_or_default, global_sorafs_gc_otel, global_sorafs_node_otel,
    global_sorafs_reconciliation_otel,
};
use norito::codec::Encode;
use norito::json::Value as JsonValue;
pub use repair::{
    RepairManager, RepairSchedulerError, RepairTaskFilters, RepairTaskSnapshot,
    RepairWatchdogReport, RepairWorkerReport,
};
use sorafs_car::{CarBuildPlan, PorProof};
use sorafs_manifest::{
    AppealFinanceReconciliationSummaryV1, ManifestV1, OrderCancelV1, OrderRequestV1, OrderSideV1,
    OrderTierV1, ReconciliationValidationError, ReputationSnapshotEventV1, ReputationSnapshotV1,
    SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1, SORAFS_RECONCILIATION_REPORT_VERSION_V1,
    SettlementChannelStatusV1, SettlementReceiptV1, SoraFsAppealFinanceAccountFlowV1,
    SoraFsAppealFinanceJurorPayoutV1, SoraFsAppealFinanceOutcomeV1, SoraFsAppealFinanceReportV1,
    SoraFsAppealFinanceWeeklyRollupV1, SoraFsModerationBallotGovernanceEventV1,
    SorafsReconciliationReportV1,
    capacity::{CapacityTelemetryV1, ReplicationOrderV1},
    deal::DealSettlementV1,
    por::{AuditOutcomeV1, AuditVerdictV1, PorChallengeV1, PorProofV1},
    potr::{PotrReceiptV1, PotrReceiptValidationError},
    proof_stream::ProofStreamTier,
    repair::{
        GC_AUDIT_EVENT_VERSION_V1, GC_AUDIT_PAYLOAD_VERSION_V1, GcAuditEventV1, GcAuditPayloadV1,
        REPAIR_AUDIT_EVENT_VERSION_V1, RepairAuditEventV1, RepairReportV1, RepairSlashProposalV1,
        RepairTaskEventV1, RepairTaskRecordV1, RepairTaskStateV1, RepairTicketId,
        SorafsAuditHeaderV1,
    },
};
use thiserror::Error;
use tokio::sync::broadcast;

use crate::{
    governance::FilesystemGovernancePublisher,
    metering::{CapacityMeter, MeteringSnapshot, ReplicationUsageSample},
    moderation::ModerationBallotRuntime,
    orderbook::OrderbookRuntime,
    potr::PotrTracker,
    scheduler::{StorageSchedulerConfig, StorageSchedulersRuntime},
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

const REPAIR_EVENT_CHANNEL_CAPACITY: usize = 128;
const REPUTATION_EVENT_CHANNEL_CAPACITY: usize = 128;
const ORDERBOOK_EVENT_CHANNEL_CAPACITY: usize = 128;
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
        total_treasury_xor: "0".to_string(),
        total_rewards_forfeited_treasury_xor: "0".to_string(),
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
        .to_string();

    Ok(reconciliation::AppealFinanceRollupReconciliationEntry {
        cycle: required_json_string(metadata, "cycle")?,
        encoded_blake3: required_json_string(index_entry, "encoded_blake3")?,
        report_count: required_json_u64(metadata, "report_count")?,
        case_count: required_json_u64(metadata, "case_count")?,
        total_treasury_xor: required_json_string(metadata, "total_treasury_xor")?,
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

fn add_appeal_finance_decimal_strings(
    left: &str,
    right: &str,
) -> Result<String, ReconciliationError> {
    let (left_int, left_frac) = parse_appeal_finance_decimal(left)?;
    let (right_int, right_frac) = parse_appeal_finance_decimal(right)?;
    let scale = left_frac.len().max(right_frac.len());
    let mut left_digits = left_int;
    left_digits.push_str(&right_pad_fraction(&left_frac, scale));
    let mut right_digits = right_int;
    right_digits.push_str(&right_pad_fraction(&right_frac, scale));

    let mut carry = 0_u8;
    let mut out = Vec::new();
    let left_bytes = left_digits.as_bytes();
    let right_bytes = right_digits.as_bytes();
    let width = left_bytes.len().max(right_bytes.len());
    for offset in 0..width {
        let sum =
            digit_from_right(left_bytes, offset) + digit_from_right(right_bytes, offset) + carry;
        out.push((sum % 10) + b'0');
        carry = sum / 10;
    }
    if carry > 0 {
        out.push(carry + b'0');
    }
    out.reverse();
    let mut raw = String::from_utf8(out)
        .map_err(|err| ReconciliationError::AppealFinance(format!("decimal sum utf8: {err}")))?;
    if scale == 0 {
        return Ok(trim_decimal_integer(&raw));
    }
    if raw.len() <= scale {
        raw = format!("{}{}", "0".repeat(scale + 1 - raw.len()), raw);
    }
    let point = raw.len() - scale;
    let int = trim_decimal_integer(&raw[..point]);
    let frac = raw[point..].trim_end_matches('0');
    if frac.is_empty() {
        Ok(int)
    } else {
        Ok(format!("{int}.{frac}"))
    }
}

fn parse_appeal_finance_decimal(raw: &str) -> Result<(String, String), ReconciliationError> {
    if raw.is_empty() || raw.starts_with(['+', '-']) {
        return Err(ReconciliationError::AppealFinance(format!(
            "invalid appeal finance decimal `{raw}`"
        )));
    }
    let mut parts = raw.split('.');
    let int = parts.next().unwrap_or_default();
    let frac = parts.next();
    if parts.next().is_some() {
        return Err(ReconciliationError::AppealFinance(format!(
            "invalid appeal finance decimal `{raw}`"
        )));
    }
    if int.is_empty() || !int.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ReconciliationError::AppealFinance(format!(
            "invalid appeal finance decimal `{raw}`"
        )));
    }
    let frac = frac.unwrap_or_default();
    if !frac.is_empty() && !frac.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(ReconciliationError::AppealFinance(format!(
            "invalid appeal finance decimal `{raw}`"
        )));
    }
    Ok((trim_decimal_integer(int), frac.to_string()))
}

fn right_pad_fraction(frac: &str, scale: usize) -> String {
    let mut padded = frac.to_string();
    padded.extend(std::iter::repeat_n('0', scale.saturating_sub(frac.len())));
    padded
}

fn digit_from_right(bytes: &[u8], offset: usize) -> u8 {
    bytes
        .len()
        .checked_sub(offset + 1)
        .and_then(|idx| bytes.get(idx))
        .map_or(0, |byte| byte.saturating_sub(b'0'))
}

fn trim_decimal_integer(raw: &str) -> String {
    let trimmed = raw.trim_start_matches('0');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

fn moderation_appeal_finance_report(
    record: &ModerationBallotRecord,
    tally: &ModerationBallotTally,
) -> Result<Option<SoraFsAppealFinanceReportV1>, String> {
    let Some(deposit) = &record.announcement.appeal_deposit else {
        return Ok(None);
    };
    let outcome = moderation_tally_finance_outcome(tally);
    let scale = appeal_finance_report_scale(&deposit.deposit_xor)?;
    let unit = appeal_finance_scale_unit(scale)?;
    let deposit_scaled = parse_appeal_finance_scaled(&deposit.deposit_xor, scale)?;
    let (refund_scaled, treasury_deposit_scaled, held_scaled) =
        appeal_finance_deposit_flows(deposit_scaled, outcome)?;
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

    let stipend_scaled = 25_u128
        .checked_mul(unit)
        .ok_or_else(|| "panel stipend overflow".to_string())?;
    let bonus_scaled = 10_u128
        .checked_mul(unit)
        .ok_or_else(|| "panel bonus overflow".to_string())?;
    let panel_size_scaled = u128::from(panel_size);
    let panel_reward_total_scaled = stipend_scaled
        .checked_mul(panel_size_scaled)
        .and_then(|value| value.checked_add(bonus_scaled))
        .ok_or_else(|| "panel reward total overflow".to_string())?;
    let attending_count = attending.len() as u128;
    let bonus_share_scaled = bonus_scaled / attending_count;
    let payout_total_scaled = stipend_scaled
        .checked_add(bonus_share_scaled)
        .ok_or_else(|| "juror payout total overflow".to_string())?;
    let rewards_paid_total_scaled = payout_total_scaled
        .checked_mul(attending_count)
        .ok_or_else(|| "paid reward total overflow".to_string())?;
    let rewards_forfeited_scaled =
        panel_reward_total_scaled.saturating_sub(rewards_paid_total_scaled);
    let treasury_total_scaled = treasury_deposit_scaled
        .checked_add(rewards_forfeited_scaled)
        .ok_or_else(|| "treasury total overflow".to_string())?;

    let report_id = moderation_appeal_finance_report_id(record, tally, deposit, outcome);
    let juror_payouts = attending
        .into_iter()
        .map(|juror_id| SoraFsAppealFinanceJurorPayoutV1 {
            juror_id,
            stipend_xor: format_appeal_finance_scaled(stipend_scaled, scale),
            bonus_xor: format_appeal_finance_scaled(bonus_share_scaled, scale),
            total_xor: format_appeal_finance_scaled(payout_total_scaled, scale),
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
        deposit_xor: format_appeal_finance_scaled(deposit_scaled, scale),
        refund: SoraFsAppealFinanceAccountFlowV1 {
            account_id: deposit.payer_account.clone(),
            amount_xor: format_appeal_finance_scaled(refund_scaled, scale),
        },
        treasury: SoraFsAppealFinanceAccountFlowV1 {
            account_id: deposit.destination_account.clone(),
            amount_xor: format_appeal_finance_scaled(treasury_total_scaled, scale),
        },
        held: SoraFsAppealFinanceAccountFlowV1 {
            account_id: deposit.custody_account.clone(),
            amount_xor: format_appeal_finance_scaled(held_scaled, scale),
        },
        panel_size,
        panel_reward_total_xor: format_appeal_finance_scaled(panel_reward_total_scaled, scale),
        rewards_paid_total_xor: format_appeal_finance_scaled(rewards_paid_total_scaled, scale),
        rewards_forfeited_treasury_xor: format_appeal_finance_scaled(
            rewards_forfeited_scaled,
            scale,
        ),
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
    deposit_scaled: u128,
    outcome: SoraFsAppealFinanceOutcomeV1,
) -> Result<(u128, u128, u128), String> {
    let (refund_numerator, treasury_numerator, denominator) = match outcome {
        SoraFsAppealFinanceOutcomeV1::Overturn | SoraFsAppealFinanceOutcomeV1::Modify => {
            (1_u128, 0_u128, 1_u128)
        }
        SoraFsAppealFinanceOutcomeV1::Uphold
        | SoraFsAppealFinanceOutcomeV1::WithdrawnAfterPanel => (0, 1, 1),
        SoraFsAppealFinanceOutcomeV1::WithdrawnBeforePanel => (9, 0, 10),
        SoraFsAppealFinanceOutcomeV1::Frivolous => (1, 1, 2),
        SoraFsAppealFinanceOutcomeV1::Escalated => (0, 0, 1),
    };
    let refund = deposit_scaled
        .checked_mul(refund_numerator)
        .and_then(|value| value.checked_div(denominator))
        .ok_or_else(|| "refund calculation overflow".to_string())?;
    let treasury = deposit_scaled
        .checked_mul(treasury_numerator)
        .and_then(|value| value.checked_div(denominator))
        .ok_or_else(|| "treasury calculation overflow".to_string())?;
    let held = deposit_scaled
        .saturating_sub(refund)
        .saturating_sub(treasury);
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

fn appeal_finance_report_scale(value: &str) -> Result<usize, String> {
    let (_, fractional) = parse_appeal_finance_decimal(value).map_err(|err| err.to_string())?;
    Ok(fractional.len().max(12))
}

fn appeal_finance_scale_unit(scale: usize) -> Result<u128, String> {
    if scale > 18 {
        return Err(format!(
            "appeal finance decimal scale `{scale}` exceeds the supported report scale"
        ));
    }
    let mut unit = 1_u128;
    for _ in 0..scale {
        unit = unit
            .checked_mul(10)
            .ok_or_else(|| "appeal finance scale overflow".to_string())?;
    }
    Ok(unit)
}

fn parse_appeal_finance_scaled(value: &str, scale: usize) -> Result<u128, String> {
    let (integral, fractional) =
        parse_appeal_finance_decimal(value).map_err(|err| err.to_string())?;
    if fractional.len() > scale {
        return Err(format!(
            "appeal finance decimal `{value}` has more fractional digits than scale `{scale}`"
        ));
    }
    let mut digits = integral;
    digits.push_str(&right_pad_fraction(&fractional, scale));
    digits
        .parse::<u128>()
        .map_err(|err| format!("appeal finance decimal `{value}` exceeds report limits: {err}"))
}

fn format_appeal_finance_scaled(value: u128, scale: usize) -> String {
    if scale == 0 {
        return value.to_string();
    }
    let mut raw = value.to_string();
    if raw.len() <= scale {
        raw = format!("{}{}", "0".repeat(scale + 1 - raw.len()), raw);
    }
    let split = raw.len() - scale;
    let integral = trim_decimal_integer(&raw[..split]);
    let fractional = raw[split..].trim_end_matches('0');
    if fractional.is_empty() {
        integral
    } else {
        format!("{integral}.{fractional}")
    }
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairEvent {
    /// Monotonic local stream sequence.
    pub sequence: u64,
    /// Canonical repair task transition payload.
    pub event: RepairTaskEventV1,
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
    repair_events: Arc<RwLock<Vec<RepairEvent>>>,
    repair_event_sender: broadcast::Sender<RepairEvent>,
    repair_orchestrator: Arc<RwLock<Option<Arc<dyn RepairOrchestrator>>>>,
    governance_publisher: Arc<RwLock<Option<Arc<dyn GovernancePublisher>>>>,
    latest_reputation_snapshot: Arc<RwLock<Option<ReputationSnapshotV1>>>,
    reputation_snapshots: Arc<RwLock<BTreeMap<[u8; 16], ReputationSnapshotV1>>>,
    reputation_events: Arc<RwLock<Vec<ReputationSnapshotEventV1>>>,
    reputation_event_sender: broadcast::Sender<ReputationSnapshotEventV1>,
    orderbook: Arc<RwLock<OrderbookRuntime>>,
    orderbook_events: Arc<RwLock<Vec<OrderbookEvent>>>,
    orderbook_event_sender: broadcast::Sender<OrderbookEvent>,
    moderation: Arc<RwLock<ModerationBallotRuntime>>,
    moderation_events: Arc<RwLock<Vec<ModerationBallotEvent>>>,
    moderation_event_sender: broadcast::Sender<ModerationBallotEvent>,
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

/// Error type returned by storage-related operations on [`NodeHandle`].
#[derive(Debug, Error)]
pub enum NodeStorageError {
    /// Storage subsystem is disabled in the configuration.
    #[error("SoraFS storage is disabled for this node")]
    Disabled,
    /// Underlying storage backend reported an error.
    #[error(transparent)]
    Storage(#[from] StorageError),
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
    /// Reconciliation report failed validation.
    #[error(transparent)]
    Validation(#[from] ReconciliationValidationError),
}

fn hard_fork_snapshot_bootstrap_enabled() -> bool {
    hard_fork_snapshot_bootstrap_enabled_from(env::var_os("IROHA_HARD_FORK_SNAPSHOT_BOOTSTRAP"))
}

fn hard_fork_snapshot_bootstrap_enabled_from(value: Option<std::ffi::OsString>) -> bool {
    value.is_some()
}

impl NodeHandle {
    /// Construct a new handle for the embedded storage worker.
    #[must_use]
    pub fn new(config: StorageConfig) -> Self {
        Self::new_with_policies(config, RepairConfig::default(), GcConfig::default())
    }

    /// Construct a new handle with explicit repair/GC policies.
    #[must_use]
    pub fn new_with_policies(
        config: StorageConfig,
        repair_config: RepairConfig,
        gc_config: GcConfig,
    ) -> Self {
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
                Err(StorageError::Norito(err)) if hard_fork_snapshot_bootstrap_enabled() => {
                    iroha_logger::warn!(
                        %err,
                        "hard-fork snapshot bootstrap: disabling SoraFS storage backend after legacy Norito decode failure"
                    );
                    schedulers.update_storage_bytes(0, capacity_limit);
                    None
                }
                Err(err) => {
                    panic!("failed to initialise SoraFS storage backend: {err}")
                }
            }
        } else {
            schedulers.update_storage_bytes(0, capacity_limit);
            None
        };

        let smoothing = config.smoothing_config();
        let deal_engine = DealEngine::new();
        let governance_dir = config.governance_dir().cloned();
        let governance_dag_publisher_peer_id = config.governance_dag_publisher_peer_id().cloned();
        let governance_dag_signing_key_path = config.governance_dag_signing_key_path().cloned();
        let (repair_event_sender, _) = broadcast::channel(REPAIR_EVENT_CHANNEL_CAPACITY);
        let (reputation_event_sender, _) = broadcast::channel(REPUTATION_EVENT_CHANNEL_CAPACITY);
        let (orderbook_event_sender, _) = broadcast::channel(ORDERBOOK_EVENT_CHANNEL_CAPACITY);
        let (moderation_event_sender, _) =
            broadcast::channel(MODERATION_BALLOT_EVENT_CHANNEL_CAPACITY);

        let repair = RepairManager::new_with_config_and_policy(
            repair_config.clone(),
            *repair_config.escalation_policy(),
        );
        let node = Self {
            config,
            repair_config,
            gc_config,
            capacity: Arc::new(CapacityManager::new()),
            meter: CapacityMeter::with_smoothing(smoothing),
            telemetry: Arc::new(RwLock::new(None)),
            schedulers,
            por: PorTracker::default(),
            potr: PotrTracker::default(),
            por_history: Arc::new(RwLock::new(HashMap::new())),
            storage,
            deal_engine,
            repair,
            repair_events: Arc::new(RwLock::new(Vec::new())),
            repair_event_sender,
            repair_orchestrator: Arc::new(RwLock::new(None)),
            governance_publisher: Arc::new(RwLock::new(None)),
            latest_reputation_snapshot: Arc::new(RwLock::new(None)),
            reputation_snapshots: Arc::new(RwLock::new(BTreeMap::new())),
            reputation_events: Arc::new(RwLock::new(Vec::new())),
            reputation_event_sender,
            orderbook: Arc::new(RwLock::new(OrderbookRuntime::default())),
            orderbook_events: Arc::new(RwLock::new(Vec::new())),
            orderbook_event_sender,
            moderation: Arc::new(RwLock::new(ModerationBallotRuntime::default())),
            moderation_events: Arc::new(RwLock::new(Vec::new())),
            moderation_event_sender,
        };

        if node.storage.is_some() {
            if let Some(dir) = governance_dir.clone() {
                match FilesystemGovernancePublisher::try_new(dir.clone()) {
                    Ok(publisher) => {
                        let publisher = match (
                            governance_dag_publisher_peer_id.clone(),
                            governance_dag_signing_key_path.clone(),
                        ) {
                            (Some(peer_id), Some(signing_key_path)) => publisher
                                .with_runtime_dag_signer(peer_id.into_bytes(), signing_key_path),
                            (Some(_), None) | (None, Some(_)) => {
                                iroha_logger::warn!(
                                    "SoraFS governance runtime DAG signer requires both publisher peer id and signing key path"
                                );
                                Ok(publisher)
                            }
                            (None, None) => Ok(publisher),
                        };
                        match publisher {
                            Ok(publisher) => {
                                iroha_logger::info!(
                                    path = ?dir,
                                    signed_runtime_dag = governance_dag_publisher_peer_id.is_some()
                                        && governance_dag_signing_key_path.is_some(),
                                    "SoraFS governance publisher initialised"
                                );
                                node.set_governance_publisher(Arc::new(publisher));
                            }
                            Err(err) => {
                                iroha_logger::error!(
                                    ?err,
                                    path = ?dir,
                                    "failed to initialise SoraFS signed governance runtime DAG publisher"
                                );
                            }
                        }
                    }
                    Err(err) => {
                        iroha_logger::error!(
                            ?err,
                            path = ?dir,
                            "failed to initialise SoraFS governance publisher"
                        );
                    }
                }
            }
        } else if governance_dir.is_some() {
            iroha_logger::warn!(
                "skipping governance publisher initialisation: storage backend disabled"
            );
        }

        node
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

    /// Returns a clone of the embedded deal engine handle.
    #[must_use]
    pub fn deal_engine(&self) -> DealEngine {
        self.deal_engine.clone()
    }

    /// Deposit provider bond collateral (nano-XOR units).
    pub fn deposit_provider_bond(
        &self,
        provider_id: ProviderId,
        amount_nano: u128,
    ) -> ProviderSnapshot {
        self.deal_engine
            .deposit_provider_bond(provider_id, amount_nano)
    }

    /// Deposit client credit balance (nano-XOR units).
    pub fn deposit_client_credit(&self, client_id: ClientId, amount_nano: u128) -> ClientSnapshot {
        self.deal_engine
            .deposit_client_credit(client_id, amount_nano)
    }

    /// Open a deal using the supplied proposal and activation epoch.
    pub fn open_deal(
        &self,
        proposal: DealProposal,
        activation_epoch: u64,
    ) -> Result<DealRecord, DealEngineError> {
        self.deal_engine.open_deal(proposal, activation_epoch)
    }

    /// Record usage attributed to a deal and evaluate probabilistic micropayments.
    pub fn record_deal_usage(
        &self,
        report: DealUsageReport,
    ) -> Result<UsageOutcome, DealEngineError> {
        self.deal_engine.record_usage(report)
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
    /// When a governance publisher is configured, the snapshot is written to the
    /// governance artefact directory before it becomes the in-memory latest
    /// snapshot served by Torii.
    pub fn publish_reputation_snapshot(
        &self,
        snapshot: ReputationSnapshotV1,
    ) -> Result<(), GovernancePublishError> {
        snapshot
            .validate()
            .map_err(|err| GovernancePublishError::other(format!("invalid snapshot: {err}")))?;
        let encoded = norito::to_bytes(&snapshot)
            .map_err(|err| GovernancePublishError::other(format!("encode snapshot: {err}")))?;
        if let Some(publisher) = self.governance_publisher() {
            publisher.publish_reputation_snapshot(&snapshot, &encoded)?;
        }
        let mut snapshots = self
            .reputation_snapshots
            .write()
            .map_err(|_| GovernancePublishError::other("reputation snapshot index poisoned"))?;
        snapshots.insert(snapshot.snapshot_id, snapshot.clone());
        let mut events = self
            .reputation_events
            .write()
            .map_err(|_| GovernancePublishError::other("reputation event history poisoned"))?;
        let sequence = events
            .last()
            .map_or(1, |event| event.sequence.saturating_add(1));
        let event = ReputationSnapshotEventV1::from_snapshot(sequence, &snapshot)
            .map_err(|err| GovernancePublishError::other(format!("snapshot event: {err}")))?;
        events.push(event.clone());
        let _ = self.reputation_event_sender.send(event);
        let mut guard = self
            .latest_reputation_snapshot
            .write()
            .map_err(|_| GovernancePublishError::other("reputation snapshot cache poisoned"))?;
        *guard = Some(snapshot);
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
        if let Some(publisher) = self.governance_publisher() {
            publisher.publish_appeal_finance_report(&report, &encoded)?;
        }
        Ok(())
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
        let limit = limit.max(1);
        let since = since_sequence.unwrap_or(0);
        self.reputation_events.read().map_or_else(
            |_| Vec::new(),
            |guard| {
                guard
                    .iter()
                    .filter(|event| event.sequence > since)
                    .take(limit)
                    .cloned()
                    .collect()
            },
        )
    }

    /// Return the latest reputation snapshot event sequence accepted by this node.
    #[must_use]
    pub fn latest_reputation_event_sequence(&self) -> Option<u64> {
        self.reputation_events
            .read()
            .ok()
            .and_then(|guard| guard.last().map(|event| event.sequence))
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
        let limit = limit.max(1);
        let since = since_sequence.unwrap_or(0);
        self.repair_events.read().map_or_else(
            |_| Vec::new(),
            |guard| {
                guard
                    .iter()
                    .filter(|event| event.sequence > since)
                    .take(limit)
                    .cloned()
                    .collect()
            },
        )
    }

    /// Return the latest local repair event sequence accepted by this node.
    #[must_use]
    pub fn latest_repair_event_sequence(&self) -> Option<u64> {
        self.repair_events
            .read()
            .ok()
            .and_then(|guard| guard.last().map(|event| event.sequence))
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
        let limit = limit.max(1);
        let since = since_sequence.unwrap_or(0);
        self.orderbook_events.read().map_or_else(
            |_| Vec::new(),
            |guard| {
                guard
                    .iter()
                    .filter(|event| event.sequence > since)
                    .take(limit)
                    .cloned()
                    .collect()
            },
        )
    }

    /// Return the latest local orderbook event sequence accepted by this node.
    #[must_use]
    pub fn latest_orderbook_event_sequence(&self) -> Option<u64> {
        self.orderbook_events
            .read()
            .ok()
            .and_then(|guard| guard.last().map(|event| event.sequence))
    }

    /// Subscribe to live local orderbook events.
    #[must_use]
    pub fn subscribe_orderbook_events(&self) -> broadcast::Receiver<OrderbookEvent> {
        self.orderbook_event_sender.subscribe()
    }

    /// Return local moderation ballot events after `since_sequence`, capped by `limit`.
    #[must_use]
    pub fn moderation_ballot_events_since(
        &self,
        since_sequence: Option<u64>,
        limit: usize,
    ) -> Vec<ModerationBallotEvent> {
        let limit = limit.max(1);
        let since = since_sequence.unwrap_or(0);
        self.moderation_events.read().map_or_else(
            |_| Vec::new(),
            |guard| {
                guard
                    .iter()
                    .filter(|event| event.sequence > since)
                    .take(limit)
                    .cloned()
                    .collect()
            },
        )
    }

    /// Return the latest local moderation ballot event sequence accepted by this node.
    #[must_use]
    pub fn latest_moderation_ballot_event_sequence(&self) -> Option<u64> {
        self.moderation_events
            .read()
            .ok()
            .and_then(|guard| guard.last().map(|event| event.sequence))
    }

    /// Subscribe to live local moderation ballot events.
    #[must_use]
    pub fn subscribe_moderation_ballot_events(&self) -> broadcast::Receiver<ModerationBallotEvent> {
        self.moderation_event_sender.subscribe()
    }

    /// Announce a local SoraFS moderation ballot.
    ///
    /// This in-memory lifecycle store validates the ballot context, ordered
    /// roster hash, quorum, and commit/challenge/reveal windows. It does not
    /// publish an on-chain record or Governance DAG event yet.
    pub fn announce_moderation_ballot(
        &self,
        announcement: ModerationBallotAnnouncement,
    ) -> Result<ModerationBallotRecord, ModerationBallotRuntimeError> {
        let case_id = announcement.context.case_id.clone();
        let round_id = announcement.round_id.clone();
        let generated_at_unix_ms = announcement.announced_at_unix_ms;
        let record = {
            self.moderation
                .write()
                .map_err(|_| ModerationBallotRuntimeError::StateLockPoisoned)?
                .announce_ballot(announcement)?
        };
        self.publish_moderation_ballot_event(
            ModerationBallotEventKind::BallotAnnounced,
            generated_at_unix_ms,
            case_id,
            round_id,
            None,
            None,
        );
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
        let outcome = {
            self.moderation
                .write()
                .map_err(|_| ModerationBallotRuntimeError::StateLockPoisoned)?
                .submit_commit(commit, now_unix_ms)?
        };
        self.publish_moderation_ballot_event(
            ModerationBallotEventKind::CommitAccepted,
            now_unix_ms,
            case_id,
            round_id,
            Some(juror_id),
            None,
        );
        Ok(outcome)
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
        let outcome = {
            self.moderation
                .write()
                .map_err(|_| ModerationBallotRuntimeError::StateLockPoisoned)?
                .submit_reveal(reveal, now_unix_ms)?
        };
        self.publish_moderation_ballot_event(
            ModerationBallotEventKind::RevealAccepted,
            now_unix_ms,
            case_id,
            round_id,
            Some(juror_id),
            None,
        );
        Ok(outcome)
    }

    /// Finalize a local SoraFS moderation ballot tally.
    pub fn tally_moderation_ballot(
        &self,
        case_id: &str,
        round_id: &str,
        now_unix_ms: u64,
    ) -> Result<ModerationBallotTally, ModerationBallotRuntimeError> {
        let tally = {
            self.moderation
                .write()
                .map_err(|_| ModerationBallotRuntimeError::StateLockPoisoned)?
                .tally_ballot(case_id, round_id, now_unix_ms)?
        };
        self.publish_moderation_ballot_event(
            ModerationBallotEventKind::BallotTallied,
            now_unix_ms,
            case_id.to_owned(),
            round_id.to_owned(),
            None,
            Some(tally.clone()),
        );
        if let Some(record) = self.moderation_ballot(case_id, round_id) {
            self.publish_moderation_appeal_finance_report(&record, &tally);
        }
        Ok(tally)
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
        let outcome = self
            .orderbook
            .write()
            .map_err(|_| OrderbookRuntimeError::StateLockPoisoned)?
            .submit_order(order, now_unix);
        match &outcome {
            Ok(outcome) => {
                global_or_default().record_sorafs_orderbook_order(
                    ORDERBOOK_METRIC_CLUSTER_LOCAL,
                    orderbook_tier_label(tier),
                    orderbook_side_label(side),
                    "accepted",
                );
                self.record_orderbook_snapshot_metrics(now_unix);
                self.publish_orderbook_event(
                    OrderbookEventKind::OrderAccepted,
                    now_unix,
                    Some(outcome.accepted_order.order_id),
                    outcome
                        .fills
                        .iter()
                        .map(|fill| fill.trade.trade_id)
                        .collect(),
                    outcome
                        .settlement_channels_opened
                        .iter()
                        .map(|channel| channel.channel_id)
                        .collect(),
                    None,
                    outcome.expired_order_ids.clone(),
                );
            }
            Err(err) => {
                let status = match err {
                    OrderbookRuntimeError::DuplicateOrderId { .. } => "duplicate",
                    OrderbookRuntimeError::Validation(_) => "rejected",
                    OrderbookRuntimeError::SequenceOverflow
                    | OrderbookRuntimeError::MissingMatchedOrder
                    | OrderbookRuntimeError::InvalidMatchedSides
                    | OrderbookRuntimeError::StateLockPoisoned
                    | OrderbookRuntimeError::SettlementChannelNotFound { .. }
                    | OrderbookRuntimeError::DuplicateReceiptId { .. }
                    | OrderbookRuntimeError::ReceiptRangeOverlap { .. }
                    | OrderbookRuntimeError::OrderNotFound { .. }
                    | OrderbookRuntimeError::CancelOwnerMismatch => "error",
                };
                global_or_default().record_sorafs_orderbook_order(
                    ORDERBOOK_METRIC_CLUSTER_LOCAL,
                    orderbook_tier_label(tier),
                    orderbook_side_label(side),
                    status,
                );
            }
        }
        outcome
    }

    /// Cancel an open order from the local SoraFS orderbook mirror.
    pub fn cancel_orderbook_order(
        &self,
        cancel: OrderCancelV1,
        now_unix: u64,
    ) -> Result<OrderbookCancelOutcome, OrderbookRuntimeError> {
        let outcome = self
            .orderbook
            .write()
            .map_err(|_| OrderbookRuntimeError::StateLockPoisoned)?
            .cancel_order(cancel);
        if let Ok(outcome) = &outcome {
            global_or_default().record_sorafs_orderbook_order(
                ORDERBOOK_METRIC_CLUSTER_LOCAL,
                "all",
                "all",
                "cancelled",
            );
            self.record_orderbook_snapshot_metrics(now_unix);
            self.publish_orderbook_event(
                OrderbookEventKind::OrderCancelled,
                now_unix,
                Some(outcome.cancelled_order.order_id),
                Vec::new(),
                Vec::new(),
                None,
                Vec::new(),
            );
        }
        outcome
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
        let outcome = self
            .orderbook
            .write()
            .map_err(|_| OrderbookRuntimeError::StateLockPoisoned)?
            .submit_receipt(receipt);
        if let Ok(outcome) = &outcome {
            self.record_orderbook_snapshot_metrics(now_unix);
            self.publish_orderbook_event(
                OrderbookEventKind::SettlementReceiptAccepted,
                now_unix,
                None,
                Vec::new(),
                vec![outcome.updated_channel.channel_id],
                Some(outcome.accepted_receipt.receipt_id),
                Vec::new(),
            );
        }
        outcome
    }

    /// Return a point-in-time snapshot of the local SoraFS orderbook mirror.
    #[must_use]
    pub fn orderbook_snapshot(&self, generated_at_unix: u64) -> OrderbookSnapshot {
        self.orderbook.read().map_or_else(
            |_| OrderbookSnapshot {
                next_sequence: 0,
                generated_at_unix,
                open_orders: Vec::new(),
                trades: Vec::new(),
                settlement_channels: Vec::new(),
                settlement_receipts: Vec::new(),
                expired_order_ids: Vec::new(),
            },
            |orderbook| orderbook.snapshot(generated_at_unix),
        )
    }

    fn record_orderbook_snapshot_metrics(&self, now_unix: u64) {
        let snapshot = self.orderbook_snapshot(now_unix);
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
        for (provider, seconds) in orderbook_provider_escrow_runways(&snapshot, now_unix) {
            metrics.set_sorafs_orderbook_escrow_runway_seconds(&provider, seconds);
        }
        metrics
            .set_sorafs_orderbook_contract_mirror_divergence(ORDERBOOK_METRIC_CLUSTER_LOCAL, false);
    }

    // Orderbook events intentionally carry the accepted object ids and derived
    // snapshot counters in one append point so replay and live streams agree.
    #[allow(clippy::too_many_arguments)]
    fn publish_orderbook_event(
        &self,
        kind: OrderbookEventKind,
        generated_at_unix: u64,
        order_id: Option<[u8; 32]>,
        trade_ids: Vec<[u8; 32]>,
        settlement_channel_ids: Vec<[u8; 32]>,
        receipt_id: Option<[u8; 32]>,
        expired_order_ids: Vec<[u8; 32]>,
    ) {
        let snapshot = self.orderbook_snapshot(generated_at_unix);
        let open_settlement_channel_count = snapshot
            .settlement_channels
            .iter()
            .filter(|channel| matches!(channel.status, SettlementChannelStatusV1::Open))
            .count() as u64;
        let Ok(mut events) = self.orderbook_events.write() else {
            return;
        };
        let sequence = events
            .last()
            .map_or(1, |event| event.sequence.saturating_add(1));
        let event = OrderbookEvent {
            sequence,
            kind,
            generated_at_unix,
            order_id,
            trade_ids,
            settlement_channel_ids,
            receipt_id,
            expired_order_ids,
            open_order_count: snapshot.open_orders.len() as u64,
            open_settlement_channel_count,
            settlement_receipt_count: snapshot.settlement_receipts.len() as u64,
        };
        events.push(event.clone());
        let _ = self.orderbook_event_sender.send(event);
    }

    fn publish_moderation_ballot_event(
        &self,
        kind: ModerationBallotEventKind,
        generated_at_unix_ms: u64,
        case_id: String,
        round_id: String,
        juror_id: Option<String>,
        tally: Option<ModerationBallotTally>,
    ) {
        let record = self.moderation_ballot(&case_id, &round_id);
        let committed_count = record
            .as_ref()
            .map_or(0, |record| record.commits.len() as u64);
        let revealed_count = record
            .as_ref()
            .map_or(0, |record| record.reveals.len() as u64);
        let Ok(mut events) = self.moderation_events.write() else {
            return;
        };
        let sequence = events
            .last()
            .map_or(1, |event| event.sequence.saturating_add(1));
        let event = ModerationBallotEvent {
            sequence,
            kind,
            generated_at_unix_ms,
            case_id,
            round_id,
            juror_id,
            committed_count,
            revealed_count,
            tally,
        };
        events.push(event.clone());
        let _ = self.moderation_event_sender.send(event.clone());
        self.publish_moderation_ballot_governance_event(&event);
    }

    fn publish_moderation_ballot_governance_event(&self, event: &ModerationBallotEvent) {
        let Some(publisher) = self.governance_publisher() else {
            return;
        };
        let governance_event = event.to_governance_event_v1();
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
        let outcome = self.deal_engine.settle(deal_id, settlement_epoch)?;
        let publisher = self.governance_publisher();
        let provider_hex = hex::encode(outcome.record.provider_id.as_bytes());
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

    fn record_repair_event(&self, event: RepairTaskEventV1) {
        let Ok(mut events) = self.repair_events.write() else {
            return;
        };
        let sequence = events
            .last()
            .map_or(1, |event| event.sequence.saturating_add(1));
        let event = RepairEvent { sequence, event };
        events.push(event.clone());
        let _ = self.repair_event_sender.send(event);
    }

    fn publish_repair_audit_event(&self, event: RepairTaskEventV1) {
        let Some(publisher) = self.governance_publisher() else {
            return;
        };
        let payload_bytes = event.encode();
        let payload_digest = Hash::new(payload_bytes);
        let header = SorafsAuditHeaderV1 {
            sequence: self.repair.next_audit_sequence(),
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
        let header = SorafsAuditHeaderV1 {
            sequence: self.repair.next_audit_sequence(),
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
    ) {
        if let Some(event) = update.event.clone() {
            self.record_repair_event(event.clone());
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
    }

    /// Capture a snapshot of the deal ledger.
    #[must_use]
    pub fn deal_snapshot(&self, deal_id: DealId) -> Option<DealSnapshot> {
        self.deal_engine.deal_snapshot(deal_id)
    }

    /// Returns a clone of the repair manager.
    #[must_use]
    pub fn repair_manager(&self) -> RepairManager {
        self.repair.clone()
    }

    /// Enqueue a repair report submitted by an auditor.
    pub fn enqueue_repair_report(
        &self,
        report: &RepairReportV1,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        let update = self.repair.enqueue_report_with_event(report.clone())?;
        self.publish_repair_update(&update, None);
        Ok(update.record)
    }

    /// Record a signed repair auditor request nonce for replay protection.
    pub fn record_repair_auditor_nonce(
        &self,
        auditor_account: &str,
        nonce: u64,
    ) -> Result<(), RepairSchedulerError> {
        self.repair.record_auditor_nonce(auditor_account, nonce)
    }

    /// Fetch repair tasks with optional filters applied.
    #[must_use]
    pub fn repair_tasks(&self, filters: RepairTaskFilters) -> Vec<RepairTaskRecordV1> {
        self.repair.list_tasks(filters)
    }

    /// Fetch repair task snapshots with optional filters applied.
    #[must_use]
    pub fn repair_task_snapshots(&self, filters: RepairTaskFilters) -> Vec<RepairTaskSnapshot> {
        self.repair.list_task_snapshots(filters)
    }

    /// Fetch repair tasks associated with the supplied manifest digest.
    #[must_use]
    pub fn repair_tasks_for_manifest(&self, manifest_digest: &[u8; 32]) -> Vec<RepairTaskRecordV1> {
        self.repair_tasks(RepairTaskFilters::for_manifest(*manifest_digest))
    }

    /// Fetch repair task snapshots associated with the supplied manifest digest.
    #[must_use]
    pub fn repair_task_snapshots_for_manifest(
        &self,
        manifest_digest: &[u8; 32],
    ) -> Vec<RepairTaskSnapshot> {
        self.repair_task_snapshots(RepairTaskFilters::for_manifest(*manifest_digest))
    }

    /// Fetch a repair task record by ticket id.
    #[must_use]
    pub fn repair_task_record(&self, ticket_id: &RepairTicketId) -> Option<RepairTaskRecordV1> {
        self.repair.task_record(ticket_id)
    }

    /// Fetch a repair task snapshot by ticket id.
    #[must_use]
    pub fn repair_task_snapshot(&self, ticket_id: &RepairTicketId) -> Option<RepairTaskSnapshot> {
        self.repair.task_snapshot(ticket_id)
    }

    /// Submit a slash proposal tied to an escalated repair ticket.
    pub fn submit_repair_slash_proposal(
        &self,
        proposal: &RepairSlashProposalV1,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        let update = self
            .repair
            .submit_slash_proposal_with_event(proposal.clone())?;
        self.publish_repair_update(&update, Some(RepairSlashStage::Submitted));
        Ok(update.record)
    }

    /// Mark the specified repair ticket as in progress.
    pub fn mark_repair_in_progress(
        &self,
        ticket_id: &RepairTicketId,
        started_at_unix: u64,
        repair_agent: Option<String>,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        let update =
            self.repair
                .mark_in_progress_with_event(ticket_id, started_at_unix, repair_agent)?;
        self.publish_repair_update(&update, None);
        Ok(update.record)
    }

    /// Mark the specified repair ticket as completed.
    pub fn mark_repair_completed(
        &self,
        ticket_id: &RepairTicketId,
        completed_at_unix: u64,
        resolution_notes: Option<String>,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        let update = self.repair.mark_completed_with_event(
            ticket_id,
            completed_at_unix,
            resolution_notes,
        )?;
        self.publish_repair_update(&update, None);
        Ok(update.record)
    }

    /// Mark the specified repair ticket as failed.
    pub fn mark_repair_failed(
        &self,
        ticket_id: &RepairTicketId,
        failed_at_unix: u64,
        reason: String,
    ) -> Result<RepairTaskRecordV1, RepairSchedulerError> {
        let update = self
            .repair
            .mark_failed_with_event(ticket_id, failed_at_unix, reason)?;
        self.publish_repair_update(&update, Some(RepairSlashStage::Drafted));
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
        let update = self.repair.claim_ticket_with_event(
            ticket_id,
            worker_id,
            claimed_at_unix,
            idempotency_key,
        )?;
        self.publish_repair_update(&update, None);
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
        let update = self.repair.complete_ticket_with_event(
            ticket_id,
            worker_id,
            completed_at_unix,
            resolution_notes,
            idempotency_key,
        )?;
        self.publish_repair_update(&update, None);
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
        let update = self.repair.fail_ticket_with_event(
            ticket_id,
            worker_id,
            failed_at_unix,
            reason,
            idempotency_key,
        )?;
        self.publish_repair_update(&update, Some(RepairSlashStage::Drafted));
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
        let report = self.repair.run_watchdog(now_unix)?;
        for event in &report.events {
            self.record_repair_event(event.clone());
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

        let candidates = self.repair.claimable_tasks(now_unix);
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
            self.publish_repair_update(&update, None);

            let manifest = storage.manifest_by_digest(&task.manifest_digest);
            let (update, stage) = match manifest {
                Some(manifest) => {
                    let total = manifest.chunk_count();
                    let missing_chunks = self
                        .schedulers
                        .with_pin(|| Self::missing_chunk_records(&manifest));
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
                        let mut outcome = self.schedulers.with_pin(|| {
                            self.rehydrate_missing_chunks_from_local_replicas(
                                storage,
                                &missing_chunks,
                            )
                        });
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
            self.publish_repair_update(&update, stage);
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
            let tasks = self.repair.tasks_for_manifest(&digest);
            if tasks.iter().any(|task| !repair_task_terminal(task)) {
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

        let repair_records = self.repair.list_tasks(RepairTaskFilters::default());
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
        let mut total_treasury_xor = "0".to_string();
        let mut total_rewards_forfeited_treasury_xor = "0".to_string();
        let mut source_report_count = 0_u64;
        let mut case_count = 0_u64;
        for entry in &entries {
            source_report_count = source_report_count.saturating_add(entry.report_count);
            case_count = case_count.saturating_add(entry.case_count);
            total_treasury_xor =
                add_appeal_finance_decimal_strings(&total_treasury_xor, &entry.total_treasury_xor)?;
            total_rewards_forfeited_treasury_xor = add_appeal_finance_decimal_strings(
                &total_rewards_forfeited_treasury_xor,
                &entry.total_rewards_forfeited_treasury_xor,
            )?;
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
        self.capacity.record_declaration(record)?;
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
        let maybe_plan = self.capacity.schedule_order(order)?;
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
        let release = self.capacity.complete_order(order_id)?;
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

    /// Record a governance-issued PoR challenge.
    pub fn record_por_challenge(&self, challenge: &PorChallengeV1) -> Result<(), PorTrackerError> {
        self.por.record_challenge(challenge)
    }

    /// Record a provider PoR proof response.
    pub fn record_por_proof(&self, proof: &PorProofV1) -> Result<(), PorTrackerError> {
        self.por.record_proof(proof)
    }

    /// Record an audit verdict and update telemetry counters accordingly.
    pub fn record_por_verdict(
        &self,
        verdict: &AuditVerdictV1,
    ) -> Result<PorVerdictOutcome, PorTrackerError> {
        let stats = self.por.record_verdict(verdict)?;
        if stats.success_samples > 0 {
            self.meter.record_por_samples(stats.success_samples, 0);
        }
        if stats.failed_samples > 0 {
            self.meter.record_por_samples(0, stats.failed_samples);
        }
        self.schedulers
            .record_por_samples(stats.success_samples, stats.failed_samples);
        let consecutive_failures = self.update_por_history_entry(verdict);
        let slash = self.evaluate_por_penalty(verdict, &stats, consecutive_failures);
        let repair_history_id = self
            .repair
            .register_por_verdict(verdict, stats.failed_samples)?;
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
        vrf_records: &HashMap<[u8; 32], ManifestVrfBundle>,
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
            let vrf = vrf_records.get(&digest);
            let planned = build_por_challenge_for_manifest(
                &manifest,
                provider_id,
                &randomness,
                vrf,
                &sample_policy,
            )?;
            challenges.push(planned);
        }

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
        let result = self.schedulers.with_pin(|| {
            storage.ingest_manifest_with_layout(manifest, plan, reader, stripe_layout, chunk_roles)
        });
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
                let retry = self.schedulers.with_pin(|| {
                    storage.ingest_manifest_with_layout(
                        manifest,
                        plan,
                        reader,
                        stripe_layout,
                        chunk_roles_retry,
                    )
                });
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
            .run_fetch(len as u64, None, || {
                storage.read_payload_range(manifest_id, offset, len)
            })
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
            .run_fetch(record.length as u64, None, || {
                storage.read_chunk(manifest_id, digest)
            })
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
            .with_por(|| storage.sample_por(manifest_id, count, seed));
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

    fn update_por_history_entry(&self, verdict: &AuditVerdictV1) -> u64 {
        let mut history = self.por_history.write().expect("por history poisoned");
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
        entry.consecutive_failures
    }

    fn evaluate_por_penalty(
        &self,
        verdict: &AuditVerdictV1,
        stats: &PorVerdictStats,
        consecutive_failures: u64,
    ) -> Option<SlashRecommendation> {
        if stats.failed_samples == 0 {
            return None;
        }
        let policy = self.config.penalty();
        if consecutive_failures < u64::from(policy.strike_threshold) {
            return None;
        }

        let mut history = self.por_history.write().expect("por history poisoned");
        let entry = history
            .entry((verdict.manifest_digest, verdict.provider_id))
            .or_default();
        if let Some(last_slash) = entry.last_slash_unix {
            let elapsed = verdict.decided_at.saturating_sub(last_slash);
            if elapsed < policy.cooldown_secs {
                return None;
            }
        }

        let provider_id = ProviderId::new(verdict.provider_id);
        let snapshot = self.deal_engine.provider_snapshot(provider_id)?;
        let bonded_total = snapshot
            .bond_available_nano
            .saturating_add(snapshot.bond_locked_nano);
        let penalty = bonded_total
            .saturating_mul(u128::from(policy.penalty_bond_bps))
            .saturating_div(10_000);
        if penalty == 0 {
            return None;
        }

        entry.last_slash_unix = Some(verdict.decided_at);
        entry.consecutive_failures = 0;

        Some(SlashRecommendation {
            provider_id,
            manifest_digest: verdict.manifest_digest,
            penalty_nano: penalty,
            strikes: policy.strike_threshold,
            reason: format!(
                "PoR failure streak reached {} (threshold {}), slashing {} bps of bonded collateral",
                consecutive_failures, policy.strike_threshold, policy.penalty_bond_bps
            ),
        })
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
        let history = self.por_history.read().expect("por history poisoned");
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
        self.storage
            .as_ref()
            .map(|arc| arc.as_ref())
            .ok_or(NodeStorageError::Disabled)
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

fn orderbook_provider_escrow_runways(
    snapshot: &OrderbookSnapshot,
    now_unix: u64,
) -> BTreeMap<String, u64> {
    let mut debit_by_channel = BTreeMap::<[u8; 32], u128>::new();
    for receipt in &snapshot.settlement_receipts {
        debit_by_channel
            .entry(receipt.channel_id)
            .and_modify(|total| *total = total.saturating_add(receipt.xor_debited.as_micro()))
            .or_insert_with(|| receipt.xor_debited.as_micro());
    }

    let mut runways_by_provider = BTreeMap::<[u8; 32], Option<u64>>::new();
    for channel in &snapshot.settlement_channels {
        let provider_runway = runways_by_provider.entry(channel.provider_id).or_default();
        if !matches!(channel.status, SettlementChannelStatusV1::Open) {
            continue;
        }

        let elapsed = now_unix.saturating_sub(channel.opened_at_unix);
        let debited = debit_by_channel
            .get(&channel.channel_id)
            .copied()
            .unwrap_or_default();
        let escrow_remaining = channel.xor_locked.as_micro();
        if elapsed == 0 || debited == 0 || escrow_remaining == 0 {
            continue;
        }

        let runway = (escrow_remaining.saturating_mul(u128::from(elapsed)) / debited)
            .min(u128::from(u64::MAX)) as u64;
        *provider_runway = Some(provider_runway.map_or(runway, |current| current.min(runway)));
    }

    runways_by_provider
        .into_iter()
        .map(|(provider, runway)| (hex::encode(provider), runway.unwrap_or(0)))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::{
        str::FromStr,
        sync::{
            Arc, Mutex,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use iroha_crypto::{Algorithm, Signature as IrohaSignature};
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
                SORAFS_MODERATION_BALLOT_COMMIT_VERSION_V1,
                SORAFS_MODERATION_BALLOT_CONTEXT_VERSION_V1,
                SORAFS_MODERATION_BALLOT_REVEAL_VERSION_V1, SoraFsModerationBallotCommitV1,
                SoraFsModerationBallotContextV1, SoraFsModerationBallotError,
                SoraFsModerationBallotRevealV1, SoraFsModerationVoteChoice,
            },
            pin_registry::StorageClass,
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
        ReputationProviderInputV1, ReputationProviderMetricsV1, ReputationReserveStageV1,
        ReputationWeightsV1, SETTLEMENT_RECEIPT_VERSION_V1,
        SORAFS_APPEAL_FINANCE_REPORT_VERSION_V1, SORAFS_RECONCILIATION_REPORT_VERSION_V1,
        SettlementChannelV1, SettlementReceiptV1, SoraFsAppealFinanceAccountFlowV1,
        SoraFsAppealFinanceJurorPayoutV1, SoraFsAppealFinanceOutcomeV1,
        SoraFsAppealFinanceReportV1, SoraFsAppealFinanceWeeklyRollupV1,
        SoraFsModerationBallotGovernanceEventKindV1, SoraFsModerationBallotGovernanceEventV1,
        SoraFsModerationVoteChoiceV1, SorafsReconciliationReportV1, build_reputation_snapshot,
        capacity::{
            CAPACITY_DECLARATION_VERSION_V1, CapacityDeclarationV1, CapacityMetadataEntry,
            ChunkerCommitmentV1, LaneCommitmentV1, REPLICATION_ORDER_VERSION_V1,
            ReplicationAssignmentV1, ReplicationOrderSlaV1, ReplicationOrderV1,
        },
        deal::{DealSettlementStatusV1, DealSettlementV1, XorAmount},
        order_cancel_signature_digest_v1, order_request_signature_digest_v1,
        provider_advert::SignatureAlgorithm,
        repair::{
            CompletedRepairStateV1, EscalatedRepairStateV1, FailedRepairStateV1,
            GC_AUDIT_EVENT_VERSION_V1, GC_AUDIT_PAYLOAD_VERSION_V1, GcAuditEventV1,
            InProgressRepairStateV1, QueuedRepairStateV1, REPAIR_ESCALATION_APPROVAL_VERSION_V1,
            REPAIR_EVIDENCE_VERSION_V1, REPAIR_REPORT_VERSION_V1, REPAIR_SLASH_PROPOSAL_VERSION_V1,
            RepairAuditEventV1, RepairCauseV1, RepairEscalationApprovalV1, RepairEvidenceV1,
            RepairManualCauseV1, RepairPorFailureCauseV1, RepairReportV1, RepairSlashProposalV1,
            RepairTaskStateV1, RepairTaskStatusV1, RepairTicketId,
        },
        settlement_receipt_signature_digest_v1,
    };
    use tempfile::TempDir;

    use super::*;
    use crate::por::test_support::{
        sample_challenge as por_sample_challenge, sample_proof as por_sample_proof,
        sample_verdict as por_sample_verdict,
    };

    fn storage_config_with_temp_dir() -> (StorageConfig, TempDir) {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_dir.path().join("storage"))
            .build();
        (cfg, temp_dir)
    }

    fn reputation_snapshot_fixture() -> ReputationSnapshotV1 {
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
            [0x42; 16],
            1_800_000_000,
            ReputationWeightsV1::default(),
            &[input],
            None,
        )
        .expect("reputation snapshot fixture")
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

    fn approval_for_default_policy(escalated_at_unix: u64) -> RepairEscalationApprovalV1 {
        let policy = *crate::config::RepairConfig::default().escalation_policy();
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
        sign_orderbook_order(
            OrderRequestV1 {
                version: ORDERBOOK_ORDER_VERSION_V1,
                order_id: [id; 32],
                side,
                tier: OrderTierV1::Hot,
                price_per_gib: XorAmount::from_micro(price_micro),
                quantity_gib: 4,
                remaining_gib: 4,
                owner_account: owner.to_vec(),
                expiry_unix: 1_800_000_100,
                nonce: u64::from(id),
                maker_fee_bps: 10,
                taker_fee_bps: 20,
                signature: orderbook_signature(),
            },
            id.saturating_add(0x20),
        )
    }

    fn orderbook_cancel(id: u8, owner: &[u8]) -> OrderCancelV1 {
        sign_orderbook_cancel(
            OrderCancelV1 {
                version: ORDERBOOK_CANCEL_VERSION_V1,
                order_id: [id; 32],
                owner_account: owner.to_vec(),
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
                xor_debited: XorAmount::from_micro(debited_micro),
                provider_credit: XorAmount::from_micro(debited_micro.saturating_sub(10)),
                fee_amount: XorAmount::from_micro(10),
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

    #[test]
    fn node_handle_orderbook_matches_crossing_orders_and_records_snapshot() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let now = 1_800_000_000;

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
    fn node_handle_orderbook_cancels_owner_order_only() {
        let cfg = StorageConfig::builder().enabled(false).build();
        let handle = NodeHandle::new(cfg);
        let now = 1_800_000_000;
        let ask = orderbook_order(3, OrderSideV1::Ask, 1_500_000, b"provider");
        handle.submit_orderbook_order(ask, now).expect("accept ask");

        assert!(matches!(
            handle.cancel_orderbook_order(orderbook_cancel(3, b"other"), now),
            Err(OrderbookRuntimeError::CancelOwnerMismatch)
        ));

        let outcome = handle
            .cancel_orderbook_order(orderbook_cancel(3, b"provider"), now)
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
            channel.xor_locked.as_micro(),
        );

        let outcome = handle
            .submit_orderbook_receipt(receipt, now.saturating_add(10))
            .expect("apply receipt");

        assert_eq!(outcome.updated_channel.remaining_bytes, 0);
        assert_eq!(outcome.open_settlement_channel_count, 0);
        assert_eq!(
            handle
                .orderbook_snapshot(now.saturating_add(10))
                .settlement_receipts
                .len(),
            1
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
        let first_debit = channel.xor_locked.as_micro() / 2;
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
            .as_micro()
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
                    updated_channel.xor_locked.as_micro(),
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
        assert_eq!(event.order_id, Some([12; 32]));
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
    fn node_handle_moderation_tally_publishes_appeal_finance_report_for_confirmed_deposit() {
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

        handle.deposit_provider_bond(provider_id, 3_000_000_000);
        handle.deposit_client_credit(client_id, 1_000_000_000);

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
        assert_eq!(settlement.expected_charge_nano, 850_000_000);
        assert_eq!(settlement.micropayment_credit_nano, 250_000_000);
        assert_eq!(settlement.client_credit_debit_nano, 600_000_000);
        assert_eq!(settlement.bond_slash_nano, 0);
        assert_eq!(settlement.outstanding_nano, 0);

        let governance = &outcome.governance;
        assert_eq!(governance.deal_id, *record.deal_id.as_bytes());
        assert_eq!(governance.status, DealSettlementStatusV1::Completed);
        assert_eq!(governance.settled_at, activation_epoch + 7);
        let ledger = &governance.ledger;
        assert_eq!(ledger.deal_id, *record.deal_id.as_bytes());
        assert_eq!(ledger.provider_id, *provider_id.as_bytes());
        assert_eq!(ledger.client_id, *client_id.as_bytes());
        assert_eq!(ledger.provider_accrual.as_micro(), 850_000);
        assert_eq!(ledger.client_liability.as_micro(), 850_000);
        assert_eq!(ledger.bond_locked.as_micro(), 2_400_000);
        assert_eq!(ledger.bond_slashed.as_micro(), 0);
        assert_eq!(ledger.captured_at, activation_epoch + 7);

        let snapshot = handle.deal_snapshot(record.deal_id).expect("snapshot");
        assert!(matches!(snapshot.status, DealStatus::Active(_)));

        let provider_snapshot = handle
            .deal_engine()
            .provider_snapshot(provider_id)
            .expect("provider snapshot");
        assert!(provider_snapshot.bond_locked_nano >= 2_400_000_000);
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

        handle.deposit_provider_bond(provider_id, 3_000_000_000);
        handle.deposit_client_credit(client_id, 1_000_000_000);

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
    fn settle_deal_writes_filesystem_governance_payloads() {
        let temp = tempfile::tempdir().expect("temp dir");
        let governance_dir = temp.path().join("governance");
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp.path().join("storage"))
            .governance_dir(Some(governance_dir.clone()))
            .build();
        let handle = NodeHandle::new(cfg);

        let provider_id = ProviderId::new([0x10; 32]);
        let client_id = ClientId::new([0x20; 32]);

        handle.deposit_provider_bond(provider_id, 1_000_000_000);
        handle.deposit_client_credit(client_id, 1_000_000_000);

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

        handle.deposit_provider_bond(provider_id, 3_000_000_000);
        handle.deposit_client_credit(client_id, 1_000_000_000);

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
            .record_por_proof(&proof)
            .expect("record proof succeeds");
        let verdict = por_sample_verdict(&challenge, proof.proof_digest());
        handle
            .record_por_verdict(&verdict)
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
        handle
            .record_por_verdict(&verdict)
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
        handle
            .record_por_verdict(&verdict)
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

        handle.deposit_provider_bond(provider, 10_000);

        let mut verdict = por_sample_verdict(&challenge, [0; 32]);
        verdict.outcome = AuditOutcomeV1::Failed;
        verdict.failure_reason = Some("proof missing".to_string());
        verdict.proof_digest = None;

        handle
            .record_por_challenge(&challenge)
            .expect("record first challenge");
        let first = handle
            .record_por_verdict(&verdict)
            .expect("record first failure");
        assert_eq!(first.consecutive_failures, 1);
        assert!(first.slash.is_none());

        // Reuse the same challenge identifier after the tracker clears it.
        let mut second_verdict = verdict.clone();
        second_verdict.decided_at += 10;
        handle
            .record_por_challenge(&challenge)
            .expect("record second challenge");
        let second = handle
            .record_por_verdict(&second_verdict)
            .expect("record second failure");

        let slash = second.slash.expect("slash recommendation expected");
        assert_eq!(slash.provider_id, provider);
        assert_eq!(slash.manifest_digest, challenge.manifest_digest);
        assert_eq!(slash.penalty_nano, 5_000);
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
        handle.deposit_provider_bond(provider, 2_000);

        let mut verdict = por_sample_verdict(&challenge, [0; 32]);
        verdict.outcome = AuditOutcomeV1::Failed;
        verdict.failure_reason = Some("timeout".to_string());
        verdict.proof_digest = None;

        handle
            .record_por_challenge(&challenge)
            .expect("record challenge");
        let first = handle.record_por_verdict(&verdict).expect("record verdict");
        assert!(first.slash.is_some());

        // Cooldown prevents an immediate second slash even though the strike threshold is 1.
        let mut later_verdict = verdict.clone();
        later_verdict.decided_at += 120;
        handle
            .record_por_challenge(&challenge)
            .expect("record challenge after cooldown start");
        let second = handle
            .record_por_verdict(&later_verdict)
            .expect("record verdict during cooldown");
        assert!(second.slash.is_none());
        assert_eq!(second.consecutive_failures, 1);
    }

    #[test]
    fn node_handle_manages_repair_queue() {
        let (cfg, _dir) = storage_config_with_temp_dir();
        let handle = NodeHandle::new(cfg);

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
                cause: RepairCauseV1::PorFailure(RepairPorFailureCauseV1 {
                    challenge_id: [0xAA; 32],
                    failed_samples: 4,
                    proof_digest: None,
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

        let tasks = handle.repair_tasks_for_manifest(&report.evidence.manifest_digest);
        assert_eq!(tasks.len(), 1);
        let provider_tasks = handle.repair_tasks(RepairTaskFilters {
            provider_id: Some(report.evidence.provider_id),
            ..RepairTaskFilters::default()
        });
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
                cause: RepairCauseV1::PorFailure(RepairPorFailureCauseV1 {
                    challenge_id: [0xAB; 32],
                    failed_samples: 6,
                    proof_digest: None,
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
            proposed_penalty_nano: 500_000_000,
            submitted_at_unix: escalated_report.submitted_at_unix + 1_200,
            rationale: "Repeated PoR failures without acknowledgement".into(),
            approval: Some(approval_for_default_policy(
                escalated_report.submitted_at_unix + 1_200,
            )),
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
            default_slash_penalty_nano: 8_000,
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
            default_slash_penalty_nano: 9_000,
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
            .expect("completed task");
        assert!(matches!(
            completed.state,
            RepairTaskStateV1::Completed(CompletedRepairStateV1 { .. })
        ));
        let escalated = handle
            .repair_task_record(&report_missing.ticket_id)
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
        let cfg = StorageConfig::builder()
            .enabled(true)
            .data_dir(temp_dir.path().join("storage"))
            .governance_dir(Some(temp_dir.path().join("governance")))
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
    fn hard_fork_bootstrap_flag_is_presence_based() {
        assert!(!hard_fork_snapshot_bootstrap_enabled_from(None));
        assert!(hard_fork_snapshot_bootstrap_enabled_from(Some(
            std::ffi::OsString::new()
        )));
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
            default_slash_penalty_nano: 42_000,
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
        assert_eq!(handle.repair_config().default_slash_penalty_nano(), 42_000);

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
            drand_signature: vec![0x44; 96],
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

        let mut vrf_records = HashMap::new();
        vrf_records.insert(
            forced.manifest_digest,
            ManifestVrfBundle {
                output: [0x55; 32],
                proof: vec![0x66; 80],
            },
        );

        let plans_with_vrf = handle
            .plan_por_challenges(randomness, &vrf_records)
            .expect("vrf-backed challenge");
        let satisfied = &plans_with_vrf[0].challenge;
        assert!(!satisfied.forced);
        assert_eq!(satisfied.vrf_output, Some([0x55; 32]));
        assert_eq!(satisfied.sample_count, 128);
        assert!(
            satisfied
                .vrf_proof
                .as_ref()
                .is_some_and(|proof| proof.len() == 80)
        );
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
            drand_signature: vec![0x66; 96],
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
    }
}
