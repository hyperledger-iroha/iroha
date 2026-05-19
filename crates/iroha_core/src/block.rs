//! Modeling block transitions.
//!
//! Operations on blocks:
//!
//! 1. Static analysis of the block. This is a _fallible_ operation
//! 2. Execution of transactions and time triggers. This is an _infallible_ operation. If there are errors during
//!    transaction execution, they are recorded in the block.
//! 3. Voting
//! 4. Pre-commit signatures check
//! 5. Apply & commit
//!
//! Operations 1 + 2 form a process we call _validation_.
//!
//! Block lifecycle stages:
//!
//! 1. Block is created by the node ([`NewBlock`]). Such blocks are assumed to be valid and do not
//!    require static validation to transform to [`ValidBlock`].
//! 2. Block is received/deserialized from disk (as [`SignedBlock`]). Such blocks require static
//!    validation before execution to transition to [`ValidBlock`].
//! 3. Block is valid ([`ValidBlock`]). It is always created in pair with [`crate::state::StateBlock`]
//!    containing the applied state changes from the block. Transaction errors are written to the
//!    block.
//! 4. Voting block ([`VotingBlock`]). Valid block might not have sufficient signatures to be committed.
//!    Voting block is a wrappper around [`ValidBlock`] and its [`crate::state::StateBlock`] intended to
//!    collect the signatures in order to transition to [`CommittedBlock`]
//! 5. Block is committed ([`CommittedBlock`]). Created from [`ValidBlock`], ensuring the
//!    signatures meet the conditions for commit (e.g. quorum across Set A + Set B validators).
//!
//! ### Scenario: this node creates a block
//!
//! Flow: [`BlockBuilder::new`], [`BlockBuilder::chain`], [`BlockBuilder::sign`],
//! [`NewBlock::validate_and_record_transactions`] (infallible), [`VotingBlock::new`], [`ValidBlock::commit`]
//!
//! ### Scenario: receive a created block
//!
//! Flow: Having [`SignedBlock`], [`ValidBlock::validate_keep_voting_block`], [`VotingBlock::new`],
//! [`ValidBlock::commit`]
//!
//! ### Scenario: receive a block via block sync
//!
//! Flow: Having [`SignedBlock`], [`ValidBlock::commit_keep_voting_block`]
//!
//! ### Scenario: genesis (init or receive), replay kura blocks
//!
//! Flow: Having [`SignedBlock`], [`ValidBlock::validate`], [`ValidBlock::commit`]
//!
//! ### Scenario: plain block execution
//!
//! Flow: Having [`SignedBlock`], [`ValidBlock::validate_unchecked`] (infallible),
//! [`ValidBlock::commit_unchecked`] (infallible)
use core::fmt;
#[cfg(feature = "bls")]
use std::sync::LazyLock;
use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, HashSet},
    hint::black_box,
    str::FromStr,
    time::Duration,
};

use iroha_config::parameters::actual::{ConsensusMode, SumeragiNpos};
use iroha_crypto::{HashOf, KeyPair, MerkleTree, PublicKey};
#[cfg(feature = "bls")]
use iroha_data_model::metadata::Metadata;
use iroha_data_model::{
    ChainId,
    account::{AccountController, AccountId, rekey::AccountAlias},
    asset::{AssetDefinitionAlias, AssetDefinitionId, AssetId},
    block::{
        consensus::{
            LaneBlockCommitment, LaneSettlementReceipt, NativeAmxLegRecord, NativeAmxReceipt,
        },
        *,
    },
    confidential::ConfidentialFeatureDigest,
    consensus::{
        ConsensusKeyRole, NposConsensusEffects, PreviousRosterEvidence,
        VALIDATOR_SET_HASH_VERSION_V1, VrfEpochRecord, VrfParticipantRecord,
    },
    da::{
        commitment::{DaCommitmentBundle, DaProofPolicyBundle},
        pin_intent::DaPinIntentBundle,
    },
    domain::DomainId,
    events::prelude::*,
    isi::{InstructionBox, RemoveKeyValueBox, SetKeyValueBox, transfer::TransferBox},
    nexus::{
        AssetHandle, AxtHandleReplayKey, AxtPolicyEntry, AxtProofEnvelope, AxtRejectReason,
        DataSpaceCatalog, DataSpaceId, LaneConfig, LaneId, LaneRelayEnvelope, ProofBlob,
    },
    peer::PeerId,
    transaction::{
        SignedTransaction, TransactionEntrypoint,
        error::{TransactionLimitError, TransactionRejectionReason},
        signed::TransactionResultInner,
    },
};
use iroha_primitives::{numeric::Numeric, small::SmallVec};
#[cfg(feature = "telemetry")]
use iroha_telemetry::metrics::NexusLaneTeuBuckets;
#[cfg(feature = "telemetry")]
use ivm::ProgramMetadata;
use mv::storage::StorageReadOnly;
#[cfg(feature = "bls")]
use norito::json::Value as JsonValue;
use rust_decimal::Decimal;
use sha2::Digest as _;

#[cfg(feature = "bls")]
fn bls_pop_from_metadata(
    metadata: &Metadata,
    key: &iroha_data_model::name::Name,
) -> Option<Vec<u8>> {
    let json = metadata.get(key)?;
    let val: JsonValue = norito::json::from_str(json.get()).ok()?;
    match val {
        JsonValue::String(s) => hex::decode(s).ok(),
        _ => None,
    }
}

#[cfg(feature = "bls")]
fn bls_small_pop_from_metadata(
    metadata: &Metadata,
    key: &iroha_data_model::name::Name,
) -> Option<Vec<u8>> {
    bls_pop_from_metadata(metadata, key)
}

/// Convert overlay build errors into transaction rejection reasons with stable labels.
fn map_overlay_error(
    err: &crate::pipeline::overlay::OverlayBuildError,
) -> TransactionRejectionReason {
    match err {
        crate::pipeline::overlay::OverlayBuildError::HeaderPolicy(e) => {
            TransactionRejectionReason::Validation(iroha_data_model::ValidationFail::IvmAdmission(
                e.clone(),
            ))
        }
        crate::pipeline::overlay::OverlayBuildError::AxtReject(ctx) => {
            TransactionRejectionReason::Validation(iroha_data_model::ValidationFail::AxtReject(
                ctx.clone(),
            ))
        }
        crate::pipeline::overlay::OverlayBuildError::AmxBudgetViolation(violation) => {
            let message = crate::pipeline::overlay::amx_timeout_message(violation);
            TransactionRejectionReason::Validation(iroha_data_model::ValidationFail::NotPermitted(
                format!(
                    "{message} code={}",
                    iroha_data_model::errors::CanonicalErrorKind::AMX_TIMEOUT_CODE
                ),
            ))
        }
        crate::pipeline::overlay::OverlayBuildError::IvmRun(ivm::VMError::AmxBudgetExceeded {
            dataspace,
            stage,
            elapsed_ms,
            budget_ms,
        }) => TransactionRejectionReason::Validation(
            iroha_data_model::ValidationFail::NotPermitted(format!(
                "{} code={}",
                crate::pipeline::overlay::amx_timeout_message(
                    &crate::smartcontracts::ivm::host::AmxBudgetViolation {
                        dataspace: *dataspace,
                        stage: *stage,
                        elapsed_ms: u32::try_from((*elapsed_ms).min(u64::from(u32::MAX)))
                            .expect("elapsed_ms clamped to u32::MAX"),
                        budget_ms: u32::try_from((*budget_ms).min(u64::from(u32::MAX)))
                            .expect("budget_ms clamped to u32::MAX"),
                    }
                ),
                iroha_data_model::errors::CanonicalErrorKind::AMX_TIMEOUT_CODE
            )),
        ),
        other => TransactionRejectionReason::Validation(
            iroha_data_model::ValidationFail::NotPermitted(other.to_string()),
        ),
    }
}

fn missing_authority_requires_rejection(
    state_tx: &crate::state::StateTransaction<'_, '_>,
    tx: &SignedTransaction,
    authority: &AccountId,
    overlay_instruction_count: usize,
    is_genesis: bool,
) -> bool {
    overlay_instruction_count > 0
        && !is_genesis
        && state_tx.world.accounts.get(authority).is_none()
        && !crate::tx::allows_unregistered_authority(tx.instructions(), authority)
}

fn validate_block_transaction_admission(
    state_tx: &mut crate::state::StateTransaction<'_, '_>,
    tx: &SignedTransaction,
    routing: crate::queue::RoutingDecision,
) -> Result<crate::tx::StatefulAdmission, TransactionRejectionReason> {
    StateBlock::validate_stateful_admission(tx, state_tx, Some(routing))
}

fn commit_stateful_admission_sequence(
    state_tx: &mut crate::state::StateTransaction<'_, '_>,
    admission: &crate::tx::StatefulAdmission,
) {
    if let Some(seq) = admission.sequence_to_commit {
        state_tx
            .world
            .tx_sequences
            .insert(admission.authority.clone(), seq);
    }
}

fn commit_stateful_admission_sequence_to_block(
    state_block: &mut StateBlock<'_>,
    admission: &crate::tx::StatefulAdmission,
) {
    if admission.sequence_to_commit.is_none() {
        return;
    }
    let mut state_tx = state_block.transaction();
    commit_stateful_admission_sequence(&mut state_tx, admission);
    state_tx.apply();
}

#[cfg(test)]
mod overlay_error_tests {
    use iroha_data_model::{
        ValidationFail,
        nexus::{AxtRejectContext, AxtRejectReason, DataSpaceId, LaneId},
    };

    use super::*;

    #[test]
    fn map_overlay_error_preserves_axt_context() {
        let ctx = AxtRejectContext {
            reason: AxtRejectReason::Manifest,
            dataspace: Some(DataSpaceId::new(7)),
            lane: Some(LaneId::new(3)),
            snapshot_version: 42,
            detail: "manifest mismatch".to_string(),
            next_min_handle_era: None,
            next_min_sub_nonce: None,
        };
        let mapped = map_overlay_error(&crate::pipeline::overlay::OverlayBuildError::AxtReject(
            ctx.clone(),
        ));
        match mapped {
            TransactionRejectionReason::Validation(ValidationFail::AxtReject(seen)) => {
                assert_eq!(seen.reason, ctx.reason);
                assert_eq!(seen.dataspace, ctx.dataspace);
                assert_eq!(seen.lane, ctx.lane);
                assert_eq!(seen.snapshot_version, ctx.snapshot_version);
                assert!(seen.detail.contains("manifest"));
            }
            other => panic!("unexpected mapping: {other:?}"),
        }
    }
}

#[cfg(feature = "telemetry")]
const PIPELINE_LAYER_WIDTH_THRESHOLDS: [u64; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
const EMPTY_CONFIDENTIAL_FEATURE_DIGEST: ConfidentialFeatureDigest =
    iroha_data_model::confidential::DEFAULT_CONFIDENTIAL_FEATURE_DIGEST;
#[cfg(feature = "telemetry")]
use settlement_router::haircut::LiquidityProfile;
use settlement_router::{MicroXor, policy::BufferStatus};
use thiserror::Error;

#[cfg(test)]
pub(crate) use self::event::EventProducer;
pub(crate) use self::event::WithEvents;
pub use self::{chained::Chained, commit::CommittedBlock, new::NewBlock, valid::ValidBlock};
#[cfg(feature = "telemetry")]
use crate::telemetry::{
    DataspacePipelineSummary, DataspaceTeuGaugeUpdate, LanePipelineSummary, LaneTeuGaugeUpdate,
    SchedulerLayerWidthBuckets,
};
use crate::{da::DaShardCursorError, fees::SwapEvidence};

#[derive(Default, Clone, Copy)]
struct DetachedFallbackReasons {
    fee_postprocessing: u64,
    user_executor: u64,
    durable_state: u64,
    unsupported_instruction: u64,
    rejected_eval: u64,
    overlay_error: u64,
}

impl DetachedFallbackReasons {
    fn add(&mut self, reason: DetachedFallbackReason) {
        match reason {
            DetachedFallbackReason::FeePostprocessing => {
                self.fee_postprocessing = self.fee_postprocessing.saturating_add(1);
            }
            DetachedFallbackReason::UserExecutor => {
                self.user_executor = self.user_executor.saturating_add(1);
            }
            DetachedFallbackReason::DurableState => {
                self.durable_state = self.durable_state.saturating_add(1);
            }
            DetachedFallbackReason::UnsupportedInstruction => {
                self.unsupported_instruction = self.unsupported_instruction.saturating_add(1);
            }
            DetachedFallbackReason::RejectedEval => {
                self.rejected_eval = self.rejected_eval.saturating_add(1);
            }
            DetachedFallbackReason::OverlayError => {
                self.overlay_error = self.overlay_error.saturating_add(1);
            }
        }
    }

    fn merge(self, other: Self) -> Self {
        Self {
            fee_postprocessing: self
                .fee_postprocessing
                .saturating_add(other.fee_postprocessing),
            user_executor: self.user_executor.saturating_add(other.user_executor),
            durable_state: self.durable_state.saturating_add(other.durable_state),
            unsupported_instruction: self
                .unsupported_instruction
                .saturating_add(other.unsupported_instruction),
            rejected_eval: self.rejected_eval.saturating_add(other.rejected_eval),
            overlay_error: self.overlay_error.saturating_add(other.overlay_error),
        }
    }
}

#[derive(Clone, Copy)]
enum DetachedFallbackReason {
    FeePostprocessing,
    UserExecutor,
    DurableState,
    UnsupportedInstruction,
    RejectedEval,
    OverlayError,
}

fn transaction_requires_fee_postprocessing(
    pipeline_cfg: &iroha_config::parameters::actual::Pipeline,
    nexus_cfg: &iroha_config::parameters::actual::Nexus,
    tx: &iroha_data_model::transaction::SignedTransaction,
) -> bool {
    if !pipeline_cfg.gas.accepted_assets.is_empty() {
        return true;
    }
    if tx.metadata().get("gas_asset_id").is_some() {
        return true;
    }
    if nexus_cfg.enabled {
        let fees = &nexus_cfg.fees;
        if fees.base_fee > Numeric::zero()
            || fees.per_byte_fee > Numeric::zero()
            || fees.per_instruction_fee > Numeric::zero()
            || fees.per_gas_unit_fee > Numeric::zero()
        {
            return true;
        }
    }
    false
}

#[derive(Default)]
struct LaneSummary {
    tx_vertices: u64,
    tx_edges: u64,
    overlay_count: u64,
    overlay_instr_total: u64,
    overlay_bytes_total: u64,
    rbc_chunks: u64,
    rbc_bytes_total: u64,
    layer_widths: Vec<u64>,
    peak_layer_width: u64,
    detached_prepared: u64,
    detached_merged: u64,
    detached_fallback: u64,
    detached_fallback_reasons: DetachedFallbackReasons,
    quarantine_executed: u64,
}

fn set_pipeline_status_snapshots(lane_summaries: &BTreeMap<LaneId, LaneSummary>) {
    let lane_activity_snapshot: Vec<crate::sumeragi::status::LaneActivitySnapshot> = lane_summaries
        .iter()
        .map(
            |(lane_id, summary)| crate::sumeragi::status::LaneActivitySnapshot {
                lane_id: lane_id.as_u32(),
                tx_vertices: summary.tx_vertices,
                tx_edges: summary.tx_edges,
                overlay_count: summary.overlay_count,
                overlay_instr_total: summary.overlay_instr_total,
                overlay_bytes_total: summary.overlay_bytes_total,
                rbc_chunks: summary.rbc_chunks,
                rbc_bytes_total: summary.rbc_bytes_total,
                detached_prepared: summary.detached_prepared,
                detached_merged: summary.detached_merged,
                detached_fallback: summary.detached_fallback,
                detached_fallback_fee_postprocessing: summary
                    .detached_fallback_reasons
                    .fee_postprocessing,
                detached_fallback_user_executor: summary.detached_fallback_reasons.user_executor,
                detached_fallback_durable_state: summary.detached_fallback_reasons.durable_state,
                detached_fallback_unsupported_instruction: summary
                    .detached_fallback_reasons
                    .unsupported_instruction,
                detached_fallback_rejected_eval: summary.detached_fallback_reasons.rejected_eval,
                detached_fallback_overlay_error: summary.detached_fallback_reasons.overlay_error,
                quarantine_executed: summary.quarantine_executed,
            },
        )
        .collect();
    let detached_fallback_reasons_total = lane_summaries
        .values()
        .fold(DetachedFallbackReasons::default(), |acc, summary| {
            acc.merge(summary.detached_fallback_reasons)
        });
    let pipeline_execution_snapshot = crate::sumeragi::status::PipelineExecutionSnapshot {
        tx_vertices_total: lane_summaries.values().map(|s| s.tx_vertices).sum(),
        tx_edges_total: lane_summaries.values().map(|s| s.tx_edges).sum(),
        overlay_count_total: lane_summaries.values().map(|s| s.overlay_count).sum(),
        overlay_instr_total: lane_summaries.values().map(|s| s.overlay_instr_total).sum(),
        overlay_bytes_total: lane_summaries.values().map(|s| s.overlay_bytes_total).sum(),
        rbc_chunks_total: lane_summaries.values().map(|s| s.rbc_chunks).sum(),
        rbc_bytes_total: lane_summaries.values().map(|s| s.rbc_bytes_total).sum(),
        detached_prepared_total: lane_summaries.values().map(|s| s.detached_prepared).sum(),
        detached_merged_total: lane_summaries.values().map(|s| s.detached_merged).sum(),
        detached_fallback_total: lane_summaries.values().map(|s| s.detached_fallback).sum(),
        detached_fallback_fee_postprocessing_total: detached_fallback_reasons_total
            .fee_postprocessing,
        detached_fallback_user_executor_total: detached_fallback_reasons_total.user_executor,
        detached_fallback_durable_state_total: detached_fallback_reasons_total.durable_state,
        detached_fallback_unsupported_instruction_total: detached_fallback_reasons_total
            .unsupported_instruction,
        detached_fallback_rejected_eval_total: detached_fallback_reasons_total.rejected_eval,
        detached_fallback_overlay_error_total: detached_fallback_reasons_total.overlay_error,
        quarantine_executed_total: lane_summaries.values().map(|s| s.quarantine_executed).sum(),
    };
    crate::sumeragi::status::set_lane_activity_snapshot(lane_activity_snapshot);
    crate::sumeragi::status::set_pipeline_execution_snapshot(pipeline_execution_snapshot);
}

#[derive(Default)]
struct LaneSettlementBuilder {
    tx_count: u64,
    total_local_micro: u128,
    total_xor_due_micro: u128,
    total_xor_after_haircut_micro: u128,
    total_xor_variance_micro: u128,
    swap_evidence: Option<SwapEvidence>,
    receipts: Vec<LaneSettlementReceipt>,
    nexus_fee_receipts: Vec<iroha_data_model::block::consensus::NexusFeeReceipt>,
    native_amx_receipts: Vec<NativeAmxReceipt>,
    buffer_snapshot: Option<SettlementBufferSnapshot>,
    source_counts: BTreeMap<AssetDefinitionId, u64>,
}

fn native_amx_receipt_for_transaction(
    tx: &SignedTransaction,
    tx_hash: HashOf<SignedTransaction>,
    block_height: u64,
    decision: crate::queue::RoutingDecision,
    dataspace_catalog: &iroha_data_model::nexus::DataSpaceCatalog,
    world: &impl WorldReadOnly,
) -> Option<NativeAmxReceipt> {
    if decision.dataspace_id != iroha_data_model::nexus::DataSpaceId::UNIVERSAL {
        return None;
    }

    let accepted = crate::tx::AcceptedTransaction::new_unchecked(Cow::Borrowed(tx));
    let participants =
        native_amx_participant_dataspaces_with_world(&accepted, dataspace_catalog, world);
    let participant_legs: Vec<_> = participants
        .into_iter()
        .filter(|dataspace| *dataspace != iroha_data_model::nexus::DataSpaceId::UNIVERSAL)
        .collect();
    if participant_legs.len() < 2 {
        return None;
    }

    let mut source_id = [0u8; iroha_crypto::Hash::LENGTH];
    source_id.copy_from_slice(tx_hash.as_ref());
    let legs = participant_legs
        .into_iter()
        .map(|dataspace_id| NativeAmxLegRecord {
            dataspace_id,
            prepared: true,
            committed: true,
        })
        .collect();

    Some(NativeAmxReceipt {
        version: 1,
        source_id,
        lane_id: decision.lane_id,
        dataspace_id: decision.dataspace_id,
        block_height,
        legs,
    })
}

fn lane_relay_envelopes_for_block(
    block_header: &BlockHeader,
    da_commitment_hash: Option<HashOf<DaCommitmentBundle>>,
    lane_settlement_commitments: &[LaneBlockCommitment],
    lane_summaries: &BTreeMap<LaneId, LaneSummary>,
) -> Vec<LaneRelayEnvelope> {
    lane_settlement_commitments
        .iter()
        .map(|commitment| {
            let rbc_bytes_total = lane_summaries
                .get(&commitment.lane_id)
                .map_or(0, |summary| summary.rbc_bytes_total);

            LaneRelayEnvelope::new(
                *block_header,
                None,
                da_commitment_hash,
                commitment.clone(),
                rbc_bytes_total,
            )
            .expect("construct lane relay envelope from settlement commitment")
        })
        .collect()
}

fn attach_manifest_roots_to_relays(
    envelopes: &mut [LaneRelayEnvelope],
    manifest_roots: &BTreeMap<DataSpaceId, [u8; 32]>,
) {
    for envelope in envelopes {
        envelope.manifest_root = manifest_roots.get(&envelope.dataspace_id).copied();
    }
}

#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
#[derive(Clone)]
struct LaneSettlementBufferConfig {
    account_id: AccountId,
    asset_definition_id: AssetDefinitionId,
    capacity: MicroXor,
}

#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
#[derive(Clone)]
pub(crate) struct SettlementBufferSnapshot {
    config: LaneSettlementBufferConfig,
    remaining: MicroXor,
    status: BufferStatus,
}

#[cfg_attr(not(feature = "telemetry"), allow(dead_code))]
impl SettlementBufferSnapshot {
    pub(crate) fn remaining(&self) -> &MicroXor {
        &self.remaining
    }

    pub(crate) fn capacity(&self) -> &MicroXor {
        &self.config.capacity
    }

    pub(crate) fn status(&self) -> BufferStatus {
        self.status
    }
}

fn parse_lane_settlement_buffer_config(
    world: &impl WorldReadOnly,
    dataspace_catalog: &DataSpaceCatalog,
    lane: &LaneConfig,
) -> Option<LaneSettlementBufferConfig> {
    let account_raw = lane
        .metadata
        .get("settlement.buffer_account")
        .or_else(|| lane.metadata.get("settlement.buffer"));
    let asset_raw = lane.metadata.get("settlement.buffer_asset");
    let capacity_raw = lane.metadata.get("settlement.buffer_capacity_micro");

    let (account_raw, asset_raw, capacity_raw) = match (account_raw, asset_raw, capacity_raw) {
        (Some(account), Some(asset), Some(capacity)) => (account, asset, capacity),
        _ => return None,
    };

    let account_id = parse_account_literal_with_world(world, dataspace_catalog, account_raw)?;
    let asset_definition_id = AssetDefinitionId::parse_address_literal(asset_raw).ok()?;
    let capacity = MicroXor::from(Decimal::from_str(capacity_raw.trim()).ok()?);

    Some(LaneSettlementBufferConfig {
        account_id,
        asset_definition_id,
        capacity,
    })
}

fn compute_settlement_buffer_snapshot(
    state_block: &StateBlock,
    lane_id: LaneId,
) -> Option<SettlementBufferSnapshot> {
    let lane = lane_metadata_by_id(state_block, lane_id)?;
    let config = parse_lane_settlement_buffer_config(
        &state_block.world,
        &state_block.nexus.dataspace_catalog,
        lane,
    )?;
    let asset_id = AssetId::new(
        config.asset_definition_id.clone(),
        config.account_id.clone(),
    );
    let assets = state_block.world.assets();
    let remaining = assets
        .get(&asset_id)
        .and_then(|value| numeric_to_decimal(value.as_ref()))
        .map_or(MicroXor::ZERO, MicroXor::from);

    let status = state_block
        .settlement_engine()
        .evaluate_buffer(&remaining, &config.capacity);

    Some(SettlementBufferSnapshot {
        config,
        remaining,
        status,
    })
}

fn lane_metadata_by_id<'state>(
    state_block: &'state StateBlock<'state>,
    lane_id: LaneId,
) -> Option<&'state LaneConfig> {
    state_block
        .nexus
        .lane_catalog
        .lanes()
        .iter()
        .find(|lane| lane.id == lane_id)
}

fn numeric_to_decimal(value: &Numeric) -> Option<Decimal> {
    let mantissa = value.try_mantissa_i128()?;
    let scale = value.scale();
    decimal_from_i128_with_scale(mantissa, scale)
}

fn decimal_from_i128_with_scale(mantissa: i128, scale: u32) -> Option<Decimal> {
    const MAX_SCALE: u32 = 28;
    if scale > MAX_SCALE {
        return None;
    }
    let negative = mantissa.is_negative();
    let magnitude = mantissa.checked_abs()?;
    let magnitude = u128::try_from(magnitude).ok()?;
    if magnitude >> 96 != 0 {
        return None;
    }
    let lo = u32::try_from(magnitude & 0xFFFF_FFFF).ok()?;
    let mid = u32::try_from((magnitude >> 32) & 0xFFFF_FFFF).ok()?;
    let hi = u32::try_from((magnitude >> 64) & 0xFFFF_FFFF).ok()?;
    Some(Decimal::from_parts(lo, mid, hi, negative, scale))
}

#[cfg(feature = "telemetry")]
fn liquidity_profile_label(profile: LiquidityProfile) -> &'static str {
    match profile {
        LiquidityProfile::Tier1 => "tier1-deep",
        LiquidityProfile::Tier2 => "tier2-medium",
        LiquidityProfile::Tier3 => "tier3-thin",
    }
}

#[cfg(feature = "telemetry")]
fn record_lane_settlement_metrics(
    telemetry: &crate::telemetry::StateTelemetry,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    builder: &LaneSettlementBuilder,
) {
    let swapline = builder.swap_evidence.as_ref().map(|e| {
        (
            liquidity_profile_label(e.liquidity_profile),
            builder.total_xor_due_micro,
        )
    });
    let haircut_bps = builder.swap_evidence.as_ref().map_or(0, |e| e.epsilon_bps);
    telemetry.record_lane_settlement_snapshot_metrics(
        lane_id,
        dataspace_id,
        builder.total_xor_due_micro,
        builder.total_xor_variance_micro,
        haircut_bps,
        swapline,
        builder.buffer_snapshot.as_ref(),
    );
    let lane_label = lane_id.as_u32().to_string();
    let dataspace_label = dataspace_id.as_u64().to_string();
    telemetry.inc_settlement_haircut_total(
        lane_label.as_str(),
        dataspace_label.as_str(),
        builder.total_xor_variance_micro,
    );
    for (asset_id, count) in &builder.source_counts {
        if *count == 0 {
            continue;
        }
        let asset_label = asset_id.to_string();
        telemetry.inc_settlement_conversion_total(
            lane_label.as_str(),
            dataspace_label.as_str(),
            asset_label.as_str(),
            *count,
        );
    }
}
// Quarantine lane: classification hook (opt-in).
// By default, no transaction is classified as quarantine.
// Tests or embedding code may set a classifier at runtime.
use std::sync::{Arc, Mutex, OnceLock};

#[cfg(feature = "telemetry")]
use crate::queue::{LaneSchedulingLimits, QueueLimits};
use crate::{
    da::proof_policy_bundle_hash,
    executor::{charge_fees_for_applied_overlay_with_encoded_len, configure_executor_fuel_budget},
    kura::{PipelineDagSnapshot, PipelineRecoverySidecar, PipelineTxSnapshot},
    pipeline::{
        gpu::{self, AccessTriplet},
        overlay::TxOverlay,
        smallset::sort_dedup_u32_in_place,
    },
    prelude::*,
    queue::{
        evaluate_policy_with_catalog_and_world, native_amx_participant_dataspaces_with_world,
        resolve_routing_decision, routing_ledger,
    },
    smartcontracts::isi::triggers::{set::SetReadOnly, specialized::LoadedActionTrait},
    state::{
        State, StateBlock, StatelessValidationContext, WorldReadOnly,
        compute_confidential_feature_digest,
    },
    sumeragi::{VotingBlock, network_topology::Topology, status},
    tx::{
        AcceptTransactionFail, LaneAssignment, SignatureRejectionCode, SignatureVerificationFail,
        enforce_fraud_policy,
    },
};
type QuarantineClassifier = fn(&iroha_data_model::transaction::SignedTransaction) -> bool;
type CommittedBlockEval = Result<CommittedBlock, (Box<ValidBlock>, Box<BlockValidationError>)>;
type WithCommittedBlockEvents = WithEvents<CommittedBlockEval>;

struct PreparedBlockTransaction {
    metadata: crate::tx::PreparedTransactionMetadata,
}

static QUARANTINE_CLASSIFIER: OnceLock<Mutex<Option<QuarantineClassifier>>> = OnceLock::new();

/// Install a quarantine classifier hook. Passing `None` disables classification.
/// The classifier should be pure and deterministic (no side-effects, no randomness).
pub fn set_quarantine_classifier(f: Option<QuarantineClassifier>) {
    let slot = QUARANTINE_CLASSIFIER.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        *guard = f;
    }
}

#[derive(Clone)]
struct AccessIds {
    reads: SmallVec<[u32; 8]>,
    writes: SmallVec<[u32; 8]>,
}

const GLOBAL_WILDCARD_KEY: &str = "*";
const STATE_KEY_PREFIX: &str = "state:";
const STATE_WILDCARD_SUFFIX: &str = "[*]";

fn state_wildcard_base(key: &str) -> Option<&str> {
    let rest = key.strip_prefix(STATE_KEY_PREFIX)?;
    if rest == "*" {
        return Some("*");
    }
    rest.strip_suffix(STATE_WILDCARD_SUFFIX)
}

fn state_map_entry_base(key: &str) -> Option<&str> {
    let rest = key.strip_prefix(STATE_KEY_PREFIX)?;
    let (base, _) = rest.split_once('/')?;
    if base.is_empty() {
        return None;
    }
    Some(base)
}

fn state_wildcard_key(base: &str) -> String {
    if base == "*" {
        format!("{STATE_KEY_PREFIX}*")
    } else {
        format!("{STATE_KEY_PREFIX}{base}{STATE_WILDCARD_SUFFIX}")
    }
}

fn union_from_sorted_triplets(
    dsu: &mut DisjointSet,
    triplets: &[crate::pipeline::gpu::AccessTriplet],
) {
    use iroha_primitives::small::SmallVec;

    let mut cur_key: Option<u32> = None;
    let mut last_writer: Option<usize> = None;
    let mut open_readers: SmallVec<[usize; 8]> = SmallVec::new();
    for entry in triplets {
        if cur_key != Some(entry.key) {
            cur_key = Some(entry.key);
            last_writer = None;
            open_readers.0.clear();
        }

        if entry.flag == 0 {
            if let Some(writer) = last_writer {
                dsu.union(entry.tx_index, writer);
            }
            open_readers.push(entry.tx_index);
        } else {
            if let Some(writer) = last_writer {
                dsu.union(entry.tx_index, writer);
            }
            for &reader in open_readers.iter() {
                dsu.union(entry.tx_index, reader);
            }
            open_readers.0.clear();
            last_writer = Some(entry.tx_index);
        }
    }
}

#[allow(clippy::explicit_iter_loop)]
fn intern_access(access: &[crate::pipeline::access::AccessSet]) -> (usize, Vec<AccessIds>) {
    use std::collections::{BTreeMap, BTreeSet};

    let mut wildcard_bases: BTreeSet<String> = BTreeSet::new();
    let mut global_present = false;
    for aset in access.iter() {
        for key in aset.read_keys.iter().chain(aset.write_keys.iter()) {
            if key == GLOBAL_WILDCARD_KEY {
                global_present = true;
            }
            if let Some(base) = state_wildcard_base(key) {
                wildcard_bases.insert(base.to_string());
            }
        }
    }

    let mut wildcard_keys: BTreeMap<String, String> = BTreeMap::new();
    for base in &wildcard_bases {
        wildcard_keys.insert(base.clone(), state_wildcard_key(base));
    }

    let mut map: BTreeMap<&str, u32> = BTreeMap::new();
    // Assign stable IDs by iterating lexicographically over all keys
    for aset in access.iter() {
        for k in aset.read_keys.iter() {
            map.entry(k.as_str()).or_insert(u32::MAX);
        }
        for k in aset.write_keys.iter() {
            map.entry(k.as_str()).or_insert(u32::MAX);
        }
    }
    for k in wildcard_keys.values() {
        map.entry(k.as_str()).or_insert(u32::MAX);
    }
    if global_present {
        map.entry(GLOBAL_WILDCARD_KEY).or_insert(u32::MAX);
    }

    let mut next: u32 = 0;
    for value in map.values_mut() {
        *value = next;
        next = next.saturating_add(1);
    }

    let key_count = next as usize;
    let mut out: Vec<AccessIds> = Vec::with_capacity(access.len());
    for aset in access.iter() {
        let mut reads: SmallVec<[u32; 8]> = SmallVec::new();
        let mut writes: SmallVec<[u32; 8]> = SmallVec::new();
        let has_global = aset.read_keys.contains(GLOBAL_WILDCARD_KEY)
            || aset.write_keys.contains(GLOBAL_WILDCARD_KEY);
        let add_state_wildcard = |key: &str, reads: &mut SmallVec<[u32; 8]>| {
            if let Some(base) = state_map_entry_base(key) {
                if let Some(wildcard_key) = wildcard_keys.get(base) {
                    reads.push(*map.get(wildcard_key.as_str()).expect("key interned"));
                }
            }
            if wildcard_bases.contains("*") && key.starts_with(STATE_KEY_PREFIX) {
                if state_wildcard_base(key) == Some("*") {
                    return;
                }
                if let Some(wildcard_key) = wildcard_keys.get("*") {
                    reads.push(*map.get(wildcard_key.as_str()).expect("key interned"));
                }
            }
        };
        for key in aset.read_keys.iter() {
            if state_wildcard_base(key).is_some() {
                writes.push(*map.get(key.as_str()).expect("all keys interned"));
            } else {
                reads.push(*map.get(key.as_str()).expect("all keys interned"));
            }
            add_state_wildcard(key, &mut reads);
        }
        for key in aset.write_keys.iter() {
            writes.push(*map.get(key.as_str()).expect("all keys interned"));
            add_state_wildcard(key, &mut reads);
        }
        if has_global {
            writes.push(*map.get(GLOBAL_WILDCARD_KEY).expect("all keys interned"));
        } else if global_present {
            reads.push(*map.get(GLOBAL_WILDCARD_KEY).expect("all keys interned"));
        }
        let len_reads = sort_dedup_u32_in_place(reads.0.as_mut_slice());
        reads.0.truncate(len_reads);
        let len_writes = sort_dedup_u32_in_place(writes.0.as_mut_slice());
        writes.0.truncate(len_writes);
        out.push(AccessIds { reads, writes });
    }

    (key_count, out)
}

#[allow(clippy::explicit_iter_loop)]
fn dag_fingerprint(
    key_count: usize,
    access_ids: &[AccessIds],
    call_hashes: &[HashOf<TransactionEntrypoint>],
) -> [u8; 32] {
    use sha2::{Digest, Sha256};

    let mut h = Sha256::new();
    h.update((key_count as u64).to_le_bytes());
    for aset in access_ids.iter() {
        h.update((aset.reads.len() as u64).to_le_bytes());
        for &r in aset.reads.iter() {
            h.update(r.to_le_bytes());
        }
        h.update((aset.writes.len() as u64).to_le_bytes());
        for &w in aset.writes.iter() {
            h.update(w.to_le_bytes());
        }
    }
    for hash in call_hashes.iter() {
        h.update(hash.as_ref());
    }

    h.finalize().into()
}

fn expected_pipeline_dag_fingerprint(
    height: u64,
    block_hash: HashOf<BlockHeader>,
    call_hashes: &[HashOf<TransactionEntrypoint>],
    sidecar: &PipelineRecoverySidecar,
) -> Option<[u8; 32]> {
    if sidecar.height != height {
        iroha_logger::debug!(
            height,
            sidecar_height = sidecar.height,
            "pipeline sidecar height mismatch; ignoring expected DAG fingerprint"
        );
        return None;
    }
    if sidecar.block_hash != block_hash {
        iroha_logger::debug!(
            height,
            expected = %block_hash,
            actual = %sidecar.block_hash,
            "pipeline sidecar block hash mismatch; ignoring expected DAG fingerprint"
        );
        return None;
    }
    let matches_block = sidecar.txs.len() == call_hashes.len()
        && sidecar
            .txs
            .iter()
            .zip(call_hashes.iter())
            .all(|(tx, hash)| tx.hash == *hash);
    if !matches_block {
        iroha_logger::debug!(
            height,
            sidecar_txs = sidecar.txs.len(),
            block_txs = call_hashes.len(),
            "pipeline sidecar does not match block transactions; ignoring expected DAG fingerprint"
        );
        return None;
    }
    Some(sidecar.dag.fingerprint)
}

fn conflict_rate_bps(vertices: u64, edges: u64) -> u64 {
    if vertices < 2 {
        return 0;
    }
    let max_edges = u128::from(vertices) * u128::from(vertices - 1) / 2;
    if max_edges == 0 {
        return 0;
    }
    let bps = u128::from(edges).saturating_mul(10_000) / max_edges;
    u64::try_from(bps).unwrap_or(u64::MAX)
}

pub(crate) fn parse_account_literal_with_world(
    world: &impl WorldReadOnly,
    dataspace_catalog: &DataSpaceCatalog,
    input: &str,
) -> Option<AccountId> {
    let literal = input.trim();
    if literal.is_empty() {
        return None;
    }

    AccountId::parse_encoded(literal)
        .ok()
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .or_else(|| {
            let alias = AccountAlias::from_literal(literal, dataspace_catalog).ok()?;
            resolve_account_alias_in_world(world, &alias)
        })
}

pub(crate) fn resolve_account_alias_in_world(
    world: &impl WorldReadOnly,
    alias: &AccountAlias,
) -> Option<AccountId> {
    if let Some(account_id) = world.account_aliases().get(alias).cloned() {
        return Some(account_id);
    }

    let mut matched_account_id: Option<AccountId> = None;
    for (account_id, value) in world.accounts().iter() {
        if value.as_ref().label() != Some(alias) {
            continue;
        }
        if let Some(existing) = matched_account_id.as_ref() {
            if existing != account_id {
                return None;
            }
        } else {
            matched_account_id = Some(account_id.clone());
        }
    }

    matched_account_id
}

pub(crate) fn parse_asset_definition_literal_with_world(
    world: &impl WorldReadOnly,
    input: &str,
    now_ms: u64,
) -> Option<AssetDefinitionId> {
    let literal = input.trim();
    if literal.is_empty() {
        return None;
    }

    AssetDefinitionId::parse_address_literal(literal)
        .ok()
        .or_else(|| {
            AssetDefinitionAlias::from_str(literal)
                .ok()
                .and_then(|alias| world.asset_definition_id_by_alias_at(&alias, now_ms))
        })
}

fn parse_account_from_access_key(
    world: &impl WorldReadOnly,
    dataspace_catalog: &DataSpaceCatalog,
    key: &str,
) -> Option<AccountId> {
    if let Some(rest) = key.strip_prefix("account:") {
        parse_account_literal_with_world(world, dataspace_catalog, rest)
    } else if let Some(rest) = key.strip_prefix("account.detail:") {
        let (account_raw, _) = rest.split_once(':')?;
        parse_account_literal_with_world(world, dataspace_catalog, account_raw)
    } else {
        None
    }
}

fn warm_overlay_chunk(overlay: &TxOverlay, chunk_size: usize) -> usize {
    let chunk = chunk_size.max(1);
    let mut warmed = 0usize;
    for instr in overlay.instructions().take(chunk) {
        let _ = black_box(instr.id());
        warmed = warmed.saturating_add(1);
    }
    warmed
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct PrefetchStats {
    account_loaded: bool,
    tx_sequence_loaded: bool,
    permissions_touched: usize,
    roles_touched: usize,
}

fn prefetch_account_stores(state_block: &StateBlock<'_>, account_id: &AccountId) -> PrefetchStats {
    let mut stats = PrefetchStats::default();
    if let Some(account) = state_block.world.accounts.get(account_id) {
        let _ = black_box(account);
        stats.account_loaded = true;
    }

    if let Some(seq) = state_block.world.tx_sequences.get(account_id) {
        let _ = black_box(seq);
        stats.tx_sequence_loaded = true;
    }

    if let Some(perms) = state_block.world.account_permissions.get(account_id) {
        for perm in perms {
            let _ = black_box(perm);
            stats.permissions_touched = stats.permissions_touched.saturating_add(1);
        }
    }

    for (role, ()) in state_block.world.account_roles.iter() {
        if role.account == *account_id {
            let _ = black_box(role);
            stats.roles_touched = stats.roles_touched.saturating_add(1);
        }
    }
    stats
}

#[cfg(test)]
mod prefetch_tests {
    use iroha_data_model::{
        Registrable,
        account::{
            Account, AccountAlias, AccountAliasDomain, AccountDetails, AccountDomainSelector,
            AccountValue,
        },
        asset::AssetDefinitionId,
        block::BlockHeader,
        domain::{Domain, DomainId},
        isi::{InstructionBox, Log},
        name::Name,
        nexus::{DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, LaneConfig},
        role::RoleId,
    };
    use iroha_logger::Level;
    use iroha_test_samples::ALICE_ID;
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        role::RoleIdWithOwner,
        state::{State, World},
    };

    #[test]
    fn parse_account_key_variants() {
        let alice = (*ALICE_ID).clone();
        let wonderland: DomainId =
            DomainId::try_new("wonderland", "universal").expect("wonderland domain");
        let mut world = World::new();
        let selector =
            AccountDomainSelector::from_domain(&wonderland).expect("selector from domain");
        world.domain_selectors.insert(selector, wonderland);
        let world_view = world.view();
        let detail_key = format!("account.detail:{alice}:quota");
        let expected = alice.clone();

        assert_eq!(
            parse_account_from_access_key(
                &world_view,
                &DataSpaceCatalog::default(),
                &format!("account:{alice}")
            ),
            Some(expected.clone())
        );
        assert_eq!(
            parse_account_from_access_key(&world_view, &DataSpaceCatalog::default(), &detail_key),
            Some(expected.clone())
        );
        assert_eq!(expected.subject_id(), alice.subject_id());
        assert!(
            parse_account_from_access_key(
                &world_view,
                &DataSpaceCatalog::default(),
                "asset:coin#wonderland",
            )
            .is_none()
        );
    }

    #[test]
    fn parse_account_literal_rejects_i105_with_domain_suffix() {
        let alice = (*ALICE_ID).clone();
        let wonderland: DomainId =
            DomainId::try_new("wonderland", "universal").expect("wonderland domain");
        let domain = Domain::new(wonderland.clone()).build(&alice);
        let account = Account::new(alice.clone()).build(&alice);
        let world = World::with([domain], [account], []);
        let world_view = world.view();

        let i105 = alice.canonical_i105().expect("i105 encoding");
        let literal = format!("{i105}@{wonderland}");
        assert_eq!(
            parse_account_literal_with_world(&world_view, &DataSpaceCatalog::default(), &literal),
            None
        );
    }

    #[test]
    fn parse_account_literal_accepts_encoded_without_selector_registry() {
        let alice = (*ALICE_ID).clone();
        let wonderland: DomainId =
            DomainId::try_new("wonderland", "universal").expect("wonderland domain");
        let domain = Domain::new(wonderland.clone()).build(&alice);
        let account = Account::new(alice.clone()).build(&alice);
        let mut world = World::with([domain], [account], []);
        // Parsing should not depend on selector-index state.
        world.domain_selectors = Default::default();
        let world_view = world.view();

        let i105 = alice.canonical_i105().expect("i105 encoding");
        assert_eq!(
            parse_account_literal_with_world(&world_view, &DataSpaceCatalog::default(), &i105),
            Some(alice)
        );
    }

    #[test]
    fn parse_account_literal_accepts_canonical_i105_without_domain_materialization() {
        let account = (*ALICE_ID).clone();
        let alpha: DomainId = DomainId::try_new("alpha", "universal").expect("alpha domain");
        let world = World::with(
            [Domain::new(alpha).build(&account)],
            [Account::new(account.clone()).build(&account)],
            [],
        );
        let world_view = world.view();

        let encoded = account
            .canonical_i105()
            .expect("canonical I105 account literal");
        assert_eq!(
            parse_account_literal_with_world(&world_view, &DataSpaceCatalog::default(), &encoded),
            Some(account),
            "canonical I105 account ids must remain valid without domain-linked account materialization"
        );
    }

    #[test]
    fn parse_account_literal_resolves_on_chain_alias_literals() {
        let domain_id: DomainId = DomainId::try_new("ivm", "universal").expect("domain");
        let account_id = (*ALICE_ID).clone();
        let alias = AccountAlias::new(
            Name::from_str("gas").expect("alias name"),
            Some(AccountAliasDomain::new(domain_id.name().clone())),
            DataSpaceId::UNIVERSAL,
        );
        let world = World::with(
            [Domain::new(domain_id.clone()).build(&account_id)],
            [Account::new(account_id.clone())
                .with_label(Some(alias))
                .build(&account_id)],
            [],
        );
        let world_view = world.view();

        assert_eq!(
            parse_account_literal_with_world(
                &world_view,
                &DataSpaceCatalog::default(),
                "gas@ivm.universal",
            ),
            Some(account_id),
            "account selectors must resolve active on-chain aliases to canonical account ids"
        );
    }

    #[test]
    fn parse_account_literal_resolves_aliases_in_non_default_dataspaces() {
        let account_id = (*ALICE_ID).clone();
        let alias = AccountAlias::domainless(
            Name::from_str("treasury").expect("alias name"),
            DataSpaceId::new(7),
        );
        let world = World::with(
            [],
            [Account::new(account_id.clone())
                .with_label(Some(alias))
                .build(&account_id)],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query);
        state.nexus.write().dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata::default(),
            DataSpaceMetadata {
                id: DataSpaceId::new(7),
                alias: "retail".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        let state_view = state.view();
        let world_view = state_view.world();

        assert_eq!(
            parse_account_literal_with_world(
                world_view,
                &state_view.nexus.dataspace_catalog,
                "treasury@retail",
            ),
            Some(account_id.clone()),
            "account selectors must resolve aliases in non-default dataspaces"
        );
    }

    #[test]
    fn parse_lane_settlement_buffer_config_resolves_account() {
        let alice = (*ALICE_ID).clone();
        let wonderland: DomainId =
            DomainId::try_new("wonderland", "universal").expect("wonderland domain");
        let mut world = World::new();
        let selector =
            AccountDomainSelector::from_domain(&wonderland).expect("selector from domain");
        world.domain_selectors.insert(selector, wonderland);
        let world_view = world.view();
        let mut lane = LaneConfig::default();
        lane.metadata
            .insert("settlement.buffer_account".to_owned(), alice.to_string());
        let expected_asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain"),
            "xor".parse().expect("asset name"),
        );
        lane.metadata.insert(
            "settlement.buffer_asset".to_owned(),
            expected_asset_definition_id.to_string(),
        );
        lane.metadata.insert(
            "settlement.buffer_capacity_micro".to_owned(),
            "1000".to_owned(),
        );

        let parsed =
            parse_lane_settlement_buffer_config(&world_view, &DataSpaceCatalog::default(), &lane)
                .expect("config parsed");
        let expected = alice.clone();
        assert_eq!(parsed.account_id, expected);
        assert_eq!(parsed.account_id.subject_id(), alice.subject_id());
        assert_eq!(parsed.asset_definition_id, expected_asset_definition_id);
        assert_eq!(
            parsed.capacity,
            MicroXor::from(Decimal::from_str("1000").expect("decimal parse"))
        );
    }

    #[test]
    fn warm_overlay_chunk_respectschunk_size() {
        let instrs = vec![
            InstructionBox::from(Log::new(Level::INFO, "a".to_owned())),
            InstructionBox::from(Log::new(Level::INFO, "b".to_owned())),
            InstructionBox::from(Log::new(Level::INFO, "c".to_owned())),
        ];
        let overlay = TxOverlay::from_instructions(instrs);
        assert_eq!(warm_overlay_chunk(&overlay, 2), 2);
        assert_eq!(warm_overlay_chunk(&overlay, 10), 3);
    }

    #[test]
    fn prefetch_account_reports_hits() {
        let alice = (*ALICE_ID).clone();
        let mut world = World::new();
        world
            .accounts
            .insert(alice.clone(), AccountValue::new(AccountDetails::default()));
        world.tx_sequences.insert(alice.clone(), 7);
        let role_id = RoleId {
            name: Name::from_str("auditor").expect("valid name"),
        };
        world
            .account_roles
            .insert(RoleIdWithOwner::new(alice.clone(), role_id), ());
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_for_testing(world, kura, query_handle);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let state_block = state.block(header);

        let prefetch_stats = prefetch_account_stores(&state_block, &alice);
        assert!(prefetch_stats.account_loaded);
        assert!(prefetch_stats.tx_sequence_loaded);
        assert_eq!(prefetch_stats.roles_touched, 1);
        // No permissions were inserted above.
        assert_eq!(prefetch_stats.permissions_touched, 0);
    }
}

#[cfg(test)]
mod pipeline_recovery_tests {
    use super::*;

    #[test]
    fn expected_pipeline_dag_fingerprint_requires_matching_block_hash() {
        let height = 1;
        let block_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x11; 32]));
        let other_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x22; 32]));
        let call_hash =
            HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::prehashed([0x33; 32]));
        let dag = PipelineDagSnapshot {
            fingerprint: [0x44; 32],
            key_count: 1,
        };
        let txs = vec![PipelineTxSnapshot {
            hash: call_hash,
            reads: Vec::new(),
            writes: Vec::new(),
        }];

        let sidecar_mismatch = PipelineRecoverySidecar::new(height, other_hash, dag, txs.clone());
        assert!(
            expected_pipeline_dag_fingerprint(height, block_hash, &[call_hash], &sidecar_mismatch)
                .is_none(),
            "sidecars anchored to a different block hash should be ignored"
        );

        let sidecar_match = PipelineRecoverySidecar::new(height, block_hash, dag, txs);
        assert_eq!(
            expected_pipeline_dag_fingerprint(height, block_hash, &[call_hash], &sidecar_match),
            Some(dag.fingerprint),
            "matching v1 sidecars should provide expected fingerprint"
        );
    }
}

#[derive(Debug)]
struct DisjointSet {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl DisjointSet {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
            rank: vec![0; size],
        }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            let root = self.find(self.parent[x]);
            self.parent[x] = root;
        }
        self.parent[x]
    }

    fn union(&mut self, a: usize, b: usize) {
        let mut ra = self.find(a);
        let mut rb = self.find(b);
        if ra == rb {
            return;
        }
        if self.rank[ra] < self.rank[rb] {
            core::mem::swap(&mut ra, &mut rb);
        }
        self.parent[rb] = ra;
        if self.rank[ra] == self.rank[rb] {
            self.rank[ra] = self.rank[ra].saturating_add(1);
        }
    }
}

#[allow(clippy::too_many_lines)]
fn build_csr(access_ids: &[AccessIds], key_count: usize) -> (Vec<usize>, Vec<usize>, Vec<usize>) {
    let n = access_ids.len();
    let mut outdeg = vec![0usize; n];
    // Pass 1: count edges
    {
        let mut last_writer: Vec<Option<usize>> = vec![None; key_count];
        let mut open_readers: Vec<SmallVec<[usize; 4]>> = vec![SmallVec::new(); key_count];
        for (idx, aset) in access_ids.iter().enumerate() {
            let mut parents: SmallVec<[usize; 8]> = SmallVec::new();
            for &k in aset.reads.iter() {
                if let Some(w) = last_writer[k as usize] {
                    parents.push(w);
                }
                open_readers[k as usize].push(idx);
            }
            for &k in aset.writes.iter() {
                if let Some(w) = last_writer[k as usize] {
                    parents.push(w);
                }
                if let Some(readers) = {
                    if open_readers[k as usize].is_empty() {
                        None
                    } else {
                        Some(std::mem::take(&mut open_readers[k as usize]))
                    }
                } {
                    for r in readers {
                        parents.push(r);
                    }
                }
                last_writer[k as usize] = Some(idx);
            }
            if !parents.is_empty() {
                parents.sort_unstable();
                dedup_sorted_usize_smallvec(&mut parents);
                for &p in parents.iter() {
                    if p != idx {
                        outdeg[p] = outdeg[p].saturating_add(1);
                    }
                }
            }
        }
    }

    let mut row_offsets = vec![0usize; n + 1];
    for i in 0..n {
        row_offsets[i + 1] = row_offsets[i] + outdeg[i];
    }
    let edge_count = row_offsets[n];
    let mut cols = vec![0usize; edge_count];
    let mut indeg = vec![0usize; n];

    // Pass 2: fill columns
    {
        let mut last_writer: Vec<Option<usize>> = vec![None; key_count];
        let mut open_readers: Vec<SmallVec<[usize; 4]>> = vec![SmallVec::new(); key_count];
        let mut cursor = row_offsets.clone();
        for (idx, aset) in access_ids.iter().enumerate() {
            let mut parents: SmallVec<[usize; 8]> = SmallVec::new();
            for &k in aset.reads.iter() {
                if let Some(w) = last_writer[k as usize] {
                    parents.push(w);
                }
                open_readers[k as usize].push(idx);
            }
            for &k in aset.writes.iter() {
                if let Some(w) = last_writer[k as usize] {
                    parents.push(w);
                }
                if let Some(readers) = {
                    if open_readers[k as usize].is_empty() {
                        None
                    } else {
                        Some(std::mem::take(&mut open_readers[k as usize]))
                    }
                } {
                    for r in readers {
                        parents.push(r);
                    }
                }
                last_writer[k as usize] = Some(idx);
            }
            if !parents.is_empty() {
                parents.sort_unstable();
                dedup_sorted_usize_smallvec(&mut parents);
                for &p in parents.iter() {
                    if p != idx {
                        let pos = cursor[p];
                        cols[pos] = idx;
                        cursor[p] = pos + 1;
                        indeg[idx] += 1;
                    }
                }
            }
        }
    }

    (row_offsets, cols, indeg)
}

fn component_iteration_order(
    components: &[Vec<usize>],
    call_hashes: &[iroha_crypto::HashOf<
        iroha_data_model::transaction::signed::TransactionEntrypoint,
    >],
) -> Vec<usize> {
    use core::cmp::Ordering;

    let mut indices: Vec<usize> = (0..components.len()).collect();
    let mut keys: Vec<
        Option<(
            iroha_crypto::HashOf<iroha_data_model::transaction::signed::TransactionEntrypoint>,
            usize,
        )>,
    > = Vec::with_capacity(indices.len());
    for component in components {
        let key = component
            .iter()
            .copied()
            .map(|idx| (call_hashes[idx], idx))
            .min_by(std::cmp::Ord::cmp);
        keys.push(key);
    }

    indices.sort_unstable_by(|&a, &b| match (&keys[a], &keys[b]) {
        (Some(ka), Some(kb)) => ka.cmp(kb),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });

    indices
}

fn schedule_components_ready_heap(
    components: &[Vec<usize>],
    row_offsets: &[usize],
    cols: &[usize],
    call_hashes: &[iroha_crypto::HashOf<
        iroha_data_model::transaction::signed::TransactionEntrypoint,
    >],
) -> Option<Vec<usize>> {
    use std::{cmp::Reverse, collections::BinaryHeap};

    let n = call_hashes.len();
    debug_assert_eq!(
        row_offsets.len(),
        n.saturating_add(1),
        "CSR row offsets must track all vertices"
    );

    if n == 0 {
        return Some(Vec::new());
    }

    let mut order = Vec::with_capacity(n);
    let mut in_component = vec![false; n];
    let mut local_indeg = vec![0usize; n];
    let mut heap: BinaryHeap<
        Reverse<(
            iroha_crypto::HashOf<iroha_data_model::transaction::signed::TransactionEntrypoint>,
            usize,
        )>,
    > = BinaryHeap::with_capacity(n);

    let ordered_components = component_iteration_order(components, call_hashes);
    for &component_idx in &ordered_components {
        let component = &components[component_idx];
        if component.is_empty() {
            continue;
        }

        for &idx in component {
            in_component[idx] = true;
            local_indeg[idx] = 0;
        }

        for &idx in component {
            let start = row_offsets[idx];
            let end = row_offsets[idx + 1];
            for &child in &cols[start..end] {
                debug_assert!(child < n, "CSR edge index out of bounds");
                if !in_component[child] {
                    return None;
                }
                local_indeg[child] = local_indeg[child].saturating_add(1);
            }
        }

        heap.clear();
        for &idx in component {
            if local_indeg[idx] == 0 {
                heap.push(Reverse((call_hashes[idx], idx)));
            }
        }

        let prior_len = order.len();
        while let Some(Reverse((_hash, node))) = heap.pop() {
            order.push(node);
            let start = row_offsets[node];
            let end = row_offsets[node + 1];
            for &child in &cols[start..end] {
                if in_component[child] {
                    let deg = local_indeg[child].saturating_sub(1);
                    local_indeg[child] = deg;
                    if deg == 0 {
                        heap.push(Reverse((call_hashes[child], child)));
                    }
                } else {
                    return None;
                }
            }
        }

        if order.len() - prior_len != component.len() {
            return None;
        }

        for &idx in component {
            in_component[idx] = false;
            local_indeg[idx] = 0;
        }
    }

    Some(order)
}

fn schedule_components_wave(
    components: &[Vec<usize>],
    row_offsets: &[usize],
    cols: &[usize],
    call_hashes: &[iroha_crypto::HashOf<
        iroha_data_model::transaction::signed::TransactionEntrypoint,
    >],
) -> Option<Vec<usize>> {
    let n = call_hashes.len();
    debug_assert_eq!(
        row_offsets.len(),
        n.saturating_add(1),
        "CSR row offsets must track all vertices"
    );

    if n == 0 {
        return Some(Vec::new());
    }

    let mut order = Vec::with_capacity(n);
    let mut in_component = vec![false; n];
    let mut local_indeg = vec![0usize; n];
    let mut ready_frontier: Vec<usize> = Vec::new();
    let mut current_layer: Vec<usize> = Vec::new();

    let ordered_components = component_iteration_order(components, call_hashes);
    for &component_idx in &ordered_components {
        let component = &components[component_idx];
        if component.is_empty() {
            continue;
        }

        for &idx in component {
            in_component[idx] = true;
            local_indeg[idx] = 0;
        }

        for &idx in component {
            let start = row_offsets[idx];
            let end = row_offsets[idx + 1];
            for &child in &cols[start..end] {
                debug_assert!(child < n, "CSR edge index out of bounds");
                if !in_component[child] {
                    return None;
                }
                local_indeg[child] = local_indeg[child].saturating_add(1);
            }
        }

        ready_frontier.clear();
        for &idx in component {
            if local_indeg[idx] == 0 {
                ready_frontier.push(idx);
            }
        }

        let prior_len = order.len();
        while !ready_frontier.is_empty() {
            ready_frontier.sort_unstable_by(|&a, &b| {
                call_hashes[a].cmp(&call_hashes[b]).then_with(|| a.cmp(&b))
            });
            current_layer.clear();
            current_layer.extend(ready_frontier.iter().copied());
            ready_frontier.clear();
            for &node in &current_layer {
                order.push(node);
                let start = row_offsets[node];
                let end = row_offsets[node + 1];
                for &child in &cols[start..end] {
                    if in_component[child] {
                        let deg = local_indeg[child].saturating_sub(1);
                        local_indeg[child] = deg;
                        if deg == 0 {
                            ready_frontier.push(child);
                        }
                    } else {
                        return None;
                    }
                }
            }
        }

        if order.len() - prior_len != component.len() {
            return None;
        }

        for &idx in component {
            in_component[idx] = false;
            local_indeg[idx] = 0;
        }
    }

    Some(order)
}

fn conflict_free_component_layers(
    components: &[Vec<usize>],
    row_offsets: &[usize],
    cols: &[usize],
    call_hashes: &[iroha_crypto::HashOf<
        iroha_data_model::transaction::signed::TransactionEntrypoint,
    >],
) -> Option<Vec<Vec<usize>>> {
    let n = call_hashes.len();
    debug_assert_eq!(
        row_offsets.len(),
        n.saturating_add(1),
        "CSR row offsets must track all vertices"
    );

    if n == 0 {
        return Some(Vec::new());
    }

    let mut in_component = vec![false; n];
    let mut local_indeg = vec![0usize; n];
    let mut ready_frontier: Vec<usize> = Vec::new();
    let mut current_layer: Vec<usize> = Vec::new();
    let mut per_comp_layers: Vec<Vec<Vec<usize>>> = Vec::with_capacity(components.len());
    let mut max_depth = 0usize;

    let ordered_components = component_iteration_order(components, call_hashes);
    for &component_idx in &ordered_components {
        let component = &components[component_idx];
        if component.is_empty() {
            per_comp_layers.push(Vec::new());
            continue;
        }

        for &idx in component {
            debug_assert!(idx < n, "component vertex index out of bounds");
            in_component[idx] = true;
            local_indeg[idx] = 0;
        }

        for &idx in component {
            let start = row_offsets[idx];
            let end = row_offsets[idx + 1];
            for &child in &cols[start..end] {
                debug_assert!(child < n, "CSR edge index out of bounds");
                if !in_component[child] {
                    return None;
                }
                local_indeg[child] = local_indeg[child].saturating_add(1);
            }
        }

        ready_frontier.clear();
        for &idx in component {
            if local_indeg[idx] == 0 {
                ready_frontier.push(idx);
            }
        }

        let mut seen = 0usize;
        let mut comp_layers: Vec<Vec<usize>> = Vec::new();
        while !ready_frontier.is_empty() {
            ready_frontier.sort_unstable_by(|&a, &b| {
                call_hashes[a].cmp(&call_hashes[b]).then_with(|| a.cmp(&b))
            });
            current_layer.clear();
            current_layer.extend(ready_frontier.iter().copied());
            ready_frontier.clear();
            let mut wave = Vec::with_capacity(current_layer.len());
            for &node in &current_layer {
                seen = seen.saturating_add(1);
                wave.push(node);
                let start = row_offsets[node];
                let end = row_offsets[node + 1];
                for &child in &cols[start..end] {
                    if !in_component[child] {
                        return None;
                    }
                    let deg = local_indeg[child].saturating_sub(1);
                    local_indeg[child] = deg;
                    if deg == 0 {
                        ready_frontier.push(child);
                    }
                }
            }
            comp_layers.push(wave);
        }

        if seen != component.len() {
            return None;
        }

        max_depth = max_depth.max(comp_layers.len());
        per_comp_layers.push(comp_layers);
        for &idx in component {
            in_component[idx] = false;
            local_indeg[idx] = 0;
        }
    }

    let mut layers: Vec<Vec<usize>> = Vec::with_capacity(max_depth);
    for depth in 0..max_depth {
        let mut wave: Vec<usize> = Vec::new();
        for comp_layers in &per_comp_layers {
            if let Some(layer) = comp_layers.get(depth) {
                wave.extend_from_slice(layer);
            }
        }
        if !wave.is_empty() {
            wave.sort_unstable_by(|&a, &b| {
                call_hashes[a].cmp(&call_hashes[b]).then_with(|| a.cmp(&b))
            });
            layers.push(wave);
        }
    }

    Some(layers)
}

fn schedule_ready_heap_global(
    row_offsets: &[usize],
    cols: &[usize],
    indeg: &[usize],
    call_hashes: &[iroha_crypto::HashOf<
        iroha_data_model::transaction::signed::TransactionEntrypoint,
    >],
) -> Vec<usize> {
    use std::{cmp::Reverse, collections::BinaryHeap};

    let n = indeg.len();
    debug_assert_eq!(
        row_offsets.len(),
        n.saturating_add(1),
        "CSR row offsets must track all vertices"
    );

    let mut indeg_s = indeg.to_vec();
    let mut heap: BinaryHeap<
        Reverse<(
            iroha_crypto::HashOf<iroha_data_model::transaction::signed::TransactionEntrypoint>,
            usize,
        )>,
    > = BinaryHeap::with_capacity(n);
    for i in 0..n {
        if indeg_s[i] == 0 {
            heap.push(Reverse((call_hashes[i], i)));
        }
    }
    let mut order = Vec::with_capacity(n);
    while let Some(Reverse((_hash, node))) = heap.pop() {
        order.push(node);
        let start = row_offsets[node];
        let end = row_offsets[node + 1];
        for &child in &cols[start..end] {
            indeg_s[child] = indeg_s[child].saturating_sub(1);
            if indeg_s[child] == 0 {
                heap.push(Reverse((call_hashes[child], child)));
            }
        }
    }
    order
}

fn schedule_wave_global(
    row_offsets: &[usize],
    cols: &[usize],
    indeg: &[usize],
    call_hashes: &[iroha_crypto::HashOf<
        iroha_data_model::transaction::signed::TransactionEntrypoint,
    >],
) -> Vec<usize> {
    let n = indeg.len();
    debug_assert_eq!(
        row_offsets.len(),
        n.saturating_add(1),
        "CSR row offsets must track all vertices"
    );

    let mut indeg_s = indeg.to_vec();
    let mut ready_frontier: Vec<usize> = Vec::with_capacity(n);
    for (i, indegree) in indeg_s.iter().enumerate() {
        if *indegree == 0 {
            ready_frontier.push(i);
        }
    }
    let mut order = Vec::with_capacity(n);
    let mut current_layer: Vec<usize> = Vec::new();
    while !ready_frontier.is_empty() {
        ready_frontier
            .sort_unstable_by(|&a, &b| call_hashes[a].cmp(&call_hashes[b]).then_with(|| a.cmp(&b)));
        current_layer.clear();
        current_layer.extend(ready_frontier.iter().copied());
        ready_frontier.clear();
        for &node in &current_layer {
            order.push(node);
            let start = row_offsets[node];
            let end = row_offsets[node + 1];
            for &child in &cols[start..end] {
                indeg_s[child] = indeg_s[child].saturating_sub(1);
                if indeg_s[child] == 0 {
                    ready_frontier.push(child);
                }
            }
        }
    }
    order
}

impl Clone for DisjointSet {
    fn clone(&self) -> Self {
        Self {
            parent: self.parent.clone(),
            rank: self.rank.clone(),
        }
    }
}

/// Sample quarantine classifier for tests: returns true if transaction metadata contains
/// a key `quarantine` with a truthy value (bool true, non-zero number, or "true"/"1"/"yes").
#[cfg(test)]
pub fn sample_quarantine_classifier(tx: &iroha_data_model::transaction::SignedTransaction) -> bool {
    use core::str::FromStr as _;
    let key = iroha_data_model::name::Name::from_str("quarantine").unwrap();
    if let Some(json) = tx.metadata().get(&key) {
        if let Ok(b) = json.clone().try_into_any_norito::<bool>() {
            return b;
        }
        if let Ok(u) = json.clone().try_into_any_norito::<u64>() {
            return u != 0;
        }
        if let Ok(s) = json.clone().try_into_any_norito::<String>() {
            let t = s.to_ascii_lowercase();
            return t == "true" || t == "1" || t == "yes";
        }
    }
    false
}

/// Structured context for AXT envelope validation failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AxtEnvelopeValidationDetails {
    /// Human-readable message describing the failure.
    pub message: String,
    /// Categorised reason label for the rejection.
    pub reason: AxtRejectReason,
    /// Policy snapshot version active during validation.
    pub snapshot_version: u64,
    /// Dataspace associated with the rejection (if known).
    pub dataspace: Option<DataSpaceId>,
    /// Lane associated with the rejection (if known).
    pub lane: Option<LaneId>,
    /// Minimum handle era hinted by the policy for refresh guidance.
    pub next_min_handle_era: Option<u64>,
    /// Minimum sub-nonce hinted by the policy for refresh guidance.
    pub next_min_sub_nonce: Option<u64>,
}

impl fmt::Display for AxtEnvelopeValidationDetails {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (reason={}, snapshot_version={}, lane={:?}, dsid={:?}",
            self.message,
            self.reason.label(),
            self.snapshot_version,
            self.lane,
            self.dataspace
        )?;
        if let Some(hint) = self.next_min_handle_era {
            write!(f, ", next_min_handle_era={hint}")?;
        }
        if let Some(hint) = self.next_min_sub_nonce {
            write!(f, ", next_min_sub_nonce={hint}")?;
        }
        write!(f, ")")
    }
}

/// Errors occurred on block validation
#[derive(Debug, displaydoc::Display, PartialEq, Eq, Error)]
pub enum BlockValidationError {
    /// Block has committed transactions
    HasCommittedTransactions,
    /// Block contained no committed overlays
    EmptyBlock,
    /// Block contains duplicate transactions
    DuplicateTransactions,
    /// Mismatch between the actual and expected hashes of the previous block. Expected: {expected:?}, actual: {actual:?}
    PrevBlockHashMismatch {
        /// Expected value
        expected: Option<HashOf<BlockHeader>>,
        /// Actual value
        actual: Option<HashOf<BlockHeader>>,
    },
    /// Mismatch between the actual and expected height of the previous block. Expected: {expected}, actual: {actual}
    PrevBlockHeightMismatch {
        /// Expected value
        expected: usize,
        /// Actual value
        actual: usize,
    },
    /// The merkle root does not match the computed one.
    MerkleRootMismatch,
    /// Execution context invalid: {0}
    ExecutionContextInvalid(String),
    /// Cannot accept a transaction
    TransactionAccept(#[from] AcceptTransactionFail),
    /// Mismatch between the actual and expected topology. Expected: {expected:?}, actual: {actual:?}
    TopologyMismatch {
        /// Expected value
        expected: Vec<PeerId>,
        /// Actual value
        actual: Vec<PeerId>,
    },
    /// Error during block signatures check
    SignatureVerification(#[from] SignatureVerificationError),
    /// Invalid genesis block: {0}
    InvalidGenesis(#[from] InvalidGenesisError),
    /// Block's creation time is earlier than that of the previous block
    BlockInThePast,
    /// Block's creation time is later than the current node local time
    BlockInTheFuture,
    /// Some transaction in the block is created after the block itself
    TransactionInTheFuture,
    /// Block confidential feature digest mismatch. Expected: {expected:?}, actual: {actual:?}
    ConfidentialFeaturesMismatch {
        /// Digest expected by the local node.
        expected: Option<ConfidentialFeatureDigest>,
        /// Digest committed in the incoming block.
        actual: Option<ConfidentialFeatureDigest>,
    },
    /// Proof policy hash mismatch. Expected: {expected:?}, actual: {actual:?}
    ProofPolicyHashMismatch {
        /// Hash derived from the local lane catalog.
        expected: HashOf<DaProofPolicyBundle>,
        /// Hash embedded in the incoming header.
        actual: Option<HashOf<DaProofPolicyBundle>>,
    },
    /// Previous-roster evidence is invalid: {0}
    PreviousRosterEvidenceInvalid(String),
    /// DA shard cursor gate failed: {0}
    DaShardCursor(#[from] DaShardCursorError),
    /// AXT envelope export contained invalid or inconsistent fragments: {0}
    AxtEnvelopeValidationFailed(AxtEnvelopeValidationDetails),
    /// NPoS consensus effects are invalid: {0}
    NposEffectsInvalid(String),
}

/// Error during signature verification
#[derive(Debug, displaydoc::Display, Clone, Copy, PartialEq, Eq, Error)]
pub enum SignatureVerificationError {
    /// The block doesn't have enough valid signatures to be committed (`{votes_count}` out of `{min_votes_for_commit}`)
    NotEnoughSignatures {
        /// Current number of signatures
        votes_count: usize,
        /// Minimal required number of signatures
        min_votes_for_commit: usize,
    },
    /// Multiple signatures were provided for the same signer index (`{signer}`)
    DuplicateSignature {
        /// Signer index that appeared more than once
        signer: usize,
    },
    /// Block signatory doesn't correspond to any in topology
    UnknownSignatory,
    /// Block signature doesn't correspond to block payload
    UnknownSignature,
    /// Missing proof-of-possession for validator consensus key
    MissingPop,
    /// The block doesn't have proxy tail signature
    ProxyTailMissing,
    /// The block doesn't have leader signature
    LeaderMissing,
    /// Block signer does not have an active consensus key for this height/role
    InactiveConsensusKey,
    /// Miscellaneous
    Other,
}

/// Errors occurred on genesis block validation
#[derive(Debug, Copy, Clone, displaydoc::Display, PartialEq, Eq, Error)]
pub enum InvalidGenesisError {
    /// Genesis block must be signed with genesis private key and not signed by any peer
    InvalidSignature,
    /// Genesis transaction must be authorized by genesis account
    UnexpectedAuthority,
    /// Genesis transactions must not contain errors
    ContainsErrors,
    /// Genesis transaction must contain instructions
    NotInstructions,
    /// Genesis block must have 1 to 16 transactions (executor upgrade, parameters, ordinary instructions, IVM trigger registrations, initial topology)
    BadTransactionsAmount,
    /// Genesis block header must start the chain (height 1, no previous hash)
    InvalidHeader,
    /// Genesis Merkle root does not match the committed transactions
    MerkleRootMismatch,
    /// Genesis result Merkle root does not match recorded results
    ResultMerkleMismatch,
    /// Genesis transactions must share a single chain id
    ChainIdMismatch,
    /// Genesis DA commitment hash does not match embedded bundle
    DaCommitmentMismatch,
    /// Genesis DA pin intent hash does not match embedded bundle
    DaPinIntentMismatch,
}

/// Validate the structural correctness of a genesis block before submitting it to the pipeline.
///
/// # Errors
///
/// Returns [`InvalidGenesisError`] when the block violates any of the required genesis invariants,
/// such as signature mismatch, invalid authorities, or malformed transactions.
#[allow(clippy::too_many_lines)]
pub fn check_genesis_block(
    block: &SignedBlock,
    genesis_account: &iroha_data_model::account::AccountId,
    expected_chain_id: &ChainId,
) -> Result<(), InvalidGenesisError> {
    const MAX_GENESIS_TRANSACTIONS: usize = 16;

    if !block.has_results() {
        return Err(InvalidGenesisError::ContainsErrors);
    }

    if block.results().any(|result| result.as_ref().is_err()) {
        return Err(InvalidGenesisError::ContainsErrors);
    }

    let signatures = block.signatures().collect::<Vec<_>>();
    let [signature] = signatures.as_slice() else {
        return Err(InvalidGenesisError::InvalidSignature);
    };
    signature
        .signature()
        .verify_hash(genesis_account.signatory(), block.hash())
        .map_err(|_| InvalidGenesisError::InvalidSignature)?;

    if block.header().height().get() != 1 || block.header().prev_block_hash().is_some() {
        return Err(InvalidGenesisError::InvalidHeader);
    }

    let transactions: Vec<_> = block.external_transactions().collect();
    let external_entrypoints: Vec<_> = block.external_entrypoints_cloned().collect();
    if transactions.is_empty() || transactions.len() > MAX_GENESIS_TRANSACTIONS {
        return Err(InvalidGenesisError::BadTransactionsAmount);
    }
    if external_entrypoints.len() != transactions.len()
        || external_entrypoints.iter().any(|entrypoint| {
            !matches!(
                entrypoint,
                iroha_data_model::transaction::TransactionEntrypoint::External(_)
            )
        })
    {
        return Err(InvalidGenesisError::BadTransactionsAmount);
    }
    let mut chain_id: Option<ChainId> = None;
    let expected_merkle_root = block
        .external_entrypoints_cloned()
        .map(|entrypoint| entrypoint.hash())
        .collect::<MerkleTree<_>>()
        .root();
    if block.header().merkle_root() != expected_merkle_root {
        return Err(InvalidGenesisError::MerkleRootMismatch);
    }
    let expected_result_root = block.result_hashes().collect::<MerkleTree<_>>().root();
    if block.header().result_merkle_root() != expected_result_root {
        return Err(InvalidGenesisError::ResultMerkleMismatch);
    }
    match (block.header().da_commitments_hash(), block.da_commitments()) {
        (None, None) => {}
        (Some(hash), Some(bundle)) => {
            let expected = bundle
                .merkle_root()
                .map(HashOf::<DaCommitmentBundle>::from_untyped_unchecked);
            if expected != Some(hash) {
                return Err(InvalidGenesisError::DaCommitmentMismatch);
            }
        }
        _ => return Err(InvalidGenesisError::DaCommitmentMismatch),
    }
    match (block.header().da_pin_intents_hash(), block.da_pin_intents()) {
        (None, None) => {}
        (Some(hash), Some(bundle)) => {
            let expected = bundle
                .merkle_root()
                .map(HashOf::<DaPinIntentBundle>::from_untyped_unchecked);
            if expected != Some(hash) {
                return Err(InvalidGenesisError::DaPinIntentMismatch);
            }
        }
        _ => return Err(InvalidGenesisError::DaPinIntentMismatch),
    }

    for transaction in transactions {
        let tx_chain = transaction.chain();
        let seen = chain_id.get_or_insert_with(|| tx_chain.clone());
        if seen != tx_chain || tx_chain != expected_chain_id {
            return Err(InvalidGenesisError::ChainIdMismatch);
        }
        if transaction.authority() != genesis_account {
            return Err(InvalidGenesisError::UnexpectedAuthority);
        }
        let iroha_data_model::transaction::Executable::Instructions(_isi) =
            transaction.instructions()
        else {
            return Err(InvalidGenesisError::NotInstructions);
        };
    }
    Ok(())
}

/// Builder for blocks
#[derive(Debug, Clone)]
pub struct BlockBuilder<B>(B);

fn signed_block_entrypoints_are_canonical(block: &SignedBlock) -> bool {
    let mut previous = None;
    for entrypoint in block.external_entrypoints_cloned() {
        let hash = entrypoint.hash();
        if previous.is_some_and(|previous| previous > hash) {
            return false;
        }
        previous = Some(hash);
    }
    true
}

mod pending {
    use iroha_primitives::time::TimeSource;
    use nonzero_ext::nonzero;

    use super::*;

    /// First stage in the life-cycle of a [`Block`].
    /// In the beginning the block is assumed to be verified and to contain only accepted transactions.
    /// Additionally the block must retain events emitted during the execution of on-chain logic during
    /// the previous round, which might then be processed by the trigger system.
    #[derive(Debug, Clone)]
    pub struct Pending {
        /// Collection of transactions which have been accepted.
        transactions: Vec<AcceptedTransaction<'static>>,
        time_source: TimeSource,
    }

    impl BlockBuilder<Pending> {
        const TIME_PADDING: Duration = Duration::from_millis(1);

        /// Create [`Self`]
        #[inline]
        pub fn new(transactions: Vec<AcceptedTransaction<'static>>) -> Self {
            Self::new_with_time_source(transactions, TimeSource::new_system())
        }

        /// Create with provided [`TimeSource`] to use for block creation time.
        pub fn new_with_time_source(
            transactions: Vec<AcceptedTransaction<'static>>,
            time_source: TimeSource,
        ) -> Self {
            // Empty blocks can be built for tests, but validation rejects them unless they carry
            // entrypoints (external transactions or time triggers) or deterministic artifacts
            // such as DA bundles; consensus should not emit them.
            let mut transactions: Vec<_> = transactions
                .into_iter()
                .enumerate()
                .map(|(idx, tx)| (tx.hash_as_entrypoint(), idx, tx))
                .collect();
            // Canonicalize payload order by (call_hash, original index) so scheduler tie-breaks
            // remain stable regardless of submission order.
            transactions.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

            Self(Pending {
                transactions: transactions.into_iter().map(|(_, _, tx)| tx).collect(),
                time_source,
            })
        }

        /// Create [`Self`] while preserving the provided transaction order.
        ///
        /// This bypasses call-hash canonicalisation and is intended for
        /// test harnesses that require strict FIFO semantics.
        #[cfg(test)]
        #[inline]
        pub(crate) fn new_preserve_order(transactions: Vec<AcceptedTransaction<'static>>) -> Self {
            Self::new_preserve_order_with_time_source(transactions, TimeSource::new_system())
        }

        /// Create with provided [`TimeSource`] while preserving transaction order.
        #[cfg(test)]
        pub(crate) fn new_preserve_order_with_time_source(
            transactions: Vec<AcceptedTransaction<'static>>,
            time_source: TimeSource,
        ) -> Self {
            Self(Pending {
                transactions,
                time_source,
            })
        }

        fn make_header(
            &self,
            prev_block: Option<&SignedBlock>,
            view_change_index: u64,
        ) -> BlockHeader {
            let prev_block_time =
                prev_block.map_or(Duration::ZERO, |block| block.header().creation_time());

            let latest_txn_time = self
                .0
                .transactions
                .iter()
                .map(crate::tx::AcceptedTransaction::creation_time)
                .max()
                // No transactions present; validation still rejects empty payloads.
                .unwrap_or(Duration::ZERO);

            let now = self.0.time_source.get_unix_time();

            // NOTE: Lower time bound must always be upheld for a valid block
            // If the clock has drifted too far this block will be rejected
            let creation_time = [
                now,
                latest_txn_time + Self::TIME_PADDING,
                prev_block_time + Self::TIME_PADDING,
            ]
            .into_iter()
            .max()
            .unwrap();

            let height = prev_block.map(|block| block.header().height()).map_or_else(
                || nonzero!(1_u64),
                |height| {
                    height
                        .checked_add(1)
                        .expect("INTERNAL BUG: Blockchain height exceeds usize::MAX")
                },
            );
            let prev_block_hash = prev_block.map(SignedBlock::hash);
            let merkle_root = self
                .0
                .transactions
                .iter()
                .map(crate::tx::AcceptedTransaction::hash_as_entrypoint)
                .collect::<MerkleTree<_>>()
                .root();
            let creation_time_ms = creation_time
                .as_millis()
                .try_into()
                .expect("Time should fit into u64");
            BlockHeader::new(
                height,
                prev_block_hash,
                merkle_root,
                None,
                creation_time_ms,
                view_change_index,
            )
        }

        /// Chain the block with existing blockchain.
        ///
        /// Upon executing this method current timestamp is stored in the block header.
        pub fn chain(
            self,
            view_change_index: u64,
            latest_block: Option<&SignedBlock>,
        ) -> BlockBuilder<Chained> {
            let mut header = self.make_header(latest_block, view_change_index);
            if header.confidential_features().is_none() {
                header.set_confidential_features(Some(EMPTY_CONFIDENTIAL_FEATURE_DIGEST));
            }
            BlockBuilder(Chained {
                header,
                transactions: self.0.transactions,
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
                previous_roster_evidence: None,
                npos_consensus_effects: None,
                execution_context: None,
            })
        }
    }
}

mod chained {
    use iroha_crypto::SignatureOf;
    use new::NewBlock;

    use super::*;

    /// When a `Pending` block is chained with the blockchain it becomes [`Chained`] block.
    #[derive(Debug, Clone)]
    pub struct Chained {
        pub(super) header: BlockHeader,
        pub(super) transactions: Vec<AcceptedTransaction<'static>>,
        pub(super) da_commitments: Option<DaCommitmentBundle>,
        pub(super) da_proof_policies: Option<DaProofPolicyBundle>,
        pub(super) da_pin_intents: Option<DaPinIntentBundle>,
        pub(super) previous_roster_evidence: Option<PreviousRosterEvidence>,
        pub(super) npos_consensus_effects: Option<NposConsensusEffects>,
        pub(super) execution_context: Option<BlockExecutionContextBundle>,
    }

    impl BlockBuilder<Chained> {
        /// Attach a DA commitment bundle and update the header hash accordingly.
        #[must_use]
        pub fn with_da_commitments(mut self, commitments: Option<DaCommitmentBundle>) -> Self {
            let hash = commitments.as_ref().and_then(|bundle| {
                if bundle.is_empty() {
                    None
                } else {
                    Some(bundle.canonical_hash())
                }
            });
            self.0.header.set_da_commitments_hash(hash);
            self.0.da_commitments = commitments;
            self
        }

        /// Attach a DA proof policy bundle and update the header hash accordingly.
        #[must_use]
        pub fn with_da_proof_policies(mut self, policies: Option<DaProofPolicyBundle>) -> Self {
            let hash = policies.as_ref().map(HashOf::new);
            self.0.header.set_da_proof_policies_hash(hash);
            self.0.da_proof_policies = policies;
            self
        }

        /// Attach a DA pin intent bundle and update the header hash accordingly.
        #[must_use]
        pub fn with_da_pin_intents(mut self, intents: Option<DaPinIntentBundle>) -> Self {
            let hash = intents
                .as_ref()
                .and_then(|bundle| bundle.merkle_root().map(HashOf::from_untyped_unchecked));
            self.0.header.set_da_pin_intents_hash(hash);
            self.0.da_pin_intents = intents;
            self
        }

        /// Attach previous-height roster evidence and update the header hash accordingly.
        #[must_use]
        pub fn with_previous_roster_evidence(
            mut self,
            evidence: Option<PreviousRosterEvidence>,
        ) -> Self {
            let hash = evidence.as_ref().map(HashOf::new);
            self.0.header.set_prev_roster_evidence_hash(hash);
            self.0.previous_roster_evidence = evidence;
            self
        }

        /// Attach deterministic `NPoS` effects and update the header hash accordingly.
        #[must_use]
        pub fn with_npos_consensus_effects(
            mut self,
            effects: Option<NposConsensusEffects>,
        ) -> Self {
            let effects = effects.filter(|bundle| !bundle.is_empty());
            let hash = effects.as_ref().map(HashOf::new);
            self.0.header.set_npos_effects_hash(hash);
            self.0.npos_consensus_effects = effects;
            self
        }

        /// Attach durable execution context and update the header hash accordingly.
        #[must_use]
        pub fn with_execution_context(
            mut self,
            context: Option<BlockExecutionContextBundle>,
        ) -> Self {
            let context = context.filter(|bundle| !bundle.is_empty());
            let hash = context.as_ref().map(HashOf::new);
            self.0.header.set_execution_context_hash(hash);
            self.0.execution_context = context;
            self
        }

        /// Attach an SCCP commitment root to the block header.
        #[must_use]
        pub fn with_sccp_commitment_root(mut self, root: Option<[u8; 32]>) -> Self {
            self.0.header.set_sccp_commitment_root(root);
            self
        }

        /// Attach the confidential feature digest that this block commits to.
        #[must_use]
        pub fn with_confidential_features(
            mut self,
            digest: Option<ConfidentialFeatureDigest>,
        ) -> Self {
            self.0.header.set_confidential_features(digest);
            self
        }

        /// Sign this block and get [`NewBlock`] using the provided validator index.
        pub fn sign_with_index(
            self,
            private_key: &PrivateKey,
            signatory_idx: u64,
        ) -> WithEvents<NewBlock> {
            let mut builder = self;
            if builder.0.da_proof_policies.is_none()
                && builder.0.header.da_proof_policies_hash().is_none()
            {
                let default_policies = crate::da::proof_policy_bundle(
                    &iroha_config::parameters::actual::LaneConfig::default(),
                );
                builder = builder.with_da_proof_policies(Some(default_policies));
            }
            #[cfg(any(test, feature = "iroha-core-tests"))]
            if builder.0.execution_context.is_none() && !builder.0.transactions.is_empty() {
                let default_context = builder
                    .0
                    .transactions
                    .iter()
                    .map(|tx| {
                        ExternalExecutionContext::new(
                            tx.hash_as_entrypoint(),
                            LaneId::SINGLE,
                            DataSpaceId::UNIVERSAL,
                        )
                    })
                    .collect::<Vec<_>>();
                builder = builder.with_execution_context(Some(BlockExecutionContextBundle::new(
                    default_context,
                )));
            }
            let signature = BlockSignature::new(
                signatory_idx,
                SignatureOf::from_hash(private_key, builder.0.header.hash()),
            );

            WithEvents::new(NewBlock {
                signature,
                header: builder.0.header,
                transactions: builder.0.transactions,
                da_commitments: builder.0.da_commitments,
                da_proof_policies: builder.0.da_proof_policies,
                da_pin_intents: builder.0.da_pin_intents,
                previous_roster_evidence: builder.0.previous_roster_evidence,
                npos_consensus_effects: builder.0.npos_consensus_effects,
                execution_context: builder.0.execution_context,
            })
        }

        /// Sign this block and get [`NewBlock`] using validator index 0.
        pub fn sign(self, private_key: &PrivateKey) -> WithEvents<NewBlock> {
            self.sign_with_index(private_key, 0)
        }
    }
}

mod new {
    use super::*;
    use crate::state::StateBlock;

    /// First stage in the life-cycle of a block.
    ///
    /// Transactions in this block are not categorized.
    #[derive(Debug, Clone)]
    pub struct NewBlock {
        pub(super) signature: BlockSignature,
        pub(super) header: BlockHeader,
        pub(super) transactions: Vec<AcceptedTransaction<'static>>,
        pub(super) da_commitments: Option<DaCommitmentBundle>,
        pub(super) da_proof_policies: Option<DaProofPolicyBundle>,
        pub(super) da_pin_intents: Option<DaPinIntentBundle>,
        pub(super) previous_roster_evidence: Option<PreviousRosterEvidence>,
        pub(super) npos_consensus_effects: Option<NposConsensusEffects>,
        pub(super) execution_context: Option<BlockExecutionContextBundle>,
    }

    impl NewBlock {
        /// Transition to [`ValidBlock`]. Skips static checks and only applies state changes.
        pub fn validate_and_record_transactions(
            self,
            state_block: &mut StateBlock<'_>,
        ) -> WithEvents<ValidBlock> {
            // Future pipeline overlap: the scheduler can pre-validate on a snapshot pinned at
            // height N-1 while proposing block N. For now we keep the simple path to preserve
            // deterministic behaviour; see docs/source/new_pipeline.md for the staged rollout.
            ValidBlock::validate_unchecked(self.into(), state_block)
        }

        /// Block signature
        pub fn signature(&self) -> &BlockSignature {
            &self.signature
        }

        /// Block header
        pub fn header(&self) -> BlockHeader {
            self.header
        }

        /// Block transactions
        pub fn transactions(&self) -> &[AcceptedTransaction<'_>] {
            &self.transactions
        }

        /// DA commitments embedded in this block, if any.
        pub fn da_commitments(&self) -> Option<&DaCommitmentBundle> {
            self.da_commitments.as_ref()
        }

        /// DA proof policies embedded in this block, if any.
        pub fn da_proof_policies(&self) -> Option<&DaProofPolicyBundle> {
            self.da_proof_policies.as_ref()
        }

        /// DA pin intents embedded in this block, if any.
        pub fn da_pin_intents(&self) -> Option<&DaPinIntentBundle> {
            self.da_pin_intents.as_ref()
        }

        /// Previous-height roster evidence embedded in this block, if any.
        pub fn previous_roster_evidence(&self) -> Option<&PreviousRosterEvidence> {
            self.previous_roster_evidence.as_ref()
        }

        /// `NPoS` consensus effects embedded in this block, if any.
        pub fn npos_consensus_effects(&self) -> Option<&NposConsensusEffects> {
            self.npos_consensus_effects.as_ref()
        }

        #[cfg(test)]
        #[allow(dead_code)]
        pub(crate) fn update_header(self, header: &BlockHeader, private_key: &PrivateKey) -> Self {
            let signature = BlockSignature::new(
                0,
                iroha_crypto::SignatureOf::from_hash(private_key, header.hash()),
            );

            Self {
                signature,
                header: *header,
                transactions: self.transactions,
                da_commitments: self.da_commitments,
                da_proof_policies: self.da_proof_policies,
                da_pin_intents: self.da_pin_intents,
                previous_roster_evidence: self.previous_roster_evidence,
                npos_consensus_effects: self.npos_consensus_effects,
                execution_context: self.execution_context,
            }
        }
    }

    impl From<NewBlock> for SignedBlock {
        fn from(block: NewBlock) -> Self {
            let mut transactions = Vec::new();
            let mut external_entrypoints = Vec::with_capacity(block.transactions.len());
            for accepted in block.transactions {
                external_entrypoints.push(accepted.entrypoint().clone());
                if let Some(signed) = accepted.external().cloned() {
                    transactions.push(signed);
                }
            }
            let mut signed_block = SignedBlock::presigned_with_da(
                block.signature,
                block.header,
                transactions,
                block.da_commitments,
            );
            signed_block.set_external_entrypoints(external_entrypoints);
            signed_block.set_da_proof_policies(block.da_proof_policies);
            signed_block.set_da_pin_intents(block.da_pin_intents);
            signed_block.set_previous_roster_evidence(block.previous_roster_evidence);
            signed_block.set_npos_consensus_effects(block.npos_consensus_effects);
            signed_block.set_execution_context(block.execution_context);
            signed_block
        }
    }

    #[cfg(test)]
    mod tests {
        use std::{borrow::Cow, time::Duration};

        use iroha_crypto::KeyPair;
        use iroha_data_model::{ChainId, isi::Log, transaction::TransactionBuilder};
        use iroha_logger::Level;
        use iroha_primitives::time::TimeSource;
        use iroha_test_samples::gen_account_in;

        use super::*;
        use crate::{block::BlockBuilder, tx::AcceptedTransaction};

        #[test]
        fn into_signed_block_preserves_transactions() {
            let chain: ChainId = "new-block-conversion".parse().expect("valid chain id");
            let (authority, keypair) = gen_account_in("wonderland");

            let tx1 = TransactionBuilder::new(chain.clone(), authority.clone())
                .with_instructions([Log::new(Level::INFO, "first".to_owned())])
                .sign(keypair.private_key());
            let tx2 = TransactionBuilder::new(chain, authority)
                .with_instructions([Log::new(Level::INFO, "second".to_owned())])
                .sign(keypair.private_key());

            let mut expected = vec![
                (tx1.hash_as_entrypoint(), 0usize, tx1.clone()),
                (tx2.hash_as_entrypoint(), 1usize, tx2.clone()),
            ];
            expected.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            let expected: Vec<_> = expected.into_iter().map(|(_, _, tx)| tx).collect();
            let accepted = vec![
                AcceptedTransaction::new_unchecked(Cow::Owned(tx1)),
                AcceptedTransaction::new_unchecked(Cow::Owned(tx2)),
            ];

            let (_handle, time_source) = TimeSource::new_mock(Duration::from_secs(1));
            let builder = BlockBuilder::new_with_time_source(accepted, time_source);
            let block_signer = KeyPair::random();

            let new_block = builder
                .chain(0, None)
                .sign(block_signer.private_key())
                .unpack(|_| {});

            let signed_block: SignedBlock = new_block.into();
            assert_eq!(signed_block.transactions_vec(), &expected);
        }

        #[test]
        fn block_builder_sign_with_index_sets_signature_index() {
            let chain: ChainId = "new-block-sign-index".parse().expect("valid chain id");
            let (authority, keypair) = gen_account_in("wonderland");
            let tx = TransactionBuilder::new(chain, authority)
                .with_instructions([Log::new(Level::INFO, "signed".to_owned())])
                .sign(keypair.private_key());

            let accepted = vec![AcceptedTransaction::new_unchecked(Cow::Owned(tx))];
            let builder = BlockBuilder::new(accepted);
            let signer = KeyPair::random();
            let signatory_idx = 7_u64;

            let new_block = builder
                .chain(0, None)
                .sign_with_index(signer.private_key(), signatory_idx)
                .unpack(|_| {});

            assert_eq!(new_block.signature().index(), signatory_idx);
        }

        #[test]
        fn preserve_order_builder_keeps_submission_sequence() {
            let chain: ChainId = "new-block-conversion".parse().expect("valid chain id");
            let (authority, keypair) = gen_account_in("wonderland");

            let tx1 = TransactionBuilder::new(chain.clone(), authority.clone())
                .with_instructions([Log::new(Level::INFO, "first".to_owned())])
                .sign(keypair.private_key());
            let tx2 = TransactionBuilder::new(chain, authority)
                .with_instructions([Log::new(Level::INFO, "second".to_owned())])
                .sign(keypair.private_key());

            let expected = vec![tx1.clone(), tx2.clone()];
            let accepted = vec![
                AcceptedTransaction::new_unchecked(Cow::Owned(tx1)),
                AcceptedTransaction::new_unchecked(Cow::Owned(tx2)),
            ];

            let (_handle, time_source) = TimeSource::new_mock(Duration::from_secs(1));
            let builder = BlockBuilder::new_preserve_order_with_time_source(accepted, time_source);
            let block_signer = KeyPair::random();

            let new_block = builder
                .chain(0, None)
                .sign(block_signer.private_key())
                .unpack(|_| {});

            let signed_block: SignedBlock = new_block.into();
            assert_eq!(signed_block.transactions_vec(), &expected);
        }
    }
}

pub(crate) mod valid {
    use std::{num::NonZeroUsize, time::Instant};

    use commit::CommittedBlock;
    #[cfg(test)]
    use iroha_data_model::soracloud::{
        SoraRuntimeReceiptV1, SoraServiceHandlerClassV1, SoraServiceHealthStatusV1,
        SoraServiceMailboxMessageV1, SoraServiceRuntimeStateV1,
    };
    use iroha_data_model::{
        ChainId,
        events::pipeline::PipelineEventBox,
        nexus::{AxtPolicySnapshot, GroupBinding, HandleBudget, HandleSubject},
    };
    use iroha_logger::warn;
    use iroha_primitives::time::TimeSource;

    use super::{
        event::{map_block_err_to_reason, map_sig_err_to_reason},
        *,
    };
    use crate::{
        smartcontracts::ivm::cache::IvmCache,
        state::{
            StateBlock, StateReadOnlyWithTransactions, storage_transactions::TransactionsReadOnly,
        },
        sumeragi::network_topology::Role,
    };
    #[cfg(test)]
    use crate::{
        soracloud_runtime::{
            SoracloudOrderedMailboxExecutionRequest, SoracloudOrderedMailboxExecutionResult,
            SoracloudRuntimeExecutionError,
        },
        state::{StateReadOnly, StateTransaction},
    };

    fn charge_rejected_overlay_fees(
        state_block_mut: &mut StateBlock<'_>,
        tx: &iroha_data_model::transaction::SignedTransaction,
        authority: &AccountId,
        overlay: &crate::pipeline::overlay::TxOverlay,
        encoded_len: usize,
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        rejection_reason: &TransactionRejectionReason,
    ) -> Result<(), TransactionRejectionReason> {
        if matches!(
            rejection_reason,
            TransactionRejectionReason::Validation(
                iroha_data_model::ValidationFail::InternalError(_)
            )
        ) {
            return Ok(());
        }

        let mut fee_tx = state_block_mut.transaction();
        fee_tx.current_lane_id = Some(lane_id);
        fee_tx.current_dataspace_id = Some(dataspace_id);
        fee_tx.world.current_dataspace_id = Some(dataspace_id);
        fee_tx.tx_call_hash = Some(iroha_crypto::Hash::from(tx.hash_as_entrypoint()));
        fee_tx.current_tx_hash = Some(tx.hash());

        charge_fees_for_applied_overlay_with_encoded_len(
            &mut fee_tx,
            authority,
            tx,
            overlay,
            encoded_len,
        )
        .map_err(TransactionRejectionReason::Validation)?;
        fee_tx.apply();
        Ok(())
    }

    /// Block that was validated and accepted.
    #[derive(Debug, Clone)]
    pub struct ValidBlock {
        block: SignedBlock,
        signatures_verified: bool,
    }

    /// Timing breakdown for block validation stages.
    #[derive(Debug, Clone, Copy, Default)]
    #[allow(clippy::struct_field_names)]
    pub struct ValidationTimings {
        /// Elapsed milliseconds for stateless checks.
        pub(crate) stateless_ms: u64,
        /// Elapsed milliseconds spent in state-dependent stateless checks.
        pub(crate) stateless_state_dependent_ms: u64,
        /// Elapsed milliseconds spent in snapshot-based stateless checks.
        pub(crate) stateless_snapshot_ms: u64,
        /// Elapsed milliseconds for execution/stateful checks.
        pub(crate) execution_ms: u64,
        /// Elapsed milliseconds spent ensuring DA indexes are hydrated.
        pub(crate) execution_da_indexes_ms: u64,
        /// Elapsed milliseconds spent creating the state block.
        pub(crate) execution_state_block_ms: u64,
        /// Elapsed milliseconds spent executing transactions.
        pub(crate) execution_tx_ms: u64,
        /// Elapsed milliseconds spent in signature micro-batching for stateless pre-pass.
        pub(crate) execution_tx_signature_batch_ms: u64,
        /// Elapsed milliseconds spent in stateless transaction validation.
        pub(crate) execution_tx_stateless_ms: u64,
        /// Elapsed milliseconds spent deriving access sets.
        pub(crate) execution_tx_access_ms: u64,
        /// Elapsed milliseconds spent building overlays.
        pub(crate) execution_tx_overlay_ms: u64,
        /// Elapsed milliseconds spent building the conflict graph/DAG.
        pub(crate) execution_tx_dag_ms: u64,
        /// Elapsed milliseconds spent scheduling the transaction order.
        pub(crate) execution_tx_schedule_ms: u64,
        /// Elapsed milliseconds spent applying overlays and finalizing results.
        pub(crate) execution_tx_apply_ms: u64,
        /// Elapsed milliseconds spent preparing apply accounting before layer execution.
        pub(crate) execution_tx_apply_setup_ms: u64,
        /// Elapsed milliseconds spent building conflict-free apply layers.
        pub(crate) execution_tx_apply_layer_build_ms: u64,
        /// Elapsed milliseconds spent executing time triggers.
        pub(crate) execution_tx_time_triggers_ms: u64,
        /// Elapsed milliseconds spent finalizing block results after time triggers.
        pub(crate) execution_tx_finalize_ms: u64,
        /// Elapsed milliseconds spent submitting FASTPQ transcript digest work.
        pub(crate) execution_tx_finalize_digest_submit_ms: u64,
        /// Elapsed milliseconds spent preparing FASTPQ entry dataspace records.
        pub(crate) execution_tx_finalize_dataspaces_ms: u64,
        /// Elapsed milliseconds spent computing and storing the transaction set hash.
        pub(crate) execution_tx_finalize_tx_set_ms: u64,
        /// Elapsed milliseconds spent finalizing and draining FASTPQ transfer transcripts.
        pub(crate) execution_tx_finalize_transcripts_ms: u64,
        /// Elapsed milliseconds spent draining AXT state for block results.
        pub(crate) execution_tx_finalize_axt_ms: u64,
        /// Elapsed milliseconds spent publishing transaction results into the block.
        pub(crate) execution_tx_finalize_set_results_ms: u64,
        /// Elapsed finalization milliseconds not attributed to a narrower sub-stage.
        pub(crate) execution_tx_finalize_other_ms: u64,
        /// Elapsed milliseconds spent preparing apply layers (validation + setup).
        pub(crate) execution_tx_apply_prep_ms: u64,
        /// Elapsed milliseconds spent executing detached overlays.
        pub(crate) execution_tx_apply_detached_ms: u64,
        /// Elapsed milliseconds spent merging detached deltas (excluding fallback).
        pub(crate) execution_tx_apply_merge_ms: u64,
        /// Elapsed milliseconds spent in sequential fallback during apply.
        pub(crate) execution_tx_apply_fallback_ms: u64,
        /// Elapsed milliseconds spent applying quarantine transactions.
        pub(crate) execution_tx_apply_quarantine_ms: u64,
        /// Elapsed milliseconds spent in the sequential apply path (when parallel apply is off).
        pub(crate) execution_tx_apply_sequential_ms: u64,
        /// Elapsed milliseconds spent materializing transaction results after apply.
        pub(crate) execution_tx_apply_results_ms: u64,
        /// Elapsed apply milliseconds not attributed to a narrower apply sub-stage.
        pub(crate) execution_tx_apply_other_ms: u64,
        /// Elapsed milliseconds spent validating AXT envelopes.
        pub(crate) execution_axt_ms: u64,
        /// Elapsed milliseconds spent validating DA shard cursors.
        pub(crate) execution_da_cursor_ms: u64,
        /// Elapsed milliseconds spent checking genesis transaction invariants.
        pub(crate) execution_genesis_clean_ms: u64,
        /// Total elapsed milliseconds for validation.
        pub(crate) total_ms: u64,
    }

    impl ValidationTimings {
        /// Create an empty timing snapshot.
        pub(crate) fn new() -> Self {
            Self::default()
        }
    }

    type Error = (Box<SignedBlock>, Box<BlockValidationError>);

    #[cfg(test)]
    fn collect_ready_soracloud_mailbox_messages(
        state_transaction: &StateTransaction<'_, '_>,
    ) -> Vec<SoraServiceMailboxMessageV1> {
        let execution_sequence =
            crate::smartcontracts::isi::soracloud::next_soracloud_audit_sequence(state_transaction);
        let consumed: BTreeSet<Hash> = state_transaction
            .world
            .soracloud_runtime_receipts
            .iter()
            .filter_map(|(_receipt_id, receipt)| receipt.mailbox_message_id)
            .collect();
        let mut messages: Vec<_> = state_transaction
            .world
            .soracloud_mailbox_messages
            .iter()
            .filter_map(|(message_id, message)| {
                if consumed.contains(message_id) {
                    return None;
                }
                if message.available_after_sequence > execution_sequence {
                    return None;
                }
                if let Some(expires_at) = message.expires_at_sequence
                    && expires_at <= execution_sequence
                {
                    return None;
                }
                Some(message.clone())
            })
            .collect();
        messages.sort_unstable_by(|left, right| {
            left.available_after_sequence
                .cmp(&right.available_after_sequence)
                .then_with(|| left.enqueue_sequence.cmp(&right.enqueue_sequence))
                .then_with(|| left.message_id.cmp(&right.message_id))
        });
        messages
    }

    #[cfg(test)]
    fn authoritative_pending_mailbox_messages(
        state_transaction: &StateTransaction<'_, '_>,
        service_name: &iroha_data_model::name::Name,
    ) -> u32 {
        let consumed: BTreeSet<Hash> = state_transaction
            .world
            .soracloud_runtime_receipts
            .iter()
            .filter_map(|(_receipt_id, receipt)| receipt.mailbox_message_id)
            .collect();
        u32::try_from(
            state_transaction
                .world
                .soracloud_mailbox_messages
                .iter()
                .filter(|(message_id, message)| {
                    !consumed.contains(message_id) && message.to_service == *service_name
                })
                .count(),
        )
        .unwrap_or(u32::MAX)
    }

    #[cfg(test)]
    fn synthetic_mailbox_runtime_failure(
        request: SoracloudOrderedMailboxExecutionRequest,
        error: SoracloudRuntimeExecutionError,
    ) -> SoracloudOrderedMailboxExecutionResult {
        let outcome_label = error.kind.label();
        let result_commitment = Hash::new(
            format!(
                "soracloud:runtime-failure:{}:{}:{}:{}:{}",
                request.mailbox_message.message_id,
                request.deployment.service_name,
                request.deployment.current_service_version,
                request.mailbox_message.to_handler,
                outcome_label,
            )
            .as_bytes(),
        );
        let receipt_id = Hash::new(
            format!(
                "soracloud:runtime-failure-receipt:{}:{}:{}:{}:{}",
                request.mailbox_message.message_id,
                request.deployment.service_name,
                request.deployment.current_service_version,
                request.execution_sequence,
                outcome_label,
            )
            .as_bytes(),
        );
        let mut runtime_state = request.runtime_state.unwrap_or(SoraServiceRuntimeStateV1 {
            schema_version: iroha_data_model::soracloud::SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
            service_name: request.deployment.service_name.clone(),
            active_service_version: request.deployment.current_service_version.clone(),
            health_status: SoraServiceHealthStatusV1::Degraded,
            load_factor_bps: 0,
            materialized_bundle_hash: request.bundle.container.bundle_hash,
            rollout_handle: request
                .deployment
                .active_rollout
                .as_ref()
                .map(|rollout| rollout.rollout_handle.clone()),
            pending_mailbox_message_count: request.authoritative_pending_mailbox_messages,
            last_receipt_id: None,
        });
        runtime_state.health_status = SoraServiceHealthStatusV1::Degraded;
        runtime_state.pending_mailbox_message_count = request
            .authoritative_pending_mailbox_messages
            .saturating_sub(1);

        SoracloudOrderedMailboxExecutionResult {
            state_mutations: Vec::new(),
            outbound_mailbox_messages: Vec::new(),
            response_bytes: Vec::new(),
            content_type: None,
            runtime_state: Some(runtime_state),
            runtime_receipt: iroha_data_model::soracloud::SoraRuntimeReceiptV1 {
                schema_version: iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
                receipt_id,
                service_name: request.deployment.service_name,
                service_version: request.deployment.current_service_version,
                handler_name: request.mailbox_message.to_handler.clone(),
                handler_class: request
                    .handler
                    .as_ref()
                    .map(|handler| handler.class)
                    .unwrap_or(iroha_data_model::soracloud::SoraServiceHandlerClassV1::Update),
                request_commitment: request.mailbox_message.payload_commitment,
                result_commitment,
                certified_by: iroha_data_model::soracloud::SoraCertifiedResponsePolicyV1::None,
                emitted_sequence: request.execution_sequence,
                mailbox_message_id: Some(request.mailbox_message.message_id),
                journal_artifact_hash: None,
                checkpoint_artifact_hash: None,
                placement_id: None,
                selected_validator_account_id: None,
                selected_peer_id: None,
            },
        }
    }

    #[cfg(test)]
    fn validate_mailbox_runtime_receipt(
        request: &SoracloudOrderedMailboxExecutionRequest,
        receipt: &SoraRuntimeReceiptV1,
    ) -> Result<(), String> {
        let deployment = &request.deployment;
        let mailbox_message = &request.mailbox_message;
        let expected_handler_class = request
            .handler
            .as_ref()
            .map(|handler| handler.class)
            .unwrap_or(SoraServiceHandlerClassV1::Update);
        if receipt.service_name.as_ref() != deployment.service_name.as_ref() {
            return Err(format!(
                "receipt service `{}` does not match request service `{}`",
                receipt.service_name.as_ref(),
                deployment.service_name.as_ref()
            ));
        }
        if receipt.service_version.as_str() != deployment.current_service_version.as_str() {
            return Err(format!(
                "receipt service version `{}` does not match request version `{}`",
                receipt.service_version.as_str(),
                deployment.current_service_version.as_str()
            ));
        }
        if receipt.handler_name.as_ref() != mailbox_message.to_handler.as_ref() {
            return Err(format!(
                "receipt handler `{}` does not match mailbox handler `{}`",
                receipt.handler_name.as_ref(),
                mailbox_message.to_handler.as_ref()
            ));
        }
        if receipt.handler_class != expected_handler_class {
            return Err(format!(
                "receipt handler class `{:?}` does not match expected `{:?}`",
                receipt.handler_class, expected_handler_class
            ));
        }
        if receipt.mailbox_message_id.as_ref() != Some(&mailbox_message.message_id) {
            return Err(format!(
                "receipt mailbox message id `{:?}` does not match request mailbox message `{}`",
                &receipt.mailbox_message_id, &mailbox_message.message_id
            ));
        }
        if receipt.request_commitment != mailbox_message.payload_commitment {
            return Err(format!(
                "receipt request commitment `{}` does not match mailbox commitment `{}`",
                &receipt.request_commitment, &mailbox_message.payload_commitment
            ));
        }
        if receipt.emitted_sequence != request.execution_sequence {
            return Err(format!(
                "receipt emitted sequence `{}` does not match request execution sequence `{}`",
                receipt.emitted_sequence, request.execution_sequence
            ));
        }
        Ok(())
    }

    /// Test-only harness for legacy block-time mailbox execution.
    ///
    /// Production replay must not depend on a local Soracloud runtime. Runtime
    /// effects must be persisted through explicit Soracloud ISIs in committed
    /// transactions so Kura replay reconstructs the same WSV on every peer.
    #[cfg(test)]
    fn execute_soracloud_mailbox_runtime(state_block: &mut StateBlock<'_>) {
        let Some(runtime) = state_block.soracloud_runtime.clone() else {
            return;
        };
        let mut state_transaction = state_block.transaction();
        let ready_messages = collect_ready_soracloud_mailbox_messages(&state_transaction);
        if ready_messages.is_empty() {
            return;
        }

        let mut failed = false;
        for message in ready_messages {
            let (deployment, bundle) =
                match crate::smartcontracts::isi::soracloud::load_active_bundle(
                    &state_transaction,
                    &message.to_service,
                ) {
                    Ok(context) => context,
                    Err(error) => {
                        warn!(
                            ?error,
                            message_id = %message.message_id,
                            service = %message.to_service,
                            "skipping Soracloud mailbox execution because the active bundle context is missing"
                        );
                        continue;
                    }
                };
            let handler = bundle
                .service
                .handlers
                .iter()
                .find(|handler| handler.handler_name == message.to_handler)
                .cloned();
            let request = SoracloudOrderedMailboxExecutionRequest {
                observed_height: state_transaction.block_height(),
                observed_block_hash: StateReadOnly::latest_block_hash(&state_transaction)
                    .map(Hash::from),
                execution_sequence:
                    crate::smartcontracts::isi::soracloud::next_soracloud_audit_sequence(
                        &state_transaction,
                    ),
                deployment,
                bundle,
                handler,
                mailbox_message: message.clone(),
                runtime_state: state_transaction
                    .world
                    .soracloud_service_runtime
                    .get(&message.to_service)
                    .cloned(),
                authoritative_pending_mailbox_messages: authoritative_pending_mailbox_messages(
                    &state_transaction,
                    &message.to_service,
                ),
            };
            let result = match runtime.execute_ordered_mailbox(request.clone()) {
                Ok(result) => result,
                Err(error) => synthetic_mailbox_runtime_failure(request.clone(), error),
            };
            let SoracloudOrderedMailboxExecutionResult {
                state_mutations,
                outbound_mailbox_messages,
                response_bytes: _response_bytes,
                content_type: _content_type,
                runtime_state,
                runtime_receipt,
            } = result;
            if let Err(error) = validate_mailbox_runtime_receipt(&request, &runtime_receipt) {
                warn!(
                    error = %error,
                    message_id = %message.message_id,
                    "Soracloud mailbox execution returned a receipt that does not match the execution request"
                );
                failed = true;
                break;
            }

            for mutation in state_mutations {
                let binding_name: iroha_data_model::name::Name = match mutation.binding_name.parse()
                {
                    Ok(binding_name) => binding_name,
                    Err(error) => {
                        warn!(
                            ?error,
                            message_id = %message.message_id,
                            binding_name = %mutation.binding_name,
                            "Soracloud mailbox execution returned an invalid binding name"
                        );
                        failed = true;
                        break;
                    }
                };
                if let Err(error) =
                    crate::smartcontracts::isi::soracloud::apply_soracloud_state_mutation(
                        &mut state_transaction,
                        &request.deployment.service_name,
                        &binding_name,
                        &mutation.state_key,
                        mutation.operation,
                        mutation.payload,
                        mutation.encryption,
                        runtime_receipt.receipt_id,
                        request.execution_sequence,
                    )
                {
                    warn!(
                        ?error,
                        message_id = %message.message_id,
                        binding_name = %binding_name,
                        state_key = %mutation.state_key,
                        "failed to persist Soracloud service-state mutation returned by mailbox execution"
                    );
                    failed = true;
                    break;
                }
            }
            if failed {
                break;
            }

            for outbound in outbound_mailbox_messages {
                if let Err(error) =
                    crate::smartcontracts::isi::soracloud::write_soracloud_mailbox_message(
                        &mut state_transaction,
                        outbound,
                    )
                {
                    warn!(
                        ?error,
                        message_id = %message.message_id,
                        "failed to persist outbound Soracloud mailbox message"
                    );
                    failed = true;
                    break;
                }
            }
            if failed {
                break;
            }
            if let Some(runtime_state) = runtime_state
                && let Err(error) =
                    crate::smartcontracts::isi::soracloud::write_soracloud_runtime_state(
                        &mut state_transaction,
                        runtime_state,
                    )
            {
                warn!(
                    ?error,
                    message_id = %message.message_id,
                    "failed to persist Soracloud runtime-state write-back"
                );
                failed = true;
                break;
            }
            if let Err(error) =
                crate::smartcontracts::isi::soracloud::write_soracloud_runtime_receipt(
                    &mut state_transaction,
                    runtime_receipt,
                )
            {
                warn!(
                    ?error,
                    message_id = %message.message_id,
                    "failed to persist Soracloud runtime receipt"
                );
                failed = true;
                break;
            }
        }

        if !failed {
            state_transaction.apply();
        }
    }

    #[cfg(feature = "telemetry")]
    type MetricsRef<'a> = Option<&'a crate::telemetry::StateTelemetry>;
    #[cfg(not(feature = "telemetry"))]
    type MetricsRef<'a> = ();

    #[derive(Debug)]
    struct StaticValidationData {
        expected_block_height: usize,
        max_clock_drift: Duration,
        tx_params: iroha_data_model::parameter::TransactionParameters,
        crypto_cfg: Arc<iroha_config::parameters::actual::Crypto>,
        pipeline_cfg: iroha_config::parameters::actual::Pipeline,
        pipeline_parallelism: crate::state::PipelineParallelism,
        aggregate_lane: LaneId,
    }

    #[allow(clippy::too_many_lines)]
    pub fn validate_axt_envelopes(
        block: &SignedBlock,
        state_block: &StateBlock<'_>,
    ) -> Result<(), BlockValidationError> {
        let snapshot = block
            .axt_policy_snapshot()
            .cloned()
            .unwrap_or_else(|| state_block.axt_policy_snapshot());
        let snapshot_version = if snapshot.version != 0 {
            snapshot.version
        } else {
            AxtPolicySnapshot::compute_version(&snapshot.entries)
        };
        let make_axt_error_with =
            |reason: AxtRejectReason,
             message: &str,
             dataspace: Option<DataSpaceId>,
             lane: Option<LaneId>,
             next_min_handle_era: Option<u64>,
             next_min_sub_nonce: Option<u64>| {
                BlockValidationError::AxtEnvelopeValidationFailed(AxtEnvelopeValidationDetails {
                    message: message.to_owned(),
                    reason,
                    snapshot_version,
                    dataspace,
                    lane,
                    next_min_handle_era,
                    next_min_sub_nonce,
                })
            };
        let axt_timing = state_block.nexus.axt;
        let policies: BTreeMap<_, _> = snapshot
            .entries
            .iter()
            .map(|binding| (binding.dsid, binding.policy))
            .collect();
        let snapshot_slot = snapshot
            .entries
            .iter()
            .map(|entry| entry.policy.current_slot)
            .filter(|slot| *slot > 0)
            .max()
            .unwrap_or(0);
        let retention_slots = state_block.nexus.axt.replay_retention_slots.get();
        let mut seen: BTreeSet<AxtHandleReplayKey> = BTreeSet::new();

        if let Some(envelopes) = block.axt_envelopes() {
            #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
            struct HandleBudgetKey {
                binding: [u8; 32],
                handle_era: u64,
                target_lane: LaneId,
                manifest_root: [u8; 32],
                scope: Vec<String>,
                subject: HandleSubject,
                group_binding: GroupBinding,
                expiry_slot: u64,
                budget: HandleBudget,
                max_clock_skew_ms: Option<u32>,
            }

            struct HandleAccumulator {
                total: u128,
                per_dsid: BTreeMap<DataSpaceId, u128>,
            }

            impl HandleAccumulator {
                fn new() -> Self {
                    Self {
                        total: 0,
                        per_dsid: BTreeMap::new(),
                    }
                }

                fn apply(
                    &mut self,
                    dsid: DataSpaceId,
                    amount: u128,
                    budget: &HandleBudget,
                ) -> Result<(), String> {
                    self.total = self
                        .total
                        .checked_add(amount)
                        .ok_or_else(|| "handle budget overflow".to_owned())?;
                    let entry = self.per_dsid.entry(dsid).or_insert(0);
                    *entry = entry
                        .checked_add(amount)
                        .ok_or_else(|| "per-dataspace budget overflow".to_owned())?;
                    if self.total > budget.remaining {
                        return Err("handle budget exceeded".to_owned());
                    }
                    if let Some(per_use) = budget.per_use
                        && *entry > per_use
                    {
                        return Err("per-use budget exceeded".to_owned());
                    }
                    Ok(())
                }
            }

            let validate_proof = |proof: &ProofBlob,
                                  dsid: DataSpaceId,
                                  policy: &AxtPolicyEntry,
                                  policy_slot: u64,
                                  min_expiry_slot: Option<u64>|
             -> Result<(), BlockValidationError> {
                if proof.payload.is_empty() {
                    return Err(make_axt_error_with(
                        AxtRejectReason::Proof,
                        "empty proof payload",
                        Some(dsid),
                        Some(policy.target_lane),
                        None,
                        None,
                    ));
                }
                if policy.manifest_root.iter().all(|byte| *byte == 0) {
                    return Err(make_axt_error_with(
                        AxtRejectReason::Manifest,
                        "policy manifest root is zeroed",
                        Some(dsid),
                        Some(policy.target_lane),
                        None,
                        None,
                    ));
                }
                let envelope = norito::decode_from_bytes::<AxtProofEnvelope>(&proof.payload)
                    .map_err(|err| {
                        make_axt_error_with(
                            AxtRejectReason::Proof,
                            &format!("proof payload is not an AXT proof envelope: {err}"),
                            Some(dsid),
                            Some(policy.target_lane),
                            None,
                            None,
                        )
                    })?;
                if envelope.dsid != dsid || envelope.manifest_root != policy.manifest_root {
                    return Err(make_axt_error_with(
                        AxtRejectReason::Manifest,
                        "proof does not match policy manifest root",
                        Some(dsid),
                        Some(policy.target_lane),
                        None,
                        None,
                    ));
                }
                if let Some(expiry_slot) = proof.expiry_slot {
                    if expiry_slot == 0 {
                        return Err(make_axt_error_with(
                            AxtRejectReason::Proof,
                            "proof expiry slot is zero",
                            Some(dsid),
                            Some(policy.target_lane),
                            None,
                            None,
                        ));
                    }
                    let expiry_deadline = ivm::axt::expiry_slot_with_skew(
                        expiry_slot,
                        axt_timing.slot_length_ms,
                        axt_timing.max_clock_skew_ms,
                        None,
                    );
                    if policy_slot > 0 && policy_slot > expiry_deadline {
                        return Err(make_axt_error_with(
                            AxtRejectReason::Expiry,
                            "proof expired relative to policy slot",
                            Some(dsid),
                            Some(policy.target_lane),
                            None,
                            None,
                        ));
                    }
                    if let Some(min_expiry) = min_expiry_slot {
                        if min_expiry > expiry_slot {
                            return Err(make_axt_error_with(
                                AxtRejectReason::Expiry,
                                "proof expires before handle",
                                Some(dsid),
                                Some(policy.target_lane),
                                None,
                                None,
                            ));
                        }
                    }
                }
                fastpq_prover::verify_axt_proof_envelope(&envelope).map_err(|err| {
                    make_axt_error_with(
                        AxtRejectReason::Proof,
                        &format!("FASTPQ verification failed: {err}"),
                        Some(dsid),
                        Some(policy.target_lane),
                        None,
                        None,
                    )
                })?;
                Ok(())
            };

            let handle_budget_key =
                |handle: &AssetHandle| -> Result<HandleBudgetKey, BlockValidationError> {
                    if handle.manifest_view_root.len() != 32 {
                        return Err(make_axt_error_with(
                            AxtRejectReason::Manifest,
                            "handle manifest root must be 32 bytes",
                            None,
                            None,
                            None,
                            None,
                        ));
                    }
                    let mut manifest_root = [0u8; 32];
                    manifest_root.copy_from_slice(&handle.manifest_view_root);
                    Ok(HandleBudgetKey {
                        binding: *handle.axt_binding.as_bytes(),
                        handle_era: handle.handle_era,
                        target_lane: handle.target_lane,
                        manifest_root,
                        scope: handle.scope.clone(),
                        subject: handle.subject.clone(),
                        group_binding: handle.group_binding.clone(),
                        expiry_slot: handle.expiry_slot,
                        budget: handle.budget,
                        max_clock_skew_ms: handle.max_clock_skew_ms,
                    })
                };

            let make_env_error =
                |lane: LaneId,
                 reason: AxtRejectReason,
                 message: &str,
                 dsid: Option<DataSpaceId>,
                 next_min_handle_era: Option<u64>,
                 next_min_sub_nonce: Option<u64>| {
                    make_axt_error_with(
                        reason,
                        message,
                        dsid,
                        Some(lane),
                        next_min_handle_era,
                        next_min_sub_nonce,
                    )
                };

            for envelope in envelopes {
                let envelope_lane = envelope.lane;
                if let Err(err) = iroha_data_model::nexus::validate_descriptor(&envelope.descriptor)
                {
                    return Err(make_env_error(
                        envelope_lane,
                        AxtRejectReason::Descriptor,
                        &format!("invalid descriptor: {err}"),
                        None,
                        None,
                        None,
                    ));
                }
                let expected_binding = envelope.descriptor.binding().map_err(|err| {
                    make_env_error(
                        envelope_lane,
                        AxtRejectReason::Descriptor,
                        &format!("failed to compute descriptor binding: {err}"),
                        None,
                        None,
                        None,
                    )
                })?;
                if expected_binding != envelope.binding {
                    return Err(make_env_error(
                        envelope_lane,
                        AxtRejectReason::Descriptor,
                        "descriptor binding does not match envelope binding",
                        None,
                        None,
                        None,
                    ));
                }
                let expected_dsids: BTreeSet<_> =
                    envelope.descriptor.dsids.iter().copied().collect();
                let mut touch_specs: BTreeMap<DataSpaceId, &iroha_data_model::nexus::AxtTouchSpec> =
                    BTreeMap::new();
                for spec in &envelope.descriptor.touches {
                    touch_specs.insert(spec.dsid, spec);
                }
                let mut touch_dsids: BTreeSet<DataSpaceId> = BTreeSet::new();
                for touch in &envelope.touches {
                    if !expected_dsids.contains(&touch.dsid) {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Descriptor,
                            "touch references undeclared dataspace",
                            Some(touch.dsid),
                            None,
                            None,
                        ));
                    }
                    if !touch_dsids.insert(touch.dsid) {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Descriptor,
                            "duplicate touch manifest for dataspace",
                            Some(touch.dsid),
                            None,
                            None,
                        ));
                    }
                    if let Some(spec) = touch_specs.get(&touch.dsid) {
                        if (!spec.read.is_empty() || !spec.write.is_empty())
                            && touch.manifest.read.is_empty()
                            && touch.manifest.write.is_empty()
                        {
                            return Err(make_env_error(
                                envelope_lane,
                                AxtRejectReason::Descriptor,
                                "missing touch manifest for dataspace",
                                Some(touch.dsid),
                                None,
                                None,
                            ));
                        }
                        if !touch
                            .manifest
                            .read
                            .iter()
                            .all(|entry| spec.read.iter().any(|prefix| entry.starts_with(prefix)))
                        {
                            return Err(make_env_error(
                                envelope_lane,
                                AxtRejectReason::Descriptor,
                                "touch manifest read entry outside descriptor",
                                Some(touch.dsid),
                                None,
                                None,
                            ));
                        }
                        if !touch
                            .manifest
                            .write
                            .iter()
                            .all(|entry| spec.write.iter().any(|prefix| entry.starts_with(prefix)))
                        {
                            return Err(make_env_error(
                                envelope_lane,
                                AxtRejectReason::Descriptor,
                                "touch manifest write entry outside descriptor",
                                Some(touch.dsid),
                                None,
                                None,
                            ));
                        }
                    } else if !touch.manifest.read.is_empty() || !touch.manifest.write.is_empty() {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Descriptor,
                            "touch manifest provided without descriptor spec",
                            Some(touch.dsid),
                            None,
                            None,
                        ));
                    }
                }
                for spec in &envelope.descriptor.touches {
                    if (!spec.read.is_empty() || !spec.write.is_empty())
                        && !touch_dsids.contains(&spec.dsid)
                    {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Descriptor,
                            "missing touch manifest for dataspace",
                            Some(spec.dsid),
                            None,
                            None,
                        ));
                    }
                }
                let mut proofs_by_ds: BTreeMap<DataSpaceId, ProofBlob> = BTreeMap::new();
                for proof in &envelope.proofs {
                    if !expected_dsids.contains(&proof.dsid) {
                        return Err(make_axt_error_with(
                            AxtRejectReason::Descriptor,
                            "proof references undeclared dataspace",
                            Some(proof.dsid),
                            Some(envelope_lane),
                            None,
                            None,
                        ));
                    }
                    let policy = policies.get(&proof.dsid).ok_or_else(|| {
                        make_axt_error_with(
                            AxtRejectReason::MissingPolicy,
                            "no policy for dataspace",
                            Some(proof.dsid),
                            Some(envelope_lane),
                            None,
                            None,
                        )
                    })?;
                    validate_proof(&proof.proof, proof.dsid, policy, policy.current_slot, None)?;
                    if proofs_by_ds
                        .insert(proof.dsid, proof.proof.clone())
                        .is_some()
                    {
                        return Err(make_axt_error_with(
                            AxtRejectReason::Proof,
                            "duplicate proof for dataspace",
                            Some(proof.dsid),
                            Some(envelope_lane),
                            None,
                            None,
                        ));
                    }
                }

                let mut dataspace_proofs_present: BTreeSet<DataSpaceId> =
                    proofs_by_ds.keys().copied().collect();
                let mut accumulators: BTreeMap<HandleBudgetKey, HandleAccumulator> =
                    BTreeMap::new();

                for fragment in &envelope.handles {
                    let binding = fragment.handle.axt_binding;
                    if binding.as_bytes() != envelope.binding.as_bytes() {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Descriptor,
                            "handle binding does not match envelope binding",
                            None,
                            None,
                            None,
                        ));
                    }
                    let policy = policies.get(&fragment.intent.asset_dsid).ok_or_else(|| {
                        make_env_error(
                            envelope_lane,
                            AxtRejectReason::MissingPolicy,
                            "no policy for dataspace",
                            Some(fragment.intent.asset_dsid),
                            None,
                            None,
                        )
                    })?;
                    if !expected_dsids.contains(&fragment.intent.asset_dsid) {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Descriptor,
                            "handle references undeclared dataspace",
                            Some(fragment.intent.asset_dsid),
                            None,
                            None,
                        ));
                    }
                    let policy_slot = policy.current_slot;
                    let record_slot = if policy_slot > 0 {
                        policy_slot
                    } else {
                        snapshot_slot
                    };
                    if fragment.handle.handle_era == 0 {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::HandleEra,
                            "handle era is zero",
                            Some(fragment.intent.asset_dsid),
                            Some(policy.min_handle_era),
                            Some(policy.min_sub_nonce),
                        ));
                    }
                    if fragment.handle.sub_nonce == 0 {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::SubNonce,
                            "handle sub-nonce is zero",
                            Some(fragment.intent.asset_dsid),
                            Some(policy.min_handle_era),
                            Some(policy.min_sub_nonce),
                        ));
                    }
                    if fragment.handle.expiry_slot == 0 {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Expiry,
                            "handle expiry slot is zero",
                            Some(fragment.intent.asset_dsid),
                            None,
                            None,
                        ));
                    }
                    if fragment.handle.scope.is_empty() {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::PolicyDenied,
                            "handle scope is empty",
                            Some(fragment.intent.asset_dsid),
                            None,
                            None,
                        ));
                    }
                    if fragment
                        .handle
                        .scope
                        .iter()
                        .all(|scope| scope != &fragment.intent.op.kind)
                    {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::PolicyDenied,
                            "handle scope does not permit intent kind",
                            Some(fragment.intent.asset_dsid),
                            None,
                            None,
                        ));
                    }
                    if fragment.handle.subject.account != fragment.intent.op.from {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::PolicyDenied,
                            "handle subject does not match intent sender",
                            Some(fragment.intent.asset_dsid),
                            None,
                            None,
                        ));
                    }
                    if fragment
                        .handle
                        .group_binding
                        .composability_group_id
                        .is_empty()
                    {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::PolicyDenied,
                            "handle composability group id is empty",
                            Some(fragment.intent.asset_dsid),
                            None,
                            None,
                        ));
                    }
                    if policy.target_lane != fragment.handle.target_lane {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Lane,
                            "handle target lane does not match policy",
                            Some(fragment.intent.asset_dsid),
                            None,
                            None,
                        ));
                    }
                    if policy.manifest_root.iter().all(|byte| *byte == 0) {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Manifest,
                            "policy manifest root is zeroed",
                            Some(fragment.intent.asset_dsid),
                            None,
                            None,
                        ));
                    }
                    if fragment
                        .handle
                        .manifest_view_root
                        .iter()
                        .all(|byte| *byte == 0)
                    {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Manifest,
                            "handle manifest root is zeroed",
                            Some(fragment.intent.asset_dsid),
                            None,
                            None,
                        ));
                    }
                    if policy.manifest_root != fragment.handle.manifest_view_root {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Manifest,
                            "handle manifest root does not match policy",
                            Some(fragment.intent.asset_dsid),
                            None,
                            None,
                        ));
                    }
                    if fragment.handle.handle_era < policy.min_handle_era {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::HandleEra,
                            "handle era below policy minimum",
                            Some(fragment.intent.asset_dsid),
                            Some(policy.min_handle_era),
                            Some(policy.min_sub_nonce),
                        ));
                    }
                    if fragment.handle.sub_nonce < policy.min_sub_nonce {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::SubNonce,
                            "handle sub-nonce below policy minimum",
                            Some(fragment.intent.asset_dsid),
                            Some(policy.min_handle_era),
                            Some(policy.min_sub_nonce),
                        ));
                    }
                    let requested_skew_ms = fragment
                        .handle
                        .max_clock_skew_ms
                        .map_or(axt_timing.max_clock_skew_ms, u64::from);
                    if requested_skew_ms > axt_timing.max_clock_skew_ms {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Expiry,
                            "handle max_clock_skew_ms exceeds configured bound",
                            Some(fragment.intent.asset_dsid),
                            None,
                            None,
                        ));
                    }
                    let expiry_slot = ivm::axt::expiry_slot_with_skew(
                        fragment.handle.expiry_slot,
                        axt_timing.slot_length_ms,
                        axt_timing.max_clock_skew_ms,
                        fragment.handle.max_clock_skew_ms,
                    );
                    if policy_slot > 0 && policy_slot > expiry_slot {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Expiry,
                            "handle expired relative to policy slot",
                            Some(fragment.intent.asset_dsid),
                            None,
                            None,
                        ));
                    }

                    let replay_key = AxtHandleReplayKey::from_handle(&fragment.handle);
                    if let Some(entry) = state_block.world.axt_replay_ledger().get(&replay_key)
                        && !entry.is_expired(record_slot, retention_slots)
                    {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::ReplayCache,
                            "handle replayed in persisted ledger",
                            Some(fragment.intent.asset_dsid),
                            Some(policy.min_handle_era),
                            Some(policy.min_sub_nonce),
                        ));
                    }

                    let proof = fragment
                        .proof
                        .clone()
                        .or_else(|| proofs_by_ds.get(&fragment.intent.asset_dsid).cloned())
                        .ok_or_else(|| {
                            make_env_error(
                                envelope_lane,
                                AxtRejectReason::Proof,
                                "missing proof for dataspace",
                                Some(fragment.intent.asset_dsid),
                                None,
                                None,
                            )
                        })?;
                    validate_proof(
                        &proof,
                        fragment.intent.asset_dsid,
                        policy,
                        policy_slot,
                        Some(fragment.handle.expiry_slot),
                    )?;
                    dataspace_proofs_present.insert(fragment.intent.asset_dsid);

                    let proof_envelope =
                        norito::decode_from_bytes::<AxtProofEnvelope>(&proof.payload).ok();
                    let committed_amount = proof_envelope
                        .as_ref()
                        .and_then(|proof_envelope| proof_envelope.committed_amount);
                    let intent_amount = fragment.intent.op.amount.parse::<u128>().ok();
                    let effective_amount = match (intent_amount, committed_amount) {
                        (Some(intent_amount), Some(committed_amount)) => {
                            if intent_amount != committed_amount {
                                return Err(make_env_error(
                                    envelope_lane,
                                    AxtRejectReason::Budget,
                                    "intent amount does not match proof committed amount",
                                    Some(fragment.intent.asset_dsid),
                                    None,
                                    None,
                                ));
                            }
                            intent_amount
                        }
                        (Some(intent_amount), None) => intent_amount,
                        (None, Some(committed_amount)) => committed_amount,
                        (None, None) => {
                            return Err(make_env_error(
                                envelope_lane,
                                AxtRejectReason::Budget,
                                "intent amount is not a valid u128 and no committed proof amount was provided",
                                Some(fragment.intent.asset_dsid),
                                None,
                                None,
                            ));
                        }
                    };
                    if effective_amount == 0 {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Budget,
                            "handle amount must be non-zero",
                            Some(fragment.intent.asset_dsid),
                            None,
                            None,
                        ));
                    }
                    if intent_amount.is_some() {
                        if fragment.amount == 0 {
                            return Err(make_env_error(
                                envelope_lane,
                                AxtRejectReason::Budget,
                                "handle amount must be non-zero",
                                Some(fragment.intent.asset_dsid),
                                None,
                                None,
                            ));
                        }
                        if fragment.amount != effective_amount {
                            return Err(make_env_error(
                                envelope_lane,
                                AxtRejectReason::Budget,
                                "handle amount does not match intent amount",
                                Some(fragment.intent.asset_dsid),
                                None,
                                None,
                            ));
                        }
                    } else {
                        if fragment.amount != 0 {
                            return Err(make_env_error(
                                envelope_lane,
                                AxtRejectReason::Budget,
                                "hidden handle amount must be redacted in fragment",
                                Some(fragment.intent.asset_dsid),
                                None,
                                None,
                            ));
                        }
                        let expected_commitment = proof_envelope
                            .as_ref()
                            .and_then(|proof_envelope| proof_envelope.amount_commitment)
                            .unwrap_or_else(|| {
                                ivm::axt::derive_amount_commitment(
                                    fragment.intent.asset_dsid,
                                    effective_amount,
                                    Some(proof.payload.as_slice()),
                                )
                            });
                        if fragment.amount_commitment != Some(expected_commitment) {
                            return Err(make_env_error(
                                envelope_lane,
                                AxtRejectReason::Budget,
                                "hidden handle amount commitment mismatch",
                                Some(fragment.intent.asset_dsid),
                                None,
                                None,
                            ));
                        }
                    }

                    let budget_key = handle_budget_key(&fragment.handle)?;
                    match accumulators.entry(budget_key) {
                        std::collections::btree_map::Entry::Occupied(mut entry) => {
                            let budget = entry.key().budget;
                            let accumulator = entry.get_mut();
                            accumulator
                                .apply(fragment.intent.asset_dsid, effective_amount, &budget)
                                .map_err(|msg| {
                                    make_env_error(
                                        envelope_lane,
                                        AxtRejectReason::Budget,
                                        &msg,
                                        Some(fragment.intent.asset_dsid),
                                        None,
                                        None,
                                    )
                                })?;
                        }
                        std::collections::btree_map::Entry::Vacant(slot) => {
                            let key_ref = slot.key();
                            let mut acc = HandleAccumulator::new();
                            acc.apply(
                                fragment.intent.asset_dsid,
                                effective_amount,
                                &key_ref.budget,
                            )
                            .map_err(|msg| {
                                make_env_error(
                                    envelope_lane,
                                    AxtRejectReason::Budget,
                                    &msg,
                                    Some(fragment.intent.asset_dsid),
                                    None,
                                    None,
                                )
                            })?;
                            slot.insert(acc);
                        }
                    }

                    if !seen.insert(replay_key) {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::ReplayCache,
                            "duplicate handle usage in block",
                            Some(fragment.intent.asset_dsid),
                            None,
                            None,
                        ));
                    }
                }

                for dsid in &expected_dsids {
                    if !dataspace_proofs_present.contains(dsid) {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Proof,
                            "proof missing for dataspace",
                            Some(*dsid),
                            None,
                            None,
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Counts of signatures attached to a block.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct SignatureTally {
        /// Total signatures present on the block (all roles).
        pub present: usize,
        /// Deduplicated signatures from commit-eligible roles (leader, validators, set-B, proxy tail).
        pub counted: usize,
        /// Signatures contributed by set-B validators (role `SetBValidator`).
        pub set_b_signatures: usize,
    }

    /// Build a signature tally for the given block under the provided topology.
    pub fn commit_signature_tally(block: &SignedBlock, topology: &Topology) -> SignatureTally {
        let mut counted = BTreeSet::new();
        let commit_roles: &[Role] = &[
            Role::Leader,
            Role::ProxyTail,
            Role::ValidatingPeer,
            Role::SetBValidator,
        ];
        for signature in topology.filter_signatures_by_roles(commit_roles, block.signatures()) {
            if let Ok(idx) = usize::try_from(signature.index()) {
                counted.insert(idx);
            }
        }

        let set_b_signatures = topology
            .filter_signatures_by_roles(&[Role::SetBValidator], block.signatures())
            .count();

        SignatureTally {
            present: block.signatures().count(),
            counted: counted.len(),
            set_b_signatures,
        }
    }

    impl ValidBlock {
        fn new_unverified(block: SignedBlock) -> Self {
            Self {
                block,
                signatures_verified: false,
            }
        }

        #[cfg(test)]
        pub(crate) fn new_unverified_for_tests(block: SignedBlock) -> Self {
            Self::new_unverified(block)
        }

        fn new_signatures_verified(block: SignedBlock) -> Self {
            Self {
                block,
                signatures_verified: true,
            }
        }

        pub(crate) fn committed_from_replay_signed_block(block: SignedBlock) -> CommittedBlock {
            Self::new_signatures_verified(block)
                .commit_unchecked()
                .unpack(|_| {})
        }

        #[cfg(test)]
        fn mark_signatures_verified(&mut self) {
            self.signatures_verified = true;
        }

        fn clear_signatures_verified(&mut self) {
            self.signatures_verified = false;
        }

        #[cfg(test)]
        fn signatures_verified_for_tests(&self) -> bool {
            self.signatures_verified
        }

        fn verify_unique_signers(block: &SignedBlock) -> Result<(), SignatureVerificationError> {
            let mut seen = BTreeSet::new();
            for signature in block.signatures() {
                let signer = usize::try_from(signature.index())
                    .map_err(|_| SignatureVerificationError::UnknownSignatory)?;
                if !seen.insert(signer) {
                    return Err(SignatureVerificationError::DuplicateSignature { signer });
                }
            }
            Ok(())
        }

        fn verify_leader_signature(
            block: &SignedBlock,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            use SignatureVerificationError::{LeaderMissing, UnknownSignature};

            // Enforce BLS-normal for leader
            if topology.leader().public_key().algorithm() != iroha_crypto::Algorithm::BlsNormal {
                return Err(LeaderMissing);
            }

            let Some(signature) = topology
                .filter_signatures_by_roles(&[Role::Leader], block.signatures())
                .next()
            else {
                return Err(LeaderMissing);
            };

            signature
                .signature()
                .verify_hash(topology.leader().public_key(), block.hash())
                .map_err(|_err| UnknownSignature)?;

            Ok(())
        }

        fn verify_validator_signatures(
            block: &SignedBlock,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            // Enforce BLS-normal for validator roles in Set A + Set B (including proxy tail).
            let valid_roles: &[Role] =
                &[Role::ValidatingPeer, Role::SetBValidator, Role::ProxyTail];

            topology
                .filter_signatures_by_roles(valid_roles, block.signatures())
                .try_for_each(|signature| {
                    use SignatureVerificationError::{UnknownSignatory, UnknownSignature};

                    let signatory =
                        usize::try_from(signature.index()).map_err(|_err| UnknownSignatory)?;
                    let signatory: &PeerId =
                        topology.as_ref().get(signatory).ok_or(UnknownSignatory)?;
                    if signatory.public_key().algorithm() != iroha_crypto::Algorithm::BlsNormal {
                        return Err(UnknownSignature);
                    }

                    signature
                        .signature()
                        .verify_hash(signatory.public_key(), block.hash())
                        .map_err(|_err| UnknownSignature)?;

                    Ok(())
                })?;

            Ok(())
        }

        fn verify_no_undefined_signatures(
            block: &SignedBlock,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            if topology
                .filter_signatures_by_roles(&[Role::Undefined], block.signatures())
                .next()
                .is_some()
            {
                return Err(SignatureVerificationError::UnknownSignatory);
            }

            Ok(())
        }

        fn verify_signer_set(
            topology: &Topology,
            signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
            allow_quorum_bypass: bool,
        ) -> Result<(), SignatureVerificationError> {
            let roster_len = topology.as_ref().len();
            if roster_len <= 1 {
                return Ok(());
            }

            let min_votes_for_commit = topology.min_votes_for_commit();

            let mut seen = BTreeSet::new();
            for signer in signers {
                let signer = usize::try_from(*signer)
                    .map_err(|_| SignatureVerificationError::UnknownSignatory)?;
                if signer >= roster_len {
                    return Err(SignatureVerificationError::UnknownSignatory);
                }
                if !seen.insert(signer) {
                    return Err(SignatureVerificationError::DuplicateSignature { signer });
                }
            }

            let votes_count = signers.len();
            if votes_count < min_votes_for_commit && !allow_quorum_bypass {
                return Err(SignatureVerificationError::NotEnoughSignatures {
                    votes_count,
                    min_votes_for_commit,
                });
            }

            Ok(())
        }

        /// Verify every signature present on the block against the provided topology.
        ///
        /// This only checks signatures that exist on the block; it does not require a particular
        /// role to be present and accepts partial signature sets as long as each entry is valid.
        fn verify_signatures_against_topology(
            block: &SignedBlock,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            let hash = block.hash();
            for signature in block.signatures() {
                let signatory = usize::try_from(signature.index())
                    .map_err(|_| SignatureVerificationError::UnknownSignatory)?;
                let peer = topology
                    .as_ref()
                    .get(signatory)
                    .ok_or(SignatureVerificationError::UnknownSignatory)?;
                let role = topology.role(peer);
                match role {
                    Role::Leader | Role::ValidatingPeer | Role::ProxyTail | Role::SetBValidator => {
                        if peer.public_key().algorithm() != iroha_crypto::Algorithm::BlsNormal {
                            return Err(SignatureVerificationError::UnknownSignature);
                        }
                        signature
                            .signature()
                            .verify_hash(peer.public_key(), hash)
                            .map_err(|_| SignatureVerificationError::UnknownSignature)?;
                    }
                    Role::Undefined => return Err(SignatureVerificationError::UnknownSignatory),
                }
            }
            Ok(())
        }

        fn verify_signatures_against_topology_with_pops(
            block: &SignedBlock,
            topology: &Topology,
            pops: &BTreeMap<PublicKey, Vec<u8>>,
        ) -> Result<(), SignatureVerificationError> {
            let hash = block.hash();
            let mut bls_normal_signatures: Vec<&[u8]> = Vec::new();
            let mut bls_normal_public_keys: Vec<&PublicKey> = Vec::new();
            let mut bls_normal_pops: Vec<&[u8]> = Vec::new();
            for signature in block.signatures() {
                let signatory = usize::try_from(signature.index())
                    .map_err(|_| SignatureVerificationError::UnknownSignatory)?;
                let peer = topology
                    .as_ref()
                    .get(signatory)
                    .ok_or(SignatureVerificationError::UnknownSignatory)?;
                let role = topology.role(peer);
                match role {
                    Role::Leader | Role::ValidatingPeer | Role::ProxyTail | Role::SetBValidator => {
                        if peer.public_key().algorithm() != iroha_crypto::Algorithm::BlsNormal {
                            return Err(SignatureVerificationError::UnknownSignature);
                        }
                        let pop = pops
                            .get(peer.public_key())
                            .ok_or(SignatureVerificationError::MissingPop)?;
                        bls_normal_signatures.push(signature.signature().payload());
                        bls_normal_public_keys.push(peer.public_key());
                        bls_normal_pops.push(pop.as_slice());
                    }
                    Role::Undefined => return Err(SignatureVerificationError::UnknownSignatory),
                }
            }
            if !bls_normal_signatures.is_empty() {
                iroha_crypto::bls_normal_verify_aggregate_same_message_fast(
                    hash.as_ref(),
                    &bls_normal_signatures,
                    &bls_normal_public_keys,
                    &bls_normal_pops,
                )
                .map_err(|_| SignatureVerificationError::UnknownSignature)?;
            }
            Ok(())
        }

        /// Validate the signature set for the block against the provided topology and key registry.
        ///
        /// Unlike [`Self::is_commit`], this accepts partial signature sets and only enforces that
        /// each present signature is unique, maps to a known validator role, and uses a live
        /// consensus key.
        pub(crate) fn validate_signatures_subset_world(
            block: &SignedBlock,
            topology: &Topology,
            world: &impl WorldReadOnly,
        ) -> Result<(), SignatureVerificationError> {
            if block.header().is_genesis() {
                return Ok(());
            }
            Self::verify_unique_signers(block)?;
            let params = world.parameters();
            let sumeragi = params.sumeragi();
            let height = block.header().height().get();
            if world.consensus_keys().is_empty() {
                Self::verify_signatures_against_topology(block, topology)?;
                return Self::enforce_consensus_key_lifecycle_world(block, topology, world);
            }
            let pops = Self::collect_validator_pops(
                world,
                height,
                sumeragi.key_overlap_grace_blocks,
                sumeragi.key_expiry_grace_blocks,
            )?;
            Self::verify_signatures_against_topology_with_pops(block, topology, &pops)?;
            Self::enforce_consensus_key_lifecycle_world(block, topology, world)
        }

        #[cfg(any(test, feature = "iroha-core-tests"))]
        #[allow(dead_code)]
        pub(crate) fn validate_signatures_subset(
            block: &SignedBlock,
            topology: &Topology,
            state: &impl StateReadOnly,
        ) -> Result<(), SignatureVerificationError> {
            Self::validate_signatures_subset_world(block, topology, state.world())
        }

        fn collect_validator_pops(
            world: &impl WorldReadOnly,
            height: u64,
            overlap_grace_blocks: u64,
            expiry_grace_blocks: u64,
        ) -> Result<BTreeMap<PublicKey, Vec<u8>>, SignatureVerificationError> {
            let mut pops: BTreeMap<PublicKey, Vec<u8>> = BTreeMap::new();
            for (id, record) in world.consensus_keys().iter() {
                if id.role != ConsensusKeyRole::Validator {
                    continue;
                }
                if !record.is_live_at(height, overlap_grace_blocks, expiry_grace_blocks) {
                    continue;
                }
                match record.public_key.algorithm() {
                    iroha_crypto::Algorithm::BlsNormal => {
                        let Some(pop) = record.pop.as_ref() else {
                            return Err(SignatureVerificationError::MissingPop);
                        };
                        if let Some(existing) = pops.get(&record.public_key) {
                            if existing.as_slice() != pop.as_slice() {
                                return Err(SignatureVerificationError::Other);
                            }
                            continue;
                        }
                        pops.insert(record.public_key.clone(), pop.clone());
                    }
                    _ => {}
                }
            }
            Ok(pops)
        }

        pub(crate) fn enforce_consensus_key_lifecycle_world(
            block: &SignedBlock,
            topology: &Topology,
            world: &impl WorldReadOnly,
        ) -> Result<(), SignatureVerificationError> {
            if block.header().is_genesis() {
                return Ok(());
            }
            // Skip enforcement until consensus keys are explicitly registered. Once any
            // registry entries exist, validators must present a live key for signing.
            if world.consensus_keys().is_empty() {
                return Ok(());
            }
            let params = world.parameters();
            let sumeragi = params.sumeragi();
            let overlap = sumeragi.key_overlap_grace_blocks;
            let expiry_grace = sumeragi.key_expiry_grace_blocks;
            let height = block.header().height().get();
            for signature in topology.filter_signatures_by_roles(
                &[
                    Role::ValidatingPeer,
                    Role::SetBValidator,
                    Role::Leader,
                    Role::ProxyTail,
                ],
                block.signatures(),
            ) {
                let signatory = usize::try_from(signature.index())
                    .map_err(|_| SignatureVerificationError::UnknownSignatory)?;
                let signatory = topology
                    .as_ref()
                    .get(signatory)
                    .ok_or(SignatureVerificationError::UnknownSignatory)?;
                let pk = signatory.public_key();
                let pk_label = pk.to_string();
                let mut found_index_record = false;
                let mut live = world
                    .consensus_keys_by_pk()
                    .get(&pk_label)
                    .is_some_and(|ids| {
                        ids.iter().any(|id| {
                            world.consensus_keys().get(id).is_some_and(|rec| {
                                found_index_record = true;
                                rec.id.role == ConsensusKeyRole::Validator
                                    && rec.is_live_at(height, overlap, expiry_grace)
                            })
                        })
                    });
                if !live && !found_index_record {
                    // Fallback when the pk index is stale or missing for this peer.
                    live = world.consensus_keys().iter().any(|(id, rec)| {
                        id.role == ConsensusKeyRole::Validator
                            && rec.public_key == *pk
                            && rec.is_live_at(height, overlap, expiry_grace)
                    });
                }
                if !live {
                    return Err(SignatureVerificationError::InactiveConsensusKey);
                }
            }
            Ok(())
        }

        pub(crate) fn enforce_consensus_key_lifecycle(
            block: &SignedBlock,
            topology: &Topology,
            state: &impl StateReadOnly,
        ) -> Result<(), SignatureVerificationError> {
            Self::enforce_consensus_key_lifecycle_world(block, topology, state.world())
        }

        fn ensure_genesis_transactions_clean(
            block: &SignedBlock,
            genesis_account: &AccountId,
            expected_chain_id: &ChainId,
        ) -> Result<(), BlockValidationError> {
            if block.header().is_genesis() {
                if !block.has_results() {
                    iroha_logger::error!(
                        "Invalid genesis block rejected during validation: execution results missing"
                    );
                    return Err(BlockValidationError::InvalidGenesis(
                        InvalidGenesisError::ContainsErrors,
                    ));
                }
                if let Err(err) = check_genesis_block(block, genesis_account, expected_chain_id) {
                    iroha_logger::error!(
                        error = %err,
                        "Invalid genesis block rejected during validation"
                    );
                    return Err(BlockValidationError::InvalidGenesis(err));
                }
            }
            Ok(())
        }

        /// Validate the given block, apply resulting state changes,
        /// and record any transaction errors back into the block.
        pub fn validate(
            mut block: SignedBlock,
            topology: &Topology,
            expected_chain_id: &ChainId,
            genesis_account: &AccountId,
            time_source: &TimeSource,
            state_block: &mut StateBlock<'_>,
        ) -> WithEvents<Result<ValidBlock, Error>> {
            if let Err(error) = Self::validate_static(
                &block,
                topology,
                expected_chain_id,
                genesis_account,
                state_block,
                false,
                time_source,
            ) {
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            let exec_witness_guard = crate::sumeragi::witness::exec_witness_guard();
            if let Err(error) =
                Self::validate_and_record_transactions(&mut block, state_block, None, true)
            {
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Err(error) = validate_axt_envelopes(&block, state_block) {
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            state_block.capture_exec_witness();
            drop(exec_witness_guard);
            if block.is_empty() {
                let error = BlockValidationError::EmptyBlock;
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Err(error) =
                Self::ensure_genesis_transactions_clean(&block, genesis_account, expected_chain_id)
            {
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            WithEvents::new(Ok(ValidBlock::new_signatures_verified(block)))
        }

        /// Validate the given block and emit a rejection event on failure using the provided callback.
        pub fn validate_with_events<F: Fn(PipelineEventBox)>(
            mut block: SignedBlock,
            topology: &Topology,
            expected_chain_id: &ChainId,
            genesis_account: &AccountId,
            time_source: &TimeSource,
            state_block: &mut StateBlock<'_>,
            send_events: F,
        ) -> WithEvents<Result<ValidBlock, Error>> {
            if let Err(error) = Self::validate_static(
                &block,
                topology,
                expected_chain_id,
                genesis_account,
                state_block,
                false,
                time_source,
            ) {
                // Emit rejection with the offending header
                let ev = PipelineEventBox::from(BlockEvent {
                    header: block.header(),
                    status: BlockStatus::Rejected(map_block_err_to_reason(&error)),
                });
                send_events(ev);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            let exec_witness_guard = crate::sumeragi::witness::exec_witness_guard();
            if let Err(error) =
                Self::validate_and_record_transactions(&mut block, state_block, None, true)
            {
                let ev = PipelineEventBox::from(BlockEvent {
                    header: block.header(),
                    status: BlockStatus::Rejected(map_block_err_to_reason(&error)),
                });
                send_events(ev);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Err(error) = validate_axt_envelopes(&block, state_block) {
                let ev = PipelineEventBox::from(BlockEvent {
                    header: block.header(),
                    status: BlockStatus::Rejected(map_block_err_to_reason(&error)),
                });
                send_events(ev);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            state_block.capture_exec_witness();
            drop(exec_witness_guard);
            if block.is_empty() {
                let error = BlockValidationError::EmptyBlock;
                let ev = PipelineEventBox::from(BlockEvent {
                    header: block.header(),
                    status: BlockStatus::Rejected(map_block_err_to_reason(&error)),
                });
                send_events(ev);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Err(error) =
                Self::ensure_genesis_transactions_clean(&block, genesis_account, expected_chain_id)
            {
                let ev = PipelineEventBox::from(BlockEvent {
                    header: block.header(),
                    status: BlockStatus::Rejected(map_block_err_to_reason(&error)),
                });
                send_events(ev);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            WithEvents::new(Ok(ValidBlock::new_signatures_verified(block)))
        }

        /// Same as [`Self::validate`] but:
        /// * Block will be validated (statically checked) with read-only state
        /// * If block is valid, voting block will be released,
        ///   and transactions will be validated (executed) with write state
        #[allow(clippy::too_many_arguments)]
        pub fn validate_keep_voting_block<'state>(
            block: SignedBlock,
            topology: &Topology,
            expected_chain_id: &ChainId,
            genesis_account: &AccountId,
            time_source: &TimeSource,
            state: &'state State,
            voting_block: &mut Option<VotingBlock>,
            soft_fork: bool,
        ) -> WithEvents<Result<(ValidBlock, StateBlock<'state>), Error>> {
            Self::validate_keep_voting_block_inner(
                block,
                topology,
                expected_chain_id,
                genesis_account,
                time_source,
                state,
                voting_block,
                soft_fork,
                None,
                false,
                false,
                false,
                None,
            )
        }

        /// Replay-specific validation entrypoint that can optionally bypass block signature checks.
        ///
        /// This is intentionally crate-private and should only be used for controlled migration or
        /// recovery scenarios where historical blocks cannot be validated with current signature
        /// semantics.
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn validate_keep_voting_block_for_replay<'state>(
            block: SignedBlock,
            topology: &Topology,
            expected_chain_id: &ChainId,
            genesis_account: &AccountId,
            time_source: &TimeSource,
            state: &'state State,
            voting_block: &mut Option<VotingBlock>,
            soft_fork: bool,
            skip_block_signatures: bool,
            trust_replay_tx_signatures: bool,
        ) -> WithEvents<Result<(ValidBlock, StateBlock<'state>), Error>> {
            Self::validate_keep_voting_block_inner(
                block,
                topology,
                expected_chain_id,
                genesis_account,
                time_source,
                state,
                voting_block,
                soft_fork,
                None,
                skip_block_signatures,
                trust_replay_tx_signatures,
                true,
                None,
            )
        }

        /// Same as [`Self::validate_keep_voting_block`], but records timing breakdowns.
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn validate_keep_voting_block_with_timing<'state>(
            block: SignedBlock,
            topology: &Topology,
            expected_chain_id: &ChainId,
            genesis_account: &AccountId,
            time_source: &TimeSource,
            state: &'state State,
            voting_block: &mut Option<VotingBlock>,
            soft_fork: bool,
            timings: &mut ValidationTimings,
        ) -> WithEvents<Result<(ValidBlock, StateBlock<'state>), Error>> {
            Self::validate_keep_voting_block_inner(
                block,
                topology,
                expected_chain_id,
                genesis_account,
                time_source,
                state,
                voting_block,
                soft_fork,
                Some(timings),
                false,
                false,
                false,
                None,
            )
        }

        /// Execute a previously validated commit candidate while preserving current-tip checks.
        ///
        /// Callers must only use this after independently verifying that local validation roots
        /// and commit-certificate roots agree for the same block. The path still checks
        /// state-dependent block invariants, transaction limits, duplicate detection, and
        /// execution-context alignment, but trusts the already validated block and transaction
        /// signatures so commit does not repeat that cryptographic work.
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn validate_prevalidated_commit_keep_voting_block_with_events_and_timing<
            'state,
            F: FnMut(PipelineEventBox),
        >(
            block: SignedBlock,
            topology: &Topology,
            expected_chain_id: &ChainId,
            genesis_account: &AccountId,
            time_source: &TimeSource,
            state: &'state State,
            voting_block: &mut Option<VotingBlock>,
            timings: &mut ValidationTimings,
            mut send_events: F,
        ) -> WithEvents<Result<(ValidBlock, StateBlock<'state>), Error>> {
            Self::validate_keep_voting_block_inner(
                block,
                topology,
                expected_chain_id,
                genesis_account,
                time_source,
                state,
                voting_block,
                false,
                Some(timings),
                true,
                true,
                false,
                Some(&mut send_events),
            )
        }

        #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
        fn validate_keep_voting_block_inner<'state>(
            mut block: SignedBlock,
            topology: &Topology,
            expected_chain_id: &ChainId,
            genesis_account: &AccountId,
            time_source: &TimeSource,
            state: &'state State,
            voting_block: &mut Option<VotingBlock>,
            soft_fork: bool,
            timings: Option<&mut ValidationTimings>,
            skip_block_signatures: bool,
            trust_replay_tx_signatures: bool,
            replay_compatibility: bool,
            mut send_events: Option<&mut dyn FnMut(PipelineEventBox)>,
        ) -> WithEvents<Result<(ValidBlock, StateBlock<'state>), Error>> {
            let total_start = Instant::now();
            let stateless_start = Instant::now();
            let to_ms = |duration: Duration| -> u64 {
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
            };
            let mut timings = timings;
            let mut emit_rejection = |block: &SignedBlock, error: &BlockValidationError| {
                if let Some(send_events) = send_events.as_deref_mut() {
                    let ev = PipelineEventBox::from(BlockEvent {
                        header: block.header(),
                        status: BlockStatus::Rejected(map_block_err_to_reason(error)),
                    });
                    send_events(ev);
                }
            };
            let record_timings =
                |timings: &mut Option<&mut ValidationTimings>,
                 stateless_elapsed: Duration,
                 execution_start: Option<Instant>| {
                    if let Some(timings) = timings.as_deref_mut() {
                        timings.stateless_ms = to_ms(stateless_elapsed);
                        timings.execution_ms =
                            execution_start.map_or(0, |start| to_ms(start.elapsed()));
                        timings.total_ms = to_ms(total_start.elapsed());
                    }
                };
            let static_state_start = Instant::now();
            let static_data = {
                let view = state.query_view();
                match Self::validate_static_state_dependent(
                    &block,
                    topology,
                    expected_chain_id,
                    genesis_account,
                    &view,
                    soft_fork,
                    time_source,
                    skip_block_signatures,
                    replay_compatibility,
                ) {
                    Ok(data) => {
                        if let Some(timings) = timings.as_deref_mut() {
                            timings.stateless_state_dependent_ms =
                                to_ms(static_state_start.elapsed());
                        }
                        data
                    }
                    Err(error) => {
                        let stateless_elapsed = stateless_start.elapsed();
                        if let Some(timings) = timings.as_deref_mut() {
                            timings.stateless_state_dependent_ms =
                                to_ms(static_state_start.elapsed());
                        }
                        record_timings(&mut timings, stateless_elapsed, None);
                        emit_rejection(&block, &error);
                        return WithEvents::new(Err((Box::new(block), Box::new(error))));
                    }
                }
            };
            let prepared_txs = Self::prepare_external_transactions(&block);
            let committed_heights = {
                let transactions_view = state.transactions.view();
                Self::committed_heights_for_prepared_transactions(&prepared_txs, &transactions_view)
            };
            let cache_cap = static_data.pipeline_cfg.stateless_cache_cap;
            let cache_enabled = cache_cap > 0 && !block.header().is_genesis();
            let max_clock_drift_ms = static_data.max_clock_drift.as_millis();
            let cache_context = if cache_enabled {
                Some(StatelessValidationContext::new(
                    expected_chain_id.clone(),
                    u64::try_from(max_clock_drift_ms).unwrap_or(u64::MAX),
                    static_data.tx_params,
                    static_data.crypto_cfg.allowed_signing.clone(),
                ))
            } else {
                None
            };
            #[cfg(feature = "telemetry")]
            let metrics = Some(&state.telemetry);
            #[cfg(not(feature = "telemetry"))]
            let metrics = ();
            let cache_now_ms = block.header().creation_time().as_millis();
            let cached_stateless_ok = cache_context.as_ref().map(|context| {
                let mut cache = state.stateless_validation_cache().lock();
                cache.set_cap(cache_cap);
                cache.ensure_context(context.clone());
                prepared_txs
                    .iter()
                    .map(|prepared| cache.get_ok(&prepared.metadata.signed_hash, cache_now_ms))
                    .collect::<Vec<_>>()
            });
            let static_snapshot_start = Instant::now();
            if let Err(error) = Self::validate_static_with_snapshot(
                &block,
                expected_chain_id,
                genesis_account,
                &static_data,
                &committed_heights,
                &prepared_txs,
                cached_stateless_ok.as_deref(),
                trust_replay_tx_signatures,
                metrics,
            ) {
                let stateless_elapsed = stateless_start.elapsed();
                if let Some(timings) = timings.as_deref_mut() {
                    timings.stateless_snapshot_ms = to_ms(static_snapshot_start.elapsed());
                }
                record_timings(&mut timings, stateless_elapsed, None);
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Err(error) = Self::validate_npos_effects_with_state(&block, state) {
                let stateless_elapsed = stateless_start.elapsed();
                record_timings(&mut timings, stateless_elapsed, None);
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Some(context) = cache_context {
                let mut cache = state.stateless_validation_cache().lock();
                cache.set_cap(cache_cap);
                cache.ensure_context(context);
                for (tx, prepared) in Self::collect_external_signed_transactions(&block)
                    .into_iter()
                    .zip(prepared_txs.iter())
                {
                    let expires_at_ms = tx
                        .time_to_live()
                        .and_then(|ttl| tx.creation_time().checked_add(ttl))
                        .map(|expires_at| expires_at.as_millis());
                    let not_before_ms = tx
                        .creation_time()
                        .as_millis()
                        .saturating_sub(max_clock_drift_ms);
                    cache.insert_ok(prepared.metadata.signed_hash, expires_at_ms, not_before_ms);
                }
            }
            if let Some(timings) = timings.as_deref_mut() {
                timings.stateless_snapshot_ms = to_ms(static_snapshot_start.elapsed());
            }
            let stateless_elapsed = stateless_start.elapsed();
            let execution_start = Instant::now();
            // Release block writer before creating new one
            let _ = voting_block.take();
            let da_indexes_start = Instant::now();
            if let Err(error) = state.ensure_da_indexes_hydrated() {
                if let Some(timings) = timings.as_deref_mut() {
                    timings.execution_da_indexes_ms = to_ms(da_indexes_start.elapsed());
                }
                record_timings(&mut timings, stateless_elapsed, Some(execution_start));
                let error = BlockValidationError::DaShardCursor(error);
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Some(timings) = timings.as_deref_mut() {
                timings.execution_da_indexes_ms = to_ms(da_indexes_start.elapsed());
            }
            let state_block_start = Instant::now();
            let mut state_block = if soft_fork {
                state.block_and_revert(block.header())
            } else {
                state.block(block.header())
            };
            state_block.replay_compatibility = replay_compatibility;
            if let Some(timings) = timings.as_deref_mut() {
                timings.execution_state_block_ms = to_ms(state_block_start.elapsed());
            }
            let exec_witness_guard = crate::sumeragi::witness::exec_witness_guard();
            let tx_start = Instant::now();
            if let Err(error) = Self::validate_and_record_transactions_with_prepared(
                &mut block,
                &mut state_block,
                timings.as_deref_mut(),
                true,
                Some(&prepared_txs),
            ) {
                drop(state_block);
                record_timings(&mut timings, stateless_elapsed, Some(execution_start));
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Some(timings) = timings.as_deref_mut() {
                timings.execution_tx_ms = to_ms(tx_start.elapsed());
            }
            let axt_start = Instant::now();
            if let Err(error) = validate_axt_envelopes(&block, &state_block) {
                drop(state_block);
                if let Some(timings) = timings.as_deref_mut() {
                    timings.execution_axt_ms = to_ms(axt_start.elapsed());
                }
                record_timings(&mut timings, stateless_elapsed, Some(execution_start));
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Some(timings) = timings.as_deref_mut() {
                timings.execution_axt_ms = to_ms(axt_start.elapsed());
            }
            let da_cursor_start = Instant::now();
            if let Err(error) = state_block.validate_da_shard_cursors(&block) {
                drop(state_block);
                if let Some(timings) = timings.as_deref_mut() {
                    timings.execution_da_cursor_ms = to_ms(da_cursor_start.elapsed());
                }
                record_timings(&mut timings, stateless_elapsed, Some(execution_start));
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Some(timings) = timings.as_deref_mut() {
                timings.execution_da_cursor_ms = to_ms(da_cursor_start.elapsed());
            }
            state_block.capture_exec_witness();
            drop(exec_witness_guard);
            if block.is_empty() {
                let error = BlockValidationError::EmptyBlock;
                drop(state_block);
                record_timings(&mut timings, stateless_elapsed, Some(execution_start));
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            let genesis_clean_start = Instant::now();
            if let Err(error) =
                Self::ensure_genesis_transactions_clean(&block, genesis_account, expected_chain_id)
            {
                drop(state_block);
                if let Some(timings) = timings.as_deref_mut() {
                    timings.execution_genesis_clean_ms = to_ms(genesis_clean_start.elapsed());
                }
                record_timings(&mut timings, stateless_elapsed, Some(execution_start));
                emit_rejection(&block, &error);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Some(timings) = timings.as_deref_mut() {
                timings.execution_genesis_clean_ms = to_ms(genesis_clean_start.elapsed());
            }
            record_timings(&mut timings, stateless_elapsed, Some(execution_start));
            WithEvents::new(Ok((
                ValidBlock::new_signatures_verified(block),
                state_block,
            )))
        }

        /// Like [`Self::validate_keep_voting_block`], but emits a rejection block event on failure.
        #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
        pub fn validate_keep_voting_block_with_events<'state, F: FnMut(PipelineEventBox)>(
            block: SignedBlock,
            topology: &Topology,
            expected_chain_id: &ChainId,
            genesis_account: &AccountId,
            time_source: &TimeSource,
            state: &'state State,
            voting_block: &mut Option<VotingBlock>,
            soft_fork: bool,
            mut send_events: F,
        ) -> WithEvents<Result<(ValidBlock, StateBlock<'state>), Error>> {
            Self::validate_keep_voting_block_inner(
                block,
                topology,
                expected_chain_id,
                genesis_account,
                time_source,
                state,
                voting_block,
                soft_fork,
                None,
                false,
                false,
                false,
                Some(&mut send_events),
            )
        }

        /// Like [`Self::validate_keep_voting_block_with_events`], but records timing breakdowns.
        #[allow(clippy::too_many_arguments)]
        pub(crate) fn validate_keep_voting_block_with_events_and_timing<
            'state,
            F: FnMut(PipelineEventBox),
        >(
            block: SignedBlock,
            topology: &Topology,
            expected_chain_id: &ChainId,
            genesis_account: &AccountId,
            time_source: &TimeSource,
            state: &'state State,
            voting_block: &mut Option<VotingBlock>,
            soft_fork: bool,
            timings: &mut ValidationTimings,
            mut send_events: F,
        ) -> WithEvents<Result<(ValidBlock, StateBlock<'state>), Error>> {
            Self::validate_keep_voting_block_inner(
                block,
                topology,
                expected_chain_id,
                genesis_account,
                time_source,
                state,
                voting_block,
                soft_fork,
                Some(timings),
                false,
                false,
                false,
                Some(&mut send_events),
            )
        }

        /// All static checks that require a state snapshot.
        #[allow(
            clippy::too_many_arguments,
            clippy::too_many_lines,
            clippy::explicit_iter_loop,
            clippy::collapsible_else_if
        )]
        fn validate_static_state_dependent(
            block: &SignedBlock,
            topology: &Topology,
            chain_id: &ChainId,
            genesis_account: &AccountId,
            state: &impl StateReadOnly,
            soft_fork: bool,
            time_source: &TimeSource,
            skip_block_signatures: bool,
            allow_missing_legacy_context: bool,
        ) -> Result<StaticValidationData, BlockValidationError> {
            let state_height = state.block_hashes().len();
            let expected_block_height = if soft_fork {
                state_height
            } else {
                state_height
                    .checked_add(1)
                    .expect("INTERNAL BUG: Block height exceeds usize::MAX")
            };
            let actual_height = block
                .header()
                .height()
                .get()
                .try_into()
                .expect("INTERNAL BUG: Block height exceeds usize::MAX");

            if expected_block_height != actual_height {
                let state_latest_hash = state.block_hashes().iter().nth_back(0).copied();
                let state_prev_hash = state.block_hashes().iter().nth_back(1).copied();
                iroha_logger::warn!(
                    expected_height = expected_block_height,
                    actual_height,
                    state_height,
                    block_prev_hash = ?block.header().prev_block_hash(),
                    block_hash = ?block.hash(),
                    state_latest_hash = ?state_latest_hash,
                    state_prev_hash = ?state_prev_hash,
                    "prev block height mismatch during static validation"
                );
                return Err(BlockValidationError::PrevBlockHeightMismatch {
                    expected: expected_block_height,
                    actual: actual_height,
                });
            }

            let params = state.world().parameters();
            let max_clock_drift = params.sumeragi().max_clock_drift();
            let tx_params = params.transaction();

            let now = time_source.now();
            let block_creation_time = block.header().creation_time();
            if block_creation_time.saturating_sub(now) > max_clock_drift {
                return Err(BlockValidationError::BlockInTheFuture);
            }

            let expected_prev_block_hash = if soft_fork {
                state.block_hashes().iter().nth_back(1).copied()
            } else {
                state.block_hashes().iter().nth_back(0).copied()
            };
            let actual_prev_block_hash = block.header().prev_block_hash();

            if expected_prev_block_hash != actual_prev_block_hash {
                return Err(BlockValidationError::PrevBlockHashMismatch {
                    expected: expected_prev_block_hash,
                    actual: actual_prev_block_hash,
                });
            }
            Self::validate_previous_roster_evidence(
                block,
                block.header().height().get(),
                actual_prev_block_hash,
            )?;
            Self::validate_npos_effects_header(block)?;
            Self::validate_execution_context_with_state(
                block,
                state,
                allow_missing_legacy_context,
            )?;

            let nexus = state.nexus();
            let expected_policy_hash = proof_policy_bundle_hash(&nexus.lane_config);
            if block.header().da_proof_policies_hash() != Some(expected_policy_hash) {
                return Err(BlockValidationError::ProofPolicyHashMismatch {
                    expected: expected_policy_hash,
                    actual: block.header().da_proof_policies_hash(),
                });
            }

            let block_height = block.header().height().get();
            let computed_digest =
                compute_confidential_feature_digest(state.world(), state.zk(), block_height);
            let expected_digest = if computed_digest.is_empty() {
                None
            } else {
                Some(computed_digest)
            };
            if block.header().confidential_features() != expected_digest {
                return Err(BlockValidationError::ConfidentialFeaturesMismatch {
                    expected: expected_digest,
                    actual: block.header().confidential_features(),
                });
            }

            if block.header().is_genesis() {
                if block.has_results() {
                    check_genesis_block(block, genesis_account, chain_id)?;
                }
            } else {
                let prev_block_time = if soft_fork {
                    state.prev_block()
                } else {
                    state.latest_block()
                }
                .expect("INTERNAL BUG: Genesis not committed")
                .header()
                .creation_time();

                if block.header().creation_time() <= prev_block_time {
                    return Err(BlockValidationError::BlockInThePast);
                }

                if !skip_block_signatures {
                    Self::verify_leader_signature(block, topology)?;
                    // Enforce BLS-normal for validator signatures (Set A + Set B).
                    Self::verify_validator_signatures(block, topology)?;
                    Self::verify_no_undefined_signatures(block, topology)?;
                    Self::verify_unique_signers(block)?;
                    Self::enforce_consensus_key_lifecycle(block, topology, state)?;
                }
            }

            let crypto_cfg = state.crypto();
            let pipeline_cfg = state.pipeline().clone();
            let pipeline_parallelism = crate::state::PipelineParallelism::new(&pipeline_cfg);
            let aggregate_lane = nexus.routing_policy.default_lane;

            Ok(StaticValidationData {
                expected_block_height,
                max_clock_drift,
                tx_params,
                crypto_cfg,
                pipeline_cfg,
                pipeline_parallelism,
                aggregate_lane,
            })
        }

        fn npos_effects_error(message: impl Into<String>) -> BlockValidationError {
            BlockValidationError::NposEffectsInvalid(message.into())
        }

        fn validate_npos_effects_header(block: &SignedBlock) -> Result<(), BlockValidationError> {
            match (
                block.header().npos_effects_hash(),
                block.npos_consensus_effects(),
            ) {
                (None, None) => Ok(()),
                (Some(_), None) => Err(Self::npos_effects_error(
                    "header references NPoS effects but payload is missing",
                )),
                (None, Some(_)) => Err(Self::npos_effects_error(
                    "payload includes NPoS effects but header hash is absent",
                )),
                (Some(expected), Some(effects)) => {
                    let actual = HashOf::new(effects);
                    if actual != expected {
                        return Err(Self::npos_effects_error("NPoS effects hash mismatch"));
                    }
                    Ok(())
                }
            }
        }

        fn vrf_epoch_record_extends_existing(
            existing: &VrfEpochRecord,
            proposed: &VrfEpochRecord,
        ) -> bool {
            if existing == proposed {
                return true;
            }

            existing.epoch == proposed.epoch
                && existing.seed == proposed.seed
                && existing.epoch_length == proposed.epoch_length
                && existing.commit_deadline_offset == proposed.commit_deadline_offset
                && existing.reveal_deadline_offset == proposed.reveal_deadline_offset
                && existing.roster_len == proposed.roster_len
                && (!existing.finalized || proposed.finalized)
                && proposed.updated_at_height >= existing.updated_at_height
                && (!existing.penalties_applied || proposed.penalties_applied)
                && existing
                    .penalties_applied_at_height
                    .is_none_or(|height| proposed.penalties_applied_at_height == Some(height))
                && Self::vrf_participants_extend_existing(
                    &existing.participants,
                    &proposed.participants,
                )
                && Self::vrf_late_reveals_extend_existing(existing, proposed)
                && existing
                    .committed_no_reveal
                    .iter()
                    .all(|signer| proposed.committed_no_reveal.contains(signer))
                && existing
                    .no_participation
                    .iter()
                    .all(|signer| proposed.no_participation.contains(signer))
                && existing
                    .validator_election
                    .as_ref()
                    .is_none_or(|election| proposed.validator_election.as_ref() == Some(election))
        }

        fn vrf_participants_extend_existing(
            existing: &[VrfParticipantRecord],
            proposed: &[VrfParticipantRecord],
        ) -> bool {
            let proposed_by_signer: BTreeMap<_, _> = proposed
                .iter()
                .map(|participant| (participant.signer, participant))
                .collect();
            proposed_by_signer.len() == proposed.len()
                && existing.iter().all(|old| {
                    proposed_by_signer.get(&old.signer).is_some_and(|new| {
                        old.commitment
                            .is_none_or(|commitment| new.commitment == Some(commitment))
                            && old.reveal.is_none_or(|reveal| new.reveal == Some(reveal))
                            && new.last_updated_height >= old.last_updated_height
                    })
                })
        }

        fn vrf_late_reveals_extend_existing(
            existing: &VrfEpochRecord,
            proposed: &VrfEpochRecord,
        ) -> bool {
            let proposed_by_signer: BTreeMap<_, _> = proposed
                .late_reveals
                .iter()
                .map(|reveal| (reveal.signer, reveal))
                .collect();
            proposed_by_signer.len() == proposed.late_reveals.len()
                && existing
                    .late_reveals
                    .iter()
                    .all(|reveal| proposed_by_signer.get(&reveal.signer) == Some(&reveal))
        }

        fn validate_npos_effects_with_state(
            block: &SignedBlock,
            state: &State,
        ) -> Result<(), BlockValidationError> {
            Self::validate_npos_effects_header(block)?;

            let block_height = block.header().height().get();
            let actual_effects = block.npos_consensus_effects();
            if let Some(effects) = actual_effects {
                let mut sorted_actions = effects.penalty_actions.clone();
                sorted_actions.sort();
                sorted_actions.dedup();
                if sorted_actions != effects.penalty_actions {
                    return Err(Self::npos_effects_error(
                        "NPoS penalty actions are not canonical",
                    ));
                }

                let mut seen_epochs = BTreeSet::new();
                for record in &effects.vrf_epoch_seals {
                    if !seen_epochs.insert(record.epoch) {
                        return Err(Self::npos_effects_error(
                            "duplicate VRF epoch seal in NPoS effects",
                        ));
                    }
                    if record.penalties_applied_at_height.is_some() && !record.penalties_applied {
                        return Err(Self::npos_effects_error(
                            "VRF epoch seal has applied height without applied marker",
                        ));
                    }
                    if !record.finalized
                        && (!record.committed_no_reveal.is_empty()
                            || !record.no_participation.is_empty())
                    {
                        return Err(Self::npos_effects_error(
                            "unfinalized VRF epoch seal includes penalty offenders",
                        ));
                    }

                    let mut offenders = record
                        .committed_no_reveal
                        .iter()
                        .chain(record.no_participation.iter())
                        .copied()
                        .collect::<Vec<_>>();
                    offenders.sort();
                    offenders.dedup();
                    if offenders.len()
                        != record.committed_no_reveal.len() + record.no_participation.len()
                    {
                        return Err(Self::npos_effects_error(
                            "VRF epoch seal contains duplicate offender indices",
                        ));
                    }
                    if offenders.iter().any(|signer| *signer >= record.roster_len) {
                        return Err(Self::npos_effects_error(
                            "VRF epoch seal contains offender outside roster",
                        ));
                    }

                    let world = state.world_view();
                    if let Some(existing) = world.vrf_epochs().get(&record.epoch)
                        && !Self::vrf_epoch_record_extends_existing(&existing, record)
                    {
                        return Err(Self::npos_effects_error(
                            "VRF epoch seal conflicts with pre-block state",
                        ));
                    }
                }
            }

            let consensus_mode = {
                let world = state.world_view();
                crate::sumeragi::effective_consensus_mode_for_height_from_world(
                    &world,
                    block_height,
                    ConsensusMode::Permissioned,
                )
            };
            let fallback_npos = SumeragiNpos::default();
            let applier = crate::sumeragi::penalties::PenaltyApplier::from_parts(
                state,
                &fallback_npos,
                consensus_mode,
                #[cfg(feature = "telemetry")]
                Some(state.metrics()),
                #[cfg(not(feature = "telemetry"))]
                None,
            );
            let expected_actions = applier
                .derive_npos_consensus_effects(block_height, std::iter::empty())
                .map_err(|err| {
                    Self::npos_effects_error(format!("failed to derive NPoS effects: {err}"))
                })?
                .penalty_actions;
            let actual_actions = actual_effects
                .map(|effects| effects.penalty_actions.as_slice())
                .unwrap_or(&[]);
            if expected_actions.as_slice() != actual_actions {
                return Err(Self::npos_effects_error(
                    "NPoS penalty actions do not match pre-block state",
                ));
            }

            Ok(())
        }

        fn validate_previous_roster_evidence(
            block: &SignedBlock,
            block_height: u64,
            prev_block_hash: Option<HashOf<BlockHeader>>,
        ) -> Result<(), BlockValidationError> {
            let embedded = block.previous_roster_evidence();
            let header_hash = block.header().prev_roster_evidence_hash();

            match (header_hash, embedded) {
                (None, None) => {}
                (Some(_), None) => {
                    return Err(BlockValidationError::PreviousRosterEvidenceInvalid(
                        "header references previous-roster evidence but payload is missing"
                            .to_owned(),
                    ));
                }
                (None, Some(_)) => {
                    return Err(BlockValidationError::PreviousRosterEvidenceInvalid(
                        "payload includes previous-roster evidence but header hash is absent"
                            .to_owned(),
                    ));
                }
                (Some(hash), Some(evidence)) => {
                    if HashOf::new(evidence) != hash {
                        return Err(BlockValidationError::PreviousRosterEvidenceInvalid(
                            "previous-roster evidence hash mismatch".to_owned(),
                        ));
                    }
                }
            }

            if block_height > 2 && embedded.is_none() {
                return Err(BlockValidationError::PreviousRosterEvidenceInvalid(
                    "missing required previous-roster evidence for height > 2".to_owned(),
                ));
            }

            let Some(evidence) = embedded else {
                return Ok(());
            };

            let expected_prev_height = block_height.saturating_sub(1);
            if evidence.height != expected_prev_height {
                return Err(BlockValidationError::PreviousRosterEvidenceInvalid(
                    format!(
                        "previous-roster evidence height mismatch: expected {expected_prev_height}, got {}",
                        evidence.height
                    ),
                ));
            }
            if Some(evidence.block_hash) != prev_block_hash {
                return Err(BlockValidationError::PreviousRosterEvidenceInvalid(
                    "previous-roster evidence block hash does not match header parent hash"
                        .to_owned(),
                ));
            }

            let checkpoint = &evidence.validator_checkpoint;
            if checkpoint.height != evidence.height || checkpoint.block_hash != evidence.block_hash
            {
                return Err(BlockValidationError::PreviousRosterEvidenceInvalid(
                    "previous-roster evidence checkpoint metadata mismatch".to_owned(),
                ));
            }

            if checkpoint.validator_set_hash_version != VALIDATOR_SET_HASH_VERSION_V1 {
                return Err(BlockValidationError::PreviousRosterEvidenceInvalid(
                    format!(
                        "unsupported validator-set hash version in previous-roster evidence: {}",
                        checkpoint.validator_set_hash_version
                    ),
                ));
            }
            if checkpoint.validator_set_hash != HashOf::new(&checkpoint.validator_set) {
                return Err(BlockValidationError::PreviousRosterEvidenceInvalid(
                    "previous-roster evidence checkpoint validator-set hash mismatch".to_owned(),
                ));
            }
            if let Some(stake_snapshot) = evidence.stake_snapshot.as_ref()
                && !stake_snapshot.matches_roster(&checkpoint.validator_set)
            {
                return Err(BlockValidationError::PreviousRosterEvidenceInvalid(
                    "previous-roster evidence stake snapshot does not match validator set"
                        .to_owned(),
                ));
            }

            Ok(())
        }

        fn execution_context_error(message: impl Into<String>) -> BlockValidationError {
            BlockValidationError::ExecutionContextInvalid(message.into())
        }

        fn validate_execution_context_header(
            block: &SignedBlock,
        ) -> Result<Option<&BlockExecutionContextBundle>, BlockValidationError> {
            match (
                block.header().execution_context_hash(),
                block.execution_context(),
            ) {
                (None, None) => Ok(None),
                (Some(_), None) => Err(Self::execution_context_error(
                    "header references execution context but payload is missing",
                )),
                (None, Some(_)) => Err(Self::execution_context_error(
                    "payload includes execution context but header hash is absent",
                )),
                (Some(expected), Some(bundle)) => {
                    let actual = HashOf::new(bundle);
                    if actual != expected {
                        return Err(Self::execution_context_error(
                            "execution context hash mismatch",
                        ));
                    }
                    Ok(Some(bundle))
                }
            }
        }

        fn validate_execution_context_alignment(
            block: &SignedBlock,
            bundle: &BlockExecutionContextBundle,
        ) -> Result<(), BlockValidationError> {
            let expected_len = block.external_entrypoint_count();
            if bundle.external.len() != expected_len {
                return Err(Self::execution_context_error(format!(
                    "execution context length mismatch: expected {expected_len}, got {}",
                    bundle.external.len()
                )));
            }

            for (idx, (entrypoint, context)) in block
                .external_entrypoints_cloned()
                .zip(bundle.external.iter())
                .enumerate()
            {
                let expected = entrypoint.hash();
                if context.entrypoint_hash != expected {
                    return Err(Self::execution_context_error(format!(
                        "execution context entrypoint hash mismatch at index {idx}"
                    )));
                }
            }

            Ok(())
        }

        fn validate_execution_context_with_state(
            block: &SignedBlock,
            state: &impl StateReadOnly,
            allow_missing_legacy_context: bool,
        ) -> Result<(), BlockValidationError> {
            let bundle = Self::validate_execution_context_header(block)?;
            let context_required = !allow_missing_legacy_context
                && !block.header().is_genesis()
                && block.external_entrypoint_count() != 0;
            let Some(bundle) = bundle else {
                return if context_required {
                    Err(Self::execution_context_error(
                        "missing execution context for external entrypoints",
                    ))
                } else {
                    Ok(())
                };
            };

            Self::validate_execution_context_alignment(block, bundle)?;
            if allow_missing_legacy_context || block.header().is_genesis() {
                return Ok(());
            }

            let nexus = state.nexus();
            for (idx, (entrypoint, context)) in block
                .external_entrypoints_cloned()
                .zip(bundle.external.iter())
                .enumerate()
            {
                let committed_decision =
                    crate::queue::RoutingDecision::new(context.lane_id, context.dataspace_id);
                resolve_routing_decision(
                    committed_decision,
                    &nexus.lane_catalog,
                    &nexus.dataspace_catalog,
                )
                .map_err(|err| {
                    Self::execution_context_error(format!(
                        "execution context route cannot be resolved at index {idx}: {err}"
                    ))
                })?;

                let accepted = crate::tx::AcceptedTransaction::new_unchecked_entrypoint(
                    Cow::Owned(entrypoint),
                );
                let decision = evaluate_policy_with_catalog_and_world(
                    &nexus.routing_policy,
                    &nexus.lane_catalog,
                    &nexus.dataspace_catalog,
                    &accepted,
                    state.world(),
                )
                .map_err(|err| {
                    Self::execution_context_error(format!(
                        "execution context routing cannot be resolved at index {idx}: {err}"
                    ))
                })?;
                let derived_decision =
                    crate::queue::RoutingDecision::new(decision.lane_id, decision.dataspace_id);
                if derived_decision != committed_decision {
                    return Err(Self::execution_context_error(format!(
                        "execution context routing mismatch at index {idx}: expected lane {} dataspace {}, got lane {} dataspace {}",
                        decision.lane_id.as_u32(),
                        decision.dataspace_id.as_u64(),
                        context.lane_id.as_u32(),
                        context.dataspace_id.as_u64(),
                    )));
                }
            }

            Ok(())
        }

        fn embedded_routing_decisions_for_signed_transactions(
            block: &SignedBlock,
            tx_count: usize,
        ) -> Option<Vec<crate::queue::RoutingDecision>> {
            let bundle = match Self::validate_execution_context_header(block) {
                Ok(Some(bundle)) => bundle,
                Ok(None) => return None,
                Err(error) => {
                    warn!(%error, "ignoring invalid embedded execution context during unchecked execution");
                    return None;
                }
            };
            if let Err(error) = Self::validate_execution_context_alignment(block, bundle) {
                warn!(%error, "ignoring misaligned embedded execution context during unchecked execution");
                return None;
            }

            let decisions = block
                .external_entrypoints_cloned()
                .zip(bundle.external.iter())
                .filter_map(|(entrypoint, context)| {
                    matches!(entrypoint, TransactionEntrypoint::External(_)).then(|| {
                        crate::queue::RoutingDecision::new(context.lane_id, context.dataspace_id)
                    })
                })
                .collect::<Vec<_>>();
            (decisions.len() == tx_count).then_some(decisions)
        }

        fn embedded_routing_decisions_for_entrypoints(
            block: &SignedBlock,
            entrypoint_count: usize,
        ) -> Option<Vec<crate::queue::RoutingDecision>> {
            let bundle = match Self::validate_execution_context_header(block) {
                Ok(Some(bundle)) => bundle,
                Ok(None) => return None,
                Err(error) => {
                    warn!(%error, "ignoring invalid embedded execution context during unchecked execution");
                    return None;
                }
            };
            if let Err(error) = Self::validate_execution_context_alignment(block, bundle) {
                warn!(%error, "ignoring misaligned embedded execution context during unchecked execution");
                return None;
            }

            let decisions = bundle
                .external
                .iter()
                .map(|context| {
                    crate::queue::RoutingDecision::new(context.lane_id, context.dataspace_id)
                })
                .collect::<Vec<_>>();
            (decisions.len() == entrypoint_count).then_some(decisions)
        }

        fn committed_heights_for_prepared_transactions(
            prepared_txs: &[PreparedBlockTransaction],
            transactions: &impl TransactionsReadOnly,
        ) -> Vec<Option<NonZeroUsize>> {
            prepared_txs
                .iter()
                .map(|prepared| transactions.get(&prepared.metadata.signed_hash))
                .collect()
        }

        fn signed_transaction_from_entrypoint(
            entrypoint: &TransactionEntrypoint,
        ) -> Option<&SignedTransaction> {
            match entrypoint {
                TransactionEntrypoint::External(tx) => Some(tx),
                TransactionEntrypoint::SealedReveal(reveal) => Some(reveal.signed_transaction()),
                TransactionEntrypoint::SealedCommitment(_)
                | TransactionEntrypoint::PrivateKaigi(_)
                | TransactionEntrypoint::Time(_) => None,
            }
        }

        fn collect_external_signed_transactions(block: &SignedBlock) -> Vec<&SignedTransaction> {
            if let Some(entries) = block.external_entrypoints_slice() {
                entries
                    .iter()
                    .filter_map(Self::signed_transaction_from_entrypoint)
                    .collect()
            } else {
                block.transactions_vec().iter().collect()
            }
        }

        fn prepare_external_transactions(block: &SignedBlock) -> Vec<PreparedBlockTransaction> {
            Self::collect_external_signed_transactions(block)
                .into_iter()
                .map(|tx| PreparedBlockTransaction {
                    metadata: crate::tx::AcceptedTransaction::prepare_signed_metadata(tx),
                })
                .collect()
        }

        #[cfg(feature = "bls")]
        #[allow(clippy::too_many_lines)]
        fn precheck_bls_transaction_signatures(
            signed_txs: &[&SignedTransaction],
            prepared_txs: &[PreparedBlockTransaction],
            cap: usize,
            prechecked_signature_results: &mut [Option<Result<(), SignatureVerificationFail>>],
            metrics: MetricsRef<'_>,
            lane_id: LaneId,
        ) {
            #[derive(Clone)]
            struct BlsItem {
                idx: usize,
                pk: iroha_crypto::PublicKey,
                pk_bytes: Vec<u8>,
                pop: Option<Vec<u8>>,
                msg: [u8; 32],
                sig: Vec<u8>,
            }

            static BLS_POP_KEY: LazyLock<iroha_data_model::name::Name> =
                LazyLock::new(|| "bls_pop".parse().expect("valid metadata key"));
            static BLS_POP_SMALL_KEY: LazyLock<iroha_data_model::name::Name> =
                LazyLock::new(|| "bls_pop_small".parse().expect("valid metadata key"));

            let mut all_normal_have_pop = true;
            let mut all_small_have_pop = true;
            let mut items_normal: Vec<BlsItem> = Vec::new();
            let mut items_small: Vec<BlsItem> = Vec::new();
            for (idx, (tx, prepared)) in signed_txs.iter().zip(prepared_txs.iter()).enumerate() {
                let AccountController::Single(signatory) = tx.authority().controller() else {
                    continue;
                };
                let small = match signatory.algorithm() {
                    iroha_crypto::Algorithm::BlsNormal => false,
                    iroha_crypto::Algorithm::BlsSmall => true,
                    _ => continue,
                };
                let h = prepared.metadata.payload_hash;
                let mut msg = [0_u8; 32];
                msg.copy_from_slice(h.as_ref());
                let sig = tx.signature().payload().payload().to_vec();
                let mut pop = None;
                if small {
                    if let Some(pop_bytes) =
                        bls_small_pop_from_metadata(tx.metadata(), &BLS_POP_SMALL_KEY)
                    {
                        if iroha_crypto::bls_small_pop_verify(signatory, &pop_bytes).is_ok() {
                            pop = Some(pop_bytes);
                        } else {
                            all_small_have_pop = false;
                        }
                    } else {
                        all_small_have_pop = false;
                    }
                } else if let Some(pop_bytes) = bls_pop_from_metadata(tx.metadata(), &BLS_POP_KEY) {
                    if iroha_crypto::bls_normal_pop_verify(signatory, &pop_bytes).is_ok() {
                        pop = Some(pop_bytes);
                    } else {
                        all_normal_have_pop = false;
                    }
                } else {
                    all_normal_have_pop = false;
                }
                let item = BlsItem {
                    idx,
                    pk: signatory.clone(),
                    pk_bytes: signatory.to_bytes().1.to_vec(),
                    pop,
                    msg,
                    sig,
                };
                if small {
                    items_small.push(item);
                } else {
                    items_normal.push(item);
                }
            }

            #[cfg(feature = "telemetry")]
            let mut same_msg_agg = 0_u64;
            #[cfg(feature = "telemetry")]
            let mut multi_msg_agg = 0_u64;
            #[cfg(feature = "telemetry")]
            let mut deterministic = 0_u64;

            #[cfg(feature = "telemetry")]
            let record_result = |same_message: bool, success: bool| {
                if let Some(metrics) = metrics {
                    metrics.inc_pipeline_sig_bls_result(lane_id, same_message, success);
                }
            };

            let mut verify_set = |items: &[BlsItem], small: bool| {
                if items.is_empty() {
                    return;
                }
                let mut groups: BTreeMap<[u8; 32], Vec<&BlsItem>> = BTreeMap::new();
                for item in items {
                    groups.entry(item.msg).or_default().push(item);
                }
                let mut singletons: Vec<&BlsItem> = Vec::new();
                for group in groups.values() {
                    if group.len() == 1 {
                        singletons.push(group[0]);
                        continue;
                    }
                    let Some(pops) = group
                        .iter()
                        .map(|item| item.pop.as_ref().map(Vec::as_slice))
                        .collect::<Option<Vec<_>>>()
                    else {
                        return;
                    };
                    let msg = group[0].msg.as_slice();
                    let sigs: Vec<&[u8]> = group.iter().map(|item| item.sig.as_slice()).collect();
                    let pks: Vec<&iroha_crypto::PublicKey> =
                        group.iter().map(|item| &item.pk).collect();
                    let ok = if small {
                        iroha_crypto::bls_small_verify_aggregate_same_message(
                            msg, &sigs, &pks, &pops,
                        )
                        .is_ok()
                    } else {
                        iroha_crypto::bls_normal_verify_aggregate_same_message(
                            msg, &sigs, &pks, &pops,
                        )
                        .is_ok()
                    };
                    #[cfg(feature = "telemetry")]
                    {
                        same_msg_agg = same_msg_agg.saturating_add(1);
                        record_result(true, ok);
                    }
                    if ok {
                        for item in group {
                            prechecked_signature_results[item.idx] = Some(Ok(()));
                        }
                    } else {
                        for item in group {
                            prechecked_signature_results[item.idx] = Some(
                                crate::tx::AcceptedTransaction::signature_verification_result(
                                    signed_txs[item.idx],
                                ),
                            );
                        }
                    }
                }
                if !singletons.is_empty() {
                    let msgs: Vec<&[u8]> =
                        singletons.iter().map(|item| item.msg.as_slice()).collect();
                    let sigs: Vec<&[u8]> =
                        singletons.iter().map(|item| item.sig.as_slice()).collect();
                    let pks: Vec<&[u8]> = singletons
                        .iter()
                        .map(|item| item.pk_bytes.as_slice())
                        .collect();
                    let ok = if small {
                        iroha_crypto::bls_small_verify_aggregate_multi_message(&msgs, &sigs, &pks)
                            .is_ok()
                    } else {
                        iroha_crypto::bls_normal_verify_aggregate_multi_message(&msgs, &sigs, &pks)
                            .is_ok()
                    };
                    #[cfg(feature = "telemetry")]
                    {
                        multi_msg_agg = multi_msg_agg.saturating_add(1);
                        record_result(false, ok);
                    }
                    if ok {
                        for item in singletons {
                            prechecked_signature_results[item.idx] = Some(Ok(()));
                        }
                    } else {
                        for item in singletons {
                            prechecked_signature_results[item.idx] = Some(
                                crate::tx::AcceptedTransaction::signature_verification_result(
                                    signed_txs[item.idx],
                                ),
                            );
                        }
                    }
                }
            };

            if cap > 0 {
                if all_normal_have_pop {
                    for chunk in items_normal.chunks(cap) {
                        verify_set(chunk, false);
                    }
                } else {
                    #[cfg(feature = "telemetry")]
                    {
                        deterministic = deterministic
                            .saturating_add(u64::try_from(items_normal.len()).unwrap_or(u64::MAX));
                    }
                }
                if all_small_have_pop {
                    for chunk in items_small.chunks(cap) {
                        verify_set(chunk, true);
                    }
                } else {
                    #[cfg(feature = "telemetry")]
                    {
                        deterministic = deterministic
                            .saturating_add(u64::try_from(items_small.len()).unwrap_or(u64::MAX));
                    }
                }
            } else {
                #[cfg(feature = "telemetry")]
                {
                    let item_count = items_normal.len().saturating_add(items_small.len());
                    deterministic =
                        deterministic.saturating_add(u64::try_from(item_count).unwrap_or(u64::MAX));
                }
            }

            #[cfg(feature = "telemetry")]
            if let Some(metrics) = metrics {
                metrics.set_pipeline_sig_bls_counts(
                    lane_id,
                    same_msg_agg,
                    multi_msg_agg,
                    deterministic,
                );
            }
            #[cfg(not(feature = "telemetry"))]
            let _ = (metrics, lane_id);
        }

        /// Static checks that do not require holding a state view.
        #[allow(
            clippy::too_many_arguments,
            clippy::too_many_lines,
            clippy::explicit_iter_loop,
            clippy::collapsible_else_if,
            clippy::items_after_statements,
            clippy::option_if_let_else,
            clippy::manual_flatten
        )]
        fn validate_static_with_snapshot(
            block: &SignedBlock,
            chain_id: &ChainId,
            genesis_account: &AccountId,
            static_data: &StaticValidationData,
            committed_heights: &[Option<NonZeroUsize>],
            prepared_txs: &[PreparedBlockTransaction],
            cached_stateless_ok: Option<&[bool]>,
            trust_replay_tx_signatures: bool,
            _metrics: MetricsRef<'_>,
        ) -> Result<(), BlockValidationError> {
            let _ = static_data.aggregate_lane;

            let max_clock_drift = static_data.max_clock_drift;
            let tx_params = static_data.tx_params;
            let expected_block_height = static_data.expected_block_height;
            let pipeline_cfg = &static_data.pipeline_cfg;
            let crypto_cfg = &static_data.crypto_cfg;
            let block_creation_time = block.header().creation_time();
            debug_assert_eq!(
                committed_heights.len(),
                prepared_txs.len(),
                "committed-height snapshot must align with block transaction list",
            );
            if committed_heights.len() != prepared_txs.len() {
                return Err(BlockValidationError::MerkleRootMismatch);
            }
            let signed_txs = Self::collect_external_signed_transactions(block);
            debug_assert_eq!(
                signed_txs.len(),
                prepared_txs.len(),
                "prepared metadata must align with signed block transactions",
            );
            if signed_txs.len() != prepared_txs.len() {
                return Err(BlockValidationError::MerkleRootMismatch);
            }
            let is_genesis_block = block.header().is_genesis();
            let mut prechecked_signature_results: Vec<
                Option<Result<(), SignatureVerificationFail>>,
            > = vec![None; prepared_txs.len()];
            #[cfg(feature = "bls")]
            Self::precheck_bls_transaction_signatures(
                &signed_txs,
                prepared_txs,
                pipeline_cfg.signature_batch_max_bls,
                &mut prechecked_signature_results,
                _metrics,
                static_data.aggregate_lane,
            );
            let mut seen_hashes: HashSet<HashOf<SignedTransaction>> =
                HashSet::with_capacity(signed_txs.len());
            let mut seen_sealed_commitments =
                HashSet::with_capacity(block.external_entrypoint_count());

            for ((tx, prepared), committed_height) in signed_txs
                .iter()
                .copied()
                .zip(prepared_txs.iter())
                .zip(committed_heights.iter())
            {
                let tx_hash = prepared.metadata.signed_hash;
                // In case of soft-fork transaction is check if it was added at the same height as candidate block.
                if committed_height
                    .as_ref()
                    .is_some_and(|height| height.get() < expected_block_height)
                {
                    return Err(BlockValidationError::HasCommittedTransactions);
                }

                if !seen_hashes.insert(tx_hash) {
                    iroha_logger::error!(
                        %tx_hash,
                        height = %block.header().height(),
                        "duplicate transaction detected during block validation"
                    );
                    return Err(BlockValidationError::DuplicateTransactions);
                }

                if tx.creation_time() >= block_creation_time {
                    return Err(BlockValidationError::TransactionInTheFuture);
                }
            }
            let mut entrypoint_hashes = Vec::with_capacity(block.external_entrypoint_count());
            let mut prepared_signed_idx = 0usize;
            if let Some(external_entrypoints) = block.external_entrypoints_slice() {
                for entrypoint in external_entrypoints {
                    match entrypoint {
                        TransactionEntrypoint::External(_) => {
                            let prepared = prepared_txs
                                .get(prepared_signed_idx)
                                .ok_or(BlockValidationError::MerkleRootMismatch)?;
                            entrypoint_hashes.push(prepared.metadata.entrypoint_hash);
                            prepared_signed_idx = prepared_signed_idx.saturating_add(1);
                        }
                        TransactionEntrypoint::SealedReveal(_) => {
                            let _prepared = prepared_txs
                                .get(prepared_signed_idx)
                                .ok_or(BlockValidationError::MerkleRootMismatch)?;
                            entrypoint_hashes.push(entrypoint.hash());
                            prepared_signed_idx = prepared_signed_idx.saturating_add(1);
                        }
                        TransactionEntrypoint::SealedCommitment(commitment) => {
                            crate::tx::validate_sealed_commitment_stateless(
                                commitment, chain_id, tx_params,
                            )
                            .map_err(BlockValidationError::TransactionAccept)?;
                            if commitment.payload().reveal_after_height
                                <= u64::try_from(expected_block_height).unwrap_or(u64::MAX)
                            {
                                return Err(BlockValidationError::TransactionAccept(
                                    AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                                        reason: "sealed transaction reveal_after_height must be greater than commit height".into(),
                                    }),
                                ));
                            }
                            if !seen_sealed_commitments.insert(*commitment.commitment()) {
                                return Err(BlockValidationError::DuplicateTransactions);
                            }
                            entrypoint_hashes.push(entrypoint.hash());
                        }
                        TransactionEntrypoint::PrivateKaigi(_) => {
                            entrypoint_hashes.push(entrypoint.hash());
                        }
                        TransactionEntrypoint::Time(_) => {
                            return Err(BlockValidationError::TransactionAccept(
                                AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                                    reason: "time entrypoints cannot be embedded as external block entrypoints".into(),
                                }),
                            ));
                        }
                    }
                }
                debug_assert_eq!(
                    prepared_signed_idx,
                    prepared_txs.len(),
                    "signed entrypoint preparation must align with external entries",
                );
            } else {
                entrypoint_hashes.extend(
                    prepared_txs
                        .iter()
                        .map(|prepared| prepared.metadata.entrypoint_hash),
                );
            }

            use rayon::prelude::*;

            let mut ed25519_prechecked = vec![false; prepared_txs.len()];
            if let Some(cached_stateless_ok) = cached_stateless_ok {
                if cached_stateless_ok.len() != prepared_txs.len() {
                    return Err(BlockValidationError::MerkleRootMismatch);
                }
            }
            let ed25519_batch_cap = pipeline_cfg.signature_batch_max_ed25519;
            if !is_genesis_block && ed25519_batch_cap > 0 {
                struct Ed25519BatchItem {
                    idx: usize,
                }

                fn verify_ed25519_batch_slices<'a>(
                    messages: &[&'a [u8]],
                    signatures: &[&'a [u8]],
                    public_keys: &[iroha_crypto::Ed25519ParsedPublicKey],
                    scratch: &mut iroha_crypto::Ed25519BatchScratch<'a>,
                ) -> Result<(), iroha_crypto::Error> {
                    iroha_crypto::ed25519_verify_batch_preparsed_deterministic_with_scratch(
                        messages,
                        signatures,
                        public_keys,
                        [0; 32],
                        scratch,
                    )
                }

                let mut items = Vec::with_capacity(prepared_txs.len());
                let mut messages = Vec::with_capacity(prepared_txs.len());
                let mut signatures = Vec::with_capacity(prepared_txs.len());
                let mut public_keys = Vec::with_capacity(prepared_txs.len());
                let mut scratch = iroha_crypto::Ed25519BatchScratch::default();
                for (idx, (tx, prepared)) in signed_txs
                    .iter()
                    .copied()
                    .zip(prepared_txs.iter())
                    .enumerate()
                {
                    let signature = tx.signature().payload().payload();
                    if ed25519_prechecked[idx] {
                        continue;
                    }
                    if signature.len() != crate::tx::ED25519_SIGNATURE_LENGTH {
                        continue;
                    }
                    let Some(public_key) = prepared.metadata.single_ed25519_key else {
                        continue;
                    };
                    items.push(Ed25519BatchItem { idx });
                    messages.push(prepared.metadata.payload_hash.as_ref().as_slice());
                    signatures.push(signature);
                    public_keys.push(public_key);
                }

                let signature_error = |tx: &SignedTransaction, detail: String| {
                    BlockValidationError::TransactionAccept(
                        AcceptTransactionFail::SignatureVerification(
                            SignatureVerificationFail::new(
                                tx.signature().clone(),
                                SignatureRejectionCode::InvalidSignature,
                                detail,
                            ),
                        ),
                    )
                };

                for range_start in (0..items.len()).step_by(ed25519_batch_cap) {
                    let range_end = range_start
                        .saturating_add(ed25519_batch_cap)
                        .min(items.len());
                    let messages = &messages[range_start..range_end];
                    let signatures = &signatures[range_start..range_end];
                    let public_keys = &public_keys[range_start..range_end];
                    if let Err(err) =
                        verify_ed25519_batch_slices(messages, signatures, public_keys, &mut scratch)
                    {
                        if let Some((relative_idx, detail)) =
                            iroha_crypto::ed25519_first_bad_preparsed_deterministic_with_scratch(
                                messages,
                                signatures,
                                public_keys,
                                [0; 32],
                                &mut scratch,
                            )
                        {
                            let idx = items[range_start + relative_idx].idx;
                            return Err(signature_error(signed_txs[idx], detail));
                        }
                        let idx = items.get(range_start).map_or(0, |item| item.idx);
                        return Err(signature_error(signed_txs[idx], err.to_string()));
                    }
                    for item in &items[range_start..range_end] {
                        ed25519_prechecked[item.idx] = true;
                    }
                }
            }
            let validate_tx = |(idx, (tx, prepared)): (
                usize,
                (&SignedTransaction, &PreparedBlockTransaction),
            )|
             -> Option<BlockValidationError> {
                let prechecked_signature_result = prechecked_signature_results
                    .get(idx)
                    .and_then(|result| result.as_ref().cloned());
                if is_genesis_block {
                    if let Some(Err(fail)) = prechecked_signature_result {
                        return Some(BlockValidationError::TransactionAccept(
                            AcceptTransactionFail::SignatureVerification(fail),
                        ));
                    }
                    return AcceptedTransaction::validate_genesis_with_now(
                        tx,
                        chain_id,
                        max_clock_drift,
                        genesis_account,
                        crypto_cfg.as_ref(),
                        block_creation_time,
                    )
                    .err()
                    .map(BlockValidationError::TransactionAccept);
                }

                let replay_signature_result = trust_replay_tx_signatures.then_some(Ok(()));
                let stateless = if let Some(prechecked_signature_result) = replay_signature_result {
                    if crate::tx::is_heartbeat_transaction(tx) {
                        AcceptedTransaction::validate_heartbeat_with_now_with_signature_result_and_prepared_metadata(
                            tx,
                            chain_id,
                            max_clock_drift,
                            tx_params,
                            crypto_cfg.as_ref(),
                            block_creation_time,
                            Some(prechecked_signature_result),
                            &prepared.metadata,
                        )
                    } else {
                        AcceptedTransaction::validate_with_now_with_signature_result_and_prepared_metadata(
                            tx,
                            chain_id,
                            max_clock_drift,
                            tx_params,
                            crypto_cfg.as_ref(),
                            block_creation_time,
                            Some(prechecked_signature_result),
                            &prepared.metadata,
                        )
                    }
                } else if let Some(prechecked_signature_result) = prechecked_signature_result {
                    if crate::tx::is_heartbeat_transaction(tx) {
                        AcceptedTransaction::validate_heartbeat_with_now_with_signature_result_and_prepared_metadata(
                            tx,
                            chain_id,
                            max_clock_drift,
                            tx_params,
                            crypto_cfg.as_ref(),
                            block_creation_time,
                            Some(prechecked_signature_result),
                            &prepared.metadata,
                        )
                    } else {
                        AcceptedTransaction::validate_with_now_with_signature_result_and_prepared_metadata(
                            tx,
                            chain_id,
                            max_clock_drift,
                            tx_params,
                            crypto_cfg.as_ref(),
                            block_creation_time,
                            Some(prechecked_signature_result),
                            &prepared.metadata,
                        )
                    }
                } else if crate::tx::is_heartbeat_transaction(tx) {
                    if ed25519_prechecked[idx] {
                        AcceptedTransaction::validate_heartbeat_with_now_after_single_ed25519_precheck_and_prepared_metadata(
                                tx,
                                chain_id,
                                max_clock_drift,
                                tx_params,
                                crypto_cfg.as_ref(),
                                block_creation_time,
                                &prepared.metadata,
                            )
                    } else {
                        AcceptedTransaction::validate_heartbeat_with_now_and_prepared_metadata(
                            tx,
                            chain_id,
                            max_clock_drift,
                            tx_params,
                            crypto_cfg.as_ref(),
                            block_creation_time,
                            &prepared.metadata,
                        )
                    }
                } else if ed25519_prechecked[idx] {
                    AcceptedTransaction::validate_with_now_after_single_ed25519_precheck_and_prepared_metadata(
                            tx,
                            chain_id,
                            max_clock_drift,
                            tx_params,
                            crypto_cfg.as_ref(),
                            block_creation_time,
                            &prepared.metadata,
                        )
                } else {
                    AcceptedTransaction::validate_with_now_and_prepared_metadata(
                        tx,
                        chain_id,
                        max_clock_drift,
                        tx_params,
                        crypto_cfg.as_ref(),
                        block_creation_time,
                        &prepared.metadata,
                    )
                };
                stateless.err().map(BlockValidationError::TransactionAccept)
            };

            let static_pool = static_data.pipeline_parallelism.pool();
            let use_parallel = static_pool.is_some() && prepared_txs.len() > 1;
            let tx_errors: Vec<Option<BlockValidationError>> = if use_parallel {
                static_pool
                    .as_ref()
                    .expect("parallel validation requires a configured pipeline pool")
                    .install(|| {
                        signed_txs
                            .par_iter()
                            .copied()
                            .zip(prepared_txs.par_iter())
                            .enumerate()
                            .map(validate_tx)
                            .collect()
                    })
            } else {
                signed_txs
                    .iter()
                    .copied()
                    .zip(prepared_txs.iter())
                    .enumerate()
                    .map(validate_tx)
                    .collect()
            };
            for maybe_err in tx_errors {
                if let Some(err) = maybe_err {
                    return Err(err);
                }
            }

            let expected_merkle_root = if let Some(pool) = static_pool.as_ref() {
                pool.install(|| {
                    let merkle_tree: MerkleTree<TransactionEntrypoint> =
                        MerkleTree::from_typed_leaves_parallel(entrypoint_hashes);
                    merkle_tree.root()
                })
            } else {
                let merkle_tree: MerkleTree<TransactionEntrypoint> =
                    entrypoint_hashes.into_iter().collect();
                merkle_tree.root()
            };
            let actual_merkle_root = block.header().merkle_root();

            if expected_merkle_root != actual_merkle_root {
                return Err(BlockValidationError::MerkleRootMismatch);
            }

            Ok(())
        }

        /// All static checks of the block.
        #[allow(
            clippy::too_many_arguments,
            clippy::too_many_lines,
            clippy::explicit_iter_loop,
            clippy::collapsible_else_if
        )]
        fn validate_static(
            block: &SignedBlock,
            topology: &Topology,
            chain_id: &ChainId,
            genesis_account: &AccountId,
            state: &StateBlock<'_>,
            soft_fork: bool,
            time_source: &TimeSource,
        ) -> Result<(), BlockValidationError> {
            let static_data = Self::validate_static_state_dependent(
                block,
                topology,
                chain_id,
                genesis_account,
                state,
                soft_fork,
                time_source,
                false,
                false,
            )?;
            let prepared_txs = Self::prepare_external_transactions(block);
            let committed_heights = Self::committed_heights_for_prepared_transactions(
                &prepared_txs,
                state.transactions(),
            );
            let cache_cap = static_data.pipeline_cfg.stateless_cache_cap;
            let cache_enabled = cache_cap > 0 && !block.header().is_genesis();
            let max_clock_drift_ms = static_data.max_clock_drift.as_millis();
            let cache_context = if cache_enabled {
                Some(StatelessValidationContext::new(
                    chain_id.clone(),
                    u64::try_from(max_clock_drift_ms).unwrap_or(u64::MAX),
                    static_data.tx_params,
                    static_data.crypto_cfg.allowed_signing.clone(),
                ))
            } else {
                None
            };
            #[cfg(feature = "telemetry")]
            let metrics = Some(state.metrics());
            #[cfg(not(feature = "telemetry"))]
            let metrics = ();
            let cache_now_ms = block.header().creation_time().as_millis();
            let cached_stateless_ok = cache_context.as_ref().map(|context| {
                let mut cache = state.stateless_validation_cache().lock();
                cache.set_cap(cache_cap);
                cache.ensure_context(context.clone());
                prepared_txs
                    .iter()
                    .map(|prepared| cache.get_ok(&prepared.metadata.signed_hash, cache_now_ms))
                    .collect::<Vec<_>>()
            });
            Self::validate_static_with_snapshot(
                block,
                chain_id,
                genesis_account,
                &static_data,
                &committed_heights,
                &prepared_txs,
                cached_stateless_ok.as_deref(),
                false,
                metrics,
            )?;
            if let Some(context) = cache_context {
                let mut cache = state.stateless_validation_cache().lock();
                cache.set_cap(cache_cap);
                cache.ensure_context(context);
                for (tx, prepared) in Self::collect_external_signed_transactions(block)
                    .into_iter()
                    .zip(prepared_txs.iter())
                {
                    let expires_at_ms = tx
                        .time_to_live()
                        .and_then(|ttl| tx.creation_time().checked_add(ttl))
                        .map(|expires_at| expires_at.as_millis());
                    let not_before_ms = tx
                        .creation_time()
                        .as_millis()
                        .saturating_sub(max_clock_drift_ms);
                    cache.insert_ok(prepared.metadata.signed_hash, expires_at_ms, not_before_ms);
                }
            }
            Ok(())
        }

        fn validate_and_record_entrypoints_sequential(
            block: &mut SignedBlock,
            state_block: &mut StateBlock<'_>,
            mut timings: Option<&mut ValidationTimings>,
            entrypoints: Vec<TransactionEntrypoint>,
        ) -> Result<(), BlockValidationError> {
            let to_ms = |duration: Duration| -> u64 {
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
            };
            let start = timings.as_ref().map(|_| Instant::now());
            let n = entrypoints.len();
            #[allow(clippy::disallowed_types)]
            let tx_hashes: std::collections::HashSet<_> = entrypoints
                .iter()
                .map(|entrypoint| {
                    HashOf::<SignedTransaction>::from_untyped_unchecked(iroha_crypto::Hash::from(
                        entrypoint.hash(),
                    ))
                })
                .collect();
            let height_u64 = block.header().height().get();
            let height_usize = height_u64.try_into().expect("block height fits usize");
            let block_height =
                std::num::NonZeroUsize::new(height_usize).expect("block height greater than zero");
            state_block
                .transactions
                .insert_block(tx_hashes, block_height);

            let embedded_routing = Self::embedded_routing_decisions_for_entrypoints(block, n);
            let (routing_decisions, routing_errors) = if let Some(decisions) = embedded_routing {
                (decisions, vec![None; n])
            } else {
                let routing_policy = &state_block.nexus.routing_policy;
                let lane_catalog = &state_block.nexus.lane_catalog;
                let dataspace_catalog = &state_block.nexus.dataspace_catalog;
                let mut decisions = Vec::with_capacity(n);
                let mut errors = Vec::with_capacity(n);
                for entrypoint in &entrypoints {
                    let accepted = crate::tx::AcceptedTransaction::new_unchecked_entrypoint(
                        Cow::Borrowed(entrypoint),
                    );
                    match evaluate_policy_with_catalog_and_world(
                        routing_policy,
                        lane_catalog,
                        dataspace_catalog,
                        &accepted,
                        &state_block.world,
                    ) {
                        Ok(decision) => {
                            decisions.push(decision);
                            errors.push(None);
                        }
                        Err(err) => {
                            decisions.push(crate::queue::RoutingDecision::default());
                            errors.push(Some(err));
                        }
                    }
                }
                (decisions, errors)
            };

            let transaction_event_hashes: Vec<_> = entrypoints
                .iter()
                .map(Self::signed_transaction_from_entrypoint)
                .map(|tx| tx.map(SignedTransaction::hash))
                .collect();
            let mut execution_order: Vec<usize> = (0..n).collect();
            let reveal_positions: Vec<usize> = execution_order
                .iter()
                .copied()
                .filter(|idx| matches!(entrypoints[*idx], TransactionEntrypoint::SealedReveal(_)))
                .collect();
            if reveal_positions.len() > 1 {
                let mut sorted_reveals = reveal_positions.clone();
                sorted_reveals.sort_by_key(|idx| match &entrypoints[*idx] {
                    TransactionEntrypoint::SealedReveal(reveal) => {
                        crate::tx::sealed_reveal_execution_key(state_block, reveal)
                    }
                    _ => unreachable!("filtered to sealed reveals"),
                });
                for (slot, sorted_idx) in reveal_positions.into_iter().zip(sorted_reveals) {
                    execution_order[slot] = sorted_idx;
                }
            }

            let mut ivm_cache = IvmCache::new();
            let mut hashes: Vec<Option<HashOf<TransactionEntrypoint>>> = vec![None; n];
            let mut results: Vec<Option<TransactionResultInner>> = vec![None; n];
            for idx in execution_order {
                let entrypoint = entrypoints[idx].clone();
                let entrypoint_hash = entrypoint.hash();
                let accepted = crate::tx::AcceptedTransaction::new_unchecked_entrypoint(
                    Cow::Owned(entrypoint),
                );
                let (hash, result) = if let Some(err) = routing_errors[idx].as_ref() {
                    (
                        entrypoint_hash,
                        Err(TransactionRejectionReason::Validation(
                            iroha_data_model::ValidationFail::NotPermitted(format!(
                                "transaction routing could not be resolved: {err}"
                            )),
                        )),
                    )
                } else {
                    state_block.validate_transaction_with_entrypoint_index_and_routing_context(
                        accepted,
                        &mut ivm_cache,
                        idx,
                        routing_decisions[idx],
                    )
                };
                hashes[idx] = Some(hash);
                results[idx] = Some(result);
            }

            let mut ordered_hashes = Vec::with_capacity(n);
            let mut ordered_results = Vec::with_capacity(n);
            for idx in 0..n {
                ordered_hashes.push(hashes[idx].unwrap_or_else(|| entrypoints[idx].hash()));
                ordered_results.push(results[idx].take().unwrap_or_else(|| {
                    Err(TransactionRejectionReason::Validation(
                        iroha_data_model::ValidationFail::InternalError(format!(
                            "missing transaction result for idx {idx}"
                        )),
                    ))
                }));
            }

            Self::execute_deterministic_pipeline_triggers(
                block,
                state_block,
                &transaction_event_hashes,
                &ordered_results,
                &routing_decisions,
            )?;

            let (time_trgs, mut time_hashes, mut time_results) =
                state_block.execute_time_triggers(&block.header());
            #[cfg(test)]
            execute_soracloud_mailbox_runtime(state_block);
            let pruned_sealed_commitments =
                crate::tx::prune_expired_sealed_commitments(state_block);
            if pruned_sealed_commitments > 0 {
                iroha_logger::debug!(
                    count = pruned_sealed_commitments,
                    "pruned expired sealed transaction commitments"
                );
            }
            let fastpq_digest_batch = state_block.submit_transfer_transcript_digest_batch();
            let mut fastpq_entry_dataspaces = std::collections::BTreeMap::new();
            for (idx, entry_hash) in ordered_hashes.iter().enumerate() {
                fastpq_entry_dataspaces.insert(
                    iroha_crypto::Hash::from(*entry_hash),
                    routing_decisions[idx].dataspace_id,
                );
            }
            for entry_hash in &time_hashes {
                fastpq_entry_dataspaces.insert(
                    iroha_crypto::Hash::from(*entry_hash),
                    DataSpaceId::UNIVERSAL,
                );
            }
            ordered_hashes.append(&mut time_hashes);
            ordered_results.append(&mut time_results);

            let mut tx_set_hashes = ordered_hashes.clone();
            tx_set_hashes.sort_unstable();
            let tx_set_hash =
                crate::fastpq::tx_set_hash_from_ordered_hashes(tx_set_hashes.iter().copied());
            state_block.set_fastpq_tx_set_hash(tx_set_hash);
            state_block.set_fastpq_entry_dataspaces(fastpq_entry_dataspaces);

            let fastpq_transcripts =
                state_block.drain_transfer_transcripts_with_pending(fastpq_digest_batch);
            let axt_envelopes = state_block.drain_axt_envelopes();
            let axt_policy_snapshot = Some(state_block.axt_policy_snapshot());
            block
                .set_transaction_results_with_transcripts(
                    time_trgs,
                    ordered_hashes.as_slice(),
                    ordered_results,
                    fastpq_transcripts,
                    axt_envelopes,
                    axt_policy_snapshot,
                )
                .map_err(|_| BlockValidationError::MerkleRootMismatch)?;
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), start) {
                let elapsed = to_ms(start.elapsed());
                timings.execution_tx_apply_ms = elapsed;
                timings.execution_tx_apply_sequential_ms = elapsed;
            }
            Ok(())
        }

        fn execute_deterministic_pipeline_triggers(
            block: &SignedBlock,
            state_block: &mut StateBlock<'_>,
            transaction_event_hashes: &[Option<HashOf<SignedTransaction>>],
            ordered_results: &[TransactionResultInner],
            routing_decisions: &[crate::queue::RoutingDecision],
        ) -> Result<(), BlockValidationError> {
            debug_assert_eq!(transaction_event_hashes.len(), ordered_results.len());
            debug_assert_eq!(routing_decisions.len(), ordered_results.len());

            let mut deterministic_pipeline_events =
                Vec::with_capacity(transaction_event_hashes.len().saturating_add(1));
            for (idx, maybe_hash) in transaction_event_hashes.iter().copied().enumerate() {
                let Some(hash) = maybe_hash else {
                    continue;
                };
                let status = match &ordered_results[idx] {
                    Ok(_) => TransactionStatus::Approved,
                    Err(reason) => TransactionStatus::Rejected(Box::new(reason.clone())),
                };
                deterministic_pipeline_events.push(PipelineEventBox::from(TransactionEvent {
                    hash,
                    block_height: Some(block.header().height()),
                    lane_id: routing_decisions[idx].lane_id,
                    dataspace_id: routing_decisions[idx].dataspace_id,
                    status,
                }));
            }
            deterministic_pipeline_events.push(PipelineEventBox::from(BlockEvent {
                header: block.header(),
                status: BlockStatus::Approved,
            }));

            let mut transaction = state_block.transaction();
            transaction
                .execute_pipeline_triggers(deterministic_pipeline_events)
                .map_err(|reason| {
                    BlockValidationError::ExecutionContextInvalid(format!(
                        "pipeline trigger execution failed: {reason}"
                    ))
                })?;
            transaction.apply();
            Ok(())
        }

        /// Validate each transaction in the block, apply resulting state changes,
        /// and record results back into the block.
        ///
        /// Must be called with a **block that is _assumed_ to be valid**.
        /// When `skip_stateless_checks` is true, signature/limit validation is skipped under the
        /// assumption that the static snapshot validation already passed.
        fn validate_and_record_transactions(
            block: &mut SignedBlock,
            state_block: &mut StateBlock<'_>,
            timings: Option<&mut ValidationTimings>,
            skip_stateless_checks: bool,
        ) -> Result<(), BlockValidationError> {
            Self::validate_and_record_transactions_with_prepared(
                block,
                state_block,
                timings,
                skip_stateless_checks,
                None,
            )
        }

        #[allow(
            clippy::too_many_lines,
            clippy::explicit_iter_loop,
            clippy::option_if_let_else,
            clippy::manual_flatten,
            clippy::option_as_ref_cloned,
            clippy::needless_option_as_deref
        )]
        fn validate_and_record_transactions_with_prepared(
            block: &mut SignedBlock,
            state_block: &mut StateBlock<'_>,
            timings: Option<&mut ValidationTimings>,
            skip_stateless_checks: bool,
            prepared_txs: Option<&[PreparedBlockTransaction]>,
        ) -> Result<(), BlockValidationError> {
            use rayon::prelude::*;

            use crate::pipeline::{
                access::{
                    AccessSetSource, derive_for_prepared_overlay_with_source,
                    derive_for_transaction_with_source,
                },
                overlay::{TxOverlay, build_prepared_overlay_for_transaction_with_accounts_zk},
            };

            let to_ms = |duration: Duration| -> u64 {
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
            };
            let mut timings = timings;
            if block.has_results() {
                if let Some(snapshot) = block.axt_policy_snapshot() {
                    state_block.install_axt_policy_snapshot(snapshot);
                }
            }

            let height = block.header().height().get();

            // Start a new witness window for this block (SBV‑AM prototype)
            crate::sumeragi::witness::start_block();

            let sequential_entrypoints =
                block.external_entrypoints_slice().and_then(|entrypoints| {
                    entrypoints
                        .iter()
                        .any(|entrypoint| !matches!(entrypoint, TransactionEntrypoint::External(_)))
                        .then(|| entrypoints.to_vec())
                });
            if let Some(entrypoints) = sequential_entrypoints {
                Self::validate_and_record_entrypoints_sequential(
                    block,
                    state_block,
                    timings,
                    entrypoints,
                )?;
                return Ok(());
            }

            // Prepare scheduling: collect transactions, their access sets, and hashes
            let txs = Self::collect_external_signed_transactions(block);
            let local_prepared_txs;
            let prepared_txs = match prepared_txs {
                Some(prepared) if prepared.len() == txs.len() => prepared,
                _ => {
                    local_prepared_txs = Self::prepare_external_transactions(block);
                    &local_prepared_txs
                }
            };
            debug_assert_eq!(
                prepared_txs.len(),
                txs.len(),
                "prepared metadata must align with external transactions"
            );
            #[allow(clippy::disallowed_types)]
            let tx_hashes: std::collections::HashSet<_> = prepared_txs
                .iter()
                .map(|prepared| prepared.metadata.signed_hash)
                .collect();
            let height_u64 = block.header().height().get();
            let height_usize = height_u64.try_into().expect("block height fits usize");
            let block_height =
                std::num::NonZeroUsize::new(height_usize).expect("block height greater than zero");
            state_block
                .transactions
                .insert_block(tx_hashes, block_height);
            // Strategy controlled by configuration (no env reliance)
            let dynamic_prepass = state_block.pipeline.dynamic_prepass;
            // Load worker bound from config once to reuse across stages
            let workers = state_block.pipeline_worker_threads();
            let pool = state_block.pipeline_thread_pool();
            let map_stateless_fail = |fail: AcceptTransactionFail| -> TransactionRejectionReason {
                match fail {
                    AcceptTransactionFail::TransactionLimit(err) => {
                        TransactionRejectionReason::LimitCheck(err)
                    }
                    AcceptTransactionFail::SignatureVerification(sig_fail) => {
                        TransactionRejectionReason::Validation(
                            iroha_data_model::ValidationFail::NotPermitted(format!(
                                "signature verification failed: {}",
                                sig_fail.detail
                            )),
                        )
                    }
                    AcceptTransactionFail::UnexpectedGenesisAccountSignature => {
                        TransactionRejectionReason::Validation(
                            iroha_data_model::ValidationFail::NotPermitted(
                                "unexpected genesis account signature".to_owned(),
                            ),
                        )
                    }
                    AcceptTransactionFail::ChainIdMismatch(mismatch) => {
                        TransactionRejectionReason::Validation(
                            iroha_data_model::ValidationFail::NotPermitted(format!(
                                "chain id mismatch: expected {} got {}",
                                mismatch.expected, mismatch.actual
                            )),
                        )
                    }
                    AcceptTransactionFail::TransactionInTheFuture => {
                        TransactionRejectionReason::Validation(
                            iroha_data_model::ValidationFail::NotPermitted(
                                "transaction creation time is in the future".to_owned(),
                            ),
                        )
                    }
                    AcceptTransactionFail::TransactionExpired {
                        expires_at_ms,
                        now_ms,
                    } => TransactionRejectionReason::Validation(
                        iroha_data_model::ValidationFail::NotPermitted(format!(
                            "transaction expired: expires_at_ms={expires_at_ms} now_ms={now_ms}"
                        )),
                    ),
                    AcceptTransactionFail::NetworkTimeUnhealthy { reason } => {
                        TransactionRejectionReason::Validation(
                            iroha_data_model::ValidationFail::NotPermitted(format!(
                                "network time service unhealthy: {reason}"
                            )),
                        )
                    }
                }
            };

            let params_snapshot = state_block.world.parameters();
            let max_clock_drift = params_snapshot.sumeragi().max_clock_drift();
            let tx_params = params_snapshot.transaction();
            let crypto_cfg = Arc::clone(&state_block.crypto);
            let chain_id = state_block.chain_id.clone();
            let block_creation_time = block.header().creation_time();
            let is_genesis_block = block.header().is_genesis();
            let debug_trace_scheduler_inputs = state_block.pipeline.debug_trace_scheduler_inputs;
            let debug_trace_tx_eval = state_block.pipeline.debug_trace_tx_eval;
            #[cfg(feature = "telemetry")]
            let fraud_telemetry: Option<&crate::telemetry::StateTelemetry> =
                Some(state_block.telemetry);
            #[cfg(not(feature = "telemetry"))]
            let fraud_telemetry: Option<&()> = None;
            let dataspace_catalog = state_block.nexus.dataspace_catalog.clone();
            let lane_catalog = &state_block.nexus.lane_catalog;
            let routing_policy = &state_block.nexus.routing_policy;
            let fraud_cfg = &state_block.fraud_monitoring;
            let cache_cap = state_block.pipeline.stateless_cache_cap;
            let cache_enabled = cache_cap > 0 && !is_genesis_block;
            let max_clock_drift_ms = max_clock_drift.as_millis();
            let cache_context = if cache_enabled {
                Some(crate::state::StatelessValidationContext::new(
                    chain_id.clone(),
                    u64::try_from(max_clock_drift_ms).unwrap_or(u64::MAX),
                    tx_params,
                    crypto_cfg.allowed_signing.clone(),
                ))
            } else {
                None
            };
            if let Some(cache_context) = cache_context.as_ref() {
                let mut cache = state_block.stateless_validation_cache().lock();
                cache.set_cap(cache_cap);
                cache.ensure_context(cache_context.clone());
            }
            let embedded_routing =
                Self::embedded_routing_decisions_for_signed_transactions(block, txs.len());
            let (routing_decisions, routing_errors) = if let Some(decisions) = embedded_routing {
                (decisions, vec![None; txs.len()])
            } else {
                let routing_results: Vec<_> = if workers > 1 {
                    if let Some(pool) = pool.as_ref() {
                        pool.install(|| {
                            txs.par_iter()
                                .map(|tx| {
                                    let accepted = crate::tx::AcceptedTransaction::new_unchecked(
                                        Cow::Borrowed(*tx),
                                    );
                                    evaluate_policy_with_catalog_and_world(
                                        routing_policy,
                                        lane_catalog,
                                        &dataspace_catalog,
                                        &accepted,
                                        &state_block.world,
                                    )
                                })
                                .collect()
                        })
                    } else {
                        txs.par_iter()
                            .map(|tx| {
                                let accepted = crate::tx::AcceptedTransaction::new_unchecked(
                                    Cow::Borrowed(*tx),
                                );
                                evaluate_policy_with_catalog_and_world(
                                    routing_policy,
                                    lane_catalog,
                                    &dataspace_catalog,
                                    &accepted,
                                    &state_block.world,
                                )
                            })
                            .collect()
                    }
                } else {
                    txs.iter()
                        .map(|tx| {
                            let accepted =
                                crate::tx::AcceptedTransaction::new_unchecked(Cow::Borrowed(*tx));
                            evaluate_policy_with_catalog_and_world(
                                routing_policy,
                                lane_catalog,
                                &dataspace_catalog,
                                &accepted,
                                &state_block.world,
                            )
                        })
                        .collect()
                };
                let mut routing_decisions = Vec::with_capacity(routing_results.len());
                let mut routing_errors = Vec::with_capacity(routing_results.len());
                for routing in routing_results {
                    match routing {
                        Ok(decision) => {
                            routing_decisions.push(decision);
                            routing_errors.push(None);
                        }
                        Err(err) => {
                            routing_decisions.push(crate::queue::RoutingDecision::default());
                            routing_errors.push(Some(err));
                        }
                    }
                }
                (routing_decisions, routing_errors)
            };
            let mut prechecked_signature_results: Vec<
                Option<Result<(), crate::tx::SignatureVerificationFail>>,
            > = vec![None; txs.len()];
            let signature_result_for_tx = |tx: &SignedTransaction| {
                crate::tx::AcceptedTransaction::signature_verification_result(tx)
            };
            let malformed_signature = |tx: &SignedTransaction, detail: &str| {
                Err(crate::tx::SignatureVerificationFail::new(
                    tx.signature().clone(),
                    crate::tx::SignatureRejectionCode::MalformedSignature,
                    detail.to_string(),
                ))
            };

            let sig_batch_start = if skip_stateless_checks {
                None
            } else {
                timings.as_ref().map(|_| Instant::now())
            };
            if !skip_stateless_checks {
                // Ed25519 deterministic micro-batching for stateless pre-pass.
                {
                    fn flush_ed25519_precheck_batch<'a>(
                        txs: &[&SignedTransaction],
                        prechecked_signature_results: &mut [Option<
                            Result<(), crate::tx::SignatureVerificationFail>,
                        >],
                        item_indices: &mut Vec<usize>,
                        messages: &mut Vec<&'a [u8]>,
                        signatures: &mut Vec<&'a [u8]>,
                        public_keys: &mut Vec<iroha_crypto::Ed25519ParsedPublicKey>,
                        scratch: &mut iroha_crypto::Ed25519BatchScratch<'a>,
                    ) {
                        if item_indices.is_empty() {
                            return;
                        }
                        if iroha_crypto::ed25519_verify_batch_preparsed_deterministic_with_scratch(
                            messages,
                            signatures,
                            public_keys,
                            [0; 32],
                            scratch,
                        )
                        .is_ok()
                        {
                            for idx in item_indices.iter().copied() {
                                prechecked_signature_results[idx] = Some(Ok(()));
                            }
                        } else {
                            for idx in item_indices.iter().copied() {
                                prechecked_signature_results[idx] = Some(
                                    crate::tx::AcceptedTransaction::signature_verification_result(
                                        txs[idx],
                                    ),
                                );
                            }
                        }
                        item_indices.clear();
                        messages.clear();
                        signatures.clear();
                        public_keys.clear();
                    }

                    let cap = if state_block.pipeline.signature_batch_max_ed25519 > 0 {
                        state_block.pipeline.signature_batch_max_ed25519
                    } else {
                        state_block.pipeline.signature_batch_max
                    };
                    let chunk_capacity = cap.max(1).min(txs.len().max(1));
                    let mut item_indices = Vec::with_capacity(chunk_capacity);
                    let mut messages = Vec::with_capacity(chunk_capacity);
                    let mut signatures = Vec::with_capacity(chunk_capacity);
                    let mut public_keys = Vec::with_capacity(chunk_capacity);
                    let mut scratch = iroha_crypto::Ed25519BatchScratch::default();
                    for (idx, (tx, prepared)) in txs.iter().zip(prepared_txs.iter()).enumerate() {
                        let AccountController::Single(signatory) = tx.authority().controller()
                        else {
                            continue;
                        };
                        if signatory.algorithm() != iroha_crypto::Algorithm::Ed25519 {
                            continue;
                        }
                        let sig_bytes = tx.signature().payload().payload();
                        if sig_bytes.len() != 64 {
                            prechecked_signature_results[idx] =
                                Some(malformed_signature(tx, "bad signature or key length"));
                            continue;
                        }
                        let Some(public_key) = prepared.metadata.single_ed25519_key else {
                            let (_algo, pk_bytes) = signatory.to_bytes();
                            if pk_bytes.len() != 32 {
                                prechecked_signature_results[idx] =
                                    Some(malformed_signature(tx, "bad signature or key length"));
                            }
                            continue;
                        };
                        if cap == 0 {
                            continue;
                        }
                        item_indices.push(idx);
                        messages.push(prepared.metadata.payload_hash.as_ref().as_slice());
                        signatures.push(sig_bytes);
                        public_keys.push(public_key);
                        if item_indices.len() == cap {
                            flush_ed25519_precheck_batch(
                                &txs,
                                &mut prechecked_signature_results,
                                &mut item_indices,
                                &mut messages,
                                &mut signatures,
                                &mut public_keys,
                                &mut scratch,
                            );
                        }
                    }
                    flush_ed25519_precheck_batch(
                        &txs,
                        &mut prechecked_signature_results,
                        &mut item_indices,
                        &mut messages,
                        &mut signatures,
                        &mut public_keys,
                        &mut scratch,
                    );
                }

                // Secp256k1 deterministic micro-batching for stateless pre-pass.
                {
                    #[derive(Clone)]
                    struct SecpItem {
                        idx: usize,
                        pk: Vec<u8>,
                        msg: [u8; 32],
                        sig: [u8; 64],
                    }
                    let mut items: Vec<SecpItem> = Vec::new();
                    for (idx, tx) in txs.iter().enumerate() {
                        let AccountController::Single(signatory) = tx.authority().controller()
                        else {
                            continue;
                        };
                        if signatory.algorithm() != iroha_crypto::Algorithm::Secp256k1 {
                            continue;
                        }
                        let (_algo, pk_bytes) = signatory.to_bytes();
                        let sig_bytes = tx.signature().payload().payload();
                        if sig_bytes.len() != 64 {
                            prechecked_signature_results[idx] =
                                Some(malformed_signature(tx, "bad secp256k1 signature length"));
                            continue;
                        }
                        let h = prepared_txs[idx].metadata.payload_hash;
                        let mut msg = [0u8; 32];
                        msg.copy_from_slice(h.as_ref());
                        let mut sig = [0u8; 64];
                        sig.copy_from_slice(sig_bytes);
                        items.push(SecpItem {
                            idx,
                            pk: pk_bytes.to_vec(),
                            msg,
                            sig,
                        });
                    }
                    let cap = state_block.pipeline.signature_batch_max_secp256k1;
                    if cap > 0 && !items.is_empty() {
                        let derive_seed = |slice: &[&SecpItem]| -> [u8; 32] {
                            let mut tuples: Vec<Vec<u8>> = slice
                                .iter()
                                .map(|it| {
                                    let mut v = Vec::with_capacity(it.pk.len() + 32 + 64);
                                    v.extend_from_slice(&it.pk);
                                    v.extend_from_slice(&it.msg);
                                    v.extend_from_slice(&it.sig);
                                    v
                                })
                                .collect();
                            tuples.sort_unstable();
                            let mut hasher = sha2::Sha256::new();
                            hasher.update(b"iroha:ecc_batch:v1:secp256k1");
                            for t in tuples.iter() {
                                hasher.update(t);
                            }
                            let out = hasher.finalize();
                            let mut seed = [0u8; 32];
                            seed.copy_from_slice(&out);
                            seed
                        };
                        let verify_batch_slice = |slice: &[&SecpItem]| -> bool {
                            let seed = derive_seed(slice);
                            let msgs: Vec<&[u8]> =
                                slice.iter().map(|it| it.msg.as_slice()).collect();
                            let sigs: Vec<&[u8]> =
                                slice.iter().map(|it| it.sig.as_slice()).collect();
                            let pks: Vec<&[u8]> = slice.iter().map(|it| it.pk.as_slice()).collect();
                            iroha_crypto::secp256k1_verify_batch_deterministic(
                                &msgs, &sigs, &pks, seed,
                            )
                            .is_ok()
                        };
                        let mut start = 0;
                        while start < items.len() {
                            let end = usize::min(start + cap, items.len());
                            let batch = &items[start..end];
                            let refs: Vec<&SecpItem> = batch.iter().collect();
                            if verify_batch_slice(&refs) {
                                for it in batch {
                                    prechecked_signature_results[it.idx] = Some(Ok(()));
                                }
                            } else {
                                for it in batch {
                                    prechecked_signature_results[it.idx] =
                                        Some(signature_result_for_tx(txs[it.idx]));
                                }
                            }
                            start = end;
                        }
                    }
                }

                // PQC deterministic micro-batching for stateless pre-pass.
                {
                    #[derive(Clone)]
                    struct PqcItem {
                        idx: usize,
                        pk: Vec<u8>,
                        msg: [u8; 32],
                        sig: Vec<u8>,
                    }
                    let mut items: Vec<PqcItem> = Vec::new();
                    for (idx, tx) in txs.iter().enumerate() {
                        let AccountController::Single(signatory) = tx.authority().controller()
                        else {
                            continue;
                        };
                        if signatory.algorithm() != iroha_crypto::Algorithm::MlDsa {
                            continue;
                        }
                        let (_algo, pk_bytes) = signatory.to_bytes();
                        let h = prepared_txs[idx].metadata.payload_hash;
                        let mut msg = [0u8; 32];
                        msg.copy_from_slice(h.as_ref());
                        let sig_bytes = tx.signature().payload().payload().to_vec();
                        items.push(PqcItem {
                            idx,
                            pk: pk_bytes.to_vec(),
                            msg,
                            sig: sig_bytes,
                        });
                    }
                    let cap = state_block.pipeline.signature_batch_max_pqc;
                    if cap > 0 && !items.is_empty() {
                        let derive_seed = |slice: &[&PqcItem]| -> [u8; 32] {
                            let mut tuples: Vec<Vec<u8>> = slice
                                .iter()
                                .map(|it| {
                                    let mut v = Vec::with_capacity(it.pk.len() + 32 + it.sig.len());
                                    v.extend_from_slice(&it.pk);
                                    v.extend_from_slice(&it.msg);
                                    v.extend_from_slice(&it.sig);
                                    v
                                })
                                .collect();
                            tuples.sort_unstable();
                            let mut hasher = sha2::Sha256::new();
                            hasher.update(b"iroha:pqc_batch:v1:dilithium3");
                            for t in tuples.iter() {
                                hasher.update(t);
                            }
                            let out = hasher.finalize();
                            let mut seed = [0u8; 32];
                            seed.copy_from_slice(&out);
                            seed
                        };
                        let verify_batch_slice = |slice: &[&PqcItem]| -> bool {
                            let seed = derive_seed(slice);
                            let msgs: Vec<&[u8]> =
                                slice.iter().map(|it| it.msg.as_slice()).collect();
                            let sigs: Vec<&[u8]> =
                                slice.iter().map(|it| it.sig.as_slice()).collect();
                            let pks: Vec<&[u8]> = slice.iter().map(|it| it.pk.as_slice()).collect();
                            iroha_crypto::pqc_verify_batch_deterministic(&msgs, &sigs, &pks, seed)
                                .is_ok()
                        };
                        let mut start = 0;
                        while start < items.len() {
                            let end = usize::min(start + cap, items.len());
                            let batch = &items[start..end];
                            let refs: Vec<&PqcItem> = batch.iter().collect();
                            if verify_batch_slice(&refs) {
                                for it in batch {
                                    prechecked_signature_results[it.idx] = Some(Ok(()));
                                }
                            } else {
                                for it in batch {
                                    prechecked_signature_results[it.idx] =
                                        Some(signature_result_for_tx(txs[it.idx]));
                                }
                            }
                            start = end;
                        }
                    }
                }

                // BLS deterministic batching for stateless pre-pass.
                #[cfg(feature = "bls")]
                {
                    #[derive(Clone)]
                    struct BlsItem {
                        idx: usize,
                        pk: iroha_crypto::PublicKey,
                        pk_bytes: Vec<u8>,
                        pop: Option<Vec<u8>>,
                        msg: [u8; 32],
                        sig: Vec<u8>,
                    }
                    static BLS_POP_KEY: LazyLock<iroha_data_model::name::Name> =
                        LazyLock::new(|| "bls_pop".parse().expect("valid metadata key"));
                    static BLS_POP_SMALL_KEY: LazyLock<iroha_data_model::name::Name> =
                        LazyLock::new(|| "bls_pop_small".parse().expect("valid metadata key"));
                    let mut all_normal_have_pop = true;
                    let mut all_small_have_pop = true;
                    let mut items_normal: Vec<BlsItem> = Vec::new();
                    let mut items_small: Vec<BlsItem> = Vec::new();
                    for (idx, tx) in txs.iter().enumerate() {
                        let AccountController::Single(signatory) = tx.authority().controller()
                        else {
                            continue;
                        };
                        let algo = signatory.algorithm();
                        let small = match algo {
                            iroha_crypto::Algorithm::BlsNormal => false,
                            iroha_crypto::Algorithm::BlsSmall => true,
                            _ => continue,
                        };
                        let h = prepared_txs[idx].metadata.payload_hash;
                        let mut msg = [0u8; 32];
                        msg.copy_from_slice(h.as_ref());
                        let sig_bytes = tx.signature().payload().payload().to_vec();
                        let mut pop = None;
                        if small {
                            if let Some(pop_hex) =
                                bls_small_pop_from_metadata(tx.metadata(), &BLS_POP_SMALL_KEY)
                            {
                                if iroha_crypto::bls_small_pop_verify(signatory, &pop_hex).is_err()
                                {
                                    all_small_have_pop = false;
                                } else {
                                    pop = Some(pop_hex);
                                }
                            } else {
                                all_small_have_pop = false;
                            }
                        } else if let Some(pop_hex) =
                            bls_pop_from_metadata(tx.metadata(), &BLS_POP_KEY)
                        {
                            if iroha_crypto::bls_normal_pop_verify(signatory, &pop_hex).is_err() {
                                all_normal_have_pop = false;
                            } else {
                                pop = Some(pop_hex);
                            }
                        } else {
                            all_normal_have_pop = false;
                        }
                        let item = BlsItem {
                            idx,
                            pk: signatory.clone(),
                            pk_bytes: signatory.to_bytes().1.to_vec(),
                            pop,
                            msg,
                            sig: sig_bytes,
                        };
                        if small {
                            items_small.push(item);
                        } else {
                            items_normal.push(item);
                        }
                    }
                    let cap = state_block.pipeline.signature_batch_max_bls;
                    let mut verify_set = |items: &[BlsItem], small: bool| {
                        if items.is_empty() {
                            return;
                        }
                        let mut groups: std::collections::BTreeMap<[u8; 32], Vec<&BlsItem>> =
                            std::collections::BTreeMap::new();
                        for item in items {
                            groups.entry(item.msg).or_default().push(item);
                        }
                        let mut singletons: Vec<&BlsItem> = Vec::new();
                        for group in groups.values() {
                            if group.len() == 1 {
                                singletons.push(group[0]);
                                continue;
                            }
                            let ok = {
                                let msg = group[0].msg.as_slice();
                                let sigs: Vec<&[u8]> =
                                    group.iter().map(|it| it.sig.as_slice()).collect();
                                let pks: Vec<&iroha_crypto::PublicKey> =
                                    group.iter().map(|it| &it.pk).collect();
                                let mut pops = Vec::with_capacity(group.len());
                                for it in group {
                                    let Some(pop) = it.pop.as_ref() else {
                                        return;
                                    };
                                    pops.push(pop.as_slice());
                                }
                                if small {
                                    iroha_crypto::bls_small_verify_aggregate_same_message(
                                        msg, &sigs, &pks, &pops,
                                    )
                                    .is_ok()
                                } else {
                                    iroha_crypto::bls_normal_verify_aggregate_same_message(
                                        msg, &sigs, &pks, &pops,
                                    )
                                    .is_ok()
                                }
                            };
                            if ok {
                                for it in group {
                                    prechecked_signature_results[it.idx] = Some(Ok(()));
                                }
                            } else {
                                for it in group {
                                    prechecked_signature_results[it.idx] =
                                        Some(signature_result_for_tx(txs[it.idx]));
                                }
                            }
                        }
                        if !singletons.is_empty() {
                            let msgs: Vec<&[u8]> =
                                singletons.iter().map(|it| it.msg.as_slice()).collect();
                            let sigs: Vec<&[u8]> =
                                singletons.iter().map(|it| it.sig.as_slice()).collect();
                            let pks: Vec<&[u8]> =
                                singletons.iter().map(|it| it.pk_bytes.as_slice()).collect();
                            let ok = if small {
                                iroha_crypto::bls_small_verify_aggregate_multi_message(
                                    &msgs, &sigs, &pks,
                                )
                                .is_ok()
                            } else {
                                iroha_crypto::bls_normal_verify_aggregate_multi_message(
                                    &msgs, &sigs, &pks,
                                )
                                .is_ok()
                            };
                            if ok {
                                for it in singletons {
                                    prechecked_signature_results[it.idx] = Some(Ok(()));
                                }
                            } else {
                                for it in singletons {
                                    prechecked_signature_results[it.idx] =
                                        Some(signature_result_for_tx(txs[it.idx]));
                                }
                            }
                        }
                    };
                    if cap > 0 {
                        if all_normal_have_pop {
                            for chunk in items_normal.chunks(cap) {
                                verify_set(chunk, false);
                            }
                        }
                        if all_small_have_pop {
                            for chunk in items_small.chunks(cap) {
                                verify_set(chunk, true);
                            }
                        }
                    }
                }
            }
            if let Some(timings) = timings.as_deref_mut() {
                if let Some(start) = sig_batch_start {
                    timings.execution_tx_signature_batch_ms = to_ms(start.elapsed());
                } else if skip_stateless_checks {
                    timings.execution_tx_signature_batch_ms = 0;
                }
            }

            let stateless_start = timings.as_ref().map(|_| Instant::now());
            #[cfg(feature = "telemetry")]
            let t_stateless_start = Instant::now();
            let mut stateless_rejections: Vec<Option<TransactionRejectionReason>> = {
                let validate_tx = |(idx, tx): (usize, &&SignedTransaction)| {
                    if let Some(err) = routing_errors[idx].as_ref() {
                        return Some(TransactionRejectionReason::Validation(
                            iroha_data_model::ValidationFail::NotPermitted(format!(
                                "transaction routing could not be resolved: {err}"
                            )),
                        ));
                    }
                    if !skip_stateless_checks && tx.creation_time() >= block_creation_time {
                        return Some(TransactionRejectionReason::Validation(
                            iroha_data_model::ValidationFail::NotPermitted(format!(
                                "transaction creation time {} is not earlier than block creation time {}",
                                tx.creation_time().as_millis(),
                                block_creation_time.as_millis()
                            )),
                        ));
                    }
                    if is_genesis_block {
                        return None;
                    }
                    let is_heartbeat = crate::tx::is_heartbeat_transaction(tx);
                    if !is_heartbeat {
                        let routing_decision = routing_decisions[idx];
                        let lane_assignment = LaneAssignment {
                            lane_id: routing_decision.lane_id,
                            dataspace_id: routing_decision.dataspace_id,
                            dataspace_catalog: &dataspace_catalog,
                        };
                        if let Err(reason) = enforce_fraud_policy(
                            fraud_cfg,
                            tx.metadata(),
                            fraud_telemetry,
                            &lane_assignment,
                        ) {
                            return Some(reason);
                        }
                    }
                    if skip_stateless_checks {
                        return None;
                    }
                    let prechecked_signature_result = prechecked_signature_results
                        .get(idx)
                        .and_then(|result| result.as_ref().cloned());
                    let stateless = if is_heartbeat {
                        AcceptedTransaction::validate_heartbeat_with_now_with_signature_result_and_prepared_metadata(
                            tx,
                            &chain_id,
                            max_clock_drift,
                            tx_params,
                            crypto_cfg.as_ref(),
                            block_creation_time,
                            prechecked_signature_result,
                            &prepared_txs[idx].metadata,
                        )
                    } else {
                        AcceptedTransaction::validate_with_now_with_signature_result_and_prepared_metadata(
                            tx,
                            &chain_id,
                            max_clock_drift,
                            tx_params,
                            crypto_cfg.as_ref(),
                            block_creation_time,
                            prechecked_signature_result,
                            &prepared_txs[idx].metadata,
                        )
                    };
                    match stateless {
                        Ok(()) => None,
                        Err(fail) => Some(map_stateless_fail(fail)),
                    }
                };
                if workers > 1 {
                    if let Some(pool) = pool.as_ref() {
                        pool.install(|| txs.par_iter().enumerate().map(validate_tx).collect())
                    } else {
                        txs.par_iter().enumerate().map(validate_tx).collect()
                    }
                } else {
                    txs.iter().enumerate().map(validate_tx).collect()
                }
            };
            #[cfg(feature = "telemetry")]
            {
                let aggregate_lane = state_block.nexus.routing_policy.default_lane;
                state_block.metrics().observe_pipeline_stage_ms(
                    aggregate_lane,
                    "stateless",
                    t_stateless_start.elapsed().as_secs_f64() * 1_000.0,
                );
            }
            if let Some(cache_context) = cache_context {
                let mut cache = state_block.stateless_validation_cache().lock();
                cache.set_cap(cache_cap);
                cache.ensure_context(cache_context);
                for (idx, tx) in txs.iter().enumerate() {
                    if stateless_rejections[idx].is_some() {
                        continue;
                    }
                    let expires_at_ms = tx
                        .time_to_live()
                        .and_then(|ttl| tx.creation_time().checked_add(ttl))
                        .map(|expires_at| expires_at.as_millis());
                    let not_before_ms = tx
                        .creation_time()
                        .as_millis()
                        .saturating_sub(max_clock_drift_ms);
                    cache.insert_ok(
                        prepared_txs[idx].metadata.signed_hash,
                        expires_at_ms,
                        not_before_ms,
                    );
                }
            }
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), stateless_start) {
                timings.execution_tx_stateless_ms = to_ms(start.elapsed());
            }
            let call_hashes: Vec<_> = prepared_txs
                .iter()
                .map(|prepared| prepared.metadata.entrypoint_hash)
                .collect();
            if debug_trace_scheduler_inputs {
                let input_hashes: Vec<_> = call_hashes.clone();
                eprintln!("[scheduler-input] call_hashes={input_hashes:?}");
            }

            // Quarantine classification (opt-in via hook; disabled by default or when cap==0)
            let q_cap = state_block.pipeline.quarantine_max_txs_per_block;
            let q_cycle_cap = state_block.pipeline.quarantine_tx_max_cycles;
            let q_time_cap = state_block.pipeline.quarantine_tx_max_millis;
            let upper_cycle_cap = state_block.pipeline.ivm_max_cycles_upper_bound;
            let classifier = QUARANTINE_CLASSIFIER
                .get()
                .and_then(|m| m.lock().ok())
                .and_then(|g| *g);
            let mut is_quarantine: Vec<bool> = vec![false; txs.len()];
            let mut quarantine_candidates: Vec<usize> = Vec::new();
            let mut quarantine_allowed: std::collections::BTreeSet<usize> =
                std::collections::BTreeSet::new();
            let mut quarantine_overflow: std::collections::BTreeSet<usize> =
                std::collections::BTreeSet::new();
            if let Some(f) = classifier {
                for (i, tx) in txs.iter().enumerate() {
                    if f(tx) {
                        is_quarantine[i] = true;
                        quarantine_candidates.push(i);
                    }
                }
                quarantine_candidates.sort_by_key(|&i| (call_hashes[i], i));
                if q_cap > 0 {
                    for &i in quarantine_candidates.iter().take(q_cap) {
                        quarantine_allowed.insert(i);
                    }
                    for &i in quarantine_candidates.iter().skip(q_cap) {
                        quarantine_overflow.insert(i);
                    }
                } else {
                    // cap 0 → reject all classified
                    for &i in &quarantine_candidates {
                        quarantine_overflow.insert(i);
                    }
                }
                #[cfg(feature = "telemetry")]
                {
                    let aggregate_lane = state_block.nexus.routing_policy.default_lane;
                    state_block.metrics().set_pipeline_quarantine_classified(
                        aggregate_lane,
                        quarantine_candidates.len() as u64,
                    );
                    state_block.metrics().set_pipeline_quarantine_overflow(
                        aggregate_lane,
                        quarantine_overflow.len() as u64,
                    );
                }
            }

            // Snapshot accounts for overlay building (prepass) — reused across txs
            let accounts_snapshot = state_block.accounts_snapshot();
            let overlay_cache_count = workers.max(1);
            let overlay_caches: Vec<_> = (0..overlay_cache_count)
                .map(|_| {
                    parking_lot::Mutex::new(IvmCache::with_capacity(
                        state_block.pipeline.cache_size,
                    ))
                })
                .collect();
            #[cfg(feature = "telemetry")]
            let overlay_aggregate_lane = state_block.nexus.routing_policy.default_lane;

            // Parallel overlay construction from configuration
            let build_parallel = state_block.pipeline.parallel_overlay;
            let overlay_start = timings.as_ref().map(|_| Instant::now());
            // Quarantine lane: overlays for quarantined transactions will be built
            // sequentially with per-tx caps below. Normal lane follows configured parallelism.
            #[cfg(feature = "telemetry")]
            let t_overlay_start = Instant::now();
            #[derive(Clone)]
            struct PreparedBlockOverlay {
                overlay: Arc<TxOverlay>,
                access_log: Option<ivm::host::AccessLog>,
            }

            let mut prepared_overlays: Vec<
                Result<PreparedBlockOverlay, crate::pipeline::overlay::OverlayBuildError>,
            > = vec![Err(crate::pipeline::overlay::OverlayBuildError::IvmHeaderParse); txs.len()];
            // Normal lane overlays
            if build_parallel && workers > 1 {
                if let Some(pool) = pool.as_ref() {
                    pool.install(|| {
                        prepared_overlays
                            .par_iter_mut()
                            .enumerate()
                            .for_each(|(i, slot)| {
                                if !is_quarantine[i] && stateless_rejections[i].is_none() {
                                    let tx = &txs[i];
                                    let metadata =
                                        crate::pipeline::overlay::resolve_streaming_metadata(
                                            state_block,
                                            tx.authority(),
                                        );
                                    let cache_idx = rayon::current_thread_index().unwrap_or(i)
                                        % overlay_caches.len();
                                    #[cfg(feature = "telemetry")]
                                    let cache_wait_start = Instant::now();
                                    let mut ivm_cache = overlay_caches[cache_idx].lock();
                                    #[cfg(feature = "telemetry")]
                                    state_block.metrics().observe_pipeline_stage_ms(
                                        overlay_aggregate_lane,
                                        "overlay_cache_wait",
                                        cache_wait_start.elapsed().as_secs_f64() * 1_000.0,
                                    );
                                    *slot =
                                        build_prepared_overlay_for_transaction_with_accounts_zk(
                                            tx,
                                            Arc::clone(&accounts_snapshot),
                                            state_block,
                                            state_block.zk().halo2.enabled
                                                || state_block.zk().stark.enabled,
                                            &block.header(),
                                            metadata,
                                            &mut ivm_cache,
                                            dynamic_prepass,
                                        )
                                        .map(|prepared| {
                                            PreparedBlockOverlay {
                                                overlay: Arc::new(prepared.overlay),
                                                access_log: prepared.access_log,
                                            }
                                        });
                                }
                            })
                    });
                } else {
                    prepared_overlays
                        .par_iter_mut()
                        .enumerate()
                        .for_each(|(i, slot)| {
                            if !is_quarantine[i] && stateless_rejections[i].is_none() {
                                let tx = &txs[i];
                                let metadata = crate::pipeline::overlay::resolve_streaming_metadata(
                                    state_block,
                                    tx.authority(),
                                );
                                let cache_idx = rayon::current_thread_index().unwrap_or(i)
                                    % overlay_caches.len();
                                #[cfg(feature = "telemetry")]
                                let cache_wait_start = Instant::now();
                                let mut ivm_cache = overlay_caches[cache_idx].lock();
                                #[cfg(feature = "telemetry")]
                                state_block.metrics().observe_pipeline_stage_ms(
                                    overlay_aggregate_lane,
                                    "overlay_cache_wait",
                                    cache_wait_start.elapsed().as_secs_f64() * 1_000.0,
                                );
                                *slot = build_prepared_overlay_for_transaction_with_accounts_zk(
                                    tx,
                                    Arc::clone(&accounts_snapshot),
                                    state_block,
                                    state_block.zk().halo2.enabled
                                        || state_block.zk().stark.enabled,
                                    &block.header(),
                                    metadata,
                                    &mut ivm_cache,
                                    dynamic_prepass,
                                )
                                .map(|prepared| {
                                    PreparedBlockOverlay {
                                        overlay: Arc::new(prepared.overlay),
                                        access_log: prepared.access_log,
                                    }
                                });
                            }
                        });
                }
            } else {
                for (i, tx) in txs.iter().enumerate() {
                    if !is_quarantine[i] && stateless_rejections[i].is_none() {
                        let metadata = crate::pipeline::overlay::resolve_streaming_metadata(
                            state_block,
                            tx.authority(),
                        );
                        #[cfg(feature = "telemetry")]
                        let cache_wait_start = Instant::now();
                        let mut ivm_cache = overlay_caches[0].lock();
                        #[cfg(feature = "telemetry")]
                        state_block.metrics().observe_pipeline_stage_ms(
                            overlay_aggregate_lane,
                            "overlay_cache_wait",
                            cache_wait_start.elapsed().as_secs_f64() * 1_000.0,
                        );
                        prepared_overlays[i] =
                            build_prepared_overlay_for_transaction_with_accounts_zk(
                                tx,
                                Arc::clone(&accounts_snapshot),
                                state_block,
                                state_block.zk().halo2.enabled || state_block.zk().stark.enabled,
                                &block.header(),
                                metadata,
                                &mut ivm_cache,
                                dynamic_prepass,
                            )
                            .map(|prepared| PreparedBlockOverlay {
                                overlay: Arc::new(prepared.overlay),
                                access_log: prepared.access_log,
                            });
                    }
                }
            }
            // Quarantine lane overlays (caps): build only for allowed; mark overflow as error
            for (i, tx) in txs.iter().enumerate() {
                if is_quarantine[i] {
                    if quarantine_overflow.contains(&i) {
                        prepared_overlays[i] =
                            Err(crate::pipeline::overlay::OverlayBuildError::QuarantineOverflow);
                    } else if quarantine_allowed.contains(&i) && stateless_rejections[i].is_none() {
                        let metadata = crate::pipeline::overlay::resolve_streaming_metadata(
                            state_block,
                            tx.authority(),
                        );
                        #[cfg(feature = "telemetry")]
                        let cache_wait_start = Instant::now();
                        let mut ivm_cache = overlay_caches[0].lock();
                        #[cfg(feature = "telemetry")]
                        state_block.metrics().observe_pipeline_stage_ms(
                            overlay_aggregate_lane,
                            "overlay_cache_wait",
                            cache_wait_start.elapsed().as_secs_f64() * 1_000.0,
                        );
                        prepared_overlays[i] =
                            crate::pipeline::overlay::build_overlay_for_transaction_quarantine(
                                tx,
                                Arc::clone(&accounts_snapshot),
                                state_block,
                                q_cycle_cap,
                                q_time_cap,
                                upper_cycle_cap,
                                metadata,
                                &mut ivm_cache,
                            )
                            .map(|overlay| PreparedBlockOverlay {
                                overlay: Arc::new(overlay),
                                access_log: None,
                            });
                    }
                }
            }
            #[cfg(feature = "telemetry")]
            {
                let aggregate_lane = state_block.nexus.routing_policy.default_lane;
                state_block.metrics().observe_pipeline_stage_ms(
                    aggregate_lane,
                    "overlays",
                    t_overlay_start.elapsed().as_secs_f64() * 1_000.0,
                );
            }
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), overlay_start) {
                timings.execution_tx_overlay_ms = to_ms(start.elapsed());
            }

            let fee_postprocessing_required: Vec<bool> = txs
                .iter()
                .map(|tx| {
                    transaction_requires_fee_postprocessing(
                        &state_block.pipeline,
                        &state_block.nexus,
                        tx,
                    )
                })
                .collect();
            let data_triggers_enabled =
                state_block
                    .world
                    .triggers
                    .data_triggers()
                    .iter()
                    .any(|(_, action)| {
                        !action.repeats.is_depleted()
                            && crate::smartcontracts::isi::triggers::trigger_is_enabled(
                                action.metadata(),
                            )
                    });

            #[cfg(feature = "telemetry")]
            let t_access_start = Instant::now();
            let access_start = timings.as_ref().map(|_| Instant::now());
            let derive_access = |(idx, tx): (usize, &&SignedTransaction)| {
                if stateless_rejections[idx].is_some() {
                    return (crate::pipeline::access::AccessSet::new(), None);
                }
                match &prepared_overlays[idx] {
                    Ok(prepared) => derive_for_prepared_overlay_with_source(
                        tx,
                        &*state_block,
                        prepared.overlay.as_ref(),
                        prepared.access_log.as_ref(),
                        dynamic_prepass,
                    ),
                    Err(_) => derive_for_transaction_with_source(
                        tx,
                        Some(&*state_block),
                        crate::pipeline::access::IvmStrategy::Conservative,
                    ),
                }
            };
            let derived: Vec<_> = if workers > 1 {
                if let Some(pool) = pool.as_ref() {
                    pool.install(|| txs.par_iter().enumerate().map(derive_access).collect())
                } else {
                    txs.par_iter().enumerate().map(derive_access).collect()
                }
            } else {
                txs.iter().enumerate().map(derive_access).collect()
            };
            let mut access_sources: Vec<Option<AccessSetSource>> =
                Vec::with_capacity(derived.len());
            let mut access: Vec<crate::pipeline::access::AccessSet> =
                Vec::with_capacity(derived.len());
            for (set, source) in derived {
                access.push(set);
                access_sources.push(source);
            }
            for (idx, set) in access.iter_mut().enumerate() {
                if fee_postprocessing_required[idx] {
                    set.add_write(GLOBAL_WILDCARD_KEY.to_owned());
                }
            }
            let mut access_set_sources = status::AccessSetSourceSummary::default();
            for source in access_sources.into_iter().flatten() {
                match source {
                    AccessSetSource::ManifestHints => {
                        access_set_sources.manifest_hints =
                            access_set_sources.manifest_hints.saturating_add(1);
                    }
                    AccessSetSource::EntrypointHints => {
                        access_set_sources.entrypoint_hints =
                            access_set_sources.entrypoint_hints.saturating_add(1);
                    }
                    AccessSetSource::PrepassMerge => {
                        access_set_sources.prepass_merge =
                            access_set_sources.prepass_merge.saturating_add(1);
                    }
                    AccessSetSource::ConservativeFallback => {
                        access_set_sources.conservative_fallback =
                            access_set_sources.conservative_fallback.saturating_add(1);
                    }
                }
            }
            status::set_access_set_source_summary(access_set_sources);
            #[cfg(feature = "telemetry")]
            {
                let telemetry = state_block.metrics();
                telemetry.inc_pipeline_access_set_source(
                    AccessSetSource::ManifestHints,
                    access_set_sources.manifest_hints,
                );
                telemetry.inc_pipeline_access_set_source(
                    AccessSetSource::EntrypointHints,
                    access_set_sources.entrypoint_hints,
                );
                telemetry.inc_pipeline_access_set_source(
                    AccessSetSource::PrepassMerge,
                    access_set_sources.prepass_merge,
                );
                telemetry.inc_pipeline_access_set_source(
                    AccessSetSource::ConservativeFallback,
                    access_set_sources.conservative_fallback,
                );
            }
            #[cfg(feature = "telemetry")]
            {
                let aggregate_lane = state_block.nexus.routing_policy.default_lane;
                state_block.metrics().observe_pipeline_stage_ms(
                    aggregate_lane,
                    "access",
                    t_access_start.elapsed().as_secs_f64() * 1_000.0,
                );
            }
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), access_start) {
                timings.execution_tx_access_ms = to_ms(start.elapsed());
            }

            let overlays: Vec<Result<Arc<TxOverlay>, crate::pipeline::overlay::OverlayBuildError>> =
                prepared_overlays
                    .iter()
                    .map(|result| {
                        result
                            .as_ref()
                            .map(|prepared| Arc::clone(&prepared.overlay))
                            .map_err(Clone::clone)
                    })
                    .collect();

            // Build conflict graph using key interning (strings -> compact IDs),
            // and partition transactions into independent components via DSF.
            // The stateless pre-pass above guarantees that only envelope-valid
            // transactions reach this stage; rejected items have already been
            // recorded and are skipped from overlay construction.
            let n = txs.len();
            let dag_start = timings.as_ref().map(|_| Instant::now());
            #[cfg(feature = "telemetry")]
            let t_dag_start = Instant::now();
            // Intern keys per block and convert access sets to ID vectors
            let (key_count, access_ids) = intern_access(&access);

            // Compute a DAG fingerprint for recovery/idempotence checks (stable across peers)
            let dag_fp = dag_fingerprint(key_count, &access_ids, &call_hashes);

            let block_hash = block.hash();
            let expected_dag_fp =
                state_block
                    .kura()
                    .read_pipeline_metadata(height)
                    .and_then(|sidecar| {
                        expected_pipeline_dag_fingerprint(
                            height,
                            block_hash,
                            &call_hashes,
                            &sidecar,
                        )
                    });

            // Compare with expected fingerprint when present; warn on mismatch (non-forking).
            if let Some(exp) = expected_dag_fp
                && exp != dag_fp
            {
                let expected_hex = hex::encode(exp);
                let actual_hex = hex::encode(dag_fp);
                iroha_logger::warn!(
                    height,
                    expected=%expected_hex,
                    actual=%actual_hex,
                    "pipeline DAG fingerprint mismatch; continuing with recomputed schedule"
                );
                // Emit a pipeline warning event for subscribers
                state_block.world.push_pipeline_warning(
                    block.header(),
                    "dag_fingerprint_mismatch",
                    &format!(
                        "DAG fingerprint mismatch: expected {expected_hex} != actual {actual_hex}"
                    ),
                );
            }

            // Persist admission sets and DAG fingerprint for idempotent recovery (best-effort).
            // Store a compact Norito sidecar for diagnostics.
            #[allow(unused)]
            {
                let txs_sidecar: Vec<PipelineTxSnapshot> = prepared_txs
                    .iter()
                    .zip(access.iter())
                    .map(|(prepared, aset)| PipelineTxSnapshot {
                        hash: prepared.metadata.entrypoint_hash,
                        reads: aset.read_keys.iter().cloned().collect(),
                        writes: aset.write_keys.iter().cloned().collect(),
                    })
                    .collect();

                let dag_snapshot = u32::try_from(key_count).map_or_else(
                    |_| {
                        iroha_logger::warn!(key_count, "pipeline key_count exceeds u32 range");
                        PipelineDagSnapshot {
                            fingerprint: dag_fp,
                            key_count: u32::MAX,
                        }
                    },
                    |count| PipelineDagSnapshot {
                        fingerprint: dag_fp,
                        key_count: count,
                    },
                );

                let mut sidecar =
                    PipelineRecoverySidecar::new(height, block_hash, dag_snapshot, txs_sidecar);
                #[cfg(feature = "zk-preverify")]
                {
                    let proofs = crate::zk::collect_trace_proofs_for_height(height);
                    if !proofs.is_empty() {
                        iroha_logger::debug!(
                            height,
                            count = proofs.len(),
                            "attaching {} trace proof digests to pipeline sidecar",
                            proofs.len()
                        );
                        sidecar.proofs = proofs;
                    }
                }
                state_block.kura().enqueue_pipeline_metadata(sidecar);
            }

            // DSF prepass: union adjacent conflicting read/write relations to find independent components
            let mut dsu = DisjointSet::new(n);
            if state_block.pipeline.gpu_key_bucket {
                let mut triplets: Vec<AccessTriplet> = Vec::with_capacity(
                    access_ids
                        .iter()
                        .map(|a| a.reads.len() + a.writes.len())
                        .sum(),
                );
                for (idx, aset) in access_ids.iter().enumerate() {
                    for &k in aset.reads.iter() {
                        triplets.push(AccessTriplet {
                            key: k,
                            tx_index: idx,
                            flag: 0,
                        });
                    }
                    for &k in aset.writes.iter() {
                        triplets.push(AccessTriplet {
                            key: k,
                            tx_index: idx,
                            flag: 1,
                        });
                    }
                }
                gpu::sort_triplets_gpu_or_cpu(&mut triplets);
                union_from_sorted_triplets(&mut dsu, &triplets);
            } else {
                use iroha_primitives::small::SmallVec;
                let mut last_writer: Vec<Option<usize>> = vec![None; key_count];
                let mut open_readers: Vec<SmallVec<[usize; 4]>> = vec![SmallVec::new(); key_count];
                for (idx, aset) in access_ids.iter().enumerate() {
                    for &k in aset.reads.iter() {
                        if let Some(w) = last_writer[k as usize] {
                            dsu.union(idx, w);
                        }
                        open_readers[k as usize].push(idx);
                    }
                    for &k in aset.writes.iter() {
                        if let Some(w) = last_writer[k as usize] {
                            dsu.union(idx, w);
                        }
                        if let Some(readers) = {
                            if open_readers[k as usize].is_empty() {
                                None
                            } else {
                                Some(std::mem::take(&mut open_readers[k as usize]))
                            }
                        } {
                            for r in readers {
                                dsu.union(idx, r);
                            }
                        }
                        last_writer[k as usize] = Some(idx);
                    }
                }
            }
            // Bucket tx indices by component root and sort components deterministically by min (call_hash, idx)
            let mut comps: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
            let mut dsu_copy = dsu.clone();
            for i in 0..n {
                let r = dsu_copy.find(i);
                comps.entry(r).or_default().push(i);
            }
            let mut components: Vec<Vec<usize>> = comps.into_values().collect();
            for comp in components.iter_mut() {
                comp.sort_unstable();
            }
            components.sort_by(|a, b| {
                let min_a = a.iter().map(|&i| (call_hashes[i], i)).min().unwrap();
                let min_b = b.iter().map(|&i| (call_hashes[i], i)).min().unwrap();
                min_a.cmp(&min_b)
            });

            let (row_offsets, cols, indeg) = build_csr(&access_ids, key_count);
            #[cfg(feature = "telemetry")]
            {
                let aggregate_lane = state_block.nexus.routing_policy.default_lane;
                state_block.metrics().observe_pipeline_stage_ms(
                    aggregate_lane,
                    "dag",
                    t_dag_start.elapsed().as_secs_f64() * 1_000.0,
                );
            }
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), dag_start) {
                timings.execution_tx_dag_ms = to_ms(start.elapsed());
            }

            // Kahn's algorithm with two deterministic variants:
            // - per-wave sort baseline (stable tie-break by (call_hash, idx))
            // - BinaryHeap ready-queue variant
            // Default remains per-wave sort; switch is controlled via pipeline config.
            let use_ready_heap = state_block.pipeline.ready_queue_heap;

            #[cfg(feature = "telemetry")]
            let t_sched_start = Instant::now();
            let schedule_start = timings.as_ref().map(|_| Instant::now());
            let order = if crate::pipeline::force_fifo_scheduler() {
                (0..n).collect()
            } else if use_ready_heap {
                schedule_components_ready_heap(&components, &row_offsets, &cols, &call_hashes)
                    .unwrap_or_else(|| {
                        schedule_ready_heap_global(&row_offsets, &cols, &indeg, &call_hashes)
                    })
            } else {
                schedule_components_wave(&components, &row_offsets, &cols, &call_hashes)
                    .unwrap_or_else(|| {
                        schedule_wave_global(&row_offsets, &cols, &indeg, &call_hashes)
                    })
            };
            if debug_trace_scheduler_inputs {
                let ordered_hashes: Vec<_> = order.iter().map(|&idx| call_hashes[idx]).collect();
                eprintln!("[scheduler] call_hash_order={ordered_hashes:?}");
            }
            // Ensure we produced a full topological order
            #[cfg(debug_assertions)]
            if order.len() != n {
                // Emit a brief diagnostic to help tests pinpoint cycles/self-deps
                let mut indeg_s = indeg.clone();
                for &i in &order {
                    let start = row_offsets[i];
                    let end = row_offsets[i + 1];
                    for &v in &cols[start..end] {
                        if indeg_s[v] > 0 {
                            indeg_s[v] -= 1;
                        }
                    }
                }
                let remaining: Vec<usize> = (0..n).filter(|i| indeg_s[*i] > 0).collect();
                eprintln!(
                    "scheduler: incomplete order ({} of {}), remaining={:?}",
                    order.len(),
                    n,
                    remaining
                );
            }
            debug_assert_eq!(order.len(), n, "scheduler must order all transactions");
            #[cfg(feature = "telemetry")]
            {
                let aggregate_lane = state_block.nexus.routing_policy.default_lane;
                state_block.metrics().observe_pipeline_stage_ms(
                    aggregate_lane,
                    "schedule",
                    t_sched_start.elapsed().as_secs_f64() * 1_000.0,
                );
            }
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), schedule_start) {
                timings.execution_tx_schedule_ms = to_ms(start.elapsed());
            }

            let apply_start = timings.as_ref().map(|_| Instant::now());
            let apply_setup_start = timings.as_ref().map(|_| Instant::now());
            let mut apply_setup_ms = 0u64;
            let mut apply_layer_build_ms = 0u64;
            let mut apply_prep_ms = 0u64;
            let mut apply_detached_ms = 0u64;
            let mut apply_merge_ms = 0u64;
            let mut apply_fallback_ms = 0u64;
            let mut apply_quarantine_ms = 0u64;
            let mut apply_sequential_ms = 0u64;
            let mut apply_results_ms = 0u64;
            let mut lane_summaries: BTreeMap<LaneId, LaneSummary> = BTreeMap::new();
            let mut dataspace_summaries: BTreeMap<(LaneId, DataSpaceId), u64> = BTreeMap::new();
            let mut pending_settlements = state_block.drain_settlement_records();
            let mut pending_nexus_fee_receipts = state_block.drain_nexus_fee_records();
            let nexus_fee_receipts_active = state_block
                .nexus
                .fees
                .lane_relay_burn_receipts_active_at(block.header().height().get());

            let mut lane_settlement_builders: BTreeMap<
                (LaneId, DataSpaceId),
                LaneSettlementBuilder,
            > = BTreeMap::new();

            #[cfg(feature = "telemetry")]
            let record_amx_abort =
                |state: &mut StateBlock<'_>, tx_index: usize, stage: &'static str| {
                    let lane_id = routing_decisions[tx_index].lane_id;
                    state.metrics().inc_amx_abort(lane_id, stage);
                };
            #[cfg(not(feature = "telemetry"))]
            let record_amx_abort =
                |_state: &mut StateBlock<'_>, _tx_index: usize, _stage: &'static str| {};

            // Telemetry: update DAG, component, lane, and dataspace metrics for this block
            #[allow(unused_variables)]
            {
                let chunk_size = (iroha_config::parameters::defaults::sumeragi::RBC_CHUNK_MAX_BYTES
                    .max(1)) as u64;

                for (idx, decision) in routing_decisions.iter().enumerate() {
                    let summary = lane_summaries.entry(decision.lane_id).or_default();
                    summary.tx_vertices = summary.tx_vertices.saturating_add(1);
                    dataspace_summaries
                        .entry((decision.lane_id, decision.dataspace_id))
                        .and_modify(|count| *count = count.saturating_add(1))
                        .or_insert(1);

                    summary.rbc_bytes_total = summary
                        .rbc_bytes_total
                        .saturating_add(prepared_txs[idx].metadata.encoded_len as u64);

                    let mut counted_settlement_tx = false;
                    if let Some(record) =
                        pending_settlements.remove(&prepared_txs[idx].metadata.signed_hash)
                    {
                        let builder = lane_settlement_builders
                            .entry((decision.lane_id, decision.dataspace_id))
                            .or_default();
                        builder.tx_count = builder.tx_count.saturating_add(1);
                        counted_settlement_tx = true;
                        builder.total_local_micro = builder
                            .total_local_micro
                            .saturating_add(record.local_amount_micro);
                        builder.total_xor_due_micro = builder
                            .total_xor_due_micro
                            .saturating_add(record.xor_due_micro);
                        builder.total_xor_after_haircut_micro = builder
                            .total_xor_after_haircut_micro
                            .saturating_add(record.xor_after_haircut_micro);
                        builder.total_xor_variance_micro = builder
                            .total_xor_variance_micro
                            .saturating_add(record.xor_variance_micro);
                        builder
                            .source_counts
                            .entry(record.asset_definition_id.clone())
                            .and_modify(|count| *count = count.saturating_add(1))
                            .or_insert(1);
                        let evidence = SwapEvidence {
                            epsilon_bps: record.epsilon_bps,
                            twap_window_seconds: record.twap_window_seconds,
                            liquidity_profile: record.liquidity_profile,
                            twap_local_per_xor: record.twap_local_per_xor,
                            volatility_bucket: record.volatility_bucket,
                        };
                        match &mut builder.swap_evidence {
                            Some(existing) => {
                                debug_assert_eq!(
                                    existing, &evidence,
                                    "lane/dataspace conversions must share swap metadata"
                                );
                            }
                            None => {
                                builder.swap_evidence = Some(evidence);
                            }
                        }
                        builder.receipts.push(record.into_lane_receipt());
                    }
                    if let Some(record) =
                        pending_nexus_fee_receipts.remove(&prepared_txs[idx].metadata.signed_hash)
                    {
                        if !nexus_fee_receipts_active {
                            iroha_logger::warn!(
                                height = block.header().height().get(),
                                tx = %prepared_txs[idx].metadata.signed_hash,
                                "dropping staged Nexus fee receipt before fee receipt activation height"
                            );
                            continue;
                        }
                        let builder = lane_settlement_builders
                            .entry((decision.lane_id, decision.dataspace_id))
                            .or_default();
                        if !counted_settlement_tx {
                            builder.tx_count = builder.tx_count.saturating_add(1);
                        }
                        builder.nexus_fee_receipts.push(record.into_lane_receipt(
                            block.header().height().get(),
                            decision.lane_id,
                            decision.dataspace_id,
                        ));
                    }
                    if let Some(receipt) = native_amx_receipt_for_transaction(
                        txs[idx],
                        prepared_txs[idx].metadata.signed_hash,
                        block.header().height().get(),
                        *decision,
                        &dataspace_catalog,
                        &state_block.world,
                    ) {
                        let builder = lane_settlement_builders
                            .entry((decision.lane_id, decision.dataspace_id))
                            .or_default();
                        if !counted_settlement_tx {
                            builder.tx_count = builder.tx_count.saturating_add(1);
                        }
                        builder.native_amx_receipts.push(receipt);
                    }
                }

                for (src, decision) in routing_decisions.iter().enumerate() {
                    let start = row_offsets[src];
                    let end = row_offsets[src + 1];
                    if start == end {
                        continue;
                    }
                    let edges_in_lane = cols[start..end]
                        .iter()
                        .filter(|&&child| routing_decisions[child].lane_id == decision.lane_id)
                        .count() as u64;
                    if edges_in_lane > 0
                        && let Some(summary) = lane_summaries.get_mut(&decision.lane_id)
                    {
                        summary.tx_edges = summary.tx_edges.saturating_add(edges_in_lane);
                    }
                }

                for (idx, overlay_result) in overlays.iter().enumerate() {
                    if let Ok(overlay) = overlay_result {
                        if overlay.is_empty() {
                            continue;
                        }
                        if let Some(summary) =
                            lane_summaries.get_mut(&routing_decisions[idx].lane_id)
                        {
                            summary.overlay_count = summary.overlay_count.saturating_add(1);
                            summary.overlay_instr_total = summary
                                .overlay_instr_total
                                .saturating_add(overlay.instruction_count() as u64);
                            summary.overlay_bytes_total = summary
                                .overlay_bytes_total
                                .saturating_add(overlay.byte_size() as u64);
                        }
                    }
                }

                for summary in lane_summaries.values_mut() {
                    summary.rbc_chunks = if summary.rbc_bytes_total == 0 {
                        0
                    } else {
                        summary.rbc_bytes_total.div_ceil(chunk_size)
                    };
                }

                let vertices_total: u64 = lane_summaries
                    .values()
                    .map(|summary| summary.tx_vertices)
                    .sum();
                let edges_total: u64 = cols.len() as u64;
                let conflict_rate_bps = conflict_rate_bps(vertices_total, edges_total);
                status::set_pipeline_conflict_rate_bps(conflict_rate_bps);
                let overlay_count_total: u64 = lane_summaries
                    .values()
                    .map(|summary| summary.overlay_count)
                    .sum();
                let overlay_instr_total: u64 = lane_summaries
                    .values()
                    .map(|summary| summary.overlay_instr_total)
                    .sum();
                let overlay_bytes_total: u64 = lane_summaries
                    .values()
                    .map(|summary| summary.overlay_bytes_total)
                    .sum();

                // Components (DSF) histogram buckets [1,2,4,8,16,32,64,128] as cumulative counts
                let comp_count: u64 = components.len() as u64;
                let comp_max: u64 = components.iter().map(|c| c.len() as u64).max().unwrap_or(0);
                let thresholds: [u64; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
                let mut comp_buckets = [0u64; 8];
                for c in components.iter() {
                    let sz = c.len() as u64;
                    for (i, &t) in thresholds.iter().enumerate() {
                        if sz <= t {
                            comp_buckets[i] += 1;
                        }
                    }
                }

                #[cfg(feature = "telemetry")]
                {
                    let aggregate_lane = state_block.nexus.routing_policy.default_lane;
                    let telemetry = state_block.metrics();
                    telemetry.set_pipeline_dag(aggregate_lane, vertices_total, edges_total);
                    telemetry.set_pipeline_conflict_rate_bps(aggregate_lane, conflict_rate_bps);
                    telemetry.set_pipeline_components(
                        aggregate_lane,
                        comp_count,
                        comp_max,
                        comp_buckets,
                    );
                    telemetry.set_pipeline_overlays(
                        aggregate_lane,
                        overlay_count_total,
                        overlay_instr_total,
                    );
                    telemetry.set_pipeline_overlay_bytes(aggregate_lane, overlay_bytes_total);
                    for ((lane_id, dataspace_id), tx_served) in &dataspace_summaries {
                        telemetry.record_dataspace_pipeline_summary(
                            *lane_id,
                            *dataspace_id,
                            DataspacePipelineSummary {
                                tx_served: *tx_served,
                            },
                        );
                    }

                    let mut committed_per_lane: BTreeMap<LaneId, u64> = BTreeMap::new();
                    for (idx, tx) in txs.iter().enumerate() {
                        let hash = prepared_txs[idx].metadata.signed_hash;
                        if let Some(routing) = routing_ledger::get(&hash) {
                            let teu = estimate_transaction_teu(tx);
                            committed_per_lane
                                .entry(routing.lane_id)
                                .and_modify(|total| *total = total.saturating_add(teu))
                                .or_insert(teu);
                        }
                    }

                    let fallback_limits = LaneSchedulingLimits::new(
                        u64::from(state_block.nexus.fusion.exit_teu),
                        u64::from(state_block.nexus.da.rotation.window_slots.get()),
                    );
                    let mut lane_limits: BTreeMap<LaneId, LaneSchedulingLimits> = BTreeMap::new();
                    for lane in state_block.nexus.lane_catalog.lanes() {
                        let limits = QueueLimits::lane_limits_from_metadata(lane, fallback_limits);
                        lane_limits.insert(lane.id, limits);
                    }
                    let mut lane_ids: BTreeSet<LaneId> = lane_summaries.keys().copied().collect();
                    lane_ids.extend(committed_per_lane.keys().copied());

                    for lane_id in lane_ids {
                        let limits = lane_limits
                            .get(&lane_id)
                            .copied()
                            .unwrap_or(fallback_limits);
                        let committed = committed_per_lane.get(&lane_id).copied().unwrap_or(0);
                        let headroom = limits.teu_capacity.saturating_sub(committed);
                        telemetry.record_nexus_scheduler_lane_teu(
                            lane_id,
                            LaneTeuGaugeUpdate {
                                capacity: limits.teu_capacity,
                                committed,
                                buckets: NexusLaneTeuBuckets {
                                    floor: committed,
                                    headroom,
                                    must_serve: 0,
                                    circuit_breaker: 0,
                                },
                                trigger_level: 0,
                                starvation_bound_slots: limits.starvation_bound_slots,
                            },
                        );
                    }

                    for &(lane_id, dataspace_id) in dataspace_summaries.keys() {
                        telemetry.record_nexus_scheduler_dataspace_teu(
                            lane_id,
                            dataspace_id,
                            DataspaceTeuGaugeUpdate {
                                backlog: 0,
                                age_slots: 0,
                                virtual_finish: 0,
                            },
                        );
                    }
                }

                let dataspace_activity_snapshot: Vec<status::DataspaceActivitySnapshot> =
                    dataspace_summaries
                        .iter()
                        .map(|((lane_id, dataspace_id), tx_served)| {
                            status::DataspaceActivitySnapshot {
                                lane_id: lane_id.as_u32(),
                                dataspace_id: dataspace_id.as_u64(),
                                tx_served: *tx_served,
                            }
                        })
                        .collect();
                status::set_dataspace_activity_snapshot(dataspace_activity_snapshot);
            }

            for ((lane_id, _), builder) in lane_settlement_builders.iter_mut() {
                if builder.buffer_snapshot.is_none() {
                    builder.buffer_snapshot =
                        compute_settlement_buffer_snapshot(state_block, *lane_id);
                }
                if let Some(snapshot) = &builder.buffer_snapshot {
                    if let Some(metadata) = lane_metadata_by_id(state_block, *lane_id) {
                        match snapshot.status {
                            BufferStatus::Normal => {}
                            BufferStatus::Alert => {
                                iroha_logger::warn!(
                                    lane = %metadata.alias,
                                    "settlement buffer for lane {} dipped below the alert threshold (<{}%)",
                                    metadata.alias,
                                    state_block
                                        .settlement_engine()
                                        .buffer_policy()
                                        .alert
                                );
                            }
                            BufferStatus::Throttle => {
                                iroha_logger::warn!(
                                    lane = %metadata.alias,
                                    "settlement buffer for lane {} entered throttle state (<{}%); reduce subsidised inclusion",
                                    metadata.alias,
                                    state_block
                                        .settlement_engine()
                                        .buffer_policy()
                                        .throttle
                                );
                            }
                            BufferStatus::XorOnly => {
                                iroha_logger::warn!(
                                    lane = %metadata.alias,
                                    "settlement buffer for lane {} entered XOR-only state (<{}%); force XOR-denominated inclusion",
                                    metadata.alias,
                                    state_block
                                        .settlement_engine()
                                        .buffer_policy()
                                        .xor_only
                                );
                            }
                            BufferStatus::Halt => {
                                iroha_logger::error!(
                                    lane = %metadata.alias,
                                    "settlement buffer for lane {} hit the halt threshold (<{}%); pause settlement until refilled",
                                    metadata.alias,
                                    state_block
                                        .settlement_engine()
                                        .buffer_policy()
                                        .halt
                                );
                            }
                        }
                    }
                }
            }

            let lane_settlement_commitments: Vec<LaneBlockCommitment> = {
                let block_height = block.header().height().get();
                lane_settlement_builders
                    .into_iter()
                    .map(|((lane_id, dataspace_id), builder)| {
                        #[cfg(feature = "telemetry")]
                        {
                            record_lane_settlement_metrics(
                                state_block.metrics(),
                                lane_id,
                                dataspace_id,
                                &builder,
                            );
                        }
                        LaneBlockCommitment {
                            block_height,
                            lane_id,
                            dataspace_id,
                            tx_count: builder.tx_count,
                            total_local_micro: builder.total_local_micro,
                            total_xor_due_micro: builder.total_xor_due_micro,
                            total_xor_after_haircut_micro: builder.total_xor_after_haircut_micro,
                            total_xor_variance_micro: builder.total_xor_variance_micro,
                            swap_metadata: builder
                                .swap_evidence
                                .map(SwapEvidence::into_lane_metadata),
                            receipts: builder.receipts,
                            nexus_fee_receipts: builder.nexus_fee_receipts,
                            native_amx_receipts: builder.native_amx_receipts,
                        }
                    })
                    .collect()
            };

            if !lane_settlement_commitments.is_empty() {
                crate::sumeragi::status::set_lane_settlement_commitments(
                    lane_settlement_commitments.clone(),
                );
                let block_header = block.header();
                let da_commitment_hash = block_header.da_commitments_hash();
                let manifest_roots: BTreeMap<DataSpaceId, [u8; 32]> = state_block
                    .axt_policy_snapshot()
                    .entries
                    .iter()
                    .filter_map(|entry| {
                        if entry.policy.manifest_root.iter().all(|byte| *byte == 0) {
                            None
                        } else {
                            Some((entry.dsid, entry.policy.manifest_root))
                        }
                    })
                    .collect();
                let mut lane_relay_envelopes = lane_relay_envelopes_for_block(
                    &block_header,
                    da_commitment_hash,
                    &lane_settlement_commitments,
                    &lane_summaries,
                );
                attach_manifest_roots_to_relays(&mut lane_relay_envelopes, &manifest_roots);
                crate::sumeragi::status::set_lane_relay_envelopes(lane_relay_envelopes);
            }

            let mut tx_results: Vec<Option<TransactionResultInner>> = vec![None; n];
            let mut record_result = |idx: usize, result: TransactionResultInner| {
                debug_assert!(
                    idx < tx_results.len(),
                    "record_result index {} out of bounds (len={})",
                    idx,
                    tx_results.len()
                );
                tx_results[idx] = Some(result);
            };
            if let Some(start) = apply_setup_start {
                apply_setup_ms = to_ms(start.elapsed());
            }
            #[cfg(feature = "telemetry")]
            let t_apply_start = Instant::now();
            #[cfg(feature = "telemetry")]
            let mut layer_widths_global: Vec<u64> = Vec::new();

            // Helper removed to avoid borrow checker conflicts; inline application below.
            // When `pipeline.gpu_key_bucket` is enabled we first attempt to build per-key
            // inverted indices via the CUDA bitonic sorter (with an identical CPU fallback).
            // Apply overlays either via parallel-detached path (per conflict-free layer)
            // or via the sequential path based on the `pipeline.parallel_apply` knob.
            if state_block.pipeline.parallel_apply {
                use rayon::prelude::*;

                use crate::state::{DetachedMergeContext, DetachedStateTransactionDelta};

                #[derive(Clone)]
                struct PreparedEntry {
                    idx: usize,
                    authority: AccountId,
                    chunk_size: usize,
                    _log_only: bool,
                }

                // Compute conflict-free layers per DSF component and merge deterministically.
                let layer_build_start = timings.as_ref().map(|_| Instant::now());
                let layers =
                    conflict_free_component_layers(&components, &row_offsets, &cols, &call_hashes)
                        .unwrap_or_else(|| {
                            let global_order =
                                schedule_wave_global(&row_offsets, &cols, &indeg, &call_hashes);
                            global_order.into_iter().map(|idx| vec![idx]).collect()
                        });
                if let Some(start) = layer_build_start {
                    apply_layer_build_ms = to_ms(start.elapsed());
                }
                // Global quarantine collection executed after normal lane
                let mut quarantine_seq: Vec<usize> = Vec::new();
                for layer in layers {
                    // Split current layer into normal/quarantine subsets deterministically
                    let mut layer_norm: Vec<usize> = Vec::new();
                    let mut layer_quar: Vec<usize> = Vec::new();
                    for &idx in &layer {
                        if let Some(reason) = stateless_rejections[idx].take() {
                            record_result(idx, Err(reason));
                            continue;
                        }
                        if idx < is_quarantine.len() && is_quarantine[idx] {
                            layer_quar.push(idx);
                        } else {
                            layer_norm.push(idx);
                        }
                    }
                    quarantine_seq.extend(layer_quar.into_iter());
                    #[cfg(feature = "telemetry")]
                    {
                        layer_widths_global.push(layer.len() as u64);
                    }
                    {
                        let mut per_lane_widths: BTreeMap<LaneId, u64> = BTreeMap::new();
                        for &idx in &layer {
                            let lane_id = routing_decisions[idx].lane_id;
                            per_lane_widths
                                .entry(lane_id)
                                .and_modify(|count| *count = count.saturating_add(1))
                                .or_insert(1);
                        }
                        for (lane_id, width) in per_lane_widths {
                            let summary = lane_summaries.entry(lane_id).or_default();
                            summary.peak_layer_width = summary.peak_layer_width.max(width);
                            summary.layer_widths.push(width);
                        }
                    }
                    let layer_prep_start = timings.as_ref().map(|_| Instant::now());
                    #[cfg(feature = "telemetry")]
                    let t_layer_prep = Instant::now();
                    let prepared_or_err: Vec<
                        Result<
                            PreparedEntry,
                            (
                                usize,
                                iroha_data_model::transaction::error::TransactionRejectionReason,
                            ),
                        >,
                    > = if workers > 1 {
                        if let Some(pool) = pool.as_ref() {
                            pool.install(|| {
                                layer_norm
                                    .par_iter()
                                    .map(|&idx| {
                                        let tx = txs[idx];
                                        let overlay = match overlays[idx].as_ref() {
                                            Ok(o) => Arc::clone(o),
                                            Err(err) => {
                                                let rej = map_overlay_error(err);
                                                return Err((idx, rej));
                                            }
                                        };
                                        let max_instrs =
                                            state_block.pipeline.overlay_max_instructions;
                                        if max_instrs > 0
                                            && overlay.instruction_count() > max_instrs
                                        {
                                            return Err((
                                                idx,
                                                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                                    iroha_data_model::ValidationFail::NotPermitted(format!(
                                                        "overlay exceeds max instructions: {} > {max_instrs}",
                                                        overlay.instruction_count()
                                                    )),
                                                ),
                                            ));
                                        }
                                        let max_bytes = state_block.pipeline.overlay_max_bytes;
                                        let byte_size = overlay.byte_size() as u64;
                                        if max_bytes > 0 && byte_size > max_bytes {
                                            return Err((
                                                idx,
                                                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                                    iroha_data_model::ValidationFail::NotPermitted(format!(
                                                        "overlay exceeds max bytes: {byte_size} > {max_bytes}"
                                                    )),
                                                ),
                                            ));
                                        }
                                        Ok(PreparedEntry {
                                            idx,
                                            authority: tx.authority().clone(),
                                            chunk_size: state_block
                                                .pipeline
                                                .overlay_chunk_instructions
                                                .max(1),
                                            _log_only: false,
                                        })
                                    })
                                    .collect()
                            })
                        } else {
                            layer_norm
                                .par_iter()
                                .map(|&idx| {
                                    let tx = txs[idx];
                                    let overlay = match overlays[idx].as_ref() {
                                        Ok(o) => Arc::clone(o),
                                        Err(err) => {
                                            let rej = map_overlay_error(err);
                                            return Err((idx, rej));
                                        }
                                    };
                                    let max_instrs = state_block.pipeline.overlay_max_instructions;
                                    if max_instrs > 0 && overlay.instruction_count() > max_instrs {
                                        return Err((
                                            idx,
                                            iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                                iroha_data_model::ValidationFail::NotPermitted(format!(
                                                    "overlay exceeds max instructions: {} > {max_instrs}",
                                                    overlay.instruction_count()
                                                )),
                                            ),
                                        ));
                                    }
                                    let max_bytes = state_block.pipeline.overlay_max_bytes;
                                    let byte_size = overlay.byte_size() as u64;
                                    if max_bytes > 0 && byte_size > max_bytes {
                                        return Err((
                                            idx,
                                            iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                                iroha_data_model::ValidationFail::NotPermitted(format!(
                                                    "overlay exceeds max bytes: {byte_size} > {max_bytes}"
                                                )),
                                            ),
                                        ));
                                    }
                                    Ok(PreparedEntry {
                                        idx,
                                        authority: tx.authority().clone(),
                                        chunk_size: state_block
                                            .pipeline
                                            .overlay_chunk_instructions
                                            .max(1),
                                        _log_only: false,
                                    })
                                })
                                .collect()
                        }
                    } else {
                        layer_norm.iter().map(|&idx| {
                            let tx = txs[idx];
                            let overlay = match overlays[idx].as_ref() {
                                Ok(o) => Arc::clone(o),
                                Err(err) => {
                                    let rej = map_overlay_error(err);
                                    return Err((idx, rej));
                                }
                            };
                            let max_instrs = state_block.pipeline.overlay_max_instructions;
                            if max_instrs > 0 && overlay.instruction_count() > max_instrs {
                                return Err((idx, iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                    iroha_data_model::ValidationFail::NotPermitted(format!("overlay exceeds max instructions: {} > {max_instrs}", overlay.instruction_count())),
                                )));
                            }
                            let max_bytes = state_block.pipeline.overlay_max_bytes;
                            let byte_size = overlay.byte_size() as u64;
                            if max_bytes > 0 && byte_size > max_bytes {
                                return Err((idx, iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                    iroha_data_model::ValidationFail::NotPermitted(format!("overlay exceeds max bytes: {byte_size} > {max_bytes}")),
                                )));
                            }
                            Ok(PreparedEntry {
                                idx,
                                authority: tx.authority().clone(),
                                chunk_size: state_block.pipeline.overlay_chunk_instructions.max(1),
                                _log_only: false,
                            })
                        }).collect()
                    };
                    #[cfg(feature = "telemetry")]
                    {
                        let aggregate_lane = state_block.nexus.routing_policy.default_lane;
                        let elapsed_ms = t_layer_prep.elapsed().as_secs_f64() * 1_000.0;
                        state_block.metrics().observe_pipeline_stage_ms(
                            aggregate_lane,
                            "layers_prep",
                            elapsed_ms,
                        );
                        state_block
                            .metrics()
                            .observe_amx_prepare_ms(aggregate_lane, elapsed_ms);
                    }
                    if let (Some(_), Some(start)) = (timings.as_ref(), layer_prep_start) {
                        apply_prep_ms = apply_prep_ms.saturating_add(to_ms(start.elapsed()));
                    }

                    let mut prepared: Vec<PreparedEntry> = Vec::new();
                    for item in prepared_or_err {
                        match item {
                            Ok(p) => {
                                let lane_id = routing_decisions[p.idx].lane_id;
                                let summary = lane_summaries.entry(lane_id).or_default();
                                summary.detached_prepared =
                                    summary.detached_prepared.saturating_add(1);
                                prepared.push(p);
                            }
                            Err((idx, reason)) => {
                                record_amx_abort(state_block, idx, "prepare");
                                record_result(idx, Err(reason));
                            }
                        }
                    }
                    prepared.sort_by_key(|p| (call_hashes[p.idx], p.idx));

                    let layer_exec_start = timings.as_ref().map(|_| Instant::now());
                    #[cfg(feature = "telemetry")]
                    let t_layer_exec = Instant::now();
                    // Deterministically prefetch authority/account state and warm the first
                    // instruction chunk for each overlay to reduce merge stalls.
                    let mut accounts_to_prefetch: BTreeSet<AccountId> = BTreeSet::new();
                    for entry in &prepared {
                        accounts_to_prefetch.insert(entry.authority.clone());
                        if let Some(access_set) = access.get(entry.idx) {
                            for key in access_set.read_keys.iter() {
                                if let Some(account) = parse_account_from_access_key(
                                    &state_block.world,
                                    &state_block.nexus.dataspace_catalog,
                                    key,
                                ) {
                                    accounts_to_prefetch.insert(account);
                                }
                            }
                            for key in access_set.write_keys.iter() {
                                if let Some(account) = parse_account_from_access_key(
                                    &state_block.world,
                                    &state_block.nexus.dataspace_catalog,
                                    key,
                                ) {
                                    accounts_to_prefetch.insert(account);
                                }
                            }
                        }
                        if let Some(Ok(overlay)) = overlays.get(entry.idx) {
                            let _ = warm_overlay_chunk(overlay, entry.chunk_size);
                        }
                    }
                    for account_id in accounts_to_prefetch {
                        let _ = prefetch_account_stores(state_block, &account_id);
                    }
                    let detached_is_genesis =
                        block.header().is_genesis() && state_block.block_hashes.is_empty();
                    let nft_metadata_target = |instruction: &InstructionBox| {
                        instruction
                            .as_any()
                            .downcast_ref::<SetKeyValueBox>()
                            .and_then(|kv| match kv {
                                SetKeyValueBox::Nft(set) => Some(set.object.clone()),
                                _ => None,
                            })
                            .or_else(|| {
                                instruction
                                        .as_any()
                                        .downcast_ref::<iroha_data_model::isi::SetKeyValue<
                                            iroha_data_model::nft::Nft,
                                        >>()
                                        .map(|set| set.object.clone())
                            })
                            .or_else(|| {
                                instruction
                                    .as_any()
                                    .downcast_ref::<RemoveKeyValueBox>()
                                    .and_then(|rm| match rm {
                                        RemoveKeyValueBox::Nft(rm) => Some(rm.object.clone()),
                                        _ => None,
                                    })
                            })
                            .or_else(|| {
                                instruction
                                    .as_any()
                                    .downcast_ref::<iroha_data_model::isi::RemoveKeyValue<
                                        iroha_data_model::nft::Nft,
                                    >>()
                                    .map(|rm| rm.object.clone())
                            })
                    };
                    let account_metadata_target = |instruction: &InstructionBox| {
                        instruction
                            .as_any()
                            .downcast_ref::<SetKeyValueBox>()
                            .and_then(|kv| match kv {
                                SetKeyValueBox::Account(set) => Some(set.object.clone()),
                                _ => None,
                            })
                            .or_else(|| {
                                instruction
                                    .as_any()
                                    .downcast_ref::<iroha_data_model::isi::SetKeyValue<
                                        iroha_data_model::account::Account,
                                    >>()
                                    .map(|set| set.object.clone())
                            })
                            .or_else(|| {
                                instruction
                                    .as_any()
                                    .downcast_ref::<RemoveKeyValueBox>()
                                    .and_then(|rm| match rm {
                                        RemoveKeyValueBox::Account(rm) => Some(rm.object.clone()),
                                        _ => None,
                                    })
                            })
                            .or_else(|| {
                                instruction
                                    .as_any()
                                    .downcast_ref::<iroha_data_model::isi::RemoveKeyValue<
                                        iroha_data_model::account::Account,
                                    >>()
                                    .map(|rm| rm.object.clone())
                            })
                    };
                    let domain_transfer_target = |instruction: &InstructionBox| {
                        instruction
                            .as_any()
                            .downcast_ref::<TransferBox>()
                            .and_then(|transfer| match transfer {
                                TransferBox::Domain(transfer) => Some(transfer.clone()),
                                _ => None,
                            })
                            .or_else(|| {
                                instruction
                                    .as_any()
                                    .downcast_ref::<iroha_data_model::isi::Transfer<
                                        iroha_data_model::account::Account,
                                        DomainId,
                                        iroha_data_model::account::Account,
                                    >>()
                                    .cloned()
                            })
                    };
                    let asset_definition_transfer_target = |instruction: &InstructionBox| {
                        instruction
                            .as_any()
                            .downcast_ref::<TransferBox>()
                            .and_then(|transfer| match transfer {
                                TransferBox::AssetDefinition(transfer) => Some(transfer.clone()),
                                _ => None,
                            })
                            .or_else(|| {
                                instruction
                                    .as_any()
                                    .downcast_ref::<iroha_data_model::isi::Transfer<
                                        iroha_data_model::account::Account,
                                        AssetDefinitionId,
                                        iroha_data_model::account::Account,
                                    >>()
                                    .cloned()
                            })
                    };
                    let nft_transfer_target = |instruction: &InstructionBox| {
                        instruction
                            .as_any()
                            .downcast_ref::<TransferBox>()
                            .and_then(|transfer| match transfer {
                                TransferBox::Nft(transfer) => Some(transfer.clone()),
                                _ => None,
                            })
                            .or_else(|| {
                                instruction
                                    .as_any()
                                    .downcast_ref::<iroha_data_model::isi::Transfer<
                                        iroha_data_model::account::Account,
                                        iroha_data_model::nft::NftId,
                                        iroha_data_model::account::Account,
                                    >>()
                                    .cloned()
                            })
                    };
                    let asset_transfer_target = |instruction: &InstructionBox| {
                        instruction
                            .as_any()
                            .downcast_ref::<TransferBox>()
                            .and_then(|transfer| match transfer {
                                TransferBox::Asset(transfer) => Some(transfer.clone()),
                                _ => None,
                            })
                            .or_else(|| {
                                instruction
                                    .as_any()
                                    .downcast_ref::<iroha_data_model::isi::Transfer<
                                        iroha_data_model::asset::Asset,
                                        iroha_primitives::numeric::Numeric,
                                        iroha_data_model::account::Account,
                                    >>()
                                    .cloned()
                            })
                    };
                    let eval_detached = |p: &PreparedEntry| {
                        if let Some(Ok(ovl)) = overlays.get(p.idx) {
                            if matches!(
                                &*state_block.world.executor,
                                crate::executor::Executor::UserProvided(_)
                            ) {
                                return (p.idx, None, Some(DetachedFallbackReason::UserExecutor));
                            }
                            if ovl.has_durable_state_changes() {
                                return (p.idx, None, Some(DetachedFallbackReason::DurableState));
                            }
                            let mut delta = DetachedStateTransactionDelta::default();
                            let mut unsupported = false;
                            let mut reject: Option<TransactionRejectionReason> = None;
                            for instr in ovl.instructions() {
                                if let Some(transfer) = asset_transfer_target(instr) {
                                    if detached_is_genesis
                                        || ovl.instruction_count() != 1
                                        || transfer.source().account() != &p.authority
                                    {
                                        unsupported = true;
                                        break;
                                    }
                                }
                                if !detached_is_genesis {
                                    if let Some(nft_id) = nft_metadata_target(instr) {
                                        match delta.can_modify_nft_metadata(
                                            &state_block.world,
                                            &p.authority,
                                            &nft_id,
                                        ) {
                                            Ok(true) => {}
                                            Ok(false) => {
                                                reject = Some(
                                                    TransactionRejectionReason::Validation(
                                                        iroha_data_model::ValidationFail::NotPermitted(
                                                            "Can't modify NFT from domain owned by another account"
                                                                .to_owned(),
                                                        ),
                                                    ),
                                                );
                                                break;
                                            }
                                            Err(err) => {
                                                reject = Some(
                                                    TransactionRejectionReason::Validation(err),
                                                );
                                                break;
                                            }
                                        }
                                    }
                                    if let Some(account_id) = account_metadata_target(instr) {
                                        match delta.can_modify_account_metadata(
                                            &state_block.world,
                                            &p.authority,
                                            &account_id,
                                        ) {
                                            Ok(true) => {}
                                            Ok(false) => {
                                                reject = Some(
                                                    TransactionRejectionReason::Validation(
                                                        iroha_data_model::ValidationFail::NotPermitted(
                                                            "Can't set value to the metadata of another account"
                                                                .to_owned(),
                                                        ),
                                                    ),
                                                );
                                                break;
                                            }
                                            Err(err) => {
                                                reject = Some(
                                                    TransactionRejectionReason::Validation(err),
                                                );
                                                break;
                                            }
                                        }
                                    }
                                    if let Some(transfer) = domain_transfer_target(instr) {
                                        match delta.can_transfer_domain(
                                            &state_block.world,
                                            &p.authority,
                                            &transfer,
                                        ) {
                                            Ok(true) => {}
                                            Ok(false) => {
                                                reject = Some(
                                                    TransactionRejectionReason::Validation(
                                                        iroha_data_model::ValidationFail::NotPermitted(
                                                            "Can't transfer domain of another account"
                                                                .to_owned(),
                                                        ),
                                                    ),
                                                );
                                                break;
                                            }
                                            Err(err) => {
                                                reject = Some(
                                                    TransactionRejectionReason::Validation(err),
                                                );
                                                break;
                                            }
                                        }
                                    }
                                    if let Some(transfer) = asset_definition_transfer_target(instr)
                                    {
                                        match delta.can_transfer_asset_definition(
                                            &state_block.world,
                                            &p.authority,
                                            &transfer,
                                        ) {
                                            Ok(true) => {}
                                            Ok(false) => {
                                                reject = Some(
                                                    TransactionRejectionReason::Validation(
                                                        iroha_data_model::ValidationFail::NotPermitted(
                                                            "Can't transfer asset definition of another account"
                                                                .to_owned(),
                                                        ),
                                                    ),
                                                );
                                                break;
                                            }
                                            Err(err) => {
                                                reject = Some(
                                                    TransactionRejectionReason::Validation(err),
                                                );
                                                break;
                                            }
                                        }
                                    }
                                    if let Some(transfer) = nft_transfer_target(instr) {
                                        match delta.can_transfer_nft(
                                            &state_block.world,
                                            &p.authority,
                                            &transfer,
                                        ) {
                                            Ok(true) => {}
                                            Ok(false) => {
                                                reject = Some(
                                                    TransactionRejectionReason::Validation(
                                                        iroha_data_model::ValidationFail::NotPermitted(
                                                            "Can't transfer NFT of another account"
                                                                .to_owned(),
                                                        ),
                                                    ),
                                                );
                                                break;
                                            }
                                            Err(err) => {
                                                reject = Some(
                                                    TransactionRejectionReason::Validation(err),
                                                );
                                                break;
                                            }
                                        }
                                    }
                                }
                                match crate::executor::execute_instruction_detached(
                                    &p.authority,
                                    instr,
                                    &mut delta,
                                ) {
                                    Ok(()) => {}
                                    Err(iroha_data_model::ValidationFail::InternalError(_)) => {
                                        unsupported = true;
                                        break;
                                    }
                                    Err(e) => {
                                        reject = Some(TransactionRejectionReason::Validation(e));
                                        break;
                                    }
                                }
                            }
                            reject.map_or_else(
                                || {
                                    if unsupported {
                                        (
                                            p.idx,
                                            None,
                                            Some(DetachedFallbackReason::UnsupportedInstruction),
                                        )
                                    } else if fee_postprocessing_required[p.idx]
                                        && (data_triggers_enabled
                                            || !delta.supports_detached_fee_postprocessing())
                                    {
                                        (
                                            p.idx,
                                            None,
                                            Some(DetachedFallbackReason::FeePostprocessing),
                                        )
                                    } else {
                                        (p.idx, Some(Ok(delta)), None)
                                    }
                                },
                                |r| {
                                    (
                                        p.idx,
                                        Some(Err(r)),
                                        Some(DetachedFallbackReason::RejectedEval),
                                    )
                                },
                            )
                        } else {
                            (p.idx, None, Some(DetachedFallbackReason::OverlayError))
                        }
                    };
                    let deltas_vec: Vec<(
                        usize,
                        Option<Result<DetachedStateTransactionDelta, TransactionRejectionReason>>,
                        Option<DetachedFallbackReason>,
                    )> = if workers > 1 {
                        if let Some(pool) = pool.as_ref() {
                            pool.install(|| prepared.par_iter().map(eval_detached).collect())
                        } else {
                            prepared.par_iter().map(eval_detached).collect()
                        }
                    } else {
                        prepared.iter().map(eval_detached).collect()
                    };
                    // Optimize lookups during merge: Vec<Option<..>> indexed by tx index
                    let mut deltas: Vec<
                        Option<Result<DetachedStateTransactionDelta, TransactionRejectionReason>>,
                    > = vec![None; n];
                    let mut detached_fallback_reasons: Vec<Option<DetachedFallbackReason>> =
                        vec![None; n];
                    for (idx, maybe, reason) in deltas_vec {
                        if idx < n {
                            deltas[idx] = maybe;
                            detached_fallback_reasons[idx] = reason;
                        }
                    }
                    #[cfg(feature = "telemetry")]
                    {
                        let aggregate_lane = state_block.nexus.routing_policy.default_lane;
                        let elapsed_ms = t_layer_exec.elapsed().as_secs_f64() * 1_000.0;
                        let metrics = state_block.metrics();
                        metrics.observe_pipeline_stage_ms(
                            aggregate_lane,
                            "layers_exec",
                            elapsed_ms,
                        );
                        metrics.observe_ivm_exec_ms(aggregate_lane, elapsed_ms);
                    }
                    if let (Some(_), Some(start)) = (timings.as_ref(), layer_exec_start) {
                        apply_detached_ms =
                            apply_detached_ms.saturating_add(to_ms(start.elapsed()));
                    }
                    #[cfg(feature = "telemetry")]
                    let t_layer_merge = Instant::now();
                    let layer_merge_start = timings.as_ref().map(|_| Instant::now());
                    let mut layer_fallback_ms = 0u64;
                    // Detached metadata merges rely on DetachedStateTransactionDelta's SoA + name
                    // interning layout to avoid redundant map probes while preserving determinism.

                    let mut apply_overlay_sequential =
                        |state_block_mut: &mut StateBlock<'_>,
                         lane_summaries_mut: &mut BTreeMap<LaneId, LaneSummary>,
                         idx: usize|
                         -> TransactionResultInner {
                            let fallback_start = timings.as_ref().map(|_| Instant::now());
                            let lane_id = routing_decisions[idx].lane_id;
                            {
                                let summary = lane_summaries_mut.entry(lane_id).or_default();
                                summary.detached_fallback =
                                    summary.detached_fallback.saturating_add(1);
                                if let Some(reason) = detached_fallback_reasons[idx] {
                                    summary.detached_fallback_reasons.add(reason);
                                }
                            }
                            let tx = txs[idx];
                            let hash = prepared_txs[idx].metadata.entrypoint_hash;
                            let overlay = match overlays[idx].as_ref() {
                                Ok(ovl) => Arc::clone(ovl),
                                Err(err) => {
                                    record_amx_abort(state_block_mut, idx, "prepare");
                                    let rej = map_overlay_error(err);
                                    return Err(rej);
                                }
                            };
                            if let Some(aset) = access.get(idx) {
                                for k in aset
                                    .read_keys
                                    .iter()
                                    .filter(|k| !aset.write_keys.contains(*k))
                                {
                                    crate::sumeragi::witness::record_read_from_access_key(
                                        state_block_mut,
                                        k,
                                    );
                                }
                            }
                            let max_instrs = state_block_mut.pipeline.overlay_max_instructions;
                            if max_instrs > 0 && overlay.instruction_count() > max_instrs {
                                record_amx_abort(state_block_mut, idx, "prepare");
                                return Err(TransactionRejectionReason::Validation(
                                    iroha_data_model::ValidationFail::NotPermitted(format!(
                                        "overlay exceeds max instructions: {} > {}",
                                        overlay.instruction_count(),
                                        max_instrs
                                    )),
                                ));
                            }
                            let max_bytes = state_block_mut.pipeline.overlay_max_bytes;
                            let byte_size = overlay.byte_size() as u64;
                            if max_bytes > 0 && byte_size > max_bytes {
                                record_amx_abort(state_block_mut, idx, "prepare");
                                return Err(TransactionRejectionReason::Validation(
                                    iroha_data_model::ValidationFail::NotPermitted(format!(
                                        "overlay exceeds max bytes: {byte_size} > {max_bytes}"
                                    )),
                                ));
                            }
                            let chunk_size =
                                state_block_mut.pipeline.overlay_chunk_instructions.max(1);
                            let mut state_tx = state_block_mut.transaction();
                            state_tx.current_lane_id = Some(routing_decisions[idx].lane_id);
                            state_tx.current_dataspace_id =
                                Some(routing_decisions[idx].dataspace_id);
                            state_tx.world.current_dataspace_id =
                                Some(routing_decisions[idx].dataspace_id);
                            let authority = tx.authority().clone();
                            state_tx.tx_call_hash = Some(iroha_crypto::Hash::from(hash));
                            state_tx.current_tx_hash = Some(prepared_txs[idx].metadata.signed_hash);
                            if missing_authority_requires_rejection(
                                &state_tx,
                                tx,
                                &authority,
                                overlay.instruction_count(),
                                block.header().is_genesis(),
                            ) {
                                return Err(TransactionRejectionReason::AccountDoesNotExist(
                                    iroha_data_model::query::error::FindError::Account(
                                        authority.clone(),
                                    ),
                                ));
                            }
                            let admission = validate_block_transaction_admission(
                                &mut state_tx,
                                tx,
                                routing_decisions[idx],
                            )?;
                            let executor = state_tx.world.executor.clone();
                            if let Err(err) = configure_executor_fuel_budget(
                                &executor,
                                &mut state_tx,
                                tx.metadata(),
                            ) {
                                return Err(TransactionRejectionReason::Validation(err));
                            }
                            let result = match overlay.apply_with_chunk(
                                &mut state_tx,
                                &authority,
                                chunk_size,
                            ) {
                                Err(e) => {
                                    let rejection_reason =
                                        TransactionRejectionReason::Validation(e);
                                    drop(state_tx);
                                    match charge_rejected_overlay_fees(
                                        state_block_mut,
                                        tx,
                                        &authority,
                                        overlay.as_ref(),
                                        prepared_txs[idx].metadata.encoded_len,
                                        routing_decisions[idx].lane_id,
                                        routing_decisions[idx].dataspace_id,
                                        &rejection_reason,
                                    ) {
                                        Ok(()) => Err(rejection_reason),
                                        Err(err) => Err(err),
                                    }
                                }
                                Ok(()) => {
                                    if let Err(err) =
                                        charge_fees_for_applied_overlay_with_encoded_len(
                                            &mut state_tx,
                                            &authority,
                                            tx,
                                            overlay.as_ref(),
                                            prepared_txs[idx].metadata.encoded_len,
                                        )
                                    {
                                        Err(TransactionRejectionReason::Validation(err))
                                    } else {
                                        match state_tx.execute_data_triggers_dfs(&authority) {
                                            Err(err) => {
                                                drop(state_tx);
                                                match charge_rejected_overlay_fees(
                                                    state_block_mut,
                                                    tx,
                                                    &authority,
                                                    overlay.as_ref(),
                                                    prepared_txs[idx].metadata.encoded_len,
                                                    routing_decisions[idx].lane_id,
                                                    routing_decisions[idx].dataspace_id,
                                                    &err,
                                                ) {
                                                    Ok(()) => Err(err),
                                                    Err(fee_err) => Err(fee_err),
                                                }
                                            }
                                            Ok(trigger_sequence) => {
                                                commit_stateful_admission_sequence(
                                                    &mut state_tx,
                                                    &admission,
                                                );
                                                state_tx.apply();
                                                Ok(trigger_sequence)
                                            }
                                        }
                                    }
                                }
                            };
                            if let Err(reason) = &result {
                                iroha_logger::debug!(
                                    tx=%hash,
                                    block=%block.hash(),
                                    reason=?reason,
                                    "Transaction rejected"
                                );
                                if debug_trace_tx_eval {
                                    eprintln!(
                                        "[core-eval] reject(fallback) hash={} ts={} auth={}",
                                        hash,
                                        tx.creation_time().as_millis(),
                                        authority,
                                    );
                                }
                            } else if debug_trace_tx_eval {
                                eprintln!(
                                    "[core-eval] ok(fallback) hash={} ts={} auth={}",
                                    hash,
                                    tx.creation_time().as_millis(),
                                    authority,
                                );
                            }
                            if let Some(start) = fallback_start {
                                layer_fallback_ms =
                                    layer_fallback_ms.saturating_add(to_ms(start.elapsed()));
                            }
                            result
                        };

                    let simple_transfer_batch = !prepared.is_empty()
                        && {
                            let precheck_tx = state_block.transaction();
                            prepared.iter().all(|p| {
                                !fee_postprocessing_required[p.idx]
                                    && matches!(
                                        deltas.get(p.idx),
                                        Some(Some(Ok(delta)))
                                            if delta.supports_uncontrolled_single_transfer_batch(&precheck_tx)
                                    )
                            })
                        };

                    if simple_transfer_batch {
                        const SIMPLE_TRANSFER_BATCH_CHUNK: usize = 4_096;

                        for prepared_chunk in prepared.chunks(SIMPLE_TRANSFER_BATCH_CHUNK) {
                            for p in prepared_chunk {
                                if let Some(aset) = access.get(p.idx) {
                                    for k in aset
                                        .read_keys
                                        .iter()
                                        .filter(|k| !aset.write_keys.contains(*k))
                                    {
                                        crate::sumeragi::witness::record_read_from_access_key(
                                            state_block,
                                            k,
                                        );
                                    }
                                }
                            }

                            let mut state_tx = state_block.transaction();
                            let mut batch_successes = 0usize;
                            let mut aborts: Vec<(usize, &'static str)> = Vec::new();

                            for p in prepared_chunk {
                                let tx = txs[p.idx];
                                let hash = prepared_txs[p.idx].metadata.entrypoint_hash;
                                state_tx.current_lane_id = Some(routing_decisions[p.idx].lane_id);
                                state_tx.current_dataspace_id =
                                    Some(routing_decisions[p.idx].dataspace_id);
                                state_tx.world.current_dataspace_id =
                                    Some(routing_decisions[p.idx].dataspace_id);
                                state_tx.tx_call_hash = Some(iroha_crypto::Hash::from(hash));
                                state_tx.current_tx_hash =
                                    Some(prepared_txs[p.idx].metadata.signed_hash);

                                if missing_authority_requires_rejection(
                                    &state_tx,
                                    tx,
                                    &p.authority,
                                    tx.instructions().instruction_count() as usize,
                                    block.header().is_genesis(),
                                ) {
                                    aborts.push((p.idx, "commit"));
                                    record_result(
                                        p.idx,
                                        Err(TransactionRejectionReason::AccountDoesNotExist(
                                            iroha_data_model::query::error::FindError::Account(
                                                p.authority.clone(),
                                            ),
                                        )),
                                    );
                                    if debug_trace_tx_eval {
                                        let ts = tx.creation_time().as_millis();
                                        eprintln!(
                                            "[core-eval] reject(no-authority) hash={} ts={} auth={}",
                                            hash, ts, p.authority,
                                        );
                                    }
                                    continue;
                                }

                                let admission = match validate_block_transaction_admission(
                                    &mut state_tx,
                                    tx,
                                    routing_decisions[p.idx],
                                ) {
                                    Ok(admission) => admission,
                                    Err(reason) => {
                                        aborts.push((p.idx, "commit"));
                                        record_result(p.idx, Err(reason));
                                        if debug_trace_tx_eval {
                                            let ts = tx.creation_time().as_millis();
                                            eprintln!(
                                                "[core-eval] reject(admission) hash={} ts={} auth={}",
                                                hash, ts, p.authority,
                                            );
                                        }
                                        continue;
                                    }
                                };

                                let result = match deltas.get(p.idx) {
                                    Some(Some(Ok(delta))) => delta
                                        .merge_uncontrolled_single_transfer_into_transaction(
                                            &mut state_tx,
                                            &p.authority,
                                        )
                                        .unwrap_or_else(|| {
                                            Err(TransactionRejectionReason::Validation(
                                                iroha_data_model::ValidationFail::NotPermitted(
                                                    "detached transfer is not eligible for batch merge"
                                                        .to_owned(),
                                                ),
                                            ))
                                        }),
                                    _ => Err(TransactionRejectionReason::Validation(
                                        iroha_data_model::ValidationFail::NotPermitted(
                                            "detached transfer batch lost its prechecked delta"
                                                .to_owned(),
                                        ),
                                    )),
                                };

                                match result {
                                    Ok(trigger_sequence) => {
                                        commit_stateful_admission_sequence(
                                            &mut state_tx,
                                            &admission,
                                        );
                                        batch_successes = batch_successes.saturating_add(1);
                                        record_result(p.idx, Ok(trigger_sequence));
                                        let lane_id = routing_decisions[p.idx].lane_id;
                                        let summary = lane_summaries.entry(lane_id).or_default();
                                        summary.detached_merged =
                                            summary.detached_merged.saturating_add(1);
                                        if debug_trace_tx_eval {
                                            let ts = tx.creation_time().as_millis();
                                            eprintln!(
                                                "[core-eval] ok(prepared-merge) hash={} ts={} auth={}",
                                                hash, ts, p.authority,
                                            );
                                        }
                                    }
                                    Err(reason) => {
                                        aborts.push((p.idx, "commit"));
                                        record_result(p.idx, Err(reason));
                                        if debug_trace_tx_eval {
                                            let ts = tx.creation_time().as_millis();
                                            eprintln!(
                                                "[core-eval] reject(prepared-merge) hash={} ts={} auth={}",
                                                hash, ts, p.authority,
                                            );
                                        }
                                    }
                                }
                            }

                            if batch_successes > 0 {
                                // Each transfer transcript already carries the transaction hash
                                // active when it was recorded. Clear the overlay hash so apply()
                                // flushes batched transcripts into their per-transaction buckets.
                                state_tx.tx_call_hash = None;
                                state_tx.apply();
                                state_block
                                    .add_committed_fragments(batch_successes.saturating_sub(1));
                            } else {
                                drop(state_tx);
                            }
                            for (idx, stage) in aborts {
                                record_amx_abort(state_block, idx, stage);
                            }
                        }
                    } else {
                        for p in prepared {
                            match deltas.get(p.idx).cloned().flatten() {
                                Some(Ok(delta)) => {
                                    // Record pure reads (read_keys minus write_keys) before applying this tx
                                    if let Some(aset) = access.get(p.idx) {
                                        for k in aset
                                            .read_keys
                                            .iter()
                                            .filter(|k| !aset.write_keys.contains(*k))
                                        {
                                            crate::sumeragi::witness::record_read_from_access_key(
                                                state_block,
                                                k,
                                            );
                                        }
                                    }
                                    let tx = txs[p.idx];
                                    let hash = prepared_txs[p.idx].metadata.entrypoint_hash;
                                    let mut state_tx = state_block.transaction();
                                    state_tx.current_lane_id =
                                        Some(routing_decisions[p.idx].lane_id);
                                    state_tx.current_dataspace_id =
                                        Some(routing_decisions[p.idx].dataspace_id);
                                    state_tx.world.current_dataspace_id =
                                        Some(routing_decisions[p.idx].dataspace_id);
                                    state_tx.tx_call_hash = Some(iroha_crypto::Hash::from(hash));
                                    state_tx.current_tx_hash =
                                        Some(prepared_txs[p.idx].metadata.signed_hash);
                                    if missing_authority_requires_rejection(
                                        &state_tx,
                                        tx,
                                        &p.authority,
                                        tx.instructions().instruction_count() as usize,
                                        block.header().is_genesis(),
                                    ) {
                                        drop(state_tx);
                                        record_amx_abort(state_block, p.idx, "commit");
                                        record_result(
                                            p.idx,
                                            Err(TransactionRejectionReason::AccountDoesNotExist(
                                                iroha_data_model::query::error::FindError::Account(
                                                    p.authority.clone(),
                                                ),
                                            )),
                                        );
                                        if debug_trace_tx_eval {
                                            let ts = tx.creation_time().as_millis();
                                            eprintln!(
                                                "[core-eval] reject(no-authority) hash={} ts={} auth={}",
                                                hash, ts, p.authority,
                                            );
                                        }
                                        continue;
                                    }
                                    let admission = match validate_block_transaction_admission(
                                        &mut state_tx,
                                        tx,
                                        routing_decisions[p.idx],
                                    ) {
                                        Ok(admission) => admission,
                                        Err(reason) => {
                                            drop(state_tx);
                                            record_amx_abort(state_block, p.idx, "commit");
                                            record_result(p.idx, Err(reason));
                                            if debug_trace_tx_eval {
                                                let ts = tx.creation_time().as_millis();
                                                eprintln!(
                                                    "[core-eval] reject(admission) hash={} ts={} auth={}",
                                                    hash, ts, p.authority,
                                                );
                                            }
                                            continue;
                                        }
                                    };
                                    let single_transfer_result = if fee_postprocessing_required
                                        [p.idx]
                                    {
                                        delta
                                                .merge_single_transfer_effects_into_transaction(
                                                    &mut state_tx,
                                                    &p.authority,
                                                )
                                                .map(|result| {
                                                    result.and_then(|()| {
                                                        let overlay = overlays[p.idx]
                                                            .as_ref()
                                                            .map_err(map_overlay_error)?;
                                                        charge_fees_for_applied_overlay_with_encoded_len(
                                                            &mut state_tx,
                                                            &p.authority,
                                                            tx,
                                                            overlay.as_ref(),
                                                            prepared_txs[p.idx].metadata.encoded_len,
                                                        )
                                                        .map_err(TransactionRejectionReason::Validation)?;
                                                        state_tx
                                                            .execute_data_triggers_dfs(&p.authority)
                                                    })
                                                })
                                    } else {
                                        delta.merge_single_transfer_into_transaction(
                                            &mut state_tx,
                                            &p.authority,
                                        )
                                    };
                                    if let Some(result) = single_transfer_result {
                                        match result {
                                            Ok(trigger_sequence) => {
                                                commit_stateful_admission_sequence(
                                                    &mut state_tx,
                                                    &admission,
                                                );
                                                state_tx.apply();
                                                record_result(p.idx, Ok(trigger_sequence));
                                                let lane_id = routing_decisions[p.idx].lane_id;
                                                let summary =
                                                    lane_summaries.entry(lane_id).or_default();
                                                summary.detached_merged =
                                                    summary.detached_merged.saturating_add(1);
                                                if debug_trace_tx_eval {
                                                    let ts = tx.creation_time().as_millis();
                                                    eprintln!(
                                                        "[core-eval] ok(prepared-merge) hash={} ts={} auth={}",
                                                        hash, ts, p.authority,
                                                    );
                                                }
                                            }
                                            Err(reason) => {
                                                drop(state_tx);
                                                record_amx_abort(state_block, p.idx, "commit");
                                                match reason {
                                                    TransactionRejectionReason::Validation(_) => {
                                                        let result = apply_overlay_sequential(
                                                            state_block,
                                                            &mut lane_summaries,
                                                            p.idx,
                                                        );
                                                        record_result(p.idx, result);
                                                    }
                                                    other => {
                                                        record_result(p.idx, Err(other));
                                                    }
                                                }
                                            }
                                        }
                                        continue;
                                    }
                                    drop(state_tx);
                                    let merge_context = DetachedMergeContext {
                                        tx_call_hash: Some(iroha_crypto::Hash::from(hash)),
                                        current_tx_hash: Some(
                                            prepared_txs[p.idx].metadata.signed_hash,
                                        ),
                                        current_lane_id: Some(routing_decisions[p.idx].lane_id),
                                        current_dataspace_id: Some(
                                            routing_decisions[p.idx].dataspace_id,
                                        ),
                                    };
                                    match delta.merge_into_with_context(
                                        state_block,
                                        &p.authority,
                                        merge_context,
                                    ) {
                                        Ok(trigger_sequence) => {
                                            commit_stateful_admission_sequence_to_block(
                                                state_block,
                                                &admission,
                                            );
                                            record_result(p.idx, Ok(trigger_sequence));
                                            let lane_id = routing_decisions[p.idx].lane_id;
                                            let summary =
                                                lane_summaries.entry(lane_id).or_default();
                                            summary.detached_merged =
                                                summary.detached_merged.saturating_add(1);
                                            if debug_trace_tx_eval {
                                                let ts = tx.creation_time().as_millis();
                                                eprintln!(
                                                    "[core-eval] ok(prepared-merge) hash={} ts={} auth={}",
                                                    hash, ts, p.authority,
                                                );
                                            }
                                        }
                                        Err(reason) => {
                                            record_amx_abort(state_block, p.idx, "commit");
                                            match reason {
                                                TransactionRejectionReason::Validation(_) => {
                                                    let result = apply_overlay_sequential(
                                                        state_block,
                                                        &mut lane_summaries,
                                                        p.idx,
                                                    );
                                                    record_result(p.idx, result);
                                                }
                                                other => {
                                                    record_result(p.idx, Err(other));
                                                }
                                            }
                                        }
                                    }
                                }
                                Some(Err(reason)) => {
                                    record_amx_abort(state_block, p.idx, "exec");
                                    match reason {
                                        TransactionRejectionReason::Validation(_) => {
                                            let result = apply_overlay_sequential(
                                                state_block,
                                                &mut lane_summaries,
                                                p.idx,
                                            );
                                            record_result(p.idx, result);
                                        }
                                        other => {
                                            let tx = txs[p.idx];
                                            let hash = prepared_txs[p.idx].metadata.entrypoint_hash;
                                            record_result(p.idx, Err(other));
                                            if debug_trace_tx_eval {
                                                let ts = tx.creation_time().as_millis();
                                                eprintln!(
                                                    "[core-eval] reject(prepared-delta) hash={} ts={} auth={}",
                                                    hash, ts, p.authority,
                                                );
                                            }
                                        }
                                    }
                                }
                                None => {
                                    let result = apply_overlay_sequential(
                                        state_block,
                                        &mut lane_summaries,
                                        p.idx,
                                    );
                                    record_result(p.idx, result);
                                }
                            }
                        }
                    }
                    #[cfg(feature = "telemetry")]
                    {
                        let aggregate_lane = state_block.nexus.routing_policy.default_lane;
                        let elapsed_ms = t_layer_merge.elapsed().as_secs_f64() * 1_000.0;
                        let metrics = state_block.metrics();
                        metrics.observe_pipeline_stage_ms(
                            aggregate_lane,
                            "layers_merge",
                            elapsed_ms,
                        );
                        metrics.observe_amx_commit_ms(aggregate_lane, elapsed_ms);
                    }
                    if let Some(start) = layer_merge_start {
                        let merge_total = to_ms(start.elapsed());
                        apply_merge_ms = apply_merge_ms
                            .saturating_add(merge_total.saturating_sub(layer_fallback_ms));
                        apply_fallback_ms = apply_fallback_ms.saturating_add(layer_fallback_ms);
                    }
                }
                // Execute quarantine transactions sequentially in deterministic order (hash, idx)
                if !quarantine_seq.is_empty() {
                    quarantine_seq.sort_by_key(|&i| (call_hashes[i], i));
                    let quarantine_start = timings.as_ref().map(|_| Instant::now());
                    #[cfg(feature = "telemetry")]
                    let t_quarantine = Instant::now();
                    for &idx in quarantine_seq.iter() {
                        if idx >= txs.len() {
                            continue;
                        }
                        let tx = txs[idx];
                        let hash = prepared_txs[idx].metadata.entrypoint_hash;
                        if let Some(reason) = stateless_rejections[idx].take() {
                            record_result(idx, Err(reason));
                            continue;
                        }
                        let overlay = match overlays[idx].as_ref() {
                            Ok(o) => Arc::clone(o),
                            Err(err) => {
                                let rej = map_overlay_error(err);
                                record_result(idx, Err(rej));
                                continue;
                            }
                        };
                        {
                            let lane_id = routing_decisions[idx].lane_id;
                            let summary = lane_summaries.entry(lane_id).or_default();
                            summary.quarantine_executed =
                                summary.quarantine_executed.saturating_add(1);
                        }
                        let max_instrs = state_block.pipeline.overlay_max_instructions;
                        if max_instrs > 0 && overlay.instruction_count() > max_instrs {
                            record_result(
                                idx,
                                Err(
                                    iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                        iroha_data_model::ValidationFail::NotPermitted(format!(
                                            "overlay exceeds max instructions: {} > {}",
                                            overlay.instruction_count(), max_instrs
                                        )),
                                    ),
                                ),
                            );
                            continue;
                        }
                        let max_bytes = state_block.pipeline.overlay_max_bytes;
                        let byte_size = overlay.byte_size() as u64;
                        if max_bytes > 0 && byte_size > max_bytes {
                            record_result(
                                idx,
                                Err(
                                    iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                        iroha_data_model::ValidationFail::NotPermitted(format!(
                                            "overlay exceeds max bytes: {byte_size} > {max_bytes}"
                                        )),
                                    ),
                                ),
                            );
                            continue;
                        }
                        let chunk_size = state_block.pipeline.overlay_chunk_instructions.max(1);
                        let authority = tx.authority().clone();
                        let result = {
                            let mut state_tx = state_block.transaction();
                            state_tx.current_lane_id = Some(routing_decisions[idx].lane_id);
                            state_tx.current_dataspace_id =
                                Some(routing_decisions[idx].dataspace_id);
                            state_tx.world.current_dataspace_id =
                                Some(routing_decisions[idx].dataspace_id);
                            state_tx.tx_call_hash = Some(iroha_crypto::Hash::from(hash));
                            state_tx.current_tx_hash = Some(prepared_txs[idx].metadata.signed_hash);
                            let missing_authority = missing_authority_requires_rejection(
                                &state_tx,
                                tx,
                                &authority,
                                overlay.instruction_count(),
                                block.header().is_genesis(),
                            );
                            if missing_authority {
                                Err(
                                    iroha_data_model::transaction::error::TransactionRejectionReason::AccountDoesNotExist(
                                        iroha_data_model::query::error::FindError::Account(authority.clone()),
                                    ),
                                )
                            } else {
                                let admission = validate_block_transaction_admission(
                                    &mut state_tx,
                                    tx,
                                    routing_decisions[idx],
                                );
                                if let Err(reason) = admission {
                                    Err(reason)
                                } else {
                                    let admission =
                                        admission.expect("admission result checked above");
                                    let executor = state_tx.world.executor.clone();
                                    if let Err(err) = configure_executor_fuel_budget(
                                        &executor,
                                        &mut state_tx,
                                        tx.metadata(),
                                    ) {
                                        Err(
                                            iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                                err,
                                            ),
                                        )
                                    } else {
                                        match overlay.apply_with_chunk(
                                            &mut state_tx,
                                            &authority,
                                            chunk_size,
                                        ) {
                                            Err(e) => {
                                                let rejection_reason =
                                                    TransactionRejectionReason::Validation(e);
                                                drop(state_tx);
                                                match charge_rejected_overlay_fees(
                                                    state_block,
                                                    tx,
                                                    &authority,
                                                    overlay.as_ref(),
                                                    prepared_txs[idx].metadata.encoded_len,
                                                    routing_decisions[idx].lane_id,
                                                    routing_decisions[idx].dataspace_id,
                                                    &rejection_reason,
                                                ) {
                                                    Ok(()) => Err(rejection_reason),
                                                    Err(err) => Err(err),
                                                }
                                            }
                                            Ok(()) => {
                                                if let Err(err) =
                                                    charge_fees_for_applied_overlay_with_encoded_len(
                                                        &mut state_tx,
                                                        &authority,
                                                        tx,
                                                        overlay.as_ref(),
                                                        prepared_txs[idx].metadata.encoded_len,
                                                    )
                                                {
                                                    Err(
                                                    iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                                        err,
                                                    ),
                                                )
                                                } else {
                                                    match state_tx
                                                        .execute_data_triggers_dfs(&authority)
                                                    {
                                                        Err(err) => {
                                                            drop(state_tx);
                                                            match charge_rejected_overlay_fees(
                                                                state_block,
                                                                tx,
                                                                &authority,
                                                                overlay.as_ref(),
                                                                prepared_txs[idx]
                                                                    .metadata
                                                                    .encoded_len,
                                                                routing_decisions[idx].lane_id,
                                                                routing_decisions[idx].dataspace_id,
                                                                &err,
                                                            ) {
                                                                Ok(()) => Err(err),
                                                                Err(fee_err) => Err(fee_err),
                                                            }
                                                        }
                                                        Ok(trigger_sequence) => {
                                                            commit_stateful_admission_sequence(
                                                                &mut state_tx,
                                                                &admission,
                                                            );
                                                            state_tx.apply();
                                                            Ok(trigger_sequence)
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        };
                        if matches!(
                            result,
                            Err(
                                iroha_data_model::transaction::error::TransactionRejectionReason::AccountDoesNotExist(
                                    _
                                )
                            )
                        ) {
                            record_amx_abort(state_block, idx, "commit");
                            record_result(idx, result);
                            continue;
                        }
                        let result_is_err = result.is_err();
                        record_result(idx, result);
                        if result_is_err {
                            record_amx_abort(state_block, idx, "exec");
                        }
                    }
                    #[cfg(feature = "telemetry")]
                    {
                        let aggregate_lane = state_block.nexus.routing_policy.default_lane;
                        state_block.metrics().observe_pipeline_stage_ms(
                            aggregate_lane,
                            "quarantine",
                            t_quarantine.elapsed().as_secs_f64() * 1_000.0,
                        );
                    }
                    if let (Some(_), Some(start)) = (timings.as_ref(), quarantine_start) {
                        apply_quarantine_ms =
                            apply_quarantine_ms.saturating_add(to_ms(start.elapsed()));
                    }
                }
            } else {
                let seq_start = timings.as_ref().map(|_| Instant::now());
                for &idx in &order {
                    let tx = txs[idx];
                    let hash = prepared_txs[idx].metadata.entrypoint_hash;
                    if let Some(reason) = stateless_rejections[idx].take() {
                        record_result(idx, Err(reason));
                        continue;
                    }
                    let overlay = match overlays[idx].as_ref() {
                        Ok(ovl) => Arc::clone(ovl),
                        Err(err) => {
                            record_amx_abort(state_block, idx, "prepare");
                            let rej = map_overlay_error(err);
                            record_result(idx, Err(rej));
                            continue;
                        }
                    };
                    let max_instrs = state_block.pipeline.overlay_max_instructions;
                    if max_instrs > 0 && overlay.instruction_count() > max_instrs {
                        record_amx_abort(state_block, idx, "prepare");
                        record_result(
                            idx,
                            Err(
                                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                    iroha_data_model::ValidationFail::NotPermitted(format!(
                                        "overlay exceeds max instructions: {} > {}",
                                        overlay.instruction_count(), max_instrs
                                    )),
                                ),
                            ),
                        );
                        continue;
                    }
                    let max_bytes = state_block.pipeline.overlay_max_bytes;
                    let byte_size = overlay.byte_size() as u64;
                    if max_bytes > 0 && byte_size > max_bytes {
                        record_amx_abort(state_block, idx, "prepare");
                        record_result(
                            idx,
                            Err(
                                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                    iroha_data_model::ValidationFail::NotPermitted(format!(
                                        "overlay exceeds max bytes: {byte_size} > {max_bytes}"
                                    )),
                                ),
                            ),
                        );
                        continue;
                    }
                    let chunk_size = state_block.pipeline.overlay_chunk_instructions.max(1);
                    let authority = tx.authority().clone();
                    let result = {
                        let mut state_tx = state_block.transaction();
                        state_tx.current_lane_id = Some(routing_decisions[idx].lane_id);
                        state_tx.current_dataspace_id = Some(routing_decisions[idx].dataspace_id);
                        state_tx.world.current_dataspace_id =
                            Some(routing_decisions[idx].dataspace_id);
                        state_tx.tx_call_hash = Some(iroha_crypto::Hash::from(hash));
                        state_tx.current_tx_hash = Some(prepared_txs[idx].metadata.signed_hash);
                        let missing_authority = missing_authority_requires_rejection(
                            &state_tx,
                            tx,
                            &authority,
                            overlay.instruction_count(),
                            block.header().is_genesis(),
                        );
                        if missing_authority {
                            Err(
                                iroha_data_model::transaction::error::TransactionRejectionReason::AccountDoesNotExist(
                                    iroha_data_model::query::error::FindError::Account(authority.clone()),
                                ),
                            )
                        } else {
                            let admission = validate_block_transaction_admission(
                                &mut state_tx,
                                tx,
                                routing_decisions[idx],
                            );
                            if let Err(reason) = admission {
                                Err(reason)
                            } else {
                                let admission = admission.expect("admission result checked above");
                                let executor = state_tx.world.executor.clone();
                                if let Err(err) = configure_executor_fuel_budget(
                                    &executor,
                                    &mut state_tx,
                                    tx.metadata(),
                                ) {
                                    Err(
                                        iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                            err,
                                        ),
                                    )
                                } else {
                                    match overlay.apply_with_chunk(
                                        &mut state_tx,
                                        &authority,
                                        chunk_size,
                                    ) {
                                        Err(e) => {
                                            let rejection_reason =
                                                TransactionRejectionReason::Validation(e);
                                            drop(state_tx);
                                            match charge_rejected_overlay_fees(
                                                state_block,
                                                tx,
                                                &authority,
                                                overlay.as_ref(),
                                                prepared_txs[idx].metadata.encoded_len,
                                                routing_decisions[idx].lane_id,
                                                routing_decisions[idx].dataspace_id,
                                                &rejection_reason,
                                            ) {
                                                Ok(()) => Err(rejection_reason),
                                                Err(err) => Err(err),
                                            }
                                        }
                                        Ok(()) => {
                                            if let Err(err) =
                                                charge_fees_for_applied_overlay_with_encoded_len(
                                                    &mut state_tx,
                                                    &authority,
                                                    tx,
                                                    overlay.as_ref(),
                                                    prepared_txs[idx].metadata.encoded_len,
                                                )
                                            {
                                                Err(
                                                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                                    err,
                                                ),
                                            )
                                            } else {
                                                match state_tx.execute_data_triggers_dfs(&authority)
                                                {
                                                    Err(err) => {
                                                        drop(state_tx);
                                                        match charge_rejected_overlay_fees(
                                                            state_block,
                                                            tx,
                                                            &authority,
                                                            overlay.as_ref(),
                                                            prepared_txs[idx].metadata.encoded_len,
                                                            routing_decisions[idx].lane_id,
                                                            routing_decisions[idx].dataspace_id,
                                                            &err,
                                                        ) {
                                                            Ok(()) => Err(err),
                                                            Err(fee_err) => Err(fee_err),
                                                        }
                                                    }
                                                    Ok(trigger_sequence) => {
                                                        commit_stateful_admission_sequence(
                                                            &mut state_tx,
                                                            &admission,
                                                        );
                                                        state_tx.apply();
                                                        Ok(trigger_sequence)
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    };
                    if matches!(
                        result,
                        Err(
                            iroha_data_model::transaction::error::TransactionRejectionReason::AccountDoesNotExist(
                                _
                            )
                        )
                    ) {
                        record_amx_abort(state_block, idx, "commit");
                        record_result(idx, result);
                        continue;
                    }
                    match &result {
                        Err(reason) => {
                            iroha_logger::debug!(tx=%hash, block=%block.hash(), reason=?reason, "Transaction rejected");
                            if debug_trace_tx_eval {
                                eprintln!(
                                    "[core-eval] reject(seq) hash={} ts={} auth={}",
                                    hash,
                                    tx.creation_time().as_millis(),
                                    authority,
                                );
                            }
                        }
                        Ok(trigger_sequence) => {
                            iroha_logger::debug!(tx=%hash, block=%block.hash(), trigger_sequence=?trigger_sequence, "Transaction approved");
                            if debug_trace_tx_eval {
                                eprintln!(
                                    "[core-eval] ok(seq) hash={} ts={} auth={}",
                                    hash,
                                    tx.creation_time().as_millis(),
                                    authority,
                                );
                            }
                        }
                    }
                    let result_is_err = result.is_err();
                    record_result(idx, result);
                    if result_is_err {
                        record_amx_abort(state_block, idx, "exec");
                    }
                }
                if let (Some(_), Some(start)) = (timings.as_ref(), seq_start) {
                    apply_sequential_ms =
                        apply_sequential_ms.saturating_add(to_ms(start.elapsed()));
                }
            }

            let apply_results_start = timings.as_ref().map(|_| Instant::now());
            super::set_pipeline_status_snapshots(&lane_summaries);
            #[cfg(feature = "telemetry")]
            {
                let aggregate_lane = state_block.nexus.routing_policy.default_lane;
                if layer_widths_global.is_empty() {
                    for summary in lane_summaries.values() {
                        if summary.tx_vertices > 0 {
                            layer_widths_global.push(summary.tx_vertices);
                        }
                    }
                }

                let telemetry = state_block.metrics();
                let block_height = state_block._curr_block.height().get();
                let mut lane_ids: BTreeSet<LaneId> = lane_summaries.keys().copied().collect();
                if lane_ids.is_empty() {
                    lane_ids.insert(aggregate_lane);
                }

                for lane_id in lane_ids.iter().copied() {
                    let summary = lane_summaries.entry(lane_id).or_default();
                    if summary.layer_widths.is_empty() {
                        if summary.tx_vertices > 0 {
                            summary.layer_widths.push(summary.tx_vertices);
                            summary.peak_layer_width = summary.tx_vertices;
                        } else {
                            summary.peak_layer_width = 0;
                        }
                    }

                    let mut sorted_widths = summary.layer_widths.clone();
                    sorted_widths.sort_unstable();
                    let layer_count = sorted_widths.len() as u64;
                    let sum: u64 = sorted_widths.iter().copied().sum();
                    let avg = if layer_count > 0 {
                        (sum + (layer_count / 2)) / layer_count
                    } else {
                        0
                    };
                    let median = if sorted_widths.is_empty() {
                        0
                    } else if sorted_widths.len() % 2 == 1 {
                        sorted_widths[sorted_widths.len() / 2]
                    } else {
                        u64::midpoint(
                            sorted_widths[sorted_widths.len() / 2 - 1],
                            sorted_widths[sorted_widths.len() / 2],
                        )
                    };
                    let util_pct = if summary.peak_layer_width > 0 {
                        (avg.saturating_mul(100)).saturating_div(summary.peak_layer_width)
                    } else {
                        0
                    };
                    let mut buckets = [0u64; 8];
                    for width in &sorted_widths {
                        for (idx, threshold) in PIPELINE_LAYER_WIDTH_THRESHOLDS.iter().enumerate() {
                            if *width <= *threshold {
                                buckets[idx] += 1;
                            }
                        }
                    }

                    telemetry.record_lane_pipeline_summary(
                        lane_id,
                        LanePipelineSummary {
                            block_height,
                            tx_vertices: summary.tx_vertices,
                            tx_edges: summary.tx_edges,
                            overlay_count: summary.overlay_count,
                            overlay_instr_total: summary.overlay_instr_total,
                            overlay_bytes_total: summary.overlay_bytes_total,
                            rbc_chunks: summary.rbc_chunks,
                            rbc_bytes_total: summary.rbc_bytes_total,
                            peak_layer_width: summary.peak_layer_width,
                            layer_count,
                            avg_layer_width: avg,
                            median_layer_width: median,
                            scheduler_utilization_pct: util_pct,
                            layer_width_buckets: SchedulerLayerWidthBuckets::from(buckets),
                            detached_prepared: summary.detached_prepared,
                            detached_merged: summary.detached_merged,
                            detached_fallback: summary.detached_fallback,
                            quarantine_executed: summary.quarantine_executed,
                        },
                    );
                }
                telemetry.update_lane_finality_lag(block_height);

                let det_prepared_total: u64 =
                    lane_summaries.values().map(|s| s.detached_prepared).sum();
                let det_merged_total: u64 =
                    lane_summaries.values().map(|s| s.detached_merged).sum();
                let det_fallback_total: u64 =
                    lane_summaries.values().map(|s| s.detached_fallback).sum();
                let det_fallback_reasons_total = lane_summaries
                    .values()
                    .fold(DetachedFallbackReasons::default(), |acc, summary| {
                        acc.merge(summary.detached_fallback_reasons)
                    });
                let quarantine_total: u64 =
                    lane_summaries.values().map(|s| s.quarantine_executed).sum();

                telemetry.set_pipeline_detached_prepared(aggregate_lane, det_prepared_total);
                telemetry.set_pipeline_detached_merged(aggregate_lane, det_merged_total);
                telemetry.set_pipeline_detached_fallback(aggregate_lane, det_fallback_total);
                telemetry.set_pipeline_detached_fallback_reason(
                    aggregate_lane,
                    "fee_postprocessing",
                    det_fallback_reasons_total.fee_postprocessing,
                );
                telemetry.set_pipeline_detached_fallback_reason(
                    aggregate_lane,
                    "user_executor",
                    det_fallback_reasons_total.user_executor,
                );
                telemetry.set_pipeline_detached_fallback_reason(
                    aggregate_lane,
                    "durable_state",
                    det_fallback_reasons_total.durable_state,
                );
                telemetry.set_pipeline_detached_fallback_reason(
                    aggregate_lane,
                    "unsupported_instruction",
                    det_fallback_reasons_total.unsupported_instruction,
                );
                telemetry.set_pipeline_detached_fallback_reason(
                    aggregate_lane,
                    "rejected_eval",
                    det_fallback_reasons_total.rejected_eval,
                );
                telemetry.set_pipeline_detached_fallback_reason(
                    aggregate_lane,
                    "overlay_error",
                    det_fallback_reasons_total.overlay_error,
                );
                telemetry.set_pipeline_quarantine_executed(aggregate_lane, quarantine_total);

                if layer_widths_global.is_empty() {
                    telemetry.set_pipeline_peak_layer_width(aggregate_lane, 0);
                    telemetry.set_pipeline_layer_count(aggregate_lane, 0);
                    telemetry.set_pipeline_scheduler_utilization_pct(aggregate_lane, 0);
                    telemetry.set_pipeline_layer_avg_median(aggregate_lane, 0, 0);
                    telemetry.set_pipeline_layer_width_hist(aggregate_lane, [0; 8]);
                } else {
                    let peak_layer_width = layer_widths_global.iter().copied().max().unwrap_or(0);
                    telemetry.set_pipeline_peak_layer_width(aggregate_lane, peak_layer_width);
                    let layer_count = layer_widths_global.len() as u64;
                    telemetry.set_pipeline_layer_count(aggregate_lane, layer_count);
                    let sum_global: u64 = layer_widths_global.iter().sum();
                    let avg = if layer_count > 0 {
                        (sum_global + (layer_count / 2)) / layer_count
                    } else {
                        0
                    };
                    let util_pct = if peak_layer_width > 0 {
                        (avg.saturating_mul(100)).saturating_div(peak_layer_width)
                    } else {
                        0
                    };
                    telemetry.set_pipeline_scheduler_utilization_pct(aggregate_lane, util_pct);
                    let mut sorted = layer_widths_global.clone();
                    sorted.sort_unstable();
                    let median = if sorted.is_empty() {
                        0
                    } else if sorted.len() % 2 == 1 {
                        sorted[sorted.len() / 2]
                    } else {
                        u64::midpoint(sorted[sorted.len() / 2 - 1], sorted[sorted.len() / 2])
                    };
                    telemetry.set_pipeline_layer_avg_median(aggregate_lane, avg, median);
                    let mut buckets = [0u64; 8];
                    for width in sorted {
                        for (idx, threshold) in PIPELINE_LAYER_WIDTH_THRESHOLDS.iter().enumerate() {
                            if width <= *threshold {
                                buckets[idx] += 1;
                            }
                        }
                    }
                    telemetry.set_pipeline_layer_width_hist(aggregate_lane, buckets);
                }
            }

            for (idx, maybe) in stateless_rejections.iter_mut().enumerate() {
                if let Some(reason) = maybe.take() {
                    record_result(idx, Err(reason));
                }
            }

            // Persist results in payload order so transaction indices (and block errors) align
            // with the serialized transaction list when applying the block.
            let mut hashes: Vec<_> = Vec::with_capacity(n);
            let mut ordered_results: Vec<TransactionResultInner> = Vec::with_capacity(n);
            for idx in 0..n {
                hashes.push(call_hashes[idx]);
                let result = tx_results[idx].take().unwrap_or_else(|| {
                    debug_assert!(false, "missing transaction result for idx {idx}");
                    Err(TransactionRejectionReason::Validation(
                        iroha_data_model::ValidationFail::InternalError(format!(
                            "missing transaction result for idx {idx}"
                        )),
                    ))
                });
                ordered_results.push(result);
            }
            if let Some(start) = apply_results_start {
                apply_results_ms = to_ms(start.elapsed());
            }

            let transaction_event_hashes: Vec<_> = prepared_txs
                .iter()
                .map(|prepared| Some(prepared.metadata.signed_hash))
                .collect();
            Self::execute_deterministic_pipeline_triggers(
                block,
                state_block,
                &transaction_event_hashes,
                &ordered_results,
                &routing_decisions,
            )?;

            let time_triggers_start = timings.as_ref().map(|_| Instant::now());
            let (time_trgs, mut time_trg_hashes, mut time_trg_results) =
                state_block.execute_time_triggers(&block.header());
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), time_triggers_start) {
                timings.execution_tx_time_triggers_ms = to_ms(start.elapsed());
            }
            #[cfg(test)]
            execute_soracloud_mailbox_runtime(state_block);
            let pruned_sealed_commitments =
                crate::tx::prune_expired_sealed_commitments(state_block);
            if pruned_sealed_commitments > 0 {
                iroha_logger::debug!(
                    count = pruned_sealed_commitments,
                    "pruned expired sealed transaction commitments"
                );
            }
            let finalize_start = timings.as_ref().map(|_| Instant::now());
            let digest_submit_start = timings.as_ref().map(|_| Instant::now());
            let fastpq_digest_batch = state_block.submit_transfer_transcript_digest_batch();
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), digest_submit_start) {
                timings.execution_tx_finalize_digest_submit_ms = to_ms(start.elapsed());
            }
            let dataspaces_start = timings.as_ref().map(|_| Instant::now());
            let mut fastpq_entry_dataspaces = std::collections::BTreeMap::new();
            for (idx, entry_hash) in call_hashes.iter().enumerate() {
                fastpq_entry_dataspaces.insert(
                    iroha_crypto::Hash::from(*entry_hash),
                    routing_decisions[idx].dataspace_id,
                );
            }
            for entry_hash in &time_trg_hashes {
                fastpq_entry_dataspaces.insert(
                    iroha_crypto::Hash::from(*entry_hash),
                    DataSpaceId::UNIVERSAL,
                );
            }
            hashes.append(&mut time_trg_hashes);
            ordered_results.append(&mut time_trg_results);
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), dataspaces_start) {
                timings.execution_tx_finalize_dataspaces_ms = to_ms(start.elapsed());
            }

            let tx_set_start = timings.as_ref().map(|_| Instant::now());
            let mut tx_set_hashes = hashes.clone();
            tx_set_hashes.sort_unstable();
            let tx_set_hash =
                crate::fastpq::tx_set_hash_from_ordered_hashes(tx_set_hashes.iter().copied());
            state_block.set_fastpq_tx_set_hash(tx_set_hash);
            state_block.set_fastpq_entry_dataspaces(fastpq_entry_dataspaces);
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), tx_set_start) {
                timings.execution_tx_finalize_tx_set_ms = to_ms(start.elapsed());
            }

            let transcripts_start = timings.as_ref().map(|_| Instant::now());
            let fastpq_transcripts =
                state_block.drain_transfer_transcripts_with_pending(fastpq_digest_batch);
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), transcripts_start) {
                timings.execution_tx_finalize_transcripts_ms = to_ms(start.elapsed());
            }
            let axt_start = timings.as_ref().map(|_| Instant::now());
            let axt_envelopes = state_block.drain_axt_envelopes();
            let axt_policy_snapshot = Some(state_block.axt_policy_snapshot());
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), axt_start) {
                timings.execution_tx_finalize_axt_ms = to_ms(start.elapsed());
            }
            let set_results_start = timings.as_ref().map(|_| Instant::now());
            block
                .set_transaction_results_with_transcripts(
                    time_trgs,
                    hashes.as_slice(),
                    ordered_results,
                    fastpq_transcripts,
                    axt_envelopes,
                    axt_policy_snapshot,
                )
                .map_err(|_| BlockValidationError::MerkleRootMismatch)?;
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), set_results_start) {
                timings.execution_tx_finalize_set_results_ms = to_ms(start.elapsed());
            }
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), finalize_start) {
                let finalize_ms = to_ms(start.elapsed());
                let known_finalize_ms = timings
                    .execution_tx_finalize_digest_submit_ms
                    .saturating_add(timings.execution_tx_finalize_dataspaces_ms)
                    .saturating_add(timings.execution_tx_finalize_tx_set_ms)
                    .saturating_add(timings.execution_tx_finalize_transcripts_ms)
                    .saturating_add(timings.execution_tx_finalize_axt_ms)
                    .saturating_add(timings.execution_tx_finalize_set_results_ms);
                timings.execution_tx_finalize_ms = finalize_ms;
                timings.execution_tx_finalize_other_ms =
                    finalize_ms.saturating_sub(known_finalize_ms);
            }
            #[cfg(feature = "telemetry")]
            {
                let aggregate_lane = state_block.nexus.routing_policy.default_lane;
                state_block.metrics().observe_pipeline_stage_ms(
                    aggregate_lane,
                    "apply",
                    t_apply_start.elapsed().as_secs_f64() * 1_000.0,
                );
            }
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), apply_start) {
                let apply_ms = to_ms(start.elapsed());
                let known_apply_ms = apply_setup_ms
                    .saturating_add(apply_layer_build_ms)
                    .saturating_add(apply_prep_ms)
                    .saturating_add(apply_detached_ms)
                    .saturating_add(apply_merge_ms)
                    .saturating_add(apply_fallback_ms)
                    .saturating_add(apply_quarantine_ms)
                    .saturating_add(apply_sequential_ms)
                    .saturating_add(apply_results_ms)
                    .saturating_add(timings.execution_tx_time_triggers_ms)
                    .saturating_add(timings.execution_tx_finalize_ms);
                timings.execution_tx_apply_ms = apply_ms;
                timings.execution_tx_apply_setup_ms = apply_setup_ms;
                timings.execution_tx_apply_layer_build_ms = apply_layer_build_ms;
                timings.execution_tx_apply_prep_ms = apply_prep_ms;
                timings.execution_tx_apply_detached_ms = apply_detached_ms;
                timings.execution_tx_apply_merge_ms = apply_merge_ms;
                timings.execution_tx_apply_fallback_ms = apply_fallback_ms;
                timings.execution_tx_apply_quarantine_ms = apply_quarantine_ms;
                timings.execution_tx_apply_sequential_ms = apply_sequential_ms;
                timings.execution_tx_apply_results_ms = apply_results_ms;
                timings.execution_tx_apply_other_ms = apply_ms.saturating_sub(known_apply_ms);
            }
            Ok(())
        }

        /// Like [`Self::validate`], but without the static check part.
        ///
        /// Useful for cases when the block is assumed to be valid:
        ///
        /// - When block is created by the node
        /// - For Explorer, which is not interested in validation and only needs
        ///   state changes
        pub fn validate_unchecked(
            mut block: SignedBlock,
            state_block: &mut StateBlock<'_>,
        ) -> WithEvents<ValidBlock> {
            assert!(
                block.header().is_genesis() || signed_block_entrypoints_are_canonical(&block),
                "unchecked block payload is not in canonical transaction entrypoint order"
            );
            let exec_witness_guard = crate::sumeragi::witness::exec_witness_guard();
            Self::validate_and_record_transactions(&mut block, state_block, None, false)
                .expect("unchecked block should have internally consistent entrypoint hashes");
            if let Err(error) = validate_axt_envelopes(&block, state_block) {
                panic!("AXT envelope validation failed on unchecked block: {error}");
            }
            state_block.capture_exec_witness();
            drop(exec_witness_guard);
            WithEvents::new(ValidBlock::new_unverified(block))
        }

        /// Add additional signature for [`Self`]
        ///
        /// # Errors
        ///
        /// If given signature doesn't match block hash
        pub fn add_signature(
            &mut self,
            signature: BlockSignature,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            use SignatureVerificationError::{Other, UnknownSignatory, UnknownSignature};

            let signatory = usize::try_from(signature.index()).map_err(|_err| UnknownSignatory)?;
            let signatory = topology.as_ref().get(signatory).ok_or(UnknownSignatory)?;

            assert_ne!(Role::Leader, topology.role(signatory));
            assert_ne!(Role::Undefined, topology.role(signatory));

            signature
                .signature()
                .verify_hash(signatory.public_key(), self.as_ref().hash())
                .map_err(|_err| UnknownSignature)?;

            self.block.add_signature(signature).map_err(|_err| Other)?;
            self.clear_signatures_verified();
            Ok(())
        }

        /// Replace block's signatures. Returns previous block signatures
        ///
        /// # Errors
        ///
        /// - Replacement signatures don't contain the leader signature
        /// - Replacement signatures contain unknown signatories
        /// - Replacement signatures contain incorrect signatures
        /// - Replacement signatures contain duplicate signatures
        pub fn replace_signatures(
            &mut self,
            signatures: BTreeSet<BlockSignature>,
            topology: &Topology,
        ) -> WithEvents<Result<BTreeSet<BlockSignature>, SignatureVerificationError>> {
            let mut seen = BTreeSet::new();
            for signature in &signatures {
                let signer = match usize::try_from(signature.index()) {
                    Ok(idx) => idx,
                    Err(_) => {
                        return WithEvents::new(Err(SignatureVerificationError::UnknownSignatory));
                    }
                };
                if !seen.insert(signer) {
                    return WithEvents::new(Err(SignatureVerificationError::DuplicateSignature {
                        signer,
                    }));
                }
            }
            let was_verified = self.signatures_verified;
            let Ok(prev_signatures) = self.block.replace_signatures(signatures) else {
                return WithEvents::new(Err(SignatureVerificationError::Other));
            };
            self.clear_signatures_verified();

            let result = if let Err(err) = Self::is_commit(self.as_ref(), topology) {
                self.block
                    .replace_signatures(prev_signatures)
                    .expect("INTERNAL BUG: invalid signatures in block");
                self.signatures_verified = was_verified;
                Err(err)
            } else {
                Ok(prev_signatures)
            };

            WithEvents::new(result)
        }

        /// Transition block to [`CommittedBlock`].
        ///
        /// # Errors
        ///
        /// - Block is missing the leader signature
        /// - Block doesn't have enough valid signatures
        pub fn commit(self, topology: &Topology) -> WithCommittedBlockEvents {
            WithEvents::new(
                match Self::is_commit_internal(self.as_ref(), topology, self.signatures_verified) {
                    Err(err) => Err((Box::new(self), Box::new(err.into()))),
                    Ok(()) => Ok(CommittedBlock(self)),
                },
            )
        }

        /// Commit using a validated commit certificate.
        ///
        /// Callers must ensure the block has already passed validation and the commit
        /// certificate was verified; this skips block-signature quorum checks.
        pub fn commit_with_certificate(self) -> WithCommittedBlockEvents {
            WithEvents::new(Ok(CommittedBlock(self)))
        }

        /// Commit using a prevalidated signer set (e.g., from a QC).
        ///
        /// The block signatures are still verified to guard against forged aggregates; `signers`
        /// must match a quorum of signatures present on the block.
        pub fn commit_with_signers(
            self,
            topology: &Topology,
            signers: &BTreeSet<crate::sumeragi::consensus::ValidatorIndex>,
            allow_quorum_bypass: bool,
        ) -> WithCommittedBlockEvents {
            let validation = (|| -> Result<(), SignatureVerificationError> {
                // Ensure the QC-reported signer set matches the expected quorum shape.
                Self::verify_signer_set(topology, signers, allow_quorum_bypass)?;
                // Block signatures can be a trimmed subset when the QC carries the quorum.
                // Validate all present signatures against the topology and ensure they
                // don't contradict the QC signer set.
                if !self.signatures_verified {
                    Self::verify_unique_signers(self.as_ref())?;
                    Self::verify_signatures_against_topology(self.as_ref(), topology)?;
                }
                Ok(())
            })();

            WithEvents::new(match validation {
                Err(err) => Err((Box::new(self), Box::new(err.into()))),
                Ok(()) => Ok(CommittedBlock(self)),
            })
        }

        /// Like [`Self::commit`], but without block signature checks.
        ///
        /// Useful e.g. for Explorer, which assumes all blocks from Iroha are valid, and
        /// only executes them to produce state changes.
        pub fn commit_unchecked(self) -> WithEvents<CommittedBlock> {
            WithEvents::new(CommittedBlock(self))
        }

        /// Validate and commit block if possible.
        ///
        /// The difference from calling [`Self::validate_keep_voting_block`] + [`ValidBlock::commit`]
        /// is that signatures are eagerly checked first.
        #[allow(clippy::too_many_arguments)]
        pub fn commit_keep_voting_block<'state, F: Fn(PipelineEventBox)>(
            block: SignedBlock,
            topology: &Topology,
            expected_chain_id: &ChainId,
            genesis_account: &AccountId,
            time_source: &TimeSource,
            state: &'state State,
            voting_block: &mut Option<VotingBlock>,
            soft_fork: bool,
            send_events: F,
        ) -> WithEvents<Result<(CommittedBlock, StateBlock<'state>), Error>> {
            if let Err(err) = Self::is_commit(&block, topology) {
                // Emit a rejection event for this block before returning the error.
                let ev = PipelineEventBox::from(BlockEvent {
                    header: block.header(),
                    status: BlockStatus::Rejected(map_sig_err_to_reason(&err)),
                });
                send_events(ev);
                return WithEvents::new(Err((Box::new(block), Box::new(err.into()))));
            }

            let result = Self::validate_keep_voting_block(
                block,
                topology,
                expected_chain_id,
                genesis_account,
                time_source,
                state,
                voting_block,
                soft_fork,
            )
            .unpack(&send_events);

            match result {
                Ok((block, state_block)) => {
                    WithEvents::new(Ok((CommittedBlock(block), state_block)))
                }
                Err((signed_block, err)) => {
                    // Emit a rejection event carrying the signed block header for visibility.
                    let ev = PipelineEventBox::from(BlockEvent {
                        header: signed_block.header(),
                        status: BlockStatus::Rejected(map_block_err_to_reason(err.as_ref())),
                    });
                    send_events(ev);
                    WithEvents::new(Err((signed_block, err)))
                }
            }
        }

        /// Check if block satisfy requirements to be committed
        ///
        /// # Errors
        ///
        /// - Block is missing the leader signature
        /// - Block doesn't have enough signatures for quorum
        pub(crate) fn is_commit(
            block: &SignedBlock,
            topology: &Topology,
        ) -> Result<(), SignatureVerificationError> {
            Self::is_commit_internal(block, topology, false)
        }

        fn is_commit_internal(
            block: &SignedBlock,
            topology: &Topology,
            signatures_verified: bool,
        ) -> Result<(), SignatureVerificationError> {
            if !block.header().is_genesis() {
                if !signatures_verified {
                    Self::verify_unique_signers(block)?;
                    Self::verify_leader_signature(block, topology)?;
                    Self::verify_signatures_against_topology(block, topology)?;
                }

                let SignatureTally {
                    present: present_signatures,
                    counted: votes_count,
                    set_b_signatures,
                } = commit_signature_tally(block, topology);

                iroha_logger::info!(
                    signatures_present = present_signatures,
                    votes = votes_count,
                    set_b_signatures,
                    min_votes = topology.min_votes_for_commit(),
                    topo_len = topology.as_ref().len(),
                    block_hash = %block.hash(),
                    "verifying block commit quorum"
                );
                if votes_count < topology.min_votes_for_commit() {
                    return Err(SignatureVerificationError::NotEnoughSignatures {
                        votes_count,
                        min_votes_for_commit: topology.min_votes_for_commit(),
                    });
                }
            }
            Ok(())
        }

        /// Add additional signatures for [`Self`].
        pub fn sign(&mut self, key_pair: &KeyPair, topology: &Topology) {
            let signatory_idx = topology
                .position(key_pair.public_key())
                .expect("INTERNAL BUG: Node is not in topology");

            self.block.sign(key_pair.private_key(), signatory_idx);
            self.clear_signatures_verified();
        }

        #[cfg(test)]
        pub(crate) fn new_dummy(leader_private_key: &PrivateKey) -> Self {
            Self::new_dummy_and_modify_header(leader_private_key, |_| {})
        }

        #[cfg(test)]
        pub(crate) fn new_dummy_and_modify_header(
            leader_private_key: &PrivateKey,
            f: impl FnOnce(&mut BlockHeader),
        ) -> Self {
            let merkle_root = MerkleTree::<TransactionEntrypoint>::default().root();
            let mut header =
                BlockHeader::new(nonzero_ext::nonzero!(2_u64), None, merkle_root, None, 0, 0);
            f(&mut header);
            if header.confidential_features().is_none() {
                header.set_confidential_features(Some(EMPTY_CONFIDENTIAL_FEATURE_DIGEST));
            }
            let builder = BlockBuilder(Chained {
                header,
                transactions: Vec::new(),
                da_commitments: None,
                da_proof_policies: None,
                da_pin_intents: None,
                previous_roster_evidence: None,
                npos_consensus_effects: None,
                execution_context: None,
            });
            let default_policies = crate::da::proof_policy_bundle(
                &iroha_config::parameters::actual::LaneConfig::default(),
            );
            let unverified_block = builder
                .with_da_proof_policies(Some(default_policies))
                .sign(leader_private_key)
                .unpack(|_| {});

            Self::new_unverified(SignedBlock::presigned(
                unverified_block.signature,
                unverified_block.header,
                unverified_block
                    .transactions
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            ))
        }
    }

    impl From<ValidBlock> for SignedBlock {
        fn from(source: ValidBlock) -> Self {
            source.block
        }
    }

    impl AsRef<SignedBlock> for ValidBlock {
        fn as_ref(&self) -> &SignedBlock {
            &self.block
        }
    }

    #[cfg(any(test, feature = "iroha-core-tests"))]
    impl AsMut<SignedBlock> for ValidBlock {
        fn as_mut(&mut self) -> &mut SignedBlock {
            &mut self.block
        }
    }

    #[test]
    fn dummy_block_populates_proof_policy_hash() {
        let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let block = ValidBlock::new_dummy(kp.private_key());

        assert!(block.as_ref().header().da_proof_policies_hash().is_some());
    }

    #[cfg(test)]
    mod tests {
        use std::{
            borrow::Cow,
            collections::BTreeSet,
            num::{NonZeroU16, NonZeroU32, NonZeroU64},
            path::PathBuf,
            str::FromStr,
            sync::Arc,
            time::Duration,
        };

        use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PrivateKey, Signature, SignatureOf};
        use iroha_data_model::{
            Registrable,
            block::error::BlockRejectionReason as Reason,
            consensus::{
                ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus,
                NposConsensusEffects, NposMarkVrfPenaltiesAppliedAction, NposPenaltyAction,
                VrfEpochRecord, VrfLateRevealRecord, VrfParticipantRecord,
            },
            da::{
                commitment::{
                    DaCommitmentBundle, DaCommitmentRecord, DaProofScheme, KzgCommitment,
                    RetentionClass,
                },
                types::{BlobDigest, StorageTicketId},
            },
            isi::{Log, error::Mismatch},
            metadata::Metadata,
            nexus::{
                DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, LaneCatalog, LaneConfig, LaneId,
            },
            parameter::Parameters,
            prelude::{Account, Domain, PeerId},
            soracloud::{
                SORA_STATE_BINDING_VERSION_V1, SoraCapabilityPolicyV1,
                SoraCertifiedResponsePolicyV1, SoraContainerManifestRefV1, SoraContainerManifestV1,
                SoraContainerRuntimeV1, SoraDeploymentBundleV1, SoraLifecycleHooksV1,
                SoraMailboxContractV1, SoraNetworkPolicyV1, SoraResourceLimitsV1,
                SoraRolloutPolicyV1, SoraRuntimeReceiptV1, SoraServiceDeploymentStateV1,
                SoraServiceHandlerClassV1, SoraServiceHandlerV1, SoraServiceLifecycleActionV1,
                SoraServiceMailboxMessageV1, SoraServiceManifestV1, SoraServiceRuntimeStateV1,
                SoraStateBindingV1, SoraStateEncryptionV1, SoraStateMutabilityV1,
                SoraStateMutationOperationV1,
            },
            sorafs::pin_registry::ManifestDigest,
            transaction::{TransactionBuilder, error::TransactionLimitError},
        };
        use iroha_logger::Level;
        use iroha_primitives::time::TimeSource;
        use iroha_schema::Ident;
        use iroha_test_samples::{ALICE_ID, gen_account_in};
        use mv::cell::Cell;
        use nonzero_ext::nonzero;

        use super::*;
        use crate::{
            kura::Kura,
            query::store::LiveQueryStore,
            soracloud_runtime::{
                SoracloudApartmentExecutionRequest, SoracloudApartmentExecutionResult,
                SoracloudDeterministicStateMutation, SoracloudLocalReadRequest,
                SoracloudLocalReadResponse, SoracloudRuntime, SoracloudRuntimeExecutionError,
                SoracloudRuntimeExecutionErrorKind, SoracloudRuntimeReadHandle,
                SoracloudRuntimeSnapshot,
            },
            state::{State, World},
            sumeragi::network_topology::{Topology, test_topology_with_keys},
            tx::AcceptedTransaction,
        };

        fn insert_consensus_key(
            world: &mut World,
            name: &str,
            keypair: &KeyPair,
            activation_height: u64,
            expiry_height: Option<u64>,
            status: ConsensusKeyStatus,
        ) -> ConsensusKeyId {
            let id = ConsensusKeyId::new(
                ConsensusKeyRole::Validator,
                Ident::from_str(name).expect("consensus key name parses"),
            );
            let pop = match keypair.public_key().algorithm() {
                Algorithm::BlsNormal => Some(
                    iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                        .expect("pop for consensus key"),
                ),
                Algorithm::BlsSmall => Some(
                    iroha_crypto::bls_small_pop_prove(keypair.private_key())
                        .expect("pop for consensus key"),
                ),
                _ => None,
            };
            let record = ConsensusKeyRecord {
                id: id.clone(),
                public_key: keypair.public_key().clone(),
                pop,
                activation_height,
                expiry_height,
                hsm: None,
                replaces: None,
                status,
            };
            world.consensus_keys.insert(id.clone(), record.clone());
            let pk_label = record.public_key.to_string();
            world
                .consensus_keys_by_pk
                .insert(pk_label, vec![id.clone()]);
            id
        }

        #[derive(Clone, Default)]
        struct CountingSoracloudRuntime {
            ordered_mailbox_calls: Arc<parking_lot::Mutex<Vec<Hash>>>,
            state_mutations: Vec<SoracloudDeterministicStateMutation>,
        }

        impl CountingSoracloudRuntime {
            fn ordered_mailbox_call_count(&self) -> usize {
                self.ordered_mailbox_calls.lock().len()
            }

            fn with_state_mutations(
                state_mutations: Vec<SoracloudDeterministicStateMutation>,
            ) -> Self {
                Self {
                    ordered_mailbox_calls: Arc::default(),
                    state_mutations,
                }
            }
        }

        impl SoracloudRuntimeReadHandle for CountingSoracloudRuntime {
            fn snapshot(&self) -> SoracloudRuntimeSnapshot {
                SoracloudRuntimeSnapshot::default()
            }

            fn state_dir(&self) -> PathBuf {
                PathBuf::from("/tmp/iroha-soracloud-runtime-test")
            }
        }

        impl SoracloudRuntime for CountingSoracloudRuntime {
            fn execute_local_read(
                &self,
                _request: SoracloudLocalReadRequest,
            ) -> Result<SoracloudLocalReadResponse, SoracloudRuntimeExecutionError> {
                Err(SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "local reads are not used in this test runtime",
                ))
            }

            fn execute_ordered_mailbox(
                &self,
                request: SoracloudOrderedMailboxExecutionRequest,
            ) -> Result<SoracloudOrderedMailboxExecutionResult, SoracloudRuntimeExecutionError>
            {
                self.ordered_mailbox_calls
                    .lock()
                    .push(request.mailbox_message.message_id);

                Ok(SoracloudOrderedMailboxExecutionResult {
                    state_mutations: self.state_mutations.clone(),
                    outbound_mailbox_messages: Vec::new(),
                    response_bytes: Vec::new(),
                    content_type: None,
                    runtime_state: Some(SoraServiceRuntimeStateV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
                        service_name: request.deployment.service_name.clone(),
                        active_service_version: request.deployment.current_service_version.clone(),
                        health_status: SoraServiceHealthStatusV1::Healthy,
                        load_factor_bps: 111,
                        materialized_bundle_hash: request.bundle.container.bundle_hash,
                        rollout_handle: request
                            .deployment
                            .active_rollout
                            .as_ref()
                            .map(|rollout| rollout.rollout_handle.clone()),
                        pending_mailbox_message_count: request
                            .authoritative_pending_mailbox_messages
                            .saturating_sub(1),
                        last_receipt_id: None,
                    }),
                    runtime_receipt: SoraRuntimeReceiptV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_RUNTIME_RECEIPT_VERSION_V1,
                        receipt_id: Hash::new(
                            format!(
                                "test-receipt:{}:{}",
                                request.deployment.service_name, request.mailbox_message.message_id
                            )
                            .as_bytes(),
                        ),
                        service_name: request.deployment.service_name,
                        service_version: request.deployment.current_service_version,
                        handler_name: request.mailbox_message.to_handler.clone(),
                        handler_class: request
                            .handler
                            .as_ref()
                            .map(|handler| handler.class)
                            .unwrap_or(SoraServiceHandlerClassV1::Update),
                        request_commitment: request.mailbox_message.payload_commitment,
                        result_commitment: Hash::new(
                            format!("test-result:{}", request.mailbox_message.message_id)
                                .as_bytes(),
                        ),
                        certified_by: SoraCertifiedResponsePolicyV1::None,
                        emitted_sequence: request.execution_sequence,
                        mailbox_message_id: Some(request.mailbox_message.message_id),
                        journal_artifact_hash: None,
                        checkpoint_artifact_hash: None,
                        placement_id: None,
                        selected_validator_account_id: None,
                        selected_peer_id: None,
                    },
                })
            }

            fn execute_apartment(
                &self,
                _request: SoracloudApartmentExecutionRequest,
            ) -> Result<SoracloudApartmentExecutionResult, SoracloudRuntimeExecutionError>
            {
                Err(SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "apartments are not used in this test runtime",
                ))
            }
        }

        fn seed_soracloud_mailbox_fixture(
            world: &mut World,
            state_bindings: Vec<SoraStateBindingV1>,
        ) -> (iroha_data_model::name::Name, Hash) {
            let service_name: iroha_data_model::name::Name =
                "portal".parse().expect("valid service name");
            let service_version = "2026.1".to_string();
            let bundle_hash = Hash::new(b"bundle:portal:2026.1");
            let bundle = SoraDeploymentBundleV1 {
                schema_version: iroha_data_model::soracloud::SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
                container: SoraContainerManifestV1 {
                    schema_version: iroha_data_model::soracloud::SORA_CONTAINER_MANIFEST_VERSION_V1,
                    runtime: SoraContainerRuntimeV1::Ivm,
                    bundle_hash,
                    bundle_path: "/bundles/portal.ivm".to_string(),
                    entrypoint: "main".to_string(),
                    args: Vec::new(),
                    env: std::collections::BTreeMap::new(),
                    inrou: None,
                    required_config_names: Vec::new(),
                    required_secret_names: Vec::new(),
                    config_exports: Vec::new(),
                    capabilities: SoraCapabilityPolicyV1 {
                        network: SoraNetworkPolicyV1::Isolated,
                        allow_wallet_signing: false,
                        allow_state_writes: false,
                        allow_model_inference: false,
                        allow_model_training: false,
                    },
                    resources: SoraResourceLimitsV1 {
                        cpu_millis: NonZeroU32::new(500).expect("nonzero cpu"),
                        memory_bytes: NonZeroU64::new(16 * 1024 * 1024).expect("nonzero memory"),
                        ephemeral_storage_bytes: NonZeroU64::new(16 * 1024 * 1024)
                            .expect("nonzero storage"),
                        max_open_files: NonZeroU32::new(256).expect("nonzero files"),
                        max_tasks: NonZeroU16::new(16).expect("nonzero tasks"),
                    },
                    lifecycle: SoraLifecycleHooksV1 {
                        start_grace_secs: NonZeroU32::new(5).expect("nonzero start grace"),
                        stop_grace_secs: NonZeroU32::new(5).expect("nonzero stop grace"),
                        healthcheck_path: Some("/health".to_string()),
                    },
                },
                service: SoraServiceManifestV1 {
                    schema_version: iroha_data_model::soracloud::SORA_SERVICE_MANIFEST_VERSION_V1,
                    service_name: service_name.clone(),
                    service_version: service_version.clone(),
                    execution_plane:
                        iroha_data_model::soracloud::SoraServiceExecutionPlaneV1::DeterministicService,
                    container: SoraContainerManifestRefV1 {
                        manifest_hash: Hash::new(b"container-manifest:portal"),
                        expected_schema_version:
                            iroha_data_model::soracloud::SORA_CONTAINER_MANIFEST_VERSION_V1,
                    },
                    replicas: NonZeroU16::new(1).expect("nonzero replicas"),
                    route: None,
                    rollout: SoraRolloutPolicyV1 {
                        canary_percent: 0,
                        max_unavailable_replicas: 0,
                        health_window_secs: NonZeroU32::new(30).expect("nonzero health window"),
                        automatic_rollback_failures: NonZeroU32::new(1).expect("nonzero rollback"),
                    },
                    economics: iroha_data_model::soracloud::SoraHttpServiceEconomicsV1::default(),
                    state_bindings,
                    lease_volumes: Vec::new(),
                    handlers: vec![SoraServiceHandlerV1 {
                        handler_name: "update".parse().expect("valid handler name"),
                        class: SoraServiceHandlerClassV1::Update,
                        entrypoint: "apply_update".to_string(),
                        route_path: Some("/update".to_string()),
                        certified_response: SoraCertifiedResponsePolicyV1::None,
                        mailbox: Some(SoraMailboxContractV1 {
                            queue_name: "updates".parse().expect("valid queue name"),
                            max_pending_messages: NonZeroU32::new(1_024)
                                .expect("nonzero pending limit"),
                            max_message_bytes: NonZeroU64::new(65_536)
                                .expect("nonzero message limit"),
                            retention_blocks: NonZeroU32::new(1_440).expect("nonzero retention"),
                        }),
                    }],
                    artifacts: Vec::new(),
                },
            };
            world.soracloud_service_revisions_mut_for_testing().insert(
                (service_name.as_ref().to_owned(), service_version.clone()),
                bundle.clone(),
            );
            world
                .soracloud_service_deployments_mut_for_testing()
                .insert(
                    service_name.clone(),
                    SoraServiceDeploymentStateV1 {
                        schema_version:
                            iroha_data_model::soracloud::SORA_SERVICE_DEPLOYMENT_STATE_VERSION_V1,
                        service_name: service_name.clone(),
                        current_service_version: service_version.clone(),
                        current_service_manifest_hash: Hash::new(b"service-manifest:portal"),
                        current_container_manifest_hash: Hash::new(b"container-manifest:portal"),
                        revision_count: 1,
                        process_generation: 1,
                        process_started_sequence: 1,
                        active_rollout: None,
                        last_rollout: None,
                        config_generation: 0,
                        secret_generation: 0,
                        service_configs: BTreeMap::new(),
                        service_secrets: BTreeMap::new(),
                        service_lease: None,
                        lease_volume_states: Vec::new(),
                    },
                );
            world.soracloud_service_runtime_mut_for_testing().insert(
                service_name.clone(),
                SoraServiceRuntimeStateV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_SERVICE_RUNTIME_STATE_VERSION_V1,
                    service_name: service_name.clone(),
                    active_service_version: service_version,
                    health_status: SoraServiceHealthStatusV1::Healthy,
                    load_factor_bps: 77,
                    materialized_bundle_hash: bundle_hash,
                    rollout_handle: None,
                    pending_mailbox_message_count: 1,
                    last_receipt_id: None,
                },
            );
            let message_id = Hash::new(b"portal-mailbox-message");
            world.soracloud_mailbox_messages_mut_for_testing().insert(
                message_id,
                SoraServiceMailboxMessageV1 {
                    schema_version:
                        iroha_data_model::soracloud::SORA_SERVICE_MAILBOX_MESSAGE_VERSION_V1,
                    message_id,
                    from_service: service_name.clone(),
                    from_handler: "update".parse().expect("valid from handler"),
                    to_service: service_name.clone(),
                    to_handler: "update".parse().expect("valid to handler"),
                    payload_bytes: b"portal-mailbox-payload".to_vec(),
                    payload_commitment: Hash::new(b"portal-mailbox-payload"),
                    enqueue_sequence: 1,
                    available_after_sequence: 1,
                    expires_at_sequence: Some(16),
                },
            );
            (service_name, message_id)
        }

        #[test]
        fn signature_changes_clear_verified_flag() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(2)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());

            block.mark_signatures_verified();
            assert!(block.signatures_verified_for_tests());

            block.sign(&key_pairs[1], &topology);
            assert!(!block.signatures_verified_for_tests());
        }

        #[test]
        fn validate_and_record_transactions_executes_soracloud_mailbox_runtime_once() {
            let mut world = World::new();
            let (service_name, message_id) = seed_soracloud_mailbox_fixture(&mut world, Vec::new());
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new_for_testing(world, kura, query_handle);
            let runtime = CountingSoracloudRuntime::default();
            state.set_soracloud_runtime(Some(Arc::new(runtime.clone())));
            let leader = KeyPair::random();

            let block = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
                .chain(0, None)
                .sign(leader.private_key())
                .unpack(|_| {});
            let mut state_block = state.block(block.header);
            let _valid = block.validate_and_record_transactions(&mut state_block);
            state_block.commit().expect("commit first mailbox block");

            {
                let view = state.view();
                let world = view.world();
                let runtime_state = world
                    .soracloud_service_runtime()
                    .get(&service_name)
                    .expect("runtime state after execution");
                let receipt = world
                    .soracloud_runtime_receipts()
                    .iter()
                    .next()
                    .map(|(_receipt_id, receipt)| receipt.clone())
                    .expect("runtime receipt recorded");

                assert_eq!(runtime.ordered_mailbox_call_count(), 1);
                assert_eq!(runtime_state.pending_mailbox_message_count, 0);
                assert_eq!(runtime_state.last_receipt_id, Some(receipt.receipt_id));
                assert_eq!(receipt.mailbox_message_id, Some(message_id));
            }

            let follow_up_header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut follow_up_state_block = state.block(follow_up_header);
            let follow_up_transaction = follow_up_state_block.transaction();
            assert!(
                collect_ready_soracloud_mailbox_messages(&follow_up_transaction).is_empty(),
                "mailbox receipts must suppress re-delivery on later blocks"
            );

            let view = state.view();
            let world = view.world();
            assert_eq!(runtime.ordered_mailbox_call_count(), 1);
            assert_eq!(world.soracloud_runtime_receipts().iter().count(), 1);
        }

        #[test]
        fn validate_and_record_transactions_persists_soracloud_mailbox_state_mutations() {
            let mut world = World::new();
            let binding_name: iroha_data_model::name::Name =
                "vault".parse().expect("valid binding name");
            let state_key = "/state/private/patient-1".to_string();
            let payload = b"portal-runtime-state-payload".to_vec();
            let payload_commitment = Hash::new(&payload);
            let (service_name, message_id) = seed_soracloud_mailbox_fixture(
                &mut world,
                vec![SoraStateBindingV1 {
                    schema_version: SORA_STATE_BINDING_VERSION_V1,
                    binding_name: binding_name.clone(),
                    scope: iroha_data_model::soracloud::SoraStateScopeV1::ServiceState,
                    mutability: SoraStateMutabilityV1::ReadWrite,
                    encryption: SoraStateEncryptionV1::Plaintext,
                    key_prefix: "/state/private".to_string(),
                    max_item_bytes: NonZeroU64::new(512).expect("nonzero item bytes"),
                    max_total_bytes: NonZeroU64::new(2_048).expect("nonzero total bytes"),
                }],
            );
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new_for_testing(world, kura, query_handle);
            let runtime = CountingSoracloudRuntime::with_state_mutations(vec![
                SoracloudDeterministicStateMutation {
                    binding_name: binding_name.to_string(),
                    state_key: state_key.clone(),
                    operation: SoraStateMutationOperationV1::Upsert,
                    encryption: SoraStateEncryptionV1::Plaintext,
                    payload_bytes: Some(u64::try_from(payload.len()).expect("payload length")),
                    payload: Some(payload),
                    payload_commitment: Some(payload_commitment),
                },
            ]);
            state.set_soracloud_runtime(Some(Arc::new(runtime.clone())));
            let leader = KeyPair::random();

            let block = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
                .chain(0, None)
                .sign(leader.private_key())
                .unpack(|_| {});
            let mut state_block = state.block(block.header);
            let _valid = block.validate_and_record_transactions(&mut state_block);
            state_block.commit().expect("commit mailbox state block");

            let receipt = {
                let view = state.view();
                let world = view.world();
                let runtime_state = world
                    .soracloud_service_runtime()
                    .get(&service_name)
                    .expect("runtime state after state mutation execution");
                let receipt = world
                    .soracloud_runtime_receipts()
                    .iter()
                    .next()
                    .map(|(_receipt_id, receipt)| receipt.clone())
                    .expect("runtime receipt recorded");
                let entry = world
                    .soracloud_service_state_entries()
                    .get(&(
                        service_name.as_ref().to_owned(),
                        binding_name.as_ref().to_owned(),
                        state_key.clone(),
                    ))
                    .expect("mailbox-driven service state entry");

                assert_eq!(runtime.ordered_mailbox_call_count(), 1);
                assert_eq!(runtime_state.pending_mailbox_message_count, 0);
                assert_eq!(runtime_state.last_receipt_id, Some(receipt.receipt_id));
                assert_eq!(receipt.mailbox_message_id, Some(message_id));
                assert_eq!(entry.encryption, SoraStateEncryptionV1::Plaintext);
                assert_eq!(entry.payload_bytes.get(), 28);
                assert_eq!(entry.payload_commitment, payload_commitment);
                assert_eq!(entry.governance_tx_hash, receipt.receipt_id);
                assert_eq!(entry.last_update_sequence, receipt.emitted_sequence);
                assert_eq!(
                    entry.source_action,
                    SoraServiceLifecycleActionV1::StateMutation
                );
                receipt
            };

            let follow_up_header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
            let mut follow_up_state_block = state.block(follow_up_header);
            let follow_up_transaction = follow_up_state_block.transaction();
            assert_eq!(
                crate::smartcontracts::isi::soracloud::next_soracloud_audit_sequence(
                    &follow_up_transaction
                ),
                receipt.emitted_sequence.saturating_add(1),
                "runtime receipts must advance the shared Soracloud execution sequence"
            );
            assert!(
                collect_ready_soracloud_mailbox_messages(&follow_up_transaction).is_empty(),
                "mailbox receipts must suppress re-delivery after state mutation write-back"
            );
        }

        fn commit_block_at_height(
            state: &State,
            kura: &Arc<Kura>,
            topology: &Topology,
            leader_private: &PrivateKey,
            height: u64,
            prev_hash: Option<HashOf<BlockHeader>>,
            creation_time_ms: u64,
        ) -> HashOf<BlockHeader> {
            let valid = ValidBlock::new_dummy_and_modify_header(leader_private, |header| {
                header
                    .set_height(NonZeroU64::new(height).expect("non-zero height in commit helper"));
                header.set_prev_block_hash(prev_hash);
                header.creation_time_ms = creation_time_ms;
            });
            let committed = valid.commit_unchecked().unpack(|_| {});
            {
                let mut state_block = state.block(committed.as_ref().header());
                let _ =
                    state_block.apply_without_execution(&committed, topology.as_ref().to_owned());
                state_block.commit().unwrap();
            }
            kura.store_block(committed.clone())
                .expect("store committed block");
            committed.as_ref().hash()
        }

        #[test]
        fn signature_verification_ok() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(7)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);

            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let block_hash = block.as_ref().hash();

            key_pairs
                .iter()
                .enumerate()
                // Include only peers in validator set
                .take(topology.min_votes_for_commit())
                // Skip leader since already singed
                .skip(1)
                .filter(|(i, _)| *i != 4) // Skip proxy tail
                .map(|(i, key_pair)| {
                    BlockSignature::new(
                        i as u64,
                        SignatureOf::from_hash(key_pair.private_key(), block_hash),
                    )
                })
                .try_for_each(|signature| block.add_signature(signature, &topology))
                .expect("Failed to add signatures");

            block.sign(&key_pairs[4], &topology);

            let _ = block.commit(&topology).unpack(|_| {}).unwrap();
        }

        #[test]
        fn signature_verification_consensus_not_required_ok() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(1)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);

            let block = ValidBlock::new_dummy(key_pairs[0].private_key());

            assert!(block.commit(&topology).unpack(|_| {}).is_ok());
        }

        /// Check requirement of having at least $2f + 1$ signatures in $3f + 1$ network
        #[test]
        fn signature_verification_not_enough_signatures() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(7)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);

            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            block.sign(&key_pairs[4], &topology);

            let err = block.commit(&topology).unpack(|_| {}).unwrap_err().1;
            assert_eq!(
                err.as_ref(),
                &BlockValidationError::SignatureVerification(
                    SignatureVerificationError::NotEnoughSignatures {
                        votes_count: 2,
                        min_votes_for_commit: topology.min_votes_for_commit(),
                    }
                )
            );
        }

        #[test]
        fn four_node_quorum_rejects_two_commit_signers() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(4)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            assert_eq!(topology.min_votes_for_commit(), 3);

            // Leader is signed by constructor; add only the proxy tail.
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            block.sign(&key_pairs[2], &topology);

            let tally = commit_signature_tally(block.as_ref(), &topology);
            assert_eq!(tally.counted, 2);
            assert_eq!(tally.present, 2);

            let err = block.commit(&topology).unpack(|_| {}).unwrap_err().1;
            assert_eq!(
                err.as_ref(),
                &BlockValidationError::SignatureVerification(
                    SignatureVerificationError::NotEnoughSignatures {
                        votes_count: 2,
                        min_votes_for_commit: 3
                    }
                )
            );
        }

        #[test]
        fn four_node_quorum_accepts_three_commit_signers() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(4)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            assert_eq!(topology.min_votes_for_commit(), 3);

            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            block.sign(&key_pairs[1], &topology); // validator
            block.sign(&key_pairs[2], &topology); // proxy tail

            let tally = commit_signature_tally(block.as_ref(), &topology);
            assert_eq!(tally.counted, 3);
            assert_eq!(tally.present, 3);
            assert_eq!(tally.set_b_signatures, 0);

            assert!(block.commit(&topology).unpack(|_| {}).is_ok());
        }

        #[test]
        fn commit_with_certificate_skips_signature_quorum() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(4)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            assert_eq!(topology.min_votes_for_commit(), 3);

            let block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let commit_result = block.commit_with_certificate().unpack(|_| {});

            assert!(
                commit_result.is_ok(),
                "commit_with_certificate should bypass signature quorum checks"
            );
            let strict_result = ValidBlock::new_dummy(key_pairs[0].private_key())
                .commit(&topology)
                .unpack(|_| {});
            assert!(
                strict_result.is_err(),
                "strict commit should still enforce signature quorum"
            );
        }

        #[test]
        fn commit_with_signers_accepts_full_roster_quorum() {
            // Six-node topology (min_votes_for_commit = 4). Provide a quorum that excludes the
            // leader (0) and proxy tail (3) but still spans the full roster.
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(6)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            assert_eq!(topology.min_votes_for_commit(), 4);

            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            // Populate the block with commit-role signatures so `is_commit` passes even though the
            // QC signer set omits the leader and proxy tail.
            block.sign(&key_pairs[1], &topology);
            block.sign(&key_pairs[2], &topology);
            block.sign(&key_pairs[4], &topology);
            block.sign(&key_pairs[5], &topology);
            let signers: BTreeSet<_> = [1_u32, 2_u32, 4_u32, 5_u32].into_iter().collect();

            let result = block
                .commit_with_signers(&topology, &signers, false)
                .unpack(|_| {});
            assert!(
                result.is_ok(),
                "quorum signers outside the first commit set should still be accepted: {result:?}"
            );
        }

        #[test]
        fn duplicate_signatures_rejected() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(2)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let block_hash = block.as_ref().hash();

            let mut signatures = BTreeSet::new();
            signatures.insert(BlockSignature::new(
                0,
                SignatureOf::from_hash(key_pairs[0].private_key(), block_hash),
            ));
            signatures.insert(BlockSignature::new(
                1,
                SignatureOf::from_hash(key_pairs[1].private_key(), block_hash),
            ));
            // Duplicate index with a different signature payload.
            let spoofing_key = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            signatures.insert(BlockSignature::new(
                1,
                SignatureOf::from_hash(spoofing_key.private_key(), block_hash),
            ));

            let err = block
                .replace_signatures(signatures, &topology)
                .unpack(|_| {})
                .unwrap_err();
            assert_eq!(
                err,
                SignatureVerificationError::DuplicateSignature { signer: 1 }
            );
            // Original signature set should remain intact after the failed replacement.
            assert_eq!(
                block.as_ref().signatures().count(),
                1,
                "failed replacement must roll back"
            );
        }

        #[test]
        fn proxy_tail_signature_mismatch_rejected() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(2)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let block_hash = block.as_ref().hash();

            let mut signatures = BTreeSet::new();
            signatures.insert(BlockSignature::new(
                0,
                SignatureOf::from_hash(key_pairs[0].private_key(), block_hash),
            ));
            // Proxy tail index signed with the wrong key.
            let wrong = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            signatures.insert(BlockSignature::new(
                1,
                SignatureOf::from_hash(wrong.private_key(), block_hash),
            ));

            let err = block
                .replace_signatures(signatures, &topology)
                .unpack(|_| {})
                .unwrap_err();
            assert_eq!(err, SignatureVerificationError::UnknownSignature);
            // Original leader-only signature remains after rollback.
            assert_eq!(block.as_ref().signatures().count(), 1);
        }

        #[test]
        fn leader_signature_mismatch_rejected() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(3)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let block_hash = block.as_ref().hash();

            let mut signatures = BTreeSet::new();
            // Leader slot signed with validator key instead of the leader's.
            signatures.insert(BlockSignature::new(
                0,
                SignatureOf::from_hash(key_pairs[1].private_key(), block_hash),
            ));
            signatures.insert(BlockSignature::new(
                1,
                SignatureOf::from_hash(key_pairs[1].private_key(), block_hash),
            ));
            signatures.insert(BlockSignature::new(
                2,
                SignatureOf::from_hash(key_pairs[2].private_key(), block_hash),
            ));

            let err = block
                .replace_signatures(signatures, &topology)
                .unpack(|_| {})
                .unwrap_err();
            assert_eq!(err, SignatureVerificationError::UnknownSignature);
            assert_eq!(block.as_ref().signatures().count(), 1);
        }

        #[test]
        fn set_b_signatures_contribute_to_quorum() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(4)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);

            // Leader signature is included by constructor
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            block.sign(&key_pairs[1], &topology); // validator
            block.sign(&key_pairs[3], &topology); // set B

            assert!(
                block.commit(&topology).unpack(|_| {}).is_ok(),
                "set B signatures should count toward quorum without requiring proxy tail"
            );
        }

        #[test]
        fn set_b_signature_mismatch_rejected() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(5)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let block_hash = block.as_ref().hash();

            // Set B signature forged with the wrong key should invalidate the block.
            let bogus_set_b = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let mut signatures = BTreeSet::new();
            signatures.insert(BlockSignature::new(
                0,
                SignatureOf::from_hash(key_pairs[0].private_key(), block_hash),
            ));
            signatures.insert(BlockSignature::new(
                1,
                SignatureOf::from_hash(key_pairs[1].private_key(), block_hash),
            ));
            signatures.insert(BlockSignature::new(
                2,
                SignatureOf::from_hash(key_pairs[2].private_key(), block_hash),
            ));
            signatures.insert(BlockSignature::new(
                3,
                SignatureOf::from_hash(bogus_set_b.private_key(), block_hash),
            ));

            let err = block
                .replace_signatures(signatures, &topology)
                .unpack(|_| {})
                .unwrap_err();
            assert_eq!(err, SignatureVerificationError::UnknownSignature);
            // Replacement should fail and leave the original leader-only signature set.
            assert_eq!(block.as_ref().signatures().count(), 1);
        }

        #[test]
        fn commit_signature_tally_tracks_present_and_counted_roles() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(4)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);

            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let block_hash = block.as_ref().hash();
            // Proxy tail signature should count toward quorum.
            block
                .add_signature(
                    BlockSignature::new(
                        2,
                        SignatureOf::from_hash(key_pairs[2].private_key(), block_hash),
                    ),
                    &topology,
                )
                .expect("proxy tail signature");
            // Set B signature counts toward quorum.
            block.sign(&key_pairs[3], &topology);

            let tally = commit_signature_tally(block.as_ref(), &topology);
            assert_eq!(tally.present, 3);
            assert_eq!(tally.counted, 3);
            assert_eq!(tally.set_b_signatures, 1);
        }

        #[test]
        fn replace_signatures_rolls_back_on_failure() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(3)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let block_hash = block.as_ref().hash();

            // Start from a valid quorum.
            block
                .add_signature(
                    BlockSignature::new(
                        1,
                        SignatureOf::from_hash(key_pairs[1].private_key(), block_hash),
                    ),
                    &topology,
                )
                .expect("validator signature");
            block.sign(&key_pairs[2], &topology);
            assert!(block.clone().commit(&topology).unpack(|_| {}).is_ok());
            let original = block.as_ref().signatures().cloned().collect::<Vec<_>>();

            // Replacement below quorum should fail and restore the original set.
            let mut replacement = BTreeSet::new();
            replacement.insert(BlockSignature::new(
                0,
                SignatureOf::from_hash(key_pairs[0].private_key(), block_hash),
            ));
            replacement.insert(BlockSignature::new(
                1,
                SignatureOf::from_hash(key_pairs[1].private_key(), block_hash),
            ));

            let err = block
                .replace_signatures(replacement, &topology)
                .unpack(|_| {})
                .unwrap_err();
            assert_eq!(
                err,
                SignatureVerificationError::NotEnoughSignatures {
                    votes_count: 2,
                    min_votes_for_commit: 3
                }
            );
            let restored: Vec<_> = block.as_ref().signatures().cloned().collect();
            assert_eq!(restored, original);
        }

        #[test]
        fn consensus_key_lifecycle_requires_proxy_tail_entry() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(3)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut block =
                ValidBlock::new_dummy_and_modify_header(key_pairs[0].private_key(), |header| {
                    header.set_height(nonzero!(5_u64));
                });
            block.sign(&key_pairs[1], &topology);
            block.sign(&key_pairs[2], &topology);

            let mut world = World::new();
            insert_consensus_key(
                &mut world,
                "leader",
                &key_pairs[0],
                1,
                None,
                ConsensusKeyStatus::Active,
            );
            insert_consensus_key(
                &mut world,
                "validator",
                &key_pairs[1],
                1,
                None,
                ConsensusKeyStatus::Active,
            );
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(world, kura, query);
            let view = state.view();

            let err = ValidBlock::enforce_consensus_key_lifecycle(block.as_ref(), &topology, &view)
                .expect_err("missing proxy tail consensus key should be rejected");
            assert_eq!(err, SignatureVerificationError::InactiveConsensusKey);
        }

        #[test]
        fn validate_signatures_subset_rejects_missing_pop() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(2)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let block = ValidBlock::new_dummy(key_pairs[0].private_key());

            let mut world = World::new();
            let id = ConsensusKeyId::new(
                ConsensusKeyRole::Validator,
                Ident::from_str("leader").expect("consensus key name parses"),
            );
            let record = ConsensusKeyRecord {
                id: id.clone(),
                public_key: key_pairs[0].public_key().clone(),
                pop: None,
                activation_height: 1,
                expiry_height: None,
                hsm: None,
                replaces: None,
                status: ConsensusKeyStatus::Active,
            };
            world.consensus_keys.insert(id.clone(), record.clone());
            world
                .consensus_keys_by_pk
                .insert(record.public_key.to_string(), vec![id.clone()]);
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(world, kura, query);
            let view = state.view();

            let err = ValidBlock::validate_signatures_subset(block.as_ref(), &topology, &view)
                .expect_err("missing pop should be rejected");
            assert_eq!(err, SignatureVerificationError::MissingPop);
        }

        #[test]
        fn validate_signatures_subset_accepts_without_consensus_registry() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(2)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let block = ValidBlock::new_dummy(key_pairs[0].private_key());

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(World::new(), kura, query);
            let view = state.view();

            ValidBlock::validate_signatures_subset(block.as_ref(), &topology, &view)
                .expect("empty consensus key registry should use direct signature checks");
        }

        #[test]
        fn consensus_key_lifecycle_honours_grace_windows() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(3)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut params = iroha_data_model::parameter::Parameters::default();
            params.sumeragi.key_overlap_grace_blocks = 2;
            params.sumeragi.key_expiry_grace_blocks = 1;

            let mut world = World::new();
            world.parameters = mv::cell::Cell::new(params);
            insert_consensus_key(
                &mut world,
                "leader",
                &key_pairs[0],
                2,
                Some(5),
                ConsensusKeyStatus::Active,
            );
            insert_consensus_key(
                &mut world,
                "validator",
                &key_pairs[1],
                2,
                Some(5),
                ConsensusKeyStatus::Active,
            );
            insert_consensus_key(
                &mut world,
                "proxy",
                &key_pairs[2],
                2,
                Some(5),
                ConsensusKeyStatus::Retiring,
            );
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(world, kura, query);
            let view = state.view();

            let mut within_grace =
                ValidBlock::new_dummy_and_modify_header(key_pairs[0].private_key(), |header| {
                    header.set_height(nonzero!(6_u64));
                });
            within_grace.sign(&key_pairs[1], &topology);
            within_grace.sign(&key_pairs[2], &topology);
            assert!(
                ValidBlock::enforce_consensus_key_lifecycle(
                    within_grace.as_ref(),
                    &topology,
                    &view
                )
                .is_ok()
            );

            let mut beyond_grace =
                ValidBlock::new_dummy_and_modify_header(key_pairs[0].private_key(), |header| {
                    header.set_height(nonzero!(7_u64));
                });
            beyond_grace.sign(&key_pairs[1], &topology);
            beyond_grace.sign(&key_pairs[2], &topology);
            let err = ValidBlock::enforce_consensus_key_lifecycle(
                beyond_grace.as_ref(),
                &topology,
                &view,
            )
            .expect_err("expired consensus keys should be rejected after grace");
            assert_eq!(err, SignatureVerificationError::InactiveConsensusKey);
        }

        #[test]
        fn consensus_key_lifecycle_falls_back_for_stale_pk_index() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(3)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);
            let mut world = World::new();
            insert_consensus_key(
                &mut world,
                "leader-active",
                &key_pairs[0],
                1,
                None,
                ConsensusKeyStatus::Active,
            );
            insert_consensus_key(
                &mut world,
                "validator",
                &key_pairs[1],
                1,
                None,
                ConsensusKeyStatus::Active,
            );
            insert_consensus_key(
                &mut world,
                "proxy",
                &key_pairs[2],
                1,
                None,
                ConsensusKeyStatus::Active,
            );
            // Simulate a stale pk → id index entry that omits the active record.
            let stale_id = ConsensusKeyId::new(
                ConsensusKeyRole::Validator,
                Ident::from_str("stale").expect("ident parses"),
            );
            world
                .consensus_keys_by_pk
                .insert(key_pairs[0].public_key().to_string(), vec![stale_id]);
            // Active record remains available after inserting stale index entry.

            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(world, kura, query);
            let view = state.view();

            let mut block =
                ValidBlock::new_dummy_and_modify_header(key_pairs[0].private_key(), |header| {
                    header.set_height(nonzero!(3_u64));
                });
            block.sign(&key_pairs[1], &topology);
            block.sign(&key_pairs[2], &topology);
            assert!(
                ValidBlock::enforce_consensus_key_lifecycle(block.as_ref(), &topology, &view)
                    .is_ok()
            );
        }

        #[test]
        fn validate_static_snapshot_accepts_valid_block() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let key_pairs = vec![KeyPair::random_with_algorithm(Algorithm::BlsNormal)];
            let topology = test_topology_with_keys(&key_pairs);
            let leader = &key_pairs[0];

            let mut world = World::new();
            insert_consensus_key(
                &mut world,
                "leader",
                leader,
                0,
                None,
                ConsensusKeyStatus::Active,
            );
            let state = State::new_for_testing(world, Arc::clone(&kura), query);

            let prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 1);

            let candidate =
                ValidBlock::new_dummy_and_modify_header(leader.private_key(), |header| {
                    header.set_height(nonzero!(2_u64));
                    header.set_prev_block_hash(Some(prev_hash));
                    header.creation_time_ms = 2;
                });
            let signed: SignedBlock = candidate.into();

            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(2));
            let static_data = {
                let view = state.query_view();
                ValidBlock::validate_static_state_dependent(
                    &signed,
                    &topology,
                    &state.chain_id,
                    &ALICE_ID,
                    &view,
                    false,
                    &time_source,
                    false,
                    false,
                )
                .expect("static state-dependent validation should succeed")
            };
            let prepared_txs = ValidBlock::prepare_external_transactions(&signed);
            let committed_heights = {
                let transactions_view = state.transactions.view();
                ValidBlock::committed_heights_for_prepared_transactions(
                    &prepared_txs,
                    &transactions_view,
                )
            };
            #[cfg(feature = "telemetry")]
            let metrics = Some(&state.telemetry);
            #[cfg(not(feature = "telemetry"))]
            let metrics = ();
            ValidBlock::validate_static_with_snapshot(
                &signed,
                &state.chain_id,
                &ALICE_ID,
                &static_data,
                &committed_heights,
                &prepared_txs,
                None,
                false,
                metrics,
            )
            .expect("static snapshot validation should succeed");
        }

        fn npos_marker(epoch: u64, height: u64) -> NposPenaltyAction {
            NposPenaltyAction::MarkVrfPenaltiesApplied(NposMarkVrfPenaltiesAppliedAction {
                epoch,
                height,
            })
        }

        fn npos_vrf_record(
            epoch: u64,
            updated_at_height: u64,
            finalized: bool,
            participants: Vec<VrfParticipantRecord>,
        ) -> VrfEpochRecord {
            VrfEpochRecord {
                epoch,
                seed: [0x42; 32],
                epoch_length: 10,
                commit_deadline_offset: 3,
                reveal_deadline_offset: 6,
                roster_len: 2,
                finalized,
                updated_at_height,
                participants,
                late_reveals: Vec::new(),
                committed_no_reveal: Vec::new(),
                no_participation: Vec::new(),
                penalties_applied: false,
                penalties_applied_at_height: None,
                validator_election: None,
            }
        }

        fn npos_vrf_participant(
            signer: u32,
            commitment_byte: u8,
            reveal_byte: Option<u8>,
            last_updated_height: u64,
        ) -> VrfParticipantRecord {
            VrfParticipantRecord {
                signer,
                commitment: Some([commitment_byte; 32]),
                reveal: reveal_byte.map(|byte| [byte; 32]),
                last_updated_height,
            }
        }

        fn npos_effects_block(
            leader_private_key: &PrivateKey,
            height: u64,
            effects: Option<NposConsensusEffects>,
        ) -> SignedBlock {
            let valid = ValidBlock::new_dummy_and_modify_header(leader_private_key, |header| {
                header.set_height(NonZeroU64::new(height).expect("non-zero height"));
            });
            let mut block: SignedBlock = valid.into();
            block.set_npos_consensus_effects(effects);
            block
        }

        fn vrf_epoch_record_for_test(epoch: u64, height: u64) -> VrfEpochRecord {
            VrfEpochRecord {
                epoch,
                seed: [0x42; 32],
                epoch_length: 10,
                commit_deadline_offset: 3,
                reveal_deadline_offset: 6,
                roster_len: 4,
                finalized: false,
                updated_at_height: height,
                participants: vec![VrfParticipantRecord {
                    signer: 0,
                    commitment: Some([0x11; 32]),
                    reveal: None,
                    last_updated_height: height,
                }],
                late_reveals: Vec::new(),
                committed_no_reveal: Vec::new(),
                no_participation: Vec::new(),
                penalties_applied: false,
                penalties_applied_at_height: None,
                validator_election: None,
            }
        }

        #[test]
        fn validate_npos_effects_allows_vrf_record_monotonic_extension() {
            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let mut world = World::new();
            let existing = vrf_epoch_record_for_test(1, 12);
            world.vrf_epochs.insert(existing.epoch, existing.clone());
            let state = State::new_for_testing(
                world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );

            let mut proposed = existing;
            proposed.updated_at_height = 14;
            proposed.participants[0].reveal = Some([0x55; 32]);
            proposed.participants[0].last_updated_height = 14;
            proposed.participants.push(VrfParticipantRecord {
                signer: 1,
                commitment: Some([0x22; 32]),
                reveal: Some([0x33; 32]),
                last_updated_height: 14,
            });
            proposed.late_reveals.push(VrfLateRevealRecord {
                signer: 0,
                reveal: [0x44; 32],
                noted_at_height: 14,
            });

            let block = npos_effects_block(
                leader.private_key(),
                15,
                Some(NposConsensusEffects {
                    vrf_epoch_seals: vec![proposed],
                    penalty_actions: Vec::new(),
                }),
            );

            ValidBlock::validate_npos_effects_with_state(&block, &state)
                .expect("monotonic VRF epoch record extension should validate");
        }

        #[test]
        fn validate_npos_effects_rejects_vrf_record_rewrite() {
            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let mut world = World::new();
            let existing = vrf_epoch_record_for_test(1, 12);
            world.vrf_epochs.insert(existing.epoch, existing.clone());
            let state = State::new_for_testing(
                world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );

            let mut proposed = existing;
            proposed.seed = [0x99; 32];
            proposed.updated_at_height = 14;

            let block = npos_effects_block(
                leader.private_key(),
                15,
                Some(NposConsensusEffects {
                    vrf_epoch_seals: vec![proposed],
                    penalty_actions: Vec::new(),
                }),
            );

            let err = ValidBlock::validate_npos_effects_with_state(&block, &state)
                .expect_err("VRF epoch record rewrite should be rejected");
            assert!(matches!(err, BlockValidationError::NposEffectsInvalid(_)));
        }

        #[test]
        fn validate_npos_effects_allows_vrf_epoch_record_extensions() {
            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let mut world = World::new();
            world.vrf_epochs.insert(
                0,
                npos_vrf_record(0, 2, false, vec![npos_vrf_participant(0, 0xAA, None, 2)]),
            );
            let state = State::new_for_testing(
                world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let block = npos_effects_block(
                leader.private_key(),
                4,
                Some(NposConsensusEffects {
                    vrf_epoch_seals: vec![npos_vrf_record(
                        0,
                        4,
                        false,
                        vec![
                            npos_vrf_participant(0, 0xAA, Some(0xBB), 4),
                            npos_vrf_participant(1, 0xCC, None, 4),
                        ],
                    )],
                    penalty_actions: Vec::new(),
                }),
            );

            ValidBlock::validate_npos_effects_with_state(&block, &state)
                .expect("monotonic VRF epoch record extension should validate");
        }

        #[test]
        fn validate_npos_effects_rejects_vrf_epoch_record_rewrites() {
            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let mut world = World::new();
            world.vrf_epochs.insert(
                0,
                npos_vrf_record(0, 2, false, vec![npos_vrf_participant(0, 0xAA, None, 2)]),
            );
            let state = State::new_for_testing(
                world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let block = npos_effects_block(
                leader.private_key(),
                4,
                Some(NposConsensusEffects {
                    vrf_epoch_seals: vec![npos_vrf_record(
                        0,
                        4,
                        false,
                        vec![npos_vrf_participant(0, 0xCC, None, 4)],
                    )],
                    penalty_actions: Vec::new(),
                }),
            );

            let err = ValidBlock::validate_npos_effects_with_state(&block, &state)
                .expect_err("VRF epoch record rewrite must be rejected");
            assert!(matches!(err, BlockValidationError::NposEffectsInvalid(_)));
        }

        #[test]
        fn validate_npos_effects_rejects_missing_required_actions() {
            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let mut world = World::new();
            world.vrf_epochs.insert(
                7,
                VrfEpochRecord {
                    epoch: 7,
                    seed: [0x42; 32],
                    epoch_length: 10,
                    commit_deadline_offset: 3,
                    reveal_deadline_offset: 6,
                    roster_len: 1,
                    finalized: true,
                    updated_at_height: 1,
                    participants: Vec::new(),
                    late_reveals: Vec::new(),
                    committed_no_reveal: Vec::new(),
                    no_participation: Vec::new(),
                    penalties_applied: false,
                    penalties_applied_at_height: None,
                    validator_election: None,
                },
            );
            let state = State::new_for_testing(
                world,
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let block = npos_effects_block(leader.private_key(), 20, None);

            let err = ValidBlock::validate_npos_effects_with_state(&block, &state)
                .expect_err("missing deterministic NPoS marker must be rejected");
            assert!(matches!(err, BlockValidationError::NposEffectsInvalid(_)));
        }

        #[test]
        fn validate_npos_effects_rejects_extra_actions() {
            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let state = State::new_for_testing(
                World::new(),
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let block = npos_effects_block(
                leader.private_key(),
                2,
                Some(NposConsensusEffects {
                    vrf_epoch_seals: Vec::new(),
                    penalty_actions: vec![npos_marker(99, 2)],
                }),
            );

            let err = ValidBlock::validate_npos_effects_with_state(&block, &state)
                .expect_err("extra deterministic NPoS action must be rejected");
            assert!(matches!(err, BlockValidationError::NposEffectsInvalid(_)));
        }

        #[test]
        fn validate_npos_effects_rejects_malformed_actions() {
            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let state = State::new_for_testing(
                World::new(),
                Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test(),
            );
            let action = npos_marker(1, 2);
            let block = npos_effects_block(
                leader.private_key(),
                2,
                Some(NposConsensusEffects {
                    vrf_epoch_seals: Vec::new(),
                    penalty_actions: vec![action.clone(), action],
                }),
            );

            let err = ValidBlock::validate_npos_effects_with_state(&block, &state)
                .expect_err("duplicate NPoS actions must be rejected");
            assert!(matches!(err, BlockValidationError::NposEffectsInvalid(_)));
        }

        #[test]
        fn validate_static_snapshot_rejects_invalid_signature() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let key_pairs = vec![KeyPair::random_with_algorithm(Algorithm::BlsNormal)];
            let topology = test_topology_with_keys(&key_pairs);
            let leader = &key_pairs[0];

            let mut world = World::new();
            insert_consensus_key(
                &mut world,
                "leader",
                leader,
                0,
                None,
                ConsensusKeyStatus::Active,
            );
            let state = State::new_for_testing(world, Arc::clone(&kura), query);

            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 1);

            let (alice_id, alice_keypair) = gen_account_in("wonderland");
            let (bob_id, _) = gen_account_in("wonderland");
            let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let tx = TransactionBuilder::new_with_time_source(
                state.chain_id.clone(),
                alice_id,
                &time_source,
            )
            .with_instructions([Log::new(Level::INFO, "test".to_string())])
            .sign(alice_keypair.private_key());
            let tx = tx.with_authority(bob_id);
            let tx = AcceptedTransaction::new_unchecked(Cow::Owned(tx));

            time_handle.advance(Duration::from_millis(1));
            let new_block = BlockBuilder::new_with_time_source(vec![tx], time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .sign(leader.private_key())
                .unpack(|_| {});
            let signed: SignedBlock = new_block.into();

            let static_data = {
                let view = state.query_view();
                ValidBlock::validate_static_state_dependent(
                    &signed,
                    &topology,
                    &state.chain_id,
                    &ALICE_ID,
                    &view,
                    false,
                    &time_source,
                    false,
                    false,
                )
                .expect("static state-dependent validation should succeed")
            };
            let prepared_txs = ValidBlock::prepare_external_transactions(&signed);
            let committed_heights = {
                let transactions_view = state.transactions.view();
                ValidBlock::committed_heights_for_prepared_transactions(
                    &prepared_txs,
                    &transactions_view,
                )
            };
            #[cfg(feature = "telemetry")]
            let metrics = Some(&state.telemetry);
            #[cfg(not(feature = "telemetry"))]
            let metrics = ();

            let err = ValidBlock::validate_static_with_snapshot(
                &signed,
                &state.chain_id,
                &ALICE_ID,
                &static_data,
                &committed_heights,
                &prepared_txs,
                None,
                false,
                metrics,
            )
            .expect_err("invalid tx signature should be rejected");
            assert!(matches!(
                err,
                BlockValidationError::TransactionAccept(
                    AcceptTransactionFail::SignatureVerification(_)
                )
            ));
        }

        #[test]
        fn validate_static_snapshot_rejects_duplicate_signed_transaction_hashes() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let key_pairs = vec![KeyPair::random_with_algorithm(Algorithm::BlsNormal)];
            let topology = test_topology_with_keys(&key_pairs);
            let leader = &key_pairs[0];

            let mut world = World::new();
            insert_consensus_key(
                &mut world,
                "leader",
                leader,
                0,
                None,
                ConsensusKeyStatus::Active,
            );
            let state = State::new_for_testing(world, Arc::clone(&kura), query);

            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 1);

            let (authority, signer) = gen_account_in("duplicate-check");
            let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let tx = TransactionBuilder::new_with_time_source(
                state.chain_id.clone(),
                authority,
                &time_source,
            )
            .with_instructions([Log::new(Level::INFO, "duplicate".to_owned())])
            .sign(signer.private_key());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));

            time_handle.advance(Duration::from_millis(1));
            let new_block = BlockBuilder::new_with_time_source(
                vec![accepted.clone(), accepted],
                time_source.clone(),
            )
            .chain(0, state.view().latest_block().as_deref())
            .sign(leader.private_key())
            .unpack(|_| {});
            let signed: SignedBlock = new_block.into();

            let static_data = {
                let view = state.query_view();
                ValidBlock::validate_static_state_dependent(
                    &signed,
                    &topology,
                    &state.chain_id,
                    &ALICE_ID,
                    &view,
                    false,
                    &time_source,
                    false,
                    false,
                )
                .expect("static state-dependent validation should succeed")
            };
            let prepared_txs = ValidBlock::prepare_external_transactions(&signed);
            let committed_heights = {
                let transactions_view = state.transactions.view();
                ValidBlock::committed_heights_for_prepared_transactions(
                    &prepared_txs,
                    &transactions_view,
                )
            };
            #[cfg(feature = "telemetry")]
            let metrics = Some(&state.telemetry);
            #[cfg(not(feature = "telemetry"))]
            let metrics = ();

            let err = ValidBlock::validate_static_with_snapshot(
                &signed,
                &state.chain_id,
                &ALICE_ID,
                &static_data,
                &committed_heights,
                &prepared_txs,
                None,
                false,
                metrics,
            )
            .expect_err("duplicate signed transaction hash should be rejected");
            assert!(matches!(err, BlockValidationError::DuplicateTransactions));
        }

        #[test]
        fn validate_static_state_dependent_rejects_missing_execution_context() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let key_pairs = vec![KeyPair::random_with_algorithm(Algorithm::BlsNormal)];
            let topology = test_topology_with_keys(&key_pairs);
            let leader = &key_pairs[0];

            let mut world = World::new();
            insert_consensus_key(
                &mut world,
                "leader",
                leader,
                0,
                None,
                ConsensusKeyStatus::Active,
            );
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 1);

            let (authority, signer) = gen_account_in("context-check");
            let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let tx = TransactionBuilder::new_with_time_source(
                state.chain_id.clone(),
                authority,
                &time_source,
            )
            .with_instructions([Log::new(Level::INFO, "context".to_owned())])
            .sign(signer.private_key());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
            time_handle.advance(Duration::from_millis(1));

            let new_block = BlockBuilder::new_with_time_source(vec![accepted], time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .sign(leader.private_key())
                .unpack(|_| {});
            let mut signed: SignedBlock = new_block.into();
            signed.set_execution_context(None);

            let err = {
                let view = state.query_view();
                match ValidBlock::validate_static_state_dependent(
                    &signed,
                    &topology,
                    &state.chain_id,
                    &ALICE_ID,
                    &view,
                    false,
                    &time_source,
                    false,
                    false,
                ) {
                    Ok(_) => panic!("live block without execution context must be rejected"),
                    Err(err) => err,
                }
            };
            assert!(matches!(
                err,
                BlockValidationError::ExecutionContextInvalid(ref message)
                    if message.contains("missing execution context")
            ));
        }

        #[test]
        fn validate_static_state_dependent_rejects_execution_context_route_mismatch() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let key_pairs = vec![KeyPair::random_with_algorithm(Algorithm::BlsNormal)];
            let topology = test_topology_with_keys(&key_pairs);
            let leader = &key_pairs[0];

            let mut world = World::new();
            insert_consensus_key(
                &mut world,
                "leader",
                leader,
                0,
                None,
                ConsensusKeyStatus::Active,
            );
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 1);

            let (authority, signer) = gen_account_in("context-check");
            let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let tx = TransactionBuilder::new_with_time_source(
                state.chain_id.clone(),
                authority,
                &time_source,
            )
            .with_instructions([Log::new(Level::INFO, "context".to_owned())])
            .sign(signer.private_key());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
            time_handle.advance(Duration::from_millis(1));

            let new_block = BlockBuilder::new_with_time_source(vec![accepted], time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .sign(leader.private_key())
                .unpack(|_| {});
            let mut signed: SignedBlock = new_block.into();
            let wrong_context =
                BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                    tx.hash_as_entrypoint(),
                    LaneId::new(7),
                    DataSpaceId::UNIVERSAL,
                )]);
            signed.set_execution_context(Some(wrong_context));

            let err = {
                let view = state.query_view();
                match ValidBlock::validate_static_state_dependent(
                    &signed,
                    &topology,
                    &state.chain_id,
                    &ALICE_ID,
                    &view,
                    false,
                    &time_source,
                    false,
                    false,
                ) {
                    Ok(_) => {
                        panic!("live block with mismatched execution context must be rejected")
                    }
                    Err(err) => err,
                }
            };
            assert!(matches!(
                err,
                BlockValidationError::ExecutionContextInvalid(ref message)
                    if message.contains("route cannot be resolved")
            ));
        }

        #[test]
        fn validate_static_state_dependent_rejects_committed_context_when_policy_derives_default() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let key_pairs = vec![KeyPair::random_with_algorithm(Algorithm::BlsNormal)];
            let topology = test_topology_with_keys(&key_pairs);
            let leader = &key_pairs[0];

            let (authority, signer) = gen_account_in("context-check");
            let domain_id = DomainId::try_new("context-check", "universal").expect("domain id");
            let domain = Domain::new(domain_id).build(&authority);
            let account = Account::new(authority.clone()).build(&authority);
            let mut world = World::with([domain], [account], []);
            insert_consensus_key(
                &mut world,
                "leader",
                leader,
                0,
                None,
                ConsensusKeyStatus::Active,
            );
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            let paynet_lane = LaneId::new(3);
            let paynet_dataspace = DataSpaceId::new(10);
            {
                let mut nexus = state.nexus.write();
                nexus.lane_catalog = LaneCatalog::new(
                    nonzero!(4_u32),
                    vec![
                        LaneConfig::default(),
                        LaneConfig {
                            id: paynet_lane,
                            dataspace_id: paynet_dataspace,
                            alias: "paynet".to_owned(),
                            ..LaneConfig::default()
                        },
                    ],
                )
                .expect("lane catalog");
                nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
                    DataSpaceMetadata::default(),
                    DataSpaceMetadata {
                        id: paynet_dataspace,
                        alias: "paynet".to_owned(),
                        description: None,
                        fault_tolerance: 1,
                    },
                ])
                .expect("dataspace catalog");
            }
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 1);

            let (time_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let tx = TransactionBuilder::new_with_time_source(
                state.chain_id.clone(),
                authority,
                &time_source,
            )
            .with_instructions([Log::new(Level::INFO, "context".to_owned())])
            .sign(signer.private_key());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx.clone()));
            time_handle.advance(Duration::from_millis(1));

            let execution_context =
                BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
                    tx.hash_as_entrypoint(),
                    paynet_lane,
                    paynet_dataspace,
                )]);
            let new_block = BlockBuilder::new_with_time_source(vec![accepted], time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .with_execution_context(Some(execution_context))
                .sign(leader.private_key())
                .unpack(|_| {});
            let signed: SignedBlock = new_block.into();

            let view = state.query_view();
            let err = ValidBlock::validate_static_state_dependent(
                &signed,
                &topology,
                &state.chain_id,
                &ALICE_ID,
                &view,
                false,
                &time_source,
                false,
                false,
            )
            .expect_err("default-routed transactions must not accept arbitrary durable routing");
            assert!(matches!(
                err,
                BlockValidationError::ExecutionContextInvalid(ref message)
                    if message.contains("routing mismatch")
            ));
        }

        #[test]
        fn validate_static_snapshot_rejects_missing_previous_roster_evidence_after_height_two() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let key_pairs = vec![KeyPair::random_with_algorithm(Algorithm::BlsNormal)];
            let topology = test_topology_with_keys(&key_pairs);
            let leader = &key_pairs[0];

            let mut world = World::new();
            insert_consensus_key(
                &mut world,
                "leader",
                leader,
                0,
                None,
                ConsensusKeyStatus::Active,
            );
            let state = State::new_for_testing(world, Arc::clone(&kura), query);

            let genesis_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 1);
            let prev_hash = commit_block_at_height(
                &state,
                &kura,
                &topology,
                leader.private_key(),
                2,
                Some(genesis_hash),
                2,
            );

            let candidate =
                ValidBlock::new_dummy_and_modify_header(leader.private_key(), |header| {
                    header.set_height(nonzero!(3_u64));
                    header.set_prev_block_hash(Some(prev_hash));
                    header.creation_time_ms = 3;
                });
            let signed: SignedBlock = candidate.into();
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(3));
            let err = {
                let view = state.query_view();
                match ValidBlock::validate_static_state_dependent(
                    &signed,
                    &topology,
                    &state.chain_id,
                    &ALICE_ID,
                    &view,
                    false,
                    &time_source,
                    false,
                    false,
                ) {
                    Ok(_) => panic!("height > 2 blocks must carry previous-roster evidence"),
                    Err(err) => err,
                }
            };
            assert!(matches!(
                err,
                BlockValidationError::PreviousRosterEvidenceInvalid(ref message)
                    if message.contains("missing required previous-roster evidence")
            ));
        }

        #[test]
        fn validate_and_record_transactions_skip_stateless_matches_full() {
            let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
            let (alice_id, alice_keypair) = gen_account_in("wonderland");
            let domain_id: DomainId =
                DomainId::try_new("wonderland", "universal").expect("valid domain");
            let account = Account::new(alice_id.clone()).build(&alice_id);
            let domain = Domain::new(domain_id).build(&alice_id);
            let world = World::with([domain], [account], []);
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(world, kura, query_handle);
            let (max_clock_drift, tx_limits) = {
                let state_view = state.world.view();
                let params = state_view.parameters();
                (params.sumeragi().max_clock_drift(), params.transaction())
            };

            let tx = TransactionBuilder::new(chain_id.clone(), alice_id)
                .with_instructions([Log::new(Level::INFO, "test".to_string())])
                .sign(alice_keypair.private_key());
            let crypto_cfg = state.crypto();
            let tx = AcceptedTransaction::accept(
                tx,
                &chain_id,
                max_clock_drift,
                tx_limits,
                crypto_cfg.as_ref(),
            )
            .expect("valid tx");

            let new_block = BlockBuilder::new(vec![tx.clone()])
                .chain(0, state.view().latest_block().as_deref())
                .sign(alice_keypair.private_key())
                .unpack(|_| {});

            let mut full_block: SignedBlock = new_block.clone().into();
            let mut state_block = state.block(full_block.header());
            ValidBlock::validate_and_record_transactions(
                &mut full_block,
                &mut state_block,
                None,
                false,
            )
            .expect("full validation should attach transaction results");
            let full_results: Vec<_> = full_block
                .results()
                .map(|result| result.as_ref().is_ok())
                .collect();
            drop(state_block);

            let mut skip_block: SignedBlock = new_block.into();
            let mut state_block = state.block(skip_block.header());
            ValidBlock::validate_and_record_transactions(
                &mut skip_block,
                &mut state_block,
                None,
                true,
            )
            .expect("skip-stateless validation should attach transaction results");
            let skip_results: Vec<_> = skip_block
                .results()
                .map(|result| result.as_ref().is_ok())
                .collect();

            assert_eq!(full_results, skip_results);
        }

        #[test]
        fn validate_keep_voting_block_rejects_unknown_da_lane() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![PeerId::new(leader.public_key().clone())]);

            let mut world = World::new();
            insert_consensus_key(
                &mut world,
                "validator",
                &leader,
                0,
                None,
                ConsensusKeyStatus::Active,
            );
            let mut params = Parameters::default();
            params.sumeragi.da_enabled = true;
            world.parameters = Cell::new(params);
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 0);
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));

            let record = DaCommitmentRecord::new(
                LaneId::new(7),
                1,
                1,
                BlobDigest::new([0xAA; 32]),
                ManifestDigest::new([0xBB; 32]),
                DaProofScheme::MerkleSha256,
                Hash::prehashed([0xCC; 32]),
                Some(KzgCommitment::new([0xDD; 48])),
                None,
                RetentionClass::default(),
                StorageTicketId::new([0xEE; 32]),
                Signature::from_bytes(&[0x11; 64]),
            );
            let bundle = DaCommitmentBundle::new(vec![record]);
            let new_block = BlockBuilder::new_with_time_source(Vec::new(), time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .with_da_commitments(Some(bundle))
                .sign(leader.private_key())
                .unpack(|_| {});
            let signed: SignedBlock = new_block.into();

            let mut voting_block = None;
            let (_handle, time_source) = TimeSource::new_mock(signed.header().creation_time());
            let result = ValidBlock::validate_keep_voting_block(
                signed,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &time_source,
                &state,
                &mut voting_block,
                false,
            )
            .unpack(|_| {});

            let Err((_, err)) = result else {
                panic!("expected DA shard cursor rejection");
            };
            assert!(matches!(
                err.as_ref(),
                BlockValidationError::DaShardCursor(DaShardCursorError::UnknownLane { .. })
            ));
        }

        #[test]
        fn validate_keep_voting_block_rejects_da_cursor_regression() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![PeerId::new(leader.public_key().clone())]);

            let mut world = World::new();
            insert_consensus_key(
                &mut world,
                "validator",
                &leader,
                0,
                None,
                ConsensusKeyStatus::Active,
            );
            let mut params = Parameters::default();
            params.sumeragi.da_enabled = true;
            world.parameters = Cell::new(params);
            let state = State::new_for_testing(world, Arc::clone(&kura), query);
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 0);
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(2));

            let advance = DaCommitmentRecord::new(
                LaneId::new(0),
                2,
                3,
                BlobDigest::new([0xAB; 32]),
                ManifestDigest::new([0xBC; 32]),
                DaProofScheme::MerkleSha256,
                Hash::prehashed([0xCD; 32]),
                Some(KzgCommitment::new([0xDE; 48])),
                None,
                RetentionClass::default(),
                StorageTicketId::new([0xEF; 32]),
                Signature::from_bytes(&[0x12; 64]),
            );
            state
                .ensure_da_indexes_hydrated()
                .expect("DA indexes hydrate for cursor regression test");
            {
                let shard_id = state.nexus_snapshot().lane_config.shard_id(advance.lane_id);
                state
                    .da_shard_cursors
                    .write()
                    .advance(shard_id, &advance, 1)
                    .expect("initial cursor advance");
            }
            {
                let cursors = state.da_shard_cursor_index();
                let cursor = cursors.get(0).expect("cursor seeded");
                assert_eq!((cursor.epoch, cursor.sequence), (2, 3));
            }

            let regression = DaCommitmentRecord::new(
                LaneId::new(0),
                2,
                2,
                BlobDigest::new([0xAA; 32]),
                ManifestDigest::new([0xBB; 32]),
                DaProofScheme::MerkleSha256,
                Hash::prehashed([0xCC; 32]),
                Some(KzgCommitment::new([0xDD; 48])),
                None,
                RetentionClass::default(),
                StorageTicketId::new([0xEE; 32]),
                Signature::from_bytes(&[0x13; 64]),
            );
            let bundle = DaCommitmentBundle::new(vec![regression]);

            let new_block = BlockBuilder::new_with_time_source(Vec::new(), time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .with_da_commitments(Some(bundle))
                .sign(leader.private_key())
                .unpack(|_| {});
            let signed: SignedBlock = new_block.into();

            let mut voting_block = None;
            let (_handle, time_source) = TimeSource::new_mock(signed.header().creation_time());
            let result = ValidBlock::validate_keep_voting_block(
                signed,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &time_source,
                &state,
                &mut voting_block,
                false,
            )
            .unpack(|_| {});

            let Err((_, err)) = result else {
                panic!("expected DA shard cursor regression rejection");
            };
            assert!(
                matches!(
                    err.as_ref(),
                    BlockValidationError::DaShardCursor(DaShardCursorError::Regression { .. })
                ),
                "unexpected error: {err:?}"
            );
        }

        #[test]
        fn validate_keep_voting_block_rejects_expired_consensus_keys() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let proxy_tail = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(leader.public_key().clone()),
                PeerId::new(proxy_tail.public_key().clone()),
            ]);

            let mut params = Parameters::default();
            params.sumeragi.key_overlap_grace_blocks = 0;
            params.sumeragi.key_expiry_grace_blocks = 0;

            let mut world = World::new();
            world.parameters = Cell::new(params);
            insert_consensus_key(
                &mut world,
                "leader-expired",
                &leader,
                0,
                Some(1),
                ConsensusKeyStatus::Active,
            );
            insert_consensus_key(
                &mut world,
                "proxy-expired",
                &proxy_tail,
                0,
                Some(1),
                ConsensusKeyStatus::Active,
            );
            let state = State::new(world, Arc::clone(&kura), query);

            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 0);

            let height = 2_u64;
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(2));
            let tx_params = state.view().world().parameters().transaction();
            let heartbeat_signer = KeyPair::random_with_algorithm(Algorithm::Ed25519);
            let heartbeat = crate::tx::build_heartbeat_transaction_with_time_source(
                state.chain_id.clone(),
                &heartbeat_signer,
                &tx_params,
                height,
                &time_source,
            );
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(heartbeat));
            let prev_block = state.view().latest_block().expect("previous block");
            let mut signed: SignedBlock =
                BlockBuilder::new_with_time_source(vec![accepted], time_source.clone())
                    .chain(0, Some(prev_block.as_ref()))
                    .sign(leader.private_key())
                    .unpack(|_| {})
                    .into();
            let block_hash = signed.hash();
            let proxy_idx = topology
                .position(proxy_tail.public_key())
                .expect("proxy tail in topology");
            signed
                .add_signature(BlockSignature::new(
                    proxy_idx as u64,
                    SignatureOf::from_hash(proxy_tail.private_key(), block_hash),
                ))
                .expect("proxy tail signature");
            assert_eq!(signed.external_transactions().count(), 1);
            let mut voting_block = None;
            let result = ValidBlock::validate_keep_voting_block(
                signed,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &time_source,
                &state,
                &mut voting_block,
                false,
            )
            .unpack(|_| {});

            let Err((_, err)) = result else {
                panic!("expected expired consensus key rejection");
            };
            assert!(matches!(
                err.as_ref(),
                BlockValidationError::SignatureVerification(
                    SignatureVerificationError::InactiveConsensusKey
                )
            ));
        }

        #[test]
        fn validate_keep_voting_block_allows_overlap_grace_window() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let proxy_tail = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(leader.public_key().clone()),
                PeerId::new(proxy_tail.public_key().clone()),
            ]);

            let mut params = Parameters::default();
            params.sumeragi.key_overlap_grace_blocks = 1;
            params.sumeragi.key_expiry_grace_blocks = 0;

            let mut world = World::new();
            world.parameters = Cell::new(params);
            insert_consensus_key(
                &mut world,
                "leader-overlap",
                &leader,
                0,
                Some(2),
                ConsensusKeyStatus::Active,
            );
            insert_consensus_key(
                &mut world,
                "proxy-overlap",
                &proxy_tail,
                0,
                Some(2),
                ConsensusKeyStatus::Retiring,
            );
            let state = State::new(world, Arc::clone(&kura), query);

            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(2));
            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 0);
            let height = 2_u64;
            let tx_params = state.view().world().parameters().transaction();
            let heartbeat_signer = KeyPair::random_with_algorithm(Algorithm::Ed25519);
            let heartbeat = crate::tx::build_heartbeat_transaction_with_time_source(
                state.chain_id.clone(),
                &heartbeat_signer,
                &tx_params,
                height,
                &time_source,
            );
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(heartbeat));
            let prev_block = state.view().latest_block().expect("previous block");
            let mut signed: SignedBlock =
                BlockBuilder::new_with_time_source(vec![accepted], time_source.clone())
                    .chain(0, Some(prev_block.as_ref()))
                    .sign(leader.private_key())
                    .unpack(|_| {})
                    .into();
            let block_hash = signed.hash();
            let proxy_idx = topology
                .position(proxy_tail.public_key())
                .expect("proxy tail in topology");
            signed
                .add_signature(BlockSignature::new(
                    proxy_idx as u64,
                    SignatureOf::from_hash(proxy_tail.private_key(), block_hash),
                ))
                .expect("proxy tail signature");
            assert_eq!(signed.external_transactions().count(), 1);
            let mut voting_block = None;
            let result = ValidBlock::validate_keep_voting_block(
                signed,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &time_source,
                &state,
                &mut voting_block,
                false,
            )
            .unpack(|_| {});

            if let Err((_, err)) = result {
                panic!("overlap grace should permit signatures at expiry height, got {err:?}");
            }
        }

        #[test]
        fn validate_keep_voting_block_rejects_missing_proxy_tail_key() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let proxy_tail = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![
                PeerId::new(leader.public_key().clone()),
                PeerId::new(proxy_tail.public_key().clone()),
            ]);

            let mut params = Parameters::default();
            params.sumeragi.key_overlap_grace_blocks = 0;
            params.sumeragi.key_expiry_grace_blocks = 0;

            let mut world = World::new();
            world.parameters = Cell::new(params);
            insert_consensus_key(
                &mut world,
                "leader-only",
                &leader,
                0,
                None,
                ConsensusKeyStatus::Active,
            );
            // Deliberately omit the proxy-tail consensus key to exercise the missing-key path.
            let state = State::new(world, Arc::clone(&kura), query);

            let prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 0);

            let mut candidate =
                ValidBlock::new_dummy_and_modify_header(leader.private_key(), |header| {
                    header.set_height(nonzero!(2_u64));
                    header.set_prev_block_hash(Some(prev_hash));
                    header.creation_time_ms = 1;
                });
            candidate.sign(&proxy_tail, &topology);
            let signed: SignedBlock = candidate.into();

            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(2));
            let mut voting_block = None;
            let result = ValidBlock::validate_keep_voting_block(
                signed,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &time_source,
                &state,
                &mut voting_block,
                false,
            )
            .unpack(|_| {});

            let Err((_, err)) = result else {
                panic!("expected missing proxy-tail consensus key rejection");
            };
            assert!(matches!(
                err.as_ref(),
                BlockValidationError::SignatureVerification(
                    SignatureVerificationError::InactiveConsensusKey
                )
            ));
        }

        #[test]
        fn maps_signature_verification_errors() {
            assert_eq!(
                map_sig_err_to_reason(&SignatureVerificationError::NotEnoughSignatures {
                    votes_count: 1,
                    min_votes_for_commit: 2
                }),
                Reason::InsufficientBlockSignatures
            );
            assert_eq!(
                map_sig_err_to_reason(&SignatureVerificationError::UnknownSignatory),
                Reason::UnknownBlockSignatory
            );
            assert_eq!(
                map_sig_err_to_reason(&SignatureVerificationError::DuplicateSignature {
                    signer: 0
                }),
                Reason::InvalidBlockSignature
            );
            assert_eq!(
                map_sig_err_to_reason(&SignatureVerificationError::UnknownSignature),
                Reason::InvalidBlockSignature
            );
            assert_eq!(
                map_sig_err_to_reason(&SignatureVerificationError::MissingPop),
                Reason::InvalidBlockSignature
            );
            assert_eq!(
                map_sig_err_to_reason(&SignatureVerificationError::ProxyTailMissing),
                Reason::ProxyTailSignatureMissing
            );
            assert_eq!(
                map_sig_err_to_reason(&SignatureVerificationError::LeaderMissing),
                Reason::LeaderSignatureMissing
            );
            assert_eq!(
                map_sig_err_to_reason(&SignatureVerificationError::Other),
                Reason::OtherSignatureError
            );
        }

        /// Check quorum requirement when proxy tail is missing.
        #[test]
        fn signature_verification_rejects_insufficient_quorum_without_proxy_tail() {
            let key_pairs =
                core::iter::repeat_with(|| KeyPair::random_with_algorithm(Algorithm::BlsNormal))
                    .take(7)
                    .collect::<Vec<_>>();
            let topology = test_topology_with_keys(&key_pairs);

            let mut block = ValidBlock::new_dummy(key_pairs[0].private_key());
            let block_hash = block.as_ref().hash();
            key_pairs
                .iter()
                .enumerate()
                // Include only peers in validator set
                .take(topology.min_votes_for_commit())
                // Skip leader since already singed
                .skip(1)
                .filter(|(i, _)| *i != 4) // Skip proxy tail
                .map(|(i, key_pair)| {
                    BlockSignature::new(
                        i as u64,
                        SignatureOf::from_hash(key_pair.private_key(), block_hash),
                    )
                })
                .try_for_each(|signature| block.add_signature(signature, &topology))
                .expect("Failed to add signatures");

            let err = block.commit(&topology).unpack(|_| {}).unwrap_err().1;
            assert_eq!(
                err.as_ref(),
                &BlockValidationError::SignatureVerification(
                    SignatureVerificationError::NotEnoughSignatures {
                        votes_count: topology.min_votes_for_commit() - 1,
                        min_votes_for_commit: topology.min_votes_for_commit(),
                    }
                )
            );
        }

        #[test]
        fn maps_block_validation_errors() {
            assert_eq!(
                map_block_err_to_reason(&BlockValidationError::MerkleRootMismatch),
                Reason::MerkleRootMismatch
            );
            assert_eq!(
                map_block_err_to_reason(&BlockValidationError::EmptyBlock),
                Reason::EmptyBlock
            );
            assert_eq!(
                map_block_err_to_reason(&BlockValidationError::DuplicateTransactions),
                Reason::TransactionValidationFailed
            );
            assert_eq!(
                map_block_err_to_reason(&BlockValidationError::SignatureVerification(
                    SignatureVerificationError::LeaderMissing
                )),
                Reason::LeaderSignatureMissing
            );
            let chain_mismatch = BlockValidationError::TransactionAccept(
                AcceptTransactionFail::ChainIdMismatch(Mismatch {
                    expected: "chain_a".parse().unwrap(),
                    actual: "chain_b".parse().unwrap(),
                }),
            );
            assert_eq!(
                map_block_err_to_reason(&chain_mismatch),
                Reason::TransactionValidationFailed
            );
            let tx_limit = BlockValidationError::TransactionAccept(
                AcceptTransactionFail::TransactionLimit(TransactionLimitError {
                    reason: "too big".into(),
                }),
            );
            assert_eq!(
                map_block_err_to_reason(&tx_limit),
                Reason::TransactionValidationFailed
            );
            assert_eq!(
                map_block_err_to_reason(&BlockValidationError::InvalidGenesis(
                    InvalidGenesisError::ContainsErrors
                )),
                Reason::InvalidGenesis
            );
            assert_eq!(
                map_block_err_to_reason(&BlockValidationError::ConfidentialFeaturesMismatch {
                    expected: None,
                    actual: None
                }),
                Reason::ConfidentialFeatureDigestMismatch
            );
            let policy_err = BlockValidationError::ProofPolicyHashMismatch {
                expected: HashOf::from_untyped_unchecked(Hash::prehashed([1; Hash::LENGTH])),
                actual: None,
            };
            assert_eq!(
                map_block_err_to_reason(&policy_err),
                Reason::DaProofPolicyMismatch
            );
        }

        #[test]
        fn maps_transaction_future_error() {
            let err = BlockValidationError::TransactionInTheFuture;
            assert_eq!(
                map_block_err_to_reason(&err),
                Reason::TransactionInTheFuture
            );
        }

        #[test]
        fn empty_block_rejected_during_validation() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let state = State::new(World::new(), Arc::clone(&kura), query);

            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let (leader_public, leader_private) = leader.into_parts();
            let topology = Topology::new(vec![PeerId::new(leader_public.clone())]);

            // Commit a dummy previous block so the state has height == 1.
            let prev_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
                header.set_height(nonzero!(1_u64));
                header.creation_time_ms = 0;
            });
            let prev_committed = prev_valid.commit_unchecked().unpack(|_| {});
            {
                let mut prev_state_block = state.block(prev_committed.as_ref().header());
                let _ = prev_state_block
                    .apply_without_execution(&prev_committed, topology.as_ref().to_owned());
                prev_state_block.commit().unwrap();
            }
            kura.store_block(prev_committed.clone())
                .expect("store previous block");
            let prev_hash = prev_committed.as_ref().hash();

            // Candidate block with no overlays (should be rejected).
            let candidate_block = {
                let valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
                    header.set_height(nonzero!(2_u64));
                    header.set_prev_block_hash(Some(prev_hash));
                    header.creation_time_ms = 1;
                    header.merkle_root = None;
                });
                SignedBlock::from(valid)
            };

            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            {
                let mut state_block = state.block(candidate_block.header());
                let validate_result = ValidBlock::validate(
                    candidate_block.clone(),
                    &topology,
                    &state.chain_id.clone(),
                    &ALICE_ID,
                    &time_source,
                    &mut state_block,
                )
                .unpack(|_| {});
                let err = match validate_result {
                    Ok(_) => panic!("empty block should be rejected"),
                    Err(err) => err,
                };
                assert!(matches!(err.1.as_ref(), BlockValidationError::EmptyBlock));
            }

            {
                let mut state_block = state.block(candidate_block.header());
                let events = std::cell::RefCell::new(Vec::new());
                let validate_result = ValidBlock::validate_with_events(
                    candidate_block.clone(),
                    &topology,
                    &state.chain_id.clone(),
                    &ALICE_ID,
                    &time_source,
                    &mut state_block,
                    |event| {
                        events.borrow_mut().push(event);
                    },
                )
                .unpack(|_| {});
                let err = match validate_result {
                    Ok(_) => panic!("empty block should be rejected"),
                    Err(err) => err,
                };
                assert!(matches!(err.1.as_ref(), BlockValidationError::EmptyBlock));
                assert!(events.borrow().iter().any(|event| {
                    matches!(
                        event,
                        PipelineEventBox::Block(block_event)
                            if matches!(block_event.status, BlockStatus::Rejected(Reason::EmptyBlock))
                    )
                }));
            }

            let mut voting_block: Option<super::super::VotingBlock> = None;
            let result = ValidBlock::validate_keep_voting_block(
                candidate_block,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &time_source,
                &state,
                &mut voting_block,
                false,
            )
            .unpack(|_| {});

            let err = match result {
                Ok(_) => panic!("empty block should be rejected"),
                Err(err) => err,
            };
            assert!(matches!(err.1.as_ref(), BlockValidationError::EmptyBlock));
        }

        #[test]
        fn da_only_block_is_not_rejected_as_empty() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let topology = Topology::new(vec![PeerId::new(leader.public_key().clone())]);

            let mut world = World::new();
            insert_consensus_key(
                &mut world,
                "validator",
                &leader,
                0,
                None,
                ConsensusKeyStatus::Active,
            );
            let mut params = Parameters::default();
            params.sumeragi.da_enabled = true;
            world.parameters = Cell::new(params);
            let state = State::new_for_testing(world, Arc::clone(&kura), query);

            let _prev_hash =
                commit_block_at_height(&state, &kura, &topology, leader.private_key(), 1, None, 0);

            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let record = DaCommitmentRecord::new(
                LaneId::new(0),
                1,
                1,
                BlobDigest::new([0xAA; 32]),
                ManifestDigest::new([0xBB; 32]),
                DaProofScheme::MerkleSha256,
                Hash::prehashed([0xCC; 32]),
                Some(KzgCommitment::new([0xDD; 48])),
                None,
                RetentionClass::default(),
                StorageTicketId::new([0xEE; 32]),
                Signature::from_bytes(&[0x11; 64]),
            );
            let bundle = DaCommitmentBundle::new(vec![record]);
            let new_block = BlockBuilder::new_with_time_source(Vec::new(), time_source.clone())
                .chain(0, state.view().latest_block().as_deref())
                .with_da_commitments(Some(bundle))
                .sign(leader.private_key())
                .unpack(|_| {});
            let signed_block: SignedBlock = new_block.into();

            let (_validation_handle, validation_time_source) =
                TimeSource::new_mock(signed_block.header().creation_time());

            {
                let mut state_block = state.block(signed_block.header());
                ValidBlock::validate(
                    signed_block.clone(),
                    &topology,
                    &state.chain_id.clone(),
                    &ALICE_ID,
                    &validation_time_source,
                    &mut state_block,
                )
                .unpack(|_| {})
                .expect("DA-only block should be accepted");
            }

            {
                let mut state_block = state.block(signed_block.header());
                let events = std::cell::RefCell::new(Vec::new());
                ValidBlock::validate_with_events(
                    signed_block.clone(),
                    &topology,
                    &state.chain_id.clone(),
                    &ALICE_ID,
                    &validation_time_source,
                    &mut state_block,
                    |event| {
                        events.borrow_mut().push(event);
                    },
                )
                .unpack(|_| {})
                .expect("DA-only block should be accepted");
                assert!(events.borrow().is_empty(), "no rejection events expected");
            }

            {
                let mut voting_block: Option<super::super::VotingBlock> = None;
                ValidBlock::validate_keep_voting_block(
                    signed_block.clone(),
                    &topology,
                    &state.chain_id.clone(),
                    &ALICE_ID,
                    &validation_time_source,
                    &state,
                    &mut voting_block,
                    false,
                )
                .unpack(|_| {})
                .expect("DA-only block should be accepted");
            }

            let mut voting_block: Option<super::super::VotingBlock> = None;
            let mut events = Vec::new();
            ValidBlock::validate_keep_voting_block_with_events(
                signed_block,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &validation_time_source,
                &state,
                &mut voting_block,
                false,
                |event| events.push(event),
            )
            .unpack(|_| {})
            .expect("DA-only block should be accepted");
            assert!(events.is_empty(), "no rejection events expected");
        }

        #[test]
        fn heartbeat_block_is_accepted() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let state = State::new(World::new(), Arc::clone(&kura), query);

            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let (leader_public, leader_private) = leader.into_parts();
            let topology = Topology::new(vec![PeerId::new(leader_public.clone())]);

            // Commit a dummy previous block so the state has height == 1.
            let prev_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
                header.set_height(nonzero!(1_u64));
                header.creation_time_ms = 0;
            });
            let prev_committed = prev_valid.commit_unchecked().unpack(|_| {});
            {
                let mut prev_state_block = state.block(prev_committed.as_ref().header());
                let _ = prev_state_block
                    .apply_without_execution(&prev_committed, topology.as_ref().to_owned());
                prev_state_block.commit().unwrap();
            }
            kura.store_block(prev_committed.clone())
                .expect("store previous block");

            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(1));
            let tx_params = state.view().world().parameters().transaction();
            let signer = KeyPair::random_with_algorithm(Algorithm::Ed25519);
            let heartbeat = crate::tx::build_heartbeat_transaction_with_time_source(
                state.chain_id.clone(),
                &signer,
                &tx_params,
                2,
                &time_source,
            );
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(heartbeat));

            let builder = BlockBuilder::new_with_time_source(vec![accepted], time_source.clone());
            let new_block = builder
                .chain(0, Some(prev_committed.as_ref()))
                .sign(&leader_private)
                .unpack(|_| {});
            let signed_block: SignedBlock = new_block.into();

            let mut voting_block: Option<super::super::VotingBlock> = None;
            let result = ValidBlock::validate_keep_voting_block(
                signed_block,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &time_source,
                &state,
                &mut voting_block,
                false,
            )
            .unpack(|_| {});

            assert!(result.is_ok(), "heartbeat block should be accepted");
        }

        #[test]
        fn rejection_only_block_is_not_treated_as_empty() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let state = State::new(World::new(), Arc::clone(&kura), query);

            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let (leader_public, leader_private) = leader.into_parts();
            let topology = Topology::new(vec![PeerId::new(leader_public.clone())]);

            // Commit a dummy previous block so the state has height == 1.
            let prev_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
                header.set_height(nonzero!(1_u64));
                header.creation_time_ms = 0;
            });
            let prev_committed = prev_valid.commit_unchecked().unpack(|_| {});
            {
                let mut prev_state_block = state.block(prev_committed.as_ref().header());
                let _ = prev_state_block
                    .apply_without_execution(&prev_committed, topology.as_ref().to_owned());
                prev_state_block.commit().unwrap();
            }
            kura.store_block(prev_committed.clone())
                .expect("store previous block");

            // Build a transaction that will be rejected (authority account is absent).
            let (authority, signer) = gen_account_in("wonderland");
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(10));
            let tx = TransactionBuilder::new_with_time_source(
                state.chain_id.clone(),
                authority,
                &time_source,
            )
            .with_instructions([Log::new(Level::INFO, "reject-only".to_owned())])
            .sign(signer.private_key());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));

            // Assemble and validate a block that contains the rejected transaction.
            let builder = BlockBuilder::new_with_time_source(vec![accepted], time_source.clone());
            let new_block = builder
                .chain(0, Some(prev_committed.as_ref()))
                .sign(&leader_private)
                .unpack(|_| {});
            let signed_block: SignedBlock = SignedBlock::from(new_block);
            let mut voting_block: Option<super::super::VotingBlock> = None;

            let result = ValidBlock::validate_keep_voting_block(
                signed_block,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &time_source,
                &state,
                &mut voting_block,
                false,
            )
            .unpack(|_| {});

            let (valid_block, state_block) =
                result.expect("rejection-only block should not be treated as empty");
            assert_eq!(valid_block.as_ref().external_transactions().count(), 1);
            assert!(valid_block.as_ref().error(0).is_some());
            assert!(!state_block.has_committed_fragments());
        }

        #[test]
        fn validate_keep_voting_block_uses_block_time_for_ttl_checks() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let state = State::new(World::new(), Arc::clone(&kura), query);

            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let (leader_public, leader_private) = leader.into_parts();
            let topology = Topology::new(vec![PeerId::new(leader_public.clone())]);

            // Seed the chain with a committed block so height and timestamps are set.
            let _ = commit_block_at_height(&state, &kura, &topology, &leader_private, 1, None, 0);

            // Build a transaction that is valid at block time but would be expired against wall-clock now.
            let (_tx_handle, tx_time_source) = TimeSource::new_mock(Duration::from_millis(0));
            let (authority, signer) = gen_account_in("ttl-synced-block");
            let mut builder = TransactionBuilder::new_with_time_source(
                state.chain_id.clone(),
                authority,
                &tx_time_source,
            );
            builder.set_ttl(Duration::from_millis(100));
            let tx = builder
                .with_instructions([Log::new(Level::INFO, "ttl-valid-at-block-time".to_owned())])
                .sign(signer.private_key());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));

            let (_block_handle, block_time_source) =
                TimeSource::new_mock(Duration::from_millis(50));
            let builder =
                BlockBuilder::new_with_time_source(vec![accepted], block_time_source.clone());
            let new_block = builder
                .chain(0, state.view().latest_block().as_deref())
                .sign(&leader_private)
                .unpack(|_| {});
            let signed_block: SignedBlock = SignedBlock::from(new_block);

            // Validate using a clock far in the future; TTL should be evaluated at block time.
            let (_handle, validation_time_source) = TimeSource::new_mock(Duration::from_secs(10));
            let mut voting_block: Option<super::super::VotingBlock> = None;
            let result = ValidBlock::validate_keep_voting_block(
                signed_block,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &validation_time_source,
                &state,
                &mut voting_block,
                false,
            )
            .unpack(|_| {});

            assert!(
                result.is_ok(),
                "block validation should use block timestamp for TTL checks"
            );
        }

        #[test]
        fn validate_keep_voting_block_populates_stateless_cache() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let mut state = State::new(World::new(), Arc::clone(&kura), query);
            let mut pipeline = state.view().pipeline().clone();
            pipeline.stateless_cache_cap = 64;
            state.set_pipeline(pipeline);

            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let (leader_public, leader_private) = leader.into_parts();
            let topology = Topology::new(vec![PeerId::new(leader_public.clone())]);

            let _ = commit_block_at_height(&state, &kura, &topology, &leader_private, 1, None, 0);

            let (_tx_handle, tx_time_source) = TimeSource::new_mock(Duration::from_millis(0));
            let (authority, signer) = gen_account_in("cache-test");
            let tx = TransactionBuilder::new_with_time_source(
                state.chain_id.clone(),
                authority,
                &tx_time_source,
            )
            .with_instructions([Log::new(Level::INFO, "cacheable".to_owned())])
            .sign(signer.private_key());
            let tx_hash = tx.hash();
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));

            let (_block_handle, block_time_source) =
                TimeSource::new_mock(Duration::from_millis(10));
            let builder =
                BlockBuilder::new_with_time_source(vec![accepted], block_time_source.clone());
            let new_block = builder
                .chain(0, state.view().latest_block().as_deref())
                .sign(&leader_private)
                .unpack(|_| {});
            let signed_block: SignedBlock = SignedBlock::from(new_block);

            let mut voting_block: Option<super::super::VotingBlock> = None;
            let result = ValidBlock::validate_keep_voting_block(
                signed_block,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &block_time_source,
                &state,
                &mut voting_block,
                false,
            )
            .unpack(|_| {});
            assert!(
                result.is_ok(),
                "validation should succeed and warm stateless cache"
            );

            let cache = state.stateless_validation_cache().lock();
            assert!(
                cache.contains_key(&tx_hash),
                "successful static validation should populate stateless cache",
            );
        }

        #[test]
        fn block_validation_rejects_invalid_signature_despite_warmed_stateless_cache() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let mut state = State::new(World::new(), Arc::clone(&kura), query);
            let mut pipeline = state.view().pipeline().clone();
            pipeline.stateless_cache_cap = 64;
            state.set_pipeline(pipeline);

            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let (leader_public, leader_private) = leader.into_parts();
            let topology = Topology::new(vec![PeerId::new(leader_public.clone())]);

            let _ = commit_block_at_height(&state, &kura, &topology, &leader_private, 1, None, 0);

            let (_tx_handle, tx_time_source) = TimeSource::new_mock(Duration::from_millis(0));
            let (authority, signer) = gen_account_in("cache-signature-test");
            let (other_authority, _) = gen_account_in("cache-signature-test");
            let valid_tx = TransactionBuilder::new_with_time_source(
                state.chain_id.clone(),
                authority,
                &tx_time_source,
            )
            .with_instructions([Log::new(Level::INFO, "cacheable".to_owned())])
            .sign(signer.private_key());

            let valid_accepted = AcceptedTransaction::new_unchecked(Cow::Owned(valid_tx.clone()));
            let (_valid_block_handle, valid_block_time_source) =
                TimeSource::new_mock(Duration::from_millis(10));
            let valid_block =
                BlockBuilder::new_with_time_source(vec![valid_accepted], valid_block_time_source)
                    .chain(0, state.view().latest_block().as_deref())
                    .sign(&leader_private)
                    .unpack(|_| {});
            let valid_signed_block: SignedBlock = valid_block.into();

            let mut voting_block: Option<super::super::VotingBlock> = None;
            ValidBlock::validate_keep_voting_block(
                valid_signed_block,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &TimeSource::new_system(),
                &state,
                &mut voting_block,
                false,
            )
            .unpack(|_| {})
            .expect("valid block should warm stateless cache");

            let invalid_tx = valid_tx.with_authority(other_authority);
            let invalid_hash = invalid_tx.hash();
            let invalid_accepted = AcceptedTransaction::new_unchecked(Cow::Owned(invalid_tx));
            let (_invalid_block_handle, invalid_block_time_source) =
                TimeSource::new_mock(Duration::from_millis(20));
            let invalid_block = BlockBuilder::new_with_time_source(
                vec![invalid_accepted],
                invalid_block_time_source,
            )
            .chain(0, state.view().latest_block().as_deref())
            .sign(&leader_private)
            .unpack(|_| {});
            let invalid_signed_block: SignedBlock = invalid_block.into();

            {
                let mut cache = state.stateless_validation_cache().lock();
                cache.insert_ok(invalid_hash.clone(), None, 0);
                assert!(
                    cache.contains_key(&invalid_hash),
                    "test setup should present the invalid transaction as cache-warmed",
                );
            }

            let mut voting_block: Option<super::super::VotingBlock> = None;
            let result = ValidBlock::validate_keep_voting_block(
                invalid_signed_block,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &TimeSource::new_system(),
                &state,
                &mut voting_block,
                false,
            )
            .unpack(|_| {});
            let Err(err) = result else {
                panic!("invalid transaction signature must reject the block");
            };

            assert!(matches!(
                *err.1,
                BlockValidationError::TransactionAccept(
                    AcceptTransactionFail::SignatureVerification(_)
                )
            ));
        }

        #[test]
        fn transaction_signature_validation_has_no_bypass_terms() {
            let needles = [
                ["signature", "_", "override"].concat(),
                ["signature", "_", "overrides"].concat(),
                ["skip", "_tx", "_signature", "_validation"].concat(),
            ];
            let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
            let mut pending = vec![src.clone()];
            let mut hits = Vec::new();

            while let Some(path) = pending.pop() {
                let metadata = std::fs::metadata(&path).expect("source path metadata");
                if metadata.is_dir() {
                    for entry in std::fs::read_dir(&path).expect("source directory readable") {
                        pending.push(entry.expect("source directory entry").path());
                    }
                    continue;
                }
                if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                    continue;
                }

                let source = std::fs::read_to_string(&path).expect("Rust source readable");
                for needle in &needles {
                    if source.contains(needle) {
                        let relative = path.strip_prefix(&src).unwrap_or(&path);
                        hits.push(format!("{} contains {needle}", relative.display()));
                    }
                }
            }

            assert!(
                hits.is_empty(),
                "forbidden source terms:\n{}",
                hits.join("\n")
            );
        }

        #[test]
        fn validate_keep_voting_block_with_events_populates_stateless_cache() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let mut state = State::new(World::new(), Arc::clone(&kura), query);
            let mut pipeline = state.view().pipeline().clone();
            pipeline.stateless_cache_cap = 64;
            state.set_pipeline(pipeline);

            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let (leader_public, leader_private) = leader.into_parts();
            let topology = Topology::new(vec![PeerId::new(leader_public.clone())]);

            let _ = commit_block_at_height(&state, &kura, &topology, &leader_private, 1, None, 0);

            let (_tx_handle, tx_time_source) = TimeSource::new_mock(Duration::from_millis(0));
            let (authority, signer) = gen_account_in("cache-test");
            let tx = TransactionBuilder::new_with_time_source(
                state.chain_id.clone(),
                authority,
                &tx_time_source,
            )
            .with_instructions([Log::new(Level::INFO, "cacheable".to_owned())])
            .sign(signer.private_key());
            let tx_hash = tx.hash();
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));

            let (_block_handle, block_time_source) =
                TimeSource::new_mock(Duration::from_millis(10));
            let builder =
                BlockBuilder::new_with_time_source(vec![accepted], block_time_source.clone());
            let new_block = builder
                .chain(0, state.view().latest_block().as_deref())
                .sign(&leader_private)
                .unpack(|_| {});
            let signed_block: SignedBlock = SignedBlock::from(new_block);

            let mut voting_block: Option<super::super::VotingBlock> = None;
            let mut events = Vec::new();
            let mut timings = ValidationTimings::new();
            let result = ValidBlock::validate_keep_voting_block_with_events_and_timing(
                signed_block,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &block_time_source,
                &state,
                &mut voting_block,
                false,
                &mut timings,
                |event| events.push(event),
            )
            .unpack(|_| {});
            assert!(
                result.is_ok(),
                "validation with events should succeed and warm stateless cache"
            );
            assert!(events.is_empty(), "no rejection events expected");
            assert!(
                timings.total_ms >= timings.stateless_ms,
                "total validation timing should cover stateless timing"
            );

            let cache = state.stateless_validation_cache().lock();
            assert!(
                cache.contains_key(&tx_hash),
                "successful static validation with events should populate stateless cache",
            );
        }

        #[test]
        fn validate_prevalidated_commit_keep_voting_block_trusts_validated_signatures() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let state = State::new(World::new(), Arc::clone(&kura), query);

            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let (leader_public, leader_private) = leader.into_parts();
            let topology = Topology::new(vec![PeerId::new(leader_public.clone())]);

            let _ = commit_block_at_height(&state, &kura, &topology, &leader_private, 1, None, 0);

            let (_tx_handle, tx_time_source) = TimeSource::new_mock(Duration::from_millis(0));
            let (authority, signer) = gen_account_in("prevalidated-commit");
            let tx = TransactionBuilder::new_with_time_source(
                state.chain_id.clone(),
                authority,
                &tx_time_source,
            )
            .with_instructions([Log::new(Level::INFO, "prevalidated".to_owned())])
            .sign(signer.private_key());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));

            let (_block_handle, block_time_source) =
                TimeSource::new_mock(Duration::from_millis(10));
            let builder =
                BlockBuilder::new_with_time_source(vec![accepted], block_time_source.clone());
            let wrong_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let new_block = builder
                .chain(0, state.view().latest_block().as_deref())
                .sign(wrong_leader.private_key())
                .unpack(|_| {});
            let signed_block: SignedBlock = SignedBlock::from(new_block);

            let mut full_voting_block: Option<super::super::VotingBlock> = None;
            let full_result = ValidBlock::validate_keep_voting_block(
                signed_block.clone(),
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &block_time_source,
                &state,
                &mut full_voting_block,
                false,
            )
            .unpack(|_| {});
            assert!(
                full_result.is_err(),
                "ordinary validation should reject the intentionally wrong leader signature"
            );

            let mut voting_block: Option<super::super::VotingBlock> = None;
            let mut events = Vec::new();
            let mut timings = ValidationTimings::new();
            let result =
                ValidBlock::validate_prevalidated_commit_keep_voting_block_with_events_and_timing(
                    signed_block,
                    &topology,
                    &state.chain_id.clone(),
                    &ALICE_ID,
                    &block_time_source,
                    &state,
                    &mut voting_block,
                    &mut timings,
                    |event| events.push(event),
                )
                .unpack(|_| {});
            assert!(
                result.is_ok(),
                "prevalidated commit execution should trust previously checked signatures"
            );
            assert!(events.is_empty(), "no rejection events expected");
            assert!(
                timings.total_ms >= timings.execution_ms,
                "prevalidated timing should still include execution"
            );
        }

        #[test]
        fn validate_populates_stateless_cache() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let mut state = State::new(World::new(), Arc::clone(&kura), query);
            let mut pipeline = state.view().pipeline().clone();
            pipeline.stateless_cache_cap = 64;
            state.set_pipeline(pipeline);

            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let (leader_public, leader_private) = leader.into_parts();
            let topology = Topology::new(vec![PeerId::new(leader_public.clone())]);

            let _ = commit_block_at_height(&state, &kura, &topology, &leader_private, 1, None, 0);

            let (_tx_handle, tx_time_source) = TimeSource::new_mock(Duration::from_millis(0));
            let (authority, signer) = gen_account_in("cache-test");
            let tx = TransactionBuilder::new_with_time_source(
                state.chain_id.clone(),
                authority,
                &tx_time_source,
            )
            .with_instructions([Log::new(Level::INFO, "cacheable".to_owned())])
            .sign(signer.private_key());
            let tx_hash = tx.hash();
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));

            let (_block_handle, block_time_source) =
                TimeSource::new_mock(Duration::from_millis(10));
            let builder =
                BlockBuilder::new_with_time_source(vec![accepted], block_time_source.clone());
            let new_block = builder
                .chain(0, state.view().latest_block().as_deref())
                .sign(&leader_private)
                .unpack(|_| {});
            let signed_block: SignedBlock = SignedBlock::from(new_block);

            let mut state_block = state.block(signed_block.header());
            let result = ValidBlock::validate(
                signed_block,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &block_time_source,
                &mut state_block,
            )
            .unpack(|_| {});
            assert!(result.is_ok(), "validation should warm stateless cache");
            drop(state_block);

            let cache = state.stateless_validation_cache().lock();
            assert!(
                cache.contains_key(&tx_hash),
                "successful static validation should populate stateless cache",
            );
        }

        #[test]
        fn validate_with_events_populates_stateless_cache() {
            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let mut state = State::new(World::new(), Arc::clone(&kura), query);
            let mut pipeline = state.view().pipeline().clone();
            pipeline.stateless_cache_cap = 64;
            state.set_pipeline(pipeline);

            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let (leader_public, leader_private) = leader.into_parts();
            let topology = Topology::new(vec![PeerId::new(leader_public.clone())]);

            let _ = commit_block_at_height(&state, &kura, &topology, &leader_private, 1, None, 0);

            let (_tx_handle, tx_time_source) = TimeSource::new_mock(Duration::from_millis(0));
            let (authority, signer) = gen_account_in("cache-test");
            let tx = TransactionBuilder::new_with_time_source(
                state.chain_id.clone(),
                authority,
                &tx_time_source,
            )
            .with_instructions([Log::new(Level::INFO, "cacheable".to_owned())])
            .sign(signer.private_key());
            let tx_hash = tx.hash();
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));

            let (_block_handle, block_time_source) =
                TimeSource::new_mock(Duration::from_millis(10));
            let builder =
                BlockBuilder::new_with_time_source(vec![accepted], block_time_source.clone());
            let new_block = builder
                .chain(0, state.view().latest_block().as_deref())
                .sign(&leader_private)
                .unpack(|_| {});
            let signed_block: SignedBlock = SignedBlock::from(new_block);

            let mut state_block = state.block(signed_block.header());
            let events = std::cell::RefCell::new(Vec::new());
            let result = ValidBlock::validate_with_events(
                signed_block,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &block_time_source,
                &mut state_block,
                |event| events.borrow_mut().push(event),
            )
            .unpack(|_| {});
            assert!(
                result.is_ok(),
                "validation with events should warm stateless cache"
            );
            assert!(events.borrow().is_empty(), "no rejection events expected");
            drop(state_block);

            let cache = state.stateless_validation_cache().lock();
            assert!(
                cache.contains_key(&tx_hash),
                "successful static validation with events should populate stateless cache",
            );
        }

        #[test]
        fn validate_keep_voting_block_enforces_fraud_policy_with_stateless_cache() {
            use std::iter;

            use iroha_config::parameters::actual::{FraudMonitoring, FraudRiskBand};
            use iroha_data_model::{
                ValidationFail, account::Account, asset::AssetDefinition, domain::Domain,
                transaction::error::TransactionRejectionReason,
            };

            let kura = Arc::new(Kura::blank_kura_for_testing());
            let query = LiveQueryStore::start_test();
            let (authority, signer) = gen_account_in("fraud-cache-test");
            let domain_id: DomainId = DomainId::try_new("fraud-cache-test", "universal")
                .expect("fraud-cache-test domain");
            let domain = Domain::new(domain_id.clone()).build(&authority);
            let account = Account::new(authority.clone()).build(&authority);
            let world = World::with([domain], [account], iter::empty::<AssetDefinition>());
            let mut state = State::new(world, Arc::clone(&kura), query);

            let mut pipeline = state.view().pipeline().clone();
            pipeline.stateless_cache_cap = 64;
            state.set_pipeline(pipeline);
            state.set_fraud_monitoring(FraudMonitoring {
                enabled: true,
                required_minimum_band: Some(FraudRiskBand::High),
                missing_assessment_grace: Duration::ZERO,
                ..Default::default()
            });

            let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
            let (leader_public, leader_private) = leader.into_parts();
            let topology = Topology::new(vec![PeerId::new(leader_public.clone())]);
            let _ = commit_block_at_height(&state, &kura, &topology, &leader_private, 1, None, 0);

            let (_tx_handle, tx_time_source) = TimeSource::new_mock(Duration::from_millis(0));
            let tx = TransactionBuilder::new_with_time_source(
                state.chain_id.clone(),
                authority,
                &tx_time_source,
            )
            .with_instructions([Log::new(Level::INFO, "fraud-check".to_owned())])
            .with_metadata(Metadata::default())
            .sign(signer.private_key());
            let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));

            let (_block_handle, block_time_source) =
                TimeSource::new_mock(Duration::from_millis(10));
            let builder = BlockBuilder::new_with_time_source(vec![accepted], block_time_source);
            let new_block = builder
                .chain(0, state.view().latest_block().as_deref())
                .sign(&leader_private)
                .unpack(|_| {});
            let signed_block = SignedBlock::from(new_block);

            let mut voting_block: Option<super::super::VotingBlock> = None;
            let (valid_block, _) = ValidBlock::validate_keep_voting_block(
                signed_block,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &TimeSource::new_system(),
                &state,
                &mut voting_block,
                false,
            )
            .unpack(|_| {})
            .expect("block validation should complete and record transaction result");

            let committed_block: SignedBlock = valid_block.into();
            let rejection = committed_block
                .error(0)
                .expect("fraud policy rejection should be recorded for missing assessment");
            match rejection {
                TransactionRejectionReason::Validation(ValidationFail::NotPermitted(msg)) => {
                    assert!(
                        msg.contains("fraud monitoring requires an attached assessment"),
                        "unexpected rejection message: {msg}"
                    );
                }
                other => panic!("unexpected rejection reason: {other:?}"),
            }
        }

        // The executor upgrade is optional; a genesis without it must still pass static checks.
        #[test]
        fn genesis_block_without_upgrade_is_valid() {
            use iroha_data_model::prelude::*;
            use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};

            let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
            let genesis_account = SAMPLE_GENESIS_ACCOUNT_ID.clone();

            let tx = TransactionBuilder::new(chain_id.clone(), genesis_account.clone())
                .with_instructions([Log::new(Level::INFO, "genesis".to_owned())])
                .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());

            let block = SignedBlock::genesis(
                vec![tx],
                SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
                None,
                None,
            );

            assert!(check_genesis_block(&block, &genesis_account, &chain_id).is_ok());
        }

        #[test]
        fn genesis_asset_definition_in_genesis_domain_is_authorized() {
            use iroha_data_model::prelude::*;
            use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};

            let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
            let genesis_account = SAMPLE_GENESIS_ACCOUNT_ID.clone();
            let asset_definition_id = AssetDefinitionId::new(
                DomainId::try_new("genesis", "universal").expect("valid domain id"),
                "xor".parse().expect("valid asset name"),
            );
            let asset_name = asset_definition_id.name().to_string();

            let tx = TransactionBuilder::new(chain_id.clone(), genesis_account.clone())
                .with_instructions([Register::asset_definition(
                    AssetDefinition::numeric(asset_definition_id).with_name(asset_name),
                )])
                .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());

            let block = SignedBlock::genesis(
                vec![tx],
                SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
                None,
                None,
            );

            assert!(check_genesis_block(&block, &genesis_account, &chain_id).is_ok());
        }

        #[test]
        fn validate_keep_voting_block_accepts_ordered_genesis_parameter_transactions() {
            use iroha_data_model::{
                parameter::{Parameter, system::SumeragiParameter},
                peer::PeerId,
                prelude::*,
            };
            use iroha_genesis::GenesisBuilder;

            use crate::{
                kura::Kura, query::store::LiveQueryStore, sumeragi::network_topology::Topology,
            };

            iroha_genesis::init_instruction_registry();

            let chain_id = ChainId::from("00000000-0000-0000-0000-000000000001");
            let genesis_keypair = KeyPair::random();
            let genesis_account = AccountId::new(genesis_keypair.public_key().clone());

            let manifest = GenesisBuilder::new_without_executor(chain_id.clone(), ".")
                .append_parameter(Parameter::Sumeragi(SumeragiParameter::MinFinalityMs(100)))
                .append_parameter(Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(100)))
                .append_parameter(Parameter::Sumeragi(SumeragiParameter::CommitTimeMs(100)))
                .next_transaction()
                .append_parameter(Parameter::Sumeragi(SumeragiParameter::CommitTimeMs(667)))
                .append_parameter(Parameter::Sumeragi(SumeragiParameter::MinFinalityMs(100)))
                .append_parameter(Parameter::Sumeragi(SumeragiParameter::BlockTimeMs(333)))
                .build_raw();

            let genesis = manifest
                .build_and_sign(&genesis_keypair)
                .expect("ordered genesis parameters should build");

            let genesis_domain =
                Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&genesis_account);
            let genesis_account_model =
                Account::new(genesis_account.clone()).build(&genesis_account);
            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(
                World::with([genesis_domain], [genesis_account_model], []),
                kura,
                query_handle,
            );
            let topology = Topology::new(vec![PeerId::new(KeyPair::random().public_key().clone())]);
            let time_source = TimeSource::new_system();
            let mut voting_block = None;

            let result = ValidBlock::validate_keep_voting_block(
                genesis.0,
                &topology,
                &chain_id,
                &genesis_account,
                &time_source,
                &state,
                &mut voting_block,
                false,
            )
            .unpack(|_| {});

            if let Err((failed_block, err)) = result {
                let results = failed_block
                    .results()
                    .map(|result| format!("{result:?}"))
                    .collect::<Vec<_>>();
                panic!(
                    "ordered genesis parameter transactions should validate: {err}; results={results:?}"
                );
            }
        }

        #[test]
        fn check_genesis_block_rejects_chain_id_mismatch() {
            use iroha_data_model::prelude::*;
            use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};

            let chain_a = ChainId::from("00000000-0000-0000-0000-000000000000");
            let chain_b = ChainId::from("11111111-1111-1111-1111-111111111111");
            let genesis_account = SAMPLE_GENESIS_ACCOUNT_ID.clone();

            let tx_a = TransactionBuilder::new(chain_a.clone(), genesis_account.clone())
                .with_instructions([Log::new(Level::INFO, "tx_a".to_owned())])
                .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
            let tx_b = TransactionBuilder::new(chain_b, genesis_account.clone())
                .with_instructions([Log::new(Level::INFO, "tx_b".to_owned())])
                .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());

            let block = SignedBlock::genesis(
                vec![tx_a, tx_b],
                SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
                None,
                None,
            );

            assert_eq!(
                check_genesis_block(&block, &genesis_account, &chain_a),
                Err(InvalidGenesisError::ChainIdMismatch)
            );
        }
    }

    #[test]
    fn rejected_block_emits_rejection_event() {
        use iroha_data_model::peer::PeerId;
        use iroha_data_model::{isi::Log, transaction::TransactionBuilder};
        use iroha_logger::Level;
        use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, gen_account_in};
        use std::{borrow::Cow, time::Duration};

        use crate::{
            kura::Kura, query::store::LiveQueryStore, sumeragi::network_topology::Topology,
            tx::AcceptedTransaction,
        };

        // Build a fresh state (height = 0)
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(World::default(), kura, query_handle);

        // Topology with two peers (consensus required);
        // only leader will sign the block, causing rejection on commit check.
        let kp1 = iroha_crypto::KeyPair::random();
        let kp2 = iroha_crypto::KeyPair::random();
        let peer1 = PeerId::new(kp1.public_key().clone());
        let peer2 = PeerId::new(kp2.public_key().clone());
        let topology = Topology::new(vec![peer1, peer2]);
        let chain_id: ChainId = "chain".parse().unwrap();

        // Create a signed block with only leader signature
        let (account_id, keypair) = gen_account_in("dummy");
        let mut builder = TransactionBuilder::new(chain_id.clone(), account_id);
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_instructions([Log::new(Level::INFO, "dummy".to_owned())])
            .sign(keypair.private_key());
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let unverified_block = BlockBuilder::new(vec![accepted])
            .chain(
                topology.view_change_index(),
                state.view().latest_block().as_deref(),
            )
            .sign(kp1.private_key())
            .unpack(|_| {});

        // Attempt commit_keep_voting_block: should reject due to insufficient signatures
        let genesis_account = SAMPLE_GENESIS_ACCOUNT_ID.clone();
        let time_source = iroha_primitives::time::TimeSource::new_system();
        let mut voting_block = None;

        let events = std::cell::RefCell::new(Vec::new());
        let result = ValidBlock::commit_keep_voting_block(
            unverified_block.into(),
            &topology,
            &chain_id,
            &genesis_account,
            &time_source,
            &state,
            &mut voting_block,
            false,
            |e| events.borrow_mut().push(e),
        )
        .unpack(|_| {});

        assert!(
            result.is_err(),
            "commit should fail with insufficient signatures"
        );
        // Ensure we emitted a rejection Block event
        assert!(events.borrow().iter().any(|ev| match ev {
            PipelineEventBox::Block(be) => matches!(be.status, BlockStatus::Rejected(_)),
            _ => false,
        }));
    }
}

mod commit {
    use super::*;

    /// Represents a block accepted by consensus.
    /// Every [`Self`] will have a different height.
    #[derive(Debug, Clone)]
    pub struct CommittedBlock(pub(super) ValidBlock);

    impl From<CommittedBlock> for ValidBlock {
        fn from(source: CommittedBlock) -> Self {
            source.0
        }
    }

    impl From<CommittedBlock> for SignedBlock {
        fn from(source: CommittedBlock) -> Self {
            source.0.into()
        }
    }

    impl AsRef<SignedBlock> for CommittedBlock {
        fn as_ref(&self) -> &SignedBlock {
            self.0.as_ref()
        }
    }

    #[cfg(any(test, feature = "iroha-core-tests"))]
    impl AsMut<SignedBlock> for CommittedBlock {
        fn as_mut(&mut self) -> &mut SignedBlock {
            self.0.as_mut()
        }
    }

    #[cfg(all(test, feature = "app_api"))]
    mod axt_validation_tests {
        use std::{collections::BTreeMap, time::Duration};

        use iroha_data_model::nexus::{
            AssetHandle, AxtBinding, AxtDescriptor, AxtEnvelopeRecord, AxtHandleFragment,
            AxtPolicyBinding, AxtPolicyEntry, AxtPolicySnapshot, AxtProofEnvelope,
            AxtProofFragment, AxtTouchFragment, AxtTouchSpec, GroupBinding, HandleBudget,
            HandleSubject, ProofBlob, RemoteSpendIntent, SpendOp, TouchManifest,
        };
        use iroha_primitives::time::TimeSource;

        use super::*;
        use crate::{
            block::valid::validate_axt_envelopes,
            kura::Kura,
            query::store::LiveQueryStore,
            state::{State, World},
        };

        const ACCOUNT_FROM_LITERAL: &str = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB";
        const ACCOUNT_TO_LITERAL: &str = "sorauﾛ1NfｷgﾉﾓﾉBｦKﾌﾘﾒoﾇﾂﾛrG81ﾋjWﾎﾕVncwﾌSｱ3pﾘﾋﾉhUS9Q76";

        fn binding_for_descriptor(descriptor: &AxtDescriptor) -> AxtBinding {
            descriptor.binding().expect("descriptor binding")
        }

        fn sample_handle(
            binding: AxtBinding,
            lane: LaneId,
            dsid: DataSpaceId,
            expiry_slot: u64,
            manifest_root: [u8; 32],
        ) -> AxtHandleFragment {
            AxtHandleFragment {
                handle: AssetHandle {
                    scope: vec!["transfer".to_owned()],
                    subject: HandleSubject {
                        account: ACCOUNT_FROM_LITERAL.to_owned(),
                        origin_dsid: Some(dsid),
                    },
                    budget: HandleBudget {
                        remaining: 10,
                        per_use: Some(10),
                    },
                    handle_era: 1,
                    sub_nonce: 1,
                    group_binding: GroupBinding {
                        composability_group_id: vec![0; 32],
                        epoch_id: 1,
                    },
                    target_lane: lane,
                    axt_binding: binding,
                    manifest_view_root: manifest_root,
                    expiry_slot,
                    max_clock_skew_ms: Some(0),
                },
                intent: RemoteSpendIntent {
                    asset_dsid: dsid,
                    op: SpendOp {
                        kind: "transfer".to_owned(),
                        from: ACCOUNT_FROM_LITERAL.to_owned(),
                        to: ACCOUNT_TO_LITERAL.to_owned(),
                        amount: "5".to_owned(),
                    },
                },
                proof: None,
                amount: 5,
                amount_commitment: None,
            }
        }

        fn proof_blob_for(
            dsid: DataSpaceId,
            manifest_root: [u8; 32],
            proof_seed: &[u8],
            expiry_slot: u64,
        ) -> ProofBlob {
            proof_blob_for_with_amount(dsid, manifest_root, proof_seed, expiry_slot, None, None)
        }

        fn proof_blob_for_with_amount(
            dsid: DataSpaceId,
            manifest_root: [u8; 32],
            proof_seed: &[u8],
            expiry_slot: u64,
            committed_amount: Option<u128>,
            amount_commitment: Option<[u8; 32]>,
        ) -> ProofBlob {
            let source_tx_commitment = test_digest(b"axt-block-test:source-tx", &[proof_seed]);
            let claim_digest = test_digest(b"axt-block-test:claim", &[proof_seed]);
            let witness_commitment = test_digest(b"axt-block-test:witness", &[proof_seed]);
            let policy_commitment = test_digest(b"axt-block-test:policy", &[&manifest_root[..]]);
            let binding = iroha_data_model::nexus::AxtFastpqBinding {
                parameter: fastpq_prover::AXT_DEFAULT_PARAMETER.to_owned(),
                source_dsid: dsid.as_u64(),
                source_dataspace: format!("test-dataspace-{}", dsid.as_u64()),
                source_receipt_id: format!(
                    "receipt-{}",
                    hex::encode(source_tx_commitment.as_ref())
                ),
                source_tx_commitment: hex::encode(source_tx_commitment.as_ref()),
                claim_type: "authorization".to_owned(),
                claim_digest: hex::encode(claim_digest.as_ref()),
                witness_commitment: hex::encode(witness_commitment.as_ref()),
                policy_commitment: hex::encode(policy_commitment.as_ref()),
                verified_effect_type: "test_effect".to_owned(),
                corridor: "test-corridor".to_owned(),
                verifier_id: "fastpq".to_owned(),
                verifier_version: "v1".to_owned(),
                target_dsids: vec![dsid.as_u64()],
                effect_binding: None,
            };

            let mut dsid_bytes = [0_u8; 16];
            dsid_bytes[..8].copy_from_slice(&dsid.as_u64().to_le_bytes());
            let mut batch = fastpq_prover::TransitionBatch::new(
                fastpq_prover::AXT_DEFAULT_PARAMETER,
                fastpq_prover::PublicInputs {
                    dsid: dsid_bytes,
                    slot: expiry_slot,
                    old_root: test_digest(b"axt-block-test:old-root", &[proof_seed]).into(),
                    new_root: manifest_root,
                    perm_root: test_digest(b"axt-block-test:perm-root", &[proof_seed]).into(),
                    tx_set_hash: test_digest(b"axt-block-test:tx-set", &[proof_seed]).into(),
                },
            );
            batch.push(fastpq_prover::StateTransition::new(
                b"axt/block/proof".to_vec(),
                proof_seed.to_vec(),
                manifest_root.to_vec(),
                fastpq_prover::OperationKind::MetaSet,
            ));
            batch.sort();
            batch.metadata.insert(
                "entry_hash".to_owned(),
                source_tx_commitment.as_ref().to_vec(),
            );
            fastpq_prover::bind_axt_batch(&mut batch, &binding).expect("bind AXT test batch");
            let proof = fastpq_prover::Prover::canonical(fastpq_prover::AXT_DEFAULT_PARAMETER)
                .expect("FASTPQ prover")
                .prove(&batch)
                .expect("FASTPQ proof");
            let fastpq_payload = fastpq_prover::encode_axt_fastpq_payload(&batch, proof)
                .expect("AXT FASTPQ payload");
            let envelope = AxtProofEnvelope {
                dsid,
                manifest_root,
                da_commitment: None,
                proof: fastpq_payload,
                fastpq_binding: Some(binding),
                committed_amount,
                amount_commitment,
            };
            ProofBlob {
                payload: norito::to_bytes(&envelope).expect("encode proof envelope"),
                expiry_slot: Some(expiry_slot),
            }
        }

        fn test_digest(domain: &[u8], parts: &[&[u8]]) -> iroha_crypto::Hash {
            let mut payload = Vec::new();
            payload.extend_from_slice(domain);
            for part in parts {
                payload.extend_from_slice(part);
            }
            iroha_crypto::Hash::new(payload)
        }

        fn build_block_with_envelopes(
            envelope: AxtEnvelopeRecord,
            snapshot: AxtPolicySnapshot,
        ) -> SignedBlock {
            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(0));
            let builder = BlockBuilder::new_with_time_source(Vec::new(), time_source);
            let signer = KeyPair::random();
            let mut block: SignedBlock = builder
                .chain(0, None)
                .sign(signer.private_key())
                .unpack(|_| {})
                .into();
            let entry_hashes: Vec<HashOf<TransactionEntrypoint>> = Vec::new();
            let results: Vec<TransactionResultInner> = Vec::new();
            block
                .set_transaction_results_with_transcripts(
                    Vec::new(),
                    &entry_hashes,
                    results,
                    BTreeMap::new(),
                    vec![envelope],
                    Some(snapshot),
                )
                .expect("empty test block should attach AXT envelope results");
            block
        }

        fn expect_axt_error(
            err: BlockValidationError,
            reason: AxtRejectReason,
            needle: &str,
        ) -> AxtEnvelopeValidationDetails {
            match err {
                BlockValidationError::AxtEnvelopeValidationFailed(details) => {
                    assert_eq!(details.reason, reason);
                    assert!(
                        details.message.contains(needle),
                        "expected `{}` in `{}`",
                        needle,
                        details.message
                    );
                    details
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }

        #[test]
        fn axt_validation_rejects_handle_clock_skew_above_config() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(99);
            let lane = LaneId::new(0);
            let policy = AxtPolicyEntry {
                manifest_root: [0x42; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 10,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: vec![AxtTouchSpec {
                    dsid,
                    read: Vec::new(),
                    write: Vec::new(),
                }],
            };
            let binding = binding_for_descriptor(&descriptor);
            let handle = AxtHandleFragment {
                handle: AssetHandle {
                    scope: vec!["transfer".to_owned()],
                    subject: HandleSubject {
                        account: ACCOUNT_FROM_LITERAL.to_owned(),
                        origin_dsid: Some(dsid),
                    },
                    budget: HandleBudget {
                        remaining: 10,
                        per_use: Some(10),
                    },
                    handle_era: 1,
                    sub_nonce: 1,
                    group_binding: GroupBinding {
                        composability_group_id: vec![0; 32],
                        epoch_id: 1,
                    },
                    target_lane: lane,
                    axt_binding: binding,
                    manifest_view_root: policy.manifest_root,
                    expiry_slot: 50,
                    max_clock_skew_ms: Some(1_000),
                },
                intent: RemoteSpendIntent {
                    asset_dsid: dsid,
                    op: SpendOp {
                        kind: "transfer".to_owned(),
                        from: ACCOUNT_FROM_LITERAL.to_owned(),
                        to: ACCOUNT_TO_LITERAL.to_owned(),
                        amount: "5".to_owned(),
                    },
                },
                proof: Some(proof_blob_for(
                    dsid,
                    policy.manifest_root,
                    b"handle-clock-skew",
                    50,
                )),
                amount: 5,
                amount_commitment: None,
            };
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: Vec::new(),
                handles: vec![handle],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());
            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(
                err,
                AxtRejectReason::Expiry,
                "max_clock_skew_ms exceeds configured bound",
            );
        }

        #[test]
        fn axt_validation_rejects_duplicate_handle_use() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(7);
            let lane = LaneId::new(1);
            let policy = AxtPolicyEntry {
                manifest_root: [0x11; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 0,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: vec![AxtTouchSpec {
                    dsid,
                    read: Vec::new(),
                    write: Vec::new(),
                }],
            };
            let binding = binding_for_descriptor(&descriptor);
            let handle = sample_handle(binding, lane, dsid, 5, policy.manifest_root);
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: vec![AxtTouchFragment {
                    dsid,
                    manifest: TouchManifest {
                        read: Vec::new(),
                        write: Vec::new(),
                    },
                }],
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: proof_blob_for(dsid, policy.manifest_root, b"duplicate-handle", 12),
                }],
                handles: vec![handle.clone(), handle],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());
            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(
                err,
                AxtRejectReason::ReplayCache,
                "duplicate handle usage in block",
            );
        }

        #[test]
        fn axt_validation_accepts_cross_lane_handles() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid_a = DataSpaceId::new(7);
            let dsid_b = DataSpaceId::new(8);
            let lane_a = LaneId::new(1);
            let lane_b = LaneId::new(2);
            let policy_a = AxtPolicyEntry {
                manifest_root: [0x11; 32],
                target_lane: lane_a,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 0,
            };
            let policy_b = AxtPolicyEntry {
                manifest_root: [0x22; 32],
                target_lane: lane_b,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 0,
            };
            state.set_axt_policy(dsid_a, policy_a);
            state.set_axt_policy(dsid_b, policy_b);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid_a, dsid_b],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let envelope = AxtEnvelopeRecord {
                binding,
                lane: lane_a,
                descriptor,
                touches: Vec::new(),
                proofs: vec![
                    AxtProofFragment {
                        dsid: dsid_a,
                        proof: proof_blob_for(dsid_a, policy_a.manifest_root, b"cross-lane-a", 25),
                    },
                    AxtProofFragment {
                        dsid: dsid_b,
                        proof: proof_blob_for(dsid_b, policy_b.manifest_root, b"cross-lane-b", 25),
                    },
                ],
                handles: vec![
                    sample_handle(binding, lane_a, dsid_a, 20, policy_a.manifest_root),
                    sample_handle(binding, lane_b, dsid_b, 20, policy_b.manifest_root),
                ],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());
            let result = validate_axt_envelopes(&block, &state_block);
            assert!(result.is_ok(), "unexpected validation error: {result:?}");
        }

        #[test]
        fn axt_validation_rejects_handle_amount_mismatch() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(7);
            let lane = LaneId::new(1);
            let policy = AxtPolicyEntry {
                manifest_root: [0x11; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 0,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: vec![AxtTouchSpec {
                    dsid,
                    read: Vec::new(),
                    write: Vec::new(),
                }],
            };
            let binding = binding_for_descriptor(&descriptor);
            let mut handle = sample_handle(binding, lane, dsid, 5, policy.manifest_root);
            handle.amount = 4;

            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: vec![AxtTouchFragment {
                    dsid,
                    manifest: TouchManifest {
                        read: Vec::new(),
                        write: Vec::new(),
                    },
                }],
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: proof_blob_for(dsid, policy.manifest_root, b"amount-mismatch", 12),
                }],
                handles: vec![handle],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(err, AxtRejectReason::Budget, "amount");
        }

        #[test]
        fn axt_validation_rejects_missing_touch_manifest() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(7);
            let lane = LaneId::new(1);
            let policy = AxtPolicyEntry {
                manifest_root: [0x11; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 0,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: vec![AxtTouchSpec {
                    dsid,
                    read: vec!["orders/".to_owned()],
                    write: vec!["ledger/".to_owned()],
                }],
            };
            let binding = binding_for_descriptor(&descriptor);
            let handle = sample_handle(binding, lane, dsid, 5, policy.manifest_root);

            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: vec![AxtTouchFragment {
                    dsid,
                    manifest: TouchManifest {
                        read: Vec::new(),
                        write: Vec::new(),
                    },
                }],
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: proof_blob_for(dsid, policy.manifest_root, b"missing-touch", 12),
                }],
                handles: vec![handle],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(err, AxtRejectReason::Descriptor, "missing touch manifest");
        }

        #[test]
        fn axt_validation_rejects_handle_without_touch_manifest() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(9);
            let lane = LaneId::new(1);
            let policy = AxtPolicyEntry {
                manifest_root: [0x23; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 0,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: vec![AxtTouchSpec {
                    dsid,
                    read: vec!["orders/".to_owned()],
                    write: Vec::new(),
                }],
            };
            let binding = binding_for_descriptor(&descriptor);
            let handle = sample_handle(binding, lane, dsid, 5, policy.manifest_root);

            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: proof_blob_for(dsid, policy.manifest_root, b"handle-without-touch", 12),
                }],
                handles: vec![handle],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(err, AxtRejectReason::Descriptor, "missing touch manifest");
        }

        #[test]
        fn axt_validation_rejects_touch_manifest_prefix_violation() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(7);
            let lane = LaneId::new(1);
            let policy = AxtPolicyEntry {
                manifest_root: [0x11; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 0,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: vec![AxtTouchSpec {
                    dsid,
                    read: vec!["orders/".to_owned()],
                    write: vec!["ledger/".to_owned()],
                }],
            };
            let binding = binding_for_descriptor(&descriptor);
            let handle = sample_handle(binding, lane, dsid, 5, policy.manifest_root);

            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: vec![AxtTouchFragment {
                    dsid,
                    manifest: TouchManifest {
                        read: vec!["payments/123".to_owned()],
                        write: Vec::new(),
                    },
                }],
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: proof_blob_for(dsid, policy.manifest_root, b"touch-prefix", 12),
                }],
                handles: vec![handle],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(
                err,
                AxtRejectReason::Descriptor,
                "touch manifest read entry",
            );
        }

        #[test]
        fn axt_validation_rejects_descriptor_binding_mismatch() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(7);
            let lane = LaneId::new(1);
            let policy = AxtPolicyEntry {
                manifest_root: [0x11; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 0,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let mut wrong_bytes = *binding.as_bytes();
            wrong_bytes[0] ^= 0xFF;
            let wrong_binding = AxtBinding::new(wrong_bytes);
            let handle = sample_handle(wrong_binding, lane, dsid, 5, policy.manifest_root);

            let envelope = AxtEnvelopeRecord {
                binding: wrong_binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: proof_blob_for(dsid, policy.manifest_root, b"descriptor-binding", 12),
                }],
                handles: vec![handle],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(
                err,
                AxtRejectReason::Descriptor,
                "descriptor binding does not match envelope binding",
            );
        }

        #[test]
        fn axt_validation_rejects_duplicate_handle_use_across_dataspaces() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid_a = DataSpaceId::new(7);
            let dsid_b = DataSpaceId::new(8);
            let lane = LaneId::new(1);
            let policy = AxtPolicyEntry {
                manifest_root: [0x11; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 0,
            };
            state.set_axt_policy(dsid_a, policy);
            state.set_axt_policy(dsid_b, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid_a, dsid_b],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let handle = sample_handle(binding, lane, dsid_a, 5, policy.manifest_root);
            let mut other = handle.clone();
            other.intent.asset_dsid = dsid_b;

            let proofs = vec![
                AxtProofFragment {
                    dsid: dsid_a,
                    proof: proof_blob_for(dsid_a, policy.manifest_root, b"cross-dsid-a", 12),
                },
                AxtProofFragment {
                    dsid: dsid_b,
                    proof: proof_blob_for(dsid_b, policy.manifest_root, b"cross-dsid-b", 12),
                },
            ];

            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs,
                handles: vec![handle, other],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(
                err,
                AxtRejectReason::ReplayCache,
                "duplicate handle usage in block",
            );
        }

        #[test]
        fn axt_validation_rejects_budget_overspend_across_sub_nonces() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(20);
            let lane = LaneId::new(5);
            let policy = AxtPolicyEntry {
                manifest_root: [0x11; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 0,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let mut first = sample_handle(binding, lane, dsid, 10, policy.manifest_root);
            first.handle.sub_nonce = 3;
            first.handle.budget.remaining = 10;
            first.handle.budget.per_use = Some(10);
            first.intent.op.amount = "7".to_owned();
            first.amount = 7;
            let mut second = first.clone();
            second.handle.sub_nonce = 4;
            second.amount = 7;

            let proof = AxtProofFragment {
                dsid,
                proof: proof_blob_for(dsid, policy.manifest_root, b"overspend-subnonce", 15),
            };

            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![proof],
                handles: vec![first, second],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(err, AxtRejectReason::Budget, "budget");
        }

        #[test]
        fn axt_validation_rejects_missing_proof_for_dataspace() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(21);
            let lane = LaneId::new(8);
            let policy = AxtPolicyEntry {
                manifest_root: [0x44; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 2,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let handle = sample_handle(binding, lane, dsid, 5, policy.manifest_root);
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: Vec::new(),
                handles: vec![handle],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(err, AxtRejectReason::Proof, "missing proof");
        }

        #[test]
        fn axt_validation_rejects_raw_manifest_root_proof() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(21);
            let lane = LaneId::new(8);
            let policy = AxtPolicyEntry {
                manifest_root: [0x44; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 2,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: ProofBlob {
                        payload: policy.manifest_root.to_vec(),
                        expiry_slot: Some(12),
                    },
                }],
                handles: Vec::new(),
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(err, AxtRejectReason::Proof, "not an AXT proof envelope");
        }

        #[test]
        fn axt_validation_rejects_expired_proof() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            state.nexus.get_mut().axt.max_clock_skew_ms = 0;
            let dsid = DataSpaceId::new(22);
            let lane = LaneId::new(9);
            let policy = AxtPolicyEntry {
                manifest_root: [0x45; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 70_000,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: proof_blob_for(dsid, policy.manifest_root, b"expired-proof", 4),
                }],
                handles: Vec::new(),
                commit_height: Some(1),
            };
            let entries = vec![AxtPolicyBinding { dsid, policy }];
            let snapshot = AxtPolicySnapshot {
                version: AxtPolicySnapshot::compute_version(&entries),
                entries,
            };
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());
            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(err, AxtRejectReason::Expiry, "expired");
        }

        #[test]
        fn axt_validation_rejects_zero_proof_expiry_slot() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(23);
            let lane = LaneId::new(10);
            let policy = AxtPolicyEntry {
                manifest_root: [0x46; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 1,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let handle = sample_handle(binding, lane, dsid, 5, policy.manifest_root);
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: proof_blob_for(dsid, policy.manifest_root, b"zero-proof-expiry", 0),
                }],
                handles: vec![handle],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(err, AxtRejectReason::Proof, "proof expiry slot is zero");
        }

        #[test]
        fn axt_validation_rejects_proof_expiry_before_handle_with_skew() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            state.nexus.get_mut().axt.max_clock_skew_ms = 1;
            let dsid = DataSpaceId::new(23);
            let lane = LaneId::new(10);
            let policy = AxtPolicyEntry {
                manifest_root: [0x55; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 1,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let mut handle = sample_handle(binding, lane, dsid, 9, policy.manifest_root);
            handle.proof = Some(proof_blob_for(
                dsid,
                policy.manifest_root,
                b"proof-before-handle",
                8,
            ));
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: Vec::new(),
                handles: vec![handle],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(err, AxtRejectReason::Expiry, "proof expires before handle");
        }

        #[test]
        fn axt_validation_rejects_manifest_mismatch_in_proof() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(23);
            let lane = LaneId::new(10);
            let policy = AxtPolicyEntry {
                manifest_root: [0x77; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 3,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let handle = sample_handle(binding, lane, dsid, 8, policy.manifest_root);
            let mismatched_envelope = AxtProofEnvelope {
                dsid,
                manifest_root: [0x99; 32],
                da_commitment: None,
                proof: vec![0xAA],
                fastpq_binding: None,
                committed_amount: None,
                amount_commitment: None,
            };
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: ProofBlob {
                        payload: norito::to_bytes(&mismatched_envelope)
                            .expect("encode mismatched envelope"),
                        expiry_slot: Some(12),
                    },
                }],
                handles: vec![handle],
                commit_height: Some(2),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(err, AxtRejectReason::Manifest, "manifest");
        }

        #[test]
        fn axt_validation_rejects_proof_dsid_mismatch() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(24);
            let lane = LaneId::new(11);
            let policy = AxtPolicyEntry {
                manifest_root: [0x78; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 4,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let handle = sample_handle(binding, lane, dsid, 9, policy.manifest_root);
            let wrong_envelope = iroha_data_model::nexus::AxtProofEnvelope {
                dsid: DataSpaceId::new(dsid.as_u64() + 1),
                manifest_root: policy.manifest_root,
                da_commitment: None,
                proof: vec![0xAA],
                fastpq_binding: None,
                committed_amount: None,
                amount_commitment: None,
            };
            let payload = norito::to_bytes(&wrong_envelope).expect("encode proof envelope");
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: ProofBlob {
                        payload,
                        expiry_slot: Some(13),
                    },
                }],
                handles: vec![handle],
                commit_height: Some(2),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(err, AxtRejectReason::Manifest, "manifest");
        }

        #[test]
        fn axt_validation_rejects_budget_overspend_in_block() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(23);
            let lane = LaneId::new(10);
            let policy = AxtPolicyEntry {
                manifest_root: [0x46; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 1,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let mut handle_one = sample_handle(binding, lane, dsid, 5, policy.manifest_root);
            handle_one.intent.op.amount = "7".to_owned();
            handle_one.amount = 7;
            let mut handle_two = handle_one.clone();
            handle_two.amount = 7;

            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: proof_blob_for(dsid, policy.manifest_root, b"budget-block", 10),
                }],
                handles: vec![handle_one, handle_two],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(err, AxtRejectReason::Budget, "budget");
        }

        #[test]
        fn axt_validation_rejects_handle_era_below_policy() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(9);
            let lane = LaneId::new(2);
            let policy = AxtPolicyEntry {
                manifest_root: [0x22; 32],
                target_lane: lane,
                min_handle_era: 2,
                min_sub_nonce: 1,
                current_slot: 0,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let handle = sample_handle(binding, lane, dsid, 5, policy.manifest_root);
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: proof_blob_for(dsid, policy.manifest_root, b"handle-era", 10),
                }],
                handles: vec![handle],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(
                err,
                AxtRejectReason::HandleEra,
                "handle era below policy minimum",
            );
        }

        #[test]
        fn axt_validation_rejects_zero_handle_expiry_slot() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(9);
            let lane = LaneId::new(2);
            let policy = AxtPolicyEntry {
                manifest_root: [0x22; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 0,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let handle = sample_handle(binding, lane, dsid, 0, policy.manifest_root);
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: proof_blob_for(dsid, policy.manifest_root, b"zero-handle-expiry", 10),
                }],
                handles: vec![handle],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(err, AxtRejectReason::Expiry, "expiry slot is zero");
        }

        #[test]
        fn axt_validation_rejects_zero_manifest_root() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(10);
            let lane = LaneId::new(3);
            let policy = AxtPolicyEntry {
                manifest_root: [0; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 1,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let mut handle = sample_handle(binding, lane, dsid, 5, policy.manifest_root);
            handle.handle.manifest_view_root = [0; 32];
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: ProofBlob {
                        payload: vec![0; 32],
                        expiry_slot: Some(10),
                    },
                }],
                handles: vec![handle],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(err, AxtRejectReason::Manifest, "manifest root is zeroed");
        }

        #[test]
        fn axt_validation_rejects_zero_manifest_root_in_policy() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(10);
            let lane = LaneId::new(3);
            let policy = AxtPolicyEntry {
                manifest_root: [0; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 0,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let mut handle = sample_handle(binding, lane, dsid, 5, policy.manifest_root);
            handle.handle.manifest_view_root = [0x55; 32];
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: ProofBlob {
                        payload: vec![0; 32],
                        expiry_slot: Some(9),
                    },
                }],
                handles: vec![handle],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(err, AxtRejectReason::Manifest, "manifest root is zeroed");
        }

        #[test]
        fn axt_validation_rejects_zero_manifest_root_in_handle() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(11);
            let lane = LaneId::new(4);
            let policy = AxtPolicyEntry {
                manifest_root: [0x33; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 0,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let mut handle = sample_handle(binding, lane, dsid, 5, policy.manifest_root);
            handle.handle.manifest_view_root = [0; 32];
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: proof_blob_for(dsid, policy.manifest_root, b"zero-root-handle", 8),
                }],
                handles: vec![handle],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(err, AxtRejectReason::Manifest, "manifest root is zeroed");
        }

        #[test]
        fn axt_validation_accepts_block_snapshot_when_state_cache_empty() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(12);
            let lane = LaneId::new(5);

            let policy = AxtPolicyEntry {
                manifest_root: [0x11; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 3,
            };
            let entries = vec![AxtPolicyBinding { dsid, policy }];
            let snapshot = AxtPolicySnapshot {
                version: AxtPolicySnapshot::compute_version(&entries),
                entries,
            };

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let handle = sample_handle(binding, lane, dsid, 5, policy.manifest_root);
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: proof_blob_for(dsid, policy.manifest_root, b"snapshot-fallback", 9),
                }],
                handles: vec![handle],
                commit_height: Some(2),
            };

            let block = build_block_with_envelopes(envelope, snapshot.clone());
            let mut state_block = state.block(block.header());
            state_block.install_axt_policy_snapshot(&snapshot);

            assert!(validate_axt_envelopes(&block, &state_block).is_ok());
        }

        #[test]
        fn axt_validation_uses_policy_slot_per_dataspace() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid_a = DataSpaceId::new(50);
            let dsid_b = DataSpaceId::new(51);
            let lane = LaneId::new(6);
            let policy_a = AxtPolicyEntry {
                manifest_root: [0x21; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 100,
            };
            let policy_b = AxtPolicyEntry {
                manifest_root: [0x22; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 5,
            };
            state.set_axt_policy(dsid_a, policy_a);
            state.set_axt_policy(dsid_b, policy_b);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid_b],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let handle = sample_handle(binding, lane, dsid_b, 10, policy_b.manifest_root);
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![AxtProofFragment {
                    dsid: dsid_b,
                    proof: proof_blob_for(dsid_b, policy_b.manifest_root, b"policy-slot-dsid", 10),
                }],
                handles: vec![handle],
                commit_height: Some(1),
            };

            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let result = validate_axt_envelopes(&block, &state_block);
            assert!(result.is_ok(), "unexpected validation error: {result:?}");
        }

        #[test]
        fn axt_validation_rejects_missing_policy_snapshot() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(13);
            let lane = LaneId::new(6);
            let manifest_root = [0x12; 32];

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: vec![AxtTouchSpec {
                    dsid,
                    read: Vec::new(),
                    write: Vec::new(),
                }],
            };
            let binding = binding_for_descriptor(&descriptor);
            let handle = sample_handle(binding, lane, dsid, 9, manifest_root);
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: vec![AxtTouchFragment {
                    dsid,
                    manifest: TouchManifest {
                        read: Vec::new(),
                        write: Vec::new(),
                    },
                }],
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: proof_blob_for(dsid, manifest_root, b"missing-policy-snapshot", 15),
                }],
                handles: vec![handle],
                commit_height: Some(3),
            };

            let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(0));
            let builder = BlockBuilder::new_with_time_source(Vec::new(), time_source);
            let signer = KeyPair::random();
            let mut block: SignedBlock = builder
                .chain(0, None)
                .sign(signer.private_key())
                .unpack(|_| {})
                .into();
            let entry_hashes: Vec<HashOf<TransactionEntrypoint>> = Vec::new();
            let results: Vec<TransactionResultInner> = Vec::new();
            block
                .set_transaction_results_with_transcripts(
                    Vec::new(),
                    &entry_hashes,
                    results,
                    BTreeMap::new(),
                    vec![envelope],
                    None,
                )
                .expect("empty test block should attach AXT envelope results");

            let state_block = state.block(block.header());
            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(
                err,
                AxtRejectReason::MissingPolicy,
                "no policy for dataspace",
            );
        }

        #[test]
        fn axt_validation_rejects_zero_manifest_root_from_snapshot() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(14);
            let lane = LaneId::new(7);
            let policy = AxtPolicyEntry {
                manifest_root: [0; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 2,
            };

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let mut handle = sample_handle(binding, lane, dsid, 11, policy.manifest_root);
            handle.handle.manifest_view_root = [0xFF; 32];
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: ProofBlob {
                        payload: vec![0; 32],
                        expiry_slot: Some(12),
                    },
                }],
                handles: vec![handle],
                commit_height: Some(4),
            };

            let entries = vec![AxtPolicyBinding { dsid, policy }];
            let snapshot = AxtPolicySnapshot {
                version: AxtPolicySnapshot::compute_version(&entries),
                entries,
            };

            let block = build_block_with_envelopes(envelope, snapshot.clone());
            let mut state_block = state.block(block.header());
            state_block.install_axt_policy_snapshot(&snapshot);

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(
                err,
                AxtRejectReason::Manifest,
                "policy manifest root is zeroed",
            );
        }

        #[test]
        fn axt_validation_accepts_hidden_amount_commitment() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(61);
            let lane = LaneId::new(8);
            let policy = AxtPolicyEntry {
                manifest_root: [0x61; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 2,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: vec![AxtTouchSpec {
                    dsid,
                    read: Vec::new(),
                    write: Vec::new(),
                }],
            };
            let binding = binding_for_descriptor(&descriptor);
            let proof = proof_blob_for_with_amount(
                dsid,
                policy.manifest_root,
                b"hidden-amount",
                9,
                Some(5),
                None,
            );
            let expected_commitment =
                ivm::axt::derive_amount_commitment(dsid, 5, Some(proof.payload.as_slice()));

            let mut handle = sample_handle(binding, lane, dsid, 9, policy.manifest_root);
            handle.intent.op.amount = "hidden".to_owned();
            handle.amount = 0;
            handle.amount_commitment = Some(expected_commitment);

            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: vec![AxtTouchFragment {
                    dsid,
                    manifest: TouchManifest {
                        read: Vec::new(),
                        write: Vec::new(),
                    },
                }],
                proofs: vec![AxtProofFragment { dsid, proof }],
                handles: vec![handle],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());
            let result = validate_axt_envelopes(&block, &state_block);
            assert!(result.is_ok(), "unexpected validation error: {result:?}");
        }

        #[test]
        fn axt_validation_rejects_hidden_amount_commitment_mismatch() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(62);
            let lane = LaneId::new(9);
            let policy = AxtPolicyEntry {
                manifest_root: [0x62; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 2,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: vec![AxtTouchSpec {
                    dsid,
                    read: Vec::new(),
                    write: Vec::new(),
                }],
            };
            let binding = binding_for_descriptor(&descriptor);
            let proof = proof_blob_for_with_amount(
                dsid,
                policy.manifest_root,
                b"hidden-amount-mismatch",
                9,
                Some(5),
                None,
            );

            let mut handle = sample_handle(binding, lane, dsid, 9, policy.manifest_root);
            handle.intent.op.amount = "hidden".to_owned();
            handle.amount = 0;
            handle.amount_commitment = Some([0xFF; 32]);

            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: vec![AxtTouchFragment {
                    dsid,
                    manifest: TouchManifest {
                        read: Vec::new(),
                        write: Vec::new(),
                    },
                }],
                proofs: vec![AxtProofFragment { dsid, proof }],
                handles: vec![handle],
                commit_height: Some(1),
            };
            let snapshot = state.axt_policy_snapshot();
            let block = build_block_with_envelopes(envelope, snapshot);
            let state_block = state.block(block.header());

            let err = validate_axt_envelopes(&block, &state_block).unwrap_err();
            expect_axt_error(err, AxtRejectReason::Budget, "commitment mismatch");
        }
    }
}

mod event {
    use std::collections::BTreeSet;

    use new::NewBlock;

    use super::*;
    use crate::state::StateBlock;

    pub trait EventProducer {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox>;
    }

    #[derive(Debug)]
    #[must_use]
    pub struct WithEvents<B>(B);

    impl<B> WithEvents<B> {
        pub(super) fn new(source: B) -> Self {
            Self(source)
        }
    }

    impl<B: EventProducer, U> WithEvents<Result<B, (U, Box<BlockValidationError>)>> {
        pub fn unpack<F: FnMut(PipelineEventBox)>(
            self,
            f: F,
        ) -> Result<B, (U, Box<BlockValidationError>)> {
            match self.0 {
                Ok(ok) => Ok(WithEvents(ok).unpack(f)),
                Err(err) => Err(WithEvents(err).unpack(f)),
            }
        }
    }
    impl<'state, B: EventProducer, U>
        WithEvents<Result<(B, StateBlock<'state>), (U, Box<BlockValidationError>)>>
    {
        pub fn unpack<F: FnMut(PipelineEventBox)>(
            self,
            f: F,
        ) -> Result<(B, StateBlock<'state>), (U, Box<BlockValidationError>)> {
            match self.0 {
                Ok((ok, state)) => Ok((WithEvents(ok).unpack(f), state)),
                Err(err) => Err(WithEvents(err).unpack(f)),
            }
        }
    }
    impl WithEvents<Result<BTreeSet<BlockSignature>, SignatureVerificationError>> {
        pub fn unpack<F: FnMut(PipelineEventBox)>(
            self,
            f: F,
        ) -> Result<BTreeSet<BlockSignature>, SignatureVerificationError> {
            match self.0 {
                Ok(ok) => Ok(ok),
                Err(err) => Err(WithEvents(err).unpack(f)),
            }
        }
    }
    impl<B: EventProducer> WithEvents<B> {
        pub fn unpack<F: FnMut(PipelineEventBox)>(self, f: F) -> B {
            self.0.produce_events().for_each(f);
            self.0
        }
    }

    impl<B, E: EventProducer> WithEvents<(B, E)> {
        pub(crate) fn unpack<F: FnMut(PipelineEventBox)>(self, f: F) -> (B, E) {
            self.0.1.produce_events().for_each(f);
            self.0
        }
    }

    impl EventProducer for NewBlock {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox> {
            let block_event = BlockEvent {
                header: self.header,
                status: BlockStatus::Created,
            };

            core::iter::once(block_event.into())
        }
    }

    impl EventProducer for ValidBlock {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox> {
            let block_height = self.as_ref().header().height();

            let block = self.as_ref();
            let tx_events = block
                .external_transactions()
                .enumerate()
                .map(move |(idx, tx)| {
                    let hash = tx.hash();
                    let routing = routing_ledger::take(&hash).unwrap_or_default();
                    let status = block.error(idx).map_or_else(
                        || TransactionStatus::Approved,
                        |error| TransactionStatus::Rejected(Box::new(error.clone())),
                    );

                    TransactionEvent {
                        hash,
                        block_height: Some(block_height),
                        lane_id: routing.lane_id,
                        dataspace_id: routing.dataspace_id,
                        status,
                    }
                });

            let block_event = core::iter::once(BlockEvent {
                header: self.as_ref().header(),
                status: BlockStatus::Approved,
            });

            tx_events
                .map(PipelineEventBox::from)
                .chain(block_event.map(Into::into))
        }
    }

    impl EventProducer for CommittedBlock {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox> {
            let block_event = core::iter::once(BlockEvent {
                header: self.as_ref().header(),
                status: BlockStatus::Committed,
            });

            block_event.map(Into::into)
        }
    }

    pub(super) fn map_sig_err_to_reason(
        err: &SignatureVerificationError,
    ) -> iroha_data_model::block::error::BlockRejectionReason {
        use iroha_data_model::block::error::BlockRejectionReason as Reason;

        match err {
            SignatureVerificationError::NotEnoughSignatures { .. } => {
                Reason::InsufficientBlockSignatures
            }
            SignatureVerificationError::DuplicateSignature { .. }
            | SignatureVerificationError::UnknownSignature
            | SignatureVerificationError::MissingPop => Reason::InvalidBlockSignature,
            SignatureVerificationError::UnknownSignatory => Reason::UnknownBlockSignatory,
            SignatureVerificationError::InactiveConsensusKey => Reason::InactiveConsensusKey,
            SignatureVerificationError::ProxyTailMissing => Reason::ProxyTailSignatureMissing,
            SignatureVerificationError::LeaderMissing => Reason::LeaderSignatureMissing,
            SignatureVerificationError::Other => Reason::OtherSignatureError,
        }
    }

    pub(super) fn map_block_err_to_reason(
        err: &BlockValidationError,
    ) -> iroha_data_model::block::error::BlockRejectionReason {
        use iroha_data_model::block::error::BlockRejectionReason as Reason;

        match err {
            BlockValidationError::HasCommittedTransactions => Reason::ContainsCommittedTransactions,
            BlockValidationError::EmptyBlock => Reason::EmptyBlock,
            BlockValidationError::DuplicateTransactions => Reason::TransactionValidationFailed,
            BlockValidationError::ExecutionContextInvalid(_) => Reason::TransactionValidationFailed,
            BlockValidationError::PrevBlockHashMismatch { .. } => Reason::PrevBlockHashMismatch,
            BlockValidationError::PrevBlockHeightMismatch { .. } => Reason::PrevBlockHeightMismatch,
            BlockValidationError::MerkleRootMismatch => Reason::MerkleRootMismatch,
            BlockValidationError::TransactionAccept(fail) => match fail {
                AcceptTransactionFail::TransactionLimit(_)
                | AcceptTransactionFail::SignatureVerification(_)
                | AcceptTransactionFail::UnexpectedGenesisAccountSignature
                | AcceptTransactionFail::ChainIdMismatch(_)
                | AcceptTransactionFail::TransactionInTheFuture
                | AcceptTransactionFail::TransactionExpired { .. }
                | AcceptTransactionFail::NetworkTimeUnhealthy { .. } => {
                    Reason::TransactionValidationFailed
                }
            },
            BlockValidationError::TopologyMismatch { .. } => Reason::TopologyMismatch,
            BlockValidationError::SignatureVerification(e) => map_sig_err_to_reason(e),
            BlockValidationError::InvalidGenesis(_) => Reason::InvalidGenesis,
            BlockValidationError::BlockInThePast => Reason::BlockInThePast,
            BlockValidationError::BlockInTheFuture => Reason::BlockInTheFuture,
            BlockValidationError::TransactionInTheFuture => Reason::TransactionInTheFuture,
            BlockValidationError::ConfidentialFeaturesMismatch { .. } => {
                Reason::ConfidentialFeatureDigestMismatch
            }
            BlockValidationError::ProofPolicyHashMismatch { .. } => Reason::DaProofPolicyMismatch,
            BlockValidationError::PreviousRosterEvidenceInvalid(_) => Reason::TopologyMismatch,
            BlockValidationError::DaShardCursor(_) => Reason::DaShardCursorViolation,
            BlockValidationError::AxtEnvelopeValidationFailed(_) => {
                Reason::TransactionValidationFailed
            }
            BlockValidationError::NposEffectsInvalid(_) => Reason::NposEffectsMismatch,
        }
    }

    impl EventProducer for BlockValidationError {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox> {
            // Rejection events require a block header to construct `BlockEvent`.
            // These are emitted by callers at sites where the header is available
            // (e.g., `commit_keep_voting_block`), so nothing is produced here.
            core::iter::empty()
        }
    }

    impl<T: EventProducer + ?Sized> EventProducer for Box<T> {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox> {
            (**self).produce_events()
        }
    }

    impl EventProducer for SignatureVerificationError {
        fn produce_events(&self) -> impl Iterator<Item = PipelineEventBox> {
            // Similar to `BlockValidationError`: emission is performed by the
            // caller at the site where the header is available.
            core::iter::empty()
        }
    }
}

fn dedup_sorted_usize_smallvec(parents: &mut iroha_primitives::small::SmallVec<[usize; 8]>) {
    if parents.0.len() <= 1 {
        return;
    }
    #[cfg(feature = "simd")]
    if let Some(len) = simd_parent_dedup::dedup_sorted_slice(parents.0.as_mut_slice()) {
        parents.0.truncate(len);
        return;
    }
    let mut write = 1usize;
    let mut last = parents.0[0];
    for i in 1..parents.0.len() {
        let value = parents.0[i];
        if value != last {
            parents.0[write] = value;
            write += 1;
            last = value;
        }
    }
    parents.0.truncate(write);
}

#[cfg(feature = "simd")]
mod simd_parent_dedup {
    use core::simd::{LaneCount, Simd, SimdPartialEq, SupportedLaneCount};

    const LANES: usize = 8;

    pub(super) fn dedup_sorted_slice(slice: &mut [usize]) -> Option<usize>
    where
        LaneCount<LANES>: SupportedLaneCount,
    {
        if slice.len() <= 1 {
            return Some(slice.len());
        }
        let mut write = 1usize;
        let mut prev = slice[0];
        let mut idx = 1usize;
        while idx + LANES <= slice.len() {
            let chunk = Simd::<usize, LANES>::from_slice(&slice[idx..idx + LANES]);
            let mut prev_arr = [prev; LANES];
            prev_arr[1..].copy_from_slice(&slice[idx..idx + LANES - 1]);
            let mask = chunk.simd_ne(Simd::from_array(prev_arr));
            let mut bits = mask.to_bitmask() as u32;
            let arr = chunk.to_array();
            while bits != 0 {
                let lane = bits.trailing_zeros() as usize;
                let value = arr[lane];
                slice[write] = value;
                write += 1;
                prev = value;
                bits &= bits - 1;
            }
            prev = arr[LANES - 1];
            idx += LANES;
        }
        while idx < slice.len() {
            let value = slice[idx];
            if value != prev {
                slice[write] = value;
                write += 1;
                prev = value;
            }
            idx += 1;
        }
        Some(write)
    }
}

/// Build a conflict graph from access sets using an incremental O(n + E) algorithm.
/// Returns adjacency list and indegree vector.
#[allow(dead_code, clippy::disallowed_types)]
fn build_conflict_graph(
    access: &[crate::pipeline::access::AccessSet],
) -> (
    Vec<iroha_primitives::small::SmallVec<[usize; 8]>>,
    Vec<usize>,
) {
    use iroha_primitives::small::SmallVec;

    // Intern keys once per block to operate on compact integer IDs while
    // preserving deterministic ordering across peers.
    let (key_count, access_ids) = intern_access(access);

    let n = access.len();
    let mut adj: Vec<SmallVec<[usize; 8]>> = vec![SmallVec::new(); n];
    let mut indeg = vec![0usize; n];

    // Track the most recent writer per interned key and readers awaiting a write.
    let mut last_writer: Vec<Option<usize>> = vec![None; key_count];
    let mut open_readers: Vec<SmallVec<[usize; 4]>> = (0..key_count)
        .map(|_| SmallVec::<[usize; 4]>::new())
        .collect();

    // Component partitioning via disjoint-set prepass is handled before scheduling.
    for (idx, aset) in access_ids.iter().enumerate() {
        // Collect parents in a small vec; sort+dedup to avoid the log factor of BTreeSet
        let mut parents: SmallVec<[usize; 8]> = SmallVec::new();

        // Read dependencies: last writer of each read key must precede this read
        for &key in aset.reads.iter() {
            let key_idx = key as usize;
            if let Some(writer) = last_writer[key_idx] {
                parents.push(writer);
            }
            open_readers[key_idx].push(idx);
        }
        // Write dependencies: last writer must precede; all open readers must precede
        for &key in aset.writes.iter() {
            let key_idx = key as usize;
            if let Some(writer) = last_writer[key_idx] {
                parents.push(writer);
            }
            let readers = &mut open_readers[key_idx];
            for &reader in readers.iter() {
                parents.push(reader);
            }
            readers.clear();
            last_writer[key_idx] = Some(idx);
        }

        if !parents.is_empty() {
            // Deterministic dedup without extra allocations
            parents.sort_unstable();
            let mut write = 0usize;
            let mut last: Option<usize> = None;
            for i in 0..parents.len() {
                let v = parents[i];
                if Some(v) != last {
                    parents[write] = v;
                    write += 1;
                    last = Some(v);
                }
            }
            while parents.len() > write {
                let _ = parents.remove(parents.len() - 1);
            }
            for p in parents {
                adj[p].push(idx);
                indeg[idx] += 1;
            }
        }
    }
    (adj, indeg)
}

#[cfg(test)]
mod dag_tests {
    use super::build_conflict_graph;
    use crate::pipeline::access::AccessSet;

    fn rw(reads: &[&str], writes: &[&str]) -> AccessSet {
        let mut s = AccessSet::new();
        for k in reads {
            s.add_read((*k).to_string());
        }
        for k in writes {
            s.add_write((*k).to_string());
        }
        s
    }

    #[test]
    fn ww_conflict_edge() {
        let a = rw(&[], &["k"]);
        let b = rw(&[], &["k"]);
        let (adj, indeg) = build_conflict_graph(&[a, b]);
        assert_eq!(indeg, vec![0, 1]);
        assert_eq!(&adj[0][..], &[1]);
        assert!(adj[1].is_empty());
    }

    #[test]
    fn state_map_wildcard_conflicts_with_map_entries() {
        let a = rw(&[], &["state:Foo/1"]);
        let b = rw(&[], &["state:Foo/2"]);
        let c = rw(&[], &["state:Foo[*]"]);
        let (adj, indeg) = build_conflict_graph(&[a, b, c]);
        assert_eq!(indeg, vec![0, 0, 2]);
        assert_eq!(&adj[0][..], &[2]);
        assert_eq!(&adj[1][..], &[2]);
        assert!(adj[2].is_empty());
    }

    #[test]
    fn global_wildcard_conflicts_with_all() {
        let a = rw(&[], &["k1"]);
        let b = rw(&[], &["*"]);
        let c = rw(&[], &["k2"]);
        let (adj, indeg) = build_conflict_graph(&[a, b, c]);
        assert_eq!(indeg, vec![0, 1, 1]);
        assert_eq!(&adj[0][..], &[1]);
        assert_eq!(&adj[1][..], &[2]);
        assert!(adj[2].is_empty());
    }

    #[test]
    fn state_global_wildcard_conflicts_with_state_entries() {
        let a = rw(&[], &["state:Foo/1"]);
        let b = rw(&[], &["state:*"]);
        let c = rw(&[], &["state:Foo/2"]);
        let (adj, indeg) = build_conflict_graph(&[a, b, c]);
        assert_eq!(indeg, vec![0, 1, 1]);
        assert_eq!(&adj[0][..], &[1]);
        assert_eq!(&adj[1][..], &[2]);
        assert!(adj[2].is_empty());
    }

    #[test]
    fn wr_conflict_edge() {
        let a = rw(&[], &["k"]);
        let b = rw(&["k"], &[]);
        let (adj, indeg) = build_conflict_graph(&[a, b]);
        assert_eq!(indeg, vec![0, 1]);
        assert_eq!(&adj[0][..], &[1]);
        assert!(adj[1].is_empty());
    }

    #[test]
    fn rw_conflict_edge() {
        let a = rw(&["k"], &[]);
        let b = rw(&[], &["k"]);
        let (adj, indeg) = build_conflict_graph(&[a, b]);
        assert_eq!(indeg, vec![0, 1]);
        assert_eq!(&adj[0][..], &[1]);
        assert!(adj[1].is_empty());
    }

    #[test]
    fn dedup_edges_for_multiple_keys() {
        let a = rw(&[], &["x", "y"]);
        let b = rw(&["x", "y"], &[]);
        let (adj, indeg) = build_conflict_graph(&[a, b]);
        assert_eq!(indeg, vec![0, 1]);
        assert_eq!(&adj[0][..], &[1]); // only one edge despite two overlapping keys
    }

    #[test]
    fn disjoint_transactions_remain_independent() {
        let a = rw(&["alpha"], &[]);
        let b = rw(&[], &["beta"]);
        let c = rw(&["gamma"], &[]);
        let (adj, indeg) = build_conflict_graph(&[a, b, c]);
        assert_eq!(indeg, vec![0, 0, 0]);
        assert!(adj.iter().all(|neighbors| neighbors.is_empty()));
    }

    #[test]
    fn chain_reads_and_writes() {
        // 0: R(A); 1: W(A); 2: R(A); 3: W(A)
        let a0 = rw(&["A"], &[]);
        let a1 = rw(&[], &["A"]);
        let a2 = rw(&["A"], &[]);
        let a3 = rw(&[], &["A"]);
        let (adj, indeg) = build_conflict_graph(&[a0, a1, a2, a3]);
        assert_eq!(indeg, vec![0, 1, 1, 2]);
        assert_eq!(&adj[0][..], &[1]);
        assert_eq!(&adj[1][..], &[2, 3]);
        assert_eq!(&adj[2][..], &[3]);
        assert!(adj[3].is_empty());
    }
}

#[cfg(test)]
mod dsu_tests {
    use iroha_primitives::small::SmallVec;

    use super::{DisjointSet, intern_access};
    use crate::pipeline::access::AccessSet;

    fn ids(reads: &[&str], writes: &[&str]) -> AccessSet {
        let mut s = AccessSet::new();
        for k in reads {
            s.add_read((*k).to_string());
        }
        for k in writes {
            s.add_write((*k).to_string());
        }
        s
    }

    #[test]
    fn dsu_partitions_independent_components() {
        // Two independent components: {0,1} share key "A"; {2,3} share key "B".
        let a0 = ids(&["A"], &[]);
        let a1 = ids(&[], &["A"]);
        let b0 = ids(&["B"], &[]);
        let b1 = ids(&[], &["B"]);
        let access = [a0, a1, b0, b1];
        // Intern
        let (key_count, access_ids) = intern_access(&access);

        let mut dsu = DisjointSet::new(access_ids.len());
        {
            let mut last_writer: Vec<Option<usize>> = vec![None; key_count];
            let mut open_readers: Vec<SmallVec<[usize; 4]>> = vec![SmallVec::new(); key_count];
            for (idx, aset) in access_ids.iter().enumerate() {
                for &k in aset.reads.iter() {
                    if let Some(w) = last_writer[k as usize] {
                        dsu.union(idx, w);
                    }
                    open_readers[k as usize].push(idx);
                }
                for &k in aset.writes.iter() {
                    if let Some(w) = last_writer[k as usize] {
                        dsu.union(idx, w);
                    }
                    if let Some(readers) = {
                        if open_readers[k as usize].is_empty() {
                            None
                        } else {
                            Some(std::mem::take(&mut open_readers[k as usize]))
                        }
                    } {
                        for r in readers {
                            dsu.union(idx, r);
                        }
                    }
                    last_writer[k as usize] = Some(idx);
                }
            }
        }
        let mut roots: Vec<usize> = Vec::new();
        let mut dsu_copy = dsu.clone();
        for i in 0..4 {
            roots.push(dsu_copy.find(i));
        }
        // Expect two distinct roots among four items
        let mut uniq = roots.clone();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(uniq.len(), 2);
    }
}

#[cfg(test)]
mod scheduler_variant_tests {
    use iroha_crypto::{Hash, HashOf};
    use iroha_data_model::transaction::signed::TransactionEntrypoint;

    fn make_hash(v: u8) -> HashOf<TransactionEntrypoint> {
        let mut b = [0u8; Hash::LENGTH];
        b[0] = v;
        b[Hash::LENGTH - 1] |= 1; // keep LSB set as per Hash invariant
        HashOf::from_untyped_unchecked(Hash::prehashed(b))
    }

    // Build a small CSR graph by hand for testing
    // adj: 0 -> [2]; 1 -> [2,3]; 2 -> []; 3 -> [4]; 4 -> []
    fn sample_graph() -> (
        Vec<usize>,
        Vec<usize>,
        Vec<usize>,
        Vec<HashOf<TransactionEntrypoint>>,
    ) {
        let row_offsets = vec![0, 1, 3, 3, 4, 4];
        let cols = vec![2, 2, 3, 4];
        let indeg = vec![0, 0, 2, 1, 1];
        let call_hashes = vec![
            make_hash(10),
            make_hash(5),
            make_hash(30),
            make_hash(7),
            make_hash(8),
        ];
        (row_offsets, cols, indeg, call_hashes)
    }

    #[test]
    fn per_wave_scheduler_deterministic_order() {
        let (row_offsets, cols, indeg, call_hashes) = sample_graph();
        // Implement per-wave scheduling locally for test
        let n = indeg.len();
        let mut indeg_s = indeg.clone();
        let mut ready = Vec::new();
        for (i, &deg) in indeg_s.iter().enumerate() {
            if deg == 0 {
                ready.push(i);
            }
        }
        let mut order = Vec::with_capacity(n);
        while !ready.is_empty() {
            ready.sort_unstable_by(|&a, &b| {
                call_hashes[a].cmp(&call_hashes[b]).then_with(|| a.cmp(&b))
            });
            let current = ready.split_off(0);
            for &i in &current {
                order.push(i);
                let (start, end) = (row_offsets[i], row_offsets[i + 1]);
                for &v in &cols[start..end] {
                    indeg_s[v] = indeg_s[v].saturating_sub(1);
                    if indeg_s[v] == 0 {
                        ready.push(v);
                    }
                }
            }
        }
        assert_eq!(order, vec![1, 0, 3, 2, 4]);
    }

    #[test]
    fn ready_heap_scheduler_topo_order() {
        use std::{cmp::Reverse, collections::BinaryHeap};
        let (row_offsets, cols, indeg, call_hashes) = sample_graph();
        let n = indeg.len();
        let mut indeg_s = indeg.clone();
        let mut heap: BinaryHeap<Reverse<(HashOf<TransactionEntrypoint>, usize)>> =
            BinaryHeap::with_capacity(n);
        for i in 0..n {
            if indeg_s[i] == 0 {
                heap.push(Reverse((call_hashes[i], i)));
            }
        }
        let mut order = Vec::with_capacity(n);
        while let Some(Reverse((_h, i))) = heap.pop() {
            order.push(i);
            let (start, end) = (row_offsets[i], row_offsets[i + 1]);
            for &v in &cols[start..end] {
                indeg_s[v] = indeg_s[v].saturating_sub(1);
                if indeg_s[v] == 0 {
                    heap.push(Reverse((call_hashes[v], v)));
                }
            }
        }

        // Valid deterministic topological order
        assert_eq!(order, vec![1, 3, 4, 0, 2]);
    }

    #[test]
    fn component_scheduler_orders_components_contiguously() {
        let components = vec![vec![2, 3, 4], vec![0, 1]];
        let row_offsets = vec![0, 1, 1, 2, 3, 3];
        let cols = vec![1, 3, 4];
        let indeg = vec![0, 1, 0, 1, 1];
        let call_hashes = vec![
            make_hash(10),
            make_hash(12),
            make_hash(5),
            make_hash(40),
            make_hash(50),
        ];

        let wave = super::schedule_components_wave(&components, &row_offsets, &cols, &call_hashes)
            .expect("component scheduling must succeed (wave)");
        assert_eq!(wave, vec![2, 3, 4, 0, 1]);

        let heap =
            super::schedule_components_ready_heap(&components, &row_offsets, &cols, &call_hashes)
                .expect("component scheduling must succeed (heap)");
        assert_eq!(heap, vec![2, 3, 4, 0, 1]);

        let global_wave = super::schedule_wave_global(&row_offsets, &cols, &indeg, &call_hashes);
        assert_eq!(global_wave, vec![2, 0, 1, 3, 4]);

        let global_heap =
            super::schedule_ready_heap_global(&row_offsets, &cols, &indeg, &call_hashes);
        assert_eq!(global_heap, vec![2, 0, 1, 3, 4]);
    }

    #[test]
    fn conflict_free_layers_merge_singletons_into_one_wave() {
        let components = vec![vec![3], vec![1], vec![0], vec![2]];
        let row_offsets = vec![0, 0, 0, 0, 0];
        let cols = Vec::new();
        let call_hashes = vec![make_hash(40), make_hash(10), make_hash(30), make_hash(20)];

        let layers =
            super::conflict_free_component_layers(&components, &row_offsets, &cols, &call_hashes)
                .expect("singleton components must schedule");

        assert_eq!(layers, vec![vec![1, 3, 2, 0]]);
    }

    #[test]
    fn conflict_free_layers_preserve_component_depths() {
        let components = vec![vec![2, 0, 1], vec![4, 3]];
        let row_offsets = vec![0, 1, 2, 2, 3, 3];
        let cols = vec![1, 2, 4];
        let call_hashes = vec![
            make_hash(20),
            make_hash(10),
            make_hash(30),
            make_hash(15),
            make_hash(5),
        ];

        let layers =
            super::conflict_free_component_layers(&components, &row_offsets, &cols, &call_hashes)
                .expect("component-local chains must schedule");

        assert_eq!(layers, vec![vec![3, 0], vec![4, 1], vec![2]]);
    }
}

#[cfg(test)]
mod tests {
    use core::time::Duration;
    use std::{borrow::Cow, num::NonZeroU64};

    use iroha_crypto::Hash;
    use iroha_data_model::{
        errors::AmxStage,
        events::pipeline::{BlockEventFilter, TransactionEventFilter},
        prelude::*,
        transaction::signed::{
            SealedTransactionCommitmentPayload, SealedTransactionReveal,
            SignedSealedTransactionCommitment, SignedTransaction,
            compute_sealed_transaction_commitment,
        },
    };
    use iroha_genesis::GENESIS_DOMAIN_ID;
    use iroha_primitives::json::Json;
    use iroha_primitives::time::TimeSource;
    use iroha_test_samples::gen_account_in;
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        block::event::map_sig_err_to_reason,
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::Execute,
        state::{State, World},
        sumeragi::network_topology::test_topology,
        tx::AcceptedTransaction,
    };

    fn dummy_accepted_transaction() -> AcceptedTransaction<'static> {
        let chain_id: ChainId = "00000000-0000-0000-0000-000000000000"
            .parse()
            .expect("valid chain id");
        let (account_id, keypair) = gen_account_in("dummy");
        let mut builder = TransactionBuilder::new(chain_id, account_id);
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_instructions([Log::new(Level::INFO, "dummy".to_owned())])
            .sign(keypair.private_key());
        AcceptedTransaction::new_unchecked(Cow::Owned(tx))
    }

    fn native_amx_test_catalog(
        paynet: DataSpaceId,
        cbuae: DataSpaceId,
    ) -> iroha_data_model::nexus::DataSpaceCatalog {
        iroha_data_model::nexus::DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: paynet,
                alias: "paynet".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
            iroha_data_model::nexus::DataSpaceMetadata {
                id: cbuae,
                alias: "cbuae".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog")
    }

    fn signed_domain_registration_tx(
        domains: &[(&str, &str)],
    ) -> (SignedTransaction, HashOf<SignedTransaction>) {
        let chain_id: ChainId = "00000000-0000-0000-0000-000000000000"
            .parse()
            .expect("valid chain id");
        let (authority_id, keypair) = gen_account_in("wonderland");
        let instructions = domains
            .iter()
            .map(|(name, dataspace_alias)| {
                InstructionBox::from(Register::domain(Domain::new(
                    DomainId::try_new(*name, *dataspace_alias).expect("domain id"),
                )))
            })
            .collect::<Vec<_>>();
        let tx = TransactionBuilder::new(chain_id, authority_id)
            .with_instructions(instructions)
            .sign(keypair.private_key());
        let tx_hash = AcceptedTransaction::prepare_signed_metadata(&tx).signed_hash;
        (tx, tx_hash)
    }

    #[test]
    fn native_amx_receipt_records_participant_dataspace_legs() {
        let paynet = DataSpaceId::new(7);
        let cbuae = DataSpaceId::new(8);
        let (tx, tx_hash) =
            signed_domain_registration_tx(&[("merchant", "paynet"), ("treasury", "cbuae")]);
        let dataspace_catalog = native_amx_test_catalog(paynet, cbuae);
        let world = World::new();
        let world_view = world.view();

        let receipt = native_amx_receipt_for_transaction(
            &tx,
            tx_hash,
            42,
            crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            &dataspace_catalog,
            &world_view,
        )
        .expect("mixed dataspace transaction should produce native AMX receipt");

        assert_eq!(receipt.version, 1);
        assert_eq!(receipt.source_id.as_slice(), tx_hash.as_ref());
        assert_eq!(receipt.lane_id, LaneId::SINGLE);
        assert_eq!(receipt.dataspace_id, DataSpaceId::UNIVERSAL);
        assert_eq!(receipt.block_height, 42);
        assert_eq!(
            receipt
                .legs
                .iter()
                .map(|leg| (leg.dataspace_id, leg.prepared, leg.committed))
                .collect::<Vec<_>>(),
            vec![(paynet, true, true), (cbuae, true, true)]
        );
    }

    #[test]
    fn native_amx_receipt_skips_non_universal_route() {
        let paynet = DataSpaceId::new(7);
        let cbuae = DataSpaceId::new(8);
        let (tx, tx_hash) =
            signed_domain_registration_tx(&[("merchant", "paynet"), ("treasury", "cbuae")]);
        let dataspace_catalog = native_amx_test_catalog(paynet, cbuae);
        let world = World::new();
        let world_view = world.view();

        let receipt = native_amx_receipt_for_transaction(
            &tx,
            tx_hash,
            42,
            crate::queue::RoutingDecision::new(LaneId::SINGLE, paynet),
            &dataspace_catalog,
            &world_view,
        );

        assert!(
            receipt.is_none(),
            "non-universal routing must not emit native AMX receipts"
        );
    }

    #[test]
    fn native_amx_receipt_skips_single_participant_universal_route() {
        let paynet = DataSpaceId::new(7);
        let cbuae = DataSpaceId::new(8);
        let (tx, tx_hash) = signed_domain_registration_tx(&[("merchant", "paynet")]);
        let dataspace_catalog = native_amx_test_catalog(paynet, cbuae);
        let world = World::new();
        let world_view = world.view();

        let receipt = native_amx_receipt_for_transaction(
            &tx,
            tx_hash,
            42,
            crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            &dataspace_catalog,
            &world_view,
        );

        assert!(
            receipt.is_none(),
            "single-participant universal routes must not emit native AMX receipts"
        );
    }

    #[test]
    fn native_amx_receipt_skips_unknown_dataspace_alias() {
        let paynet = DataSpaceId::new(7);
        let cbuae = DataSpaceId::new(8);
        let (tx, tx_hash) =
            signed_domain_registration_tx(&[("merchant", "paynet"), ("treasury", "rogue")]);
        let dataspace_catalog = native_amx_test_catalog(paynet, cbuae);
        let world = World::new();
        let world_view = world.view();

        let receipt = native_amx_receipt_for_transaction(
            &tx,
            tx_hash,
            42,
            crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            &dataspace_catalog,
            &world_view,
        );

        assert!(
            receipt.is_none(),
            "unknown dataspace aliases must not create synthetic native AMX receipt legs"
        );
    }

    #[test]
    fn native_amx_receipt_deduplicates_repeated_participant_dataspaces() {
        let paynet = DataSpaceId::new(7);
        let cbuae = DataSpaceId::new(8);
        let (tx, tx_hash) =
            signed_domain_registration_tx(&[("merchant", "paynet"), ("treasury", "paynet")]);
        let dataspace_catalog = native_amx_test_catalog(paynet, cbuae);
        let world = World::new();
        let world_view = world.view();

        let receipt = native_amx_receipt_for_transaction(
            &tx,
            tx_hash,
            42,
            crate::queue::RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL),
            &dataspace_catalog,
            &world_view,
        );

        assert!(
            receipt.is_none(),
            "repeated references to one dataspace must not be counted as multi-leg native AMX"
        );
    }

    fn seed_domain_name_lease(world: &mut World, owner: &AccountId, domain_id: &DomainId) {
        let selector = crate::sns::selector_for_domain(domain_id).expect("selector");
        let address =
            iroha_data_model::account::AccountAddress::from_account_id(owner).expect("address");
        let record = iroha_data_model::sns::NameRecordV1::new(
            selector.clone(),
            owner.clone(),
            vec![iroha_data_model::sns::NameControllerV1::account(&address)],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            Metadata::default(),
        );
        world.smart_contract_state_mut_for_testing().insert(
            crate::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );
    }

    #[allow(dead_code)]
    fn commit_block_at_height(
        state: &State,
        kura: &Arc<Kura>,
        topology: &Topology,
        leader_private: &PrivateKey,
        height: u64,
        prev_hash: Option<HashOf<BlockHeader>>,
        creation_time_ms: u64,
    ) -> HashOf<BlockHeader> {
        let valid = ValidBlock::new_dummy_and_modify_header(leader_private, |header| {
            header.set_height(NonZeroU64::new(height).expect("non-zero height in commit helper"));
            header.set_prev_block_hash(prev_hash);
            header.creation_time_ms = creation_time_ms;
        });
        let committed = valid.commit_unchecked().unpack(|_| {});
        {
            let mut state_block = state.block(committed.as_ref().header());
            let _ = state_block.apply_without_execution(&committed, topology.as_ref().to_owned());
            state_block.commit().unwrap();
        }
        kura.store_block(committed.clone())
            .expect("store committed block");
        committed.as_ref().hash()
    }

    #[test]
    fn map_overlay_error_labels_amx_budget() {
        let err =
            crate::pipeline::overlay::OverlayBuildError::IvmRun(ivm::VMError::AmxBudgetExceeded {
                dataspace: DataSpaceId::new(5),
                stage: AmxStage::Commit,
                elapsed_ms: 42,
                budget_ms: 30,
            });
        match super::map_overlay_error(&err) {
            TransactionRejectionReason::Validation(
                iroha_data_model::ValidationFail::NotPermitted(message),
            ) => {
                assert!(
                    message.contains("AMX_TIMEOUT"),
                    "message missing AMX_TIMEOUT label: {message}"
                );
                assert!(
                    message.contains("dataspace=5"),
                    "message missing dataspace label: {message}"
                );
                assert!(
                    message.contains(
                        &iroha_data_model::errors::CanonicalErrorKind::AMX_TIMEOUT_CODE.to_string()
                    ),
                    "message missing canonical code: {message}"
                );
            }
            other => panic!("unexpected rejection: {other:?}"),
        }
    }

    #[test]
    fn map_overlay_error_labels_amx_violation_variant() {
        let err = crate::pipeline::overlay::OverlayBuildError::AmxBudgetViolation(
            crate::smartcontracts::ivm::host::AmxBudgetViolation {
                dataspace: DataSpaceId::new(7),
                stage: AmxStage::Prepare,
                elapsed_ms: 99,
                budget_ms: 10,
            },
        );
        match super::map_overlay_error(&err) {
            TransactionRejectionReason::Validation(
                iroha_data_model::ValidationFail::NotPermitted(message),
            ) => {
                assert!(
                    message.contains("AMX_TIMEOUT"),
                    "message missing AMX_TIMEOUT label: {message}"
                );
                assert!(
                    message.contains("dataspace=7"),
                    "message missing dataspace label: {message}"
                );
                assert!(
                    message.contains(
                        &iroha_data_model::errors::CanonicalErrorKind::AMX_TIMEOUT_CODE.to_string()
                    ),
                    "message missing canonical code: {message}"
                );
            }
            other => panic!("unexpected rejection: {other:?}"),
        }
    }

    #[test]
    pub fn committed_and_valid_block_hashes_are_equal() {
        let peer_key_pair = KeyPair::random_with_algorithm(iroha_crypto::Algorithm::BlsNormal);
        let peer_id = PeerId::new(peer_key_pair.public_key().clone());
        let topology = Topology::new(vec![peer_id]);
        let valid_block = ValidBlock::new_dummy(peer_key_pair.private_key());
        let committed_block = valid_block
            .clone()
            .commit(&topology)
            .unpack(|_| {})
            .unwrap();

        assert_eq!(valid_block.as_ref().hash(), committed_block.as_ref().hash())
    }

    #[test]
    fn merkle_root_matches_header() {
        use std::borrow::Cow;
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");
        let (alice_id, alice_keypair) = gen_account_in("wonderland");

        let log = Log::new(Level::INFO, "test".to_string());

        let tx1 = Box::new(
            TransactionBuilder::new(chain_id.clone(), alice_id.clone())
                .with_instructions([log.clone()])
                .sign(alice_keypair.private_key()),
        );
        let tx1: &'static SignedTransaction = Box::leak(tx1);
        let tx1 = AcceptedTransaction::new_unchecked(Cow::Borrowed(tx1));

        let tx2 = Box::new(
            TransactionBuilder::new(chain_id, alice_id.clone())
                .with_instructions([log])
                .sign(alice_keypair.private_key()),
        );
        let tx2: &'static SignedTransaction = Box::leak(tx2);
        let tx2 = AcceptedTransaction::new_unchecked(Cow::Borrowed(tx2));

        let block = BlockBuilder::new(vec![tx1, tx2])
            .chain(0, None)
            .sign(alice_keypair.private_key())
            .unpack(|_| {});

        let block: Box<SignedBlock> = Box::new(block.into());
        let mut tree: Box<MerkleTree<TransactionEntrypoint>> = Box::default();
        for tx in block.external_transactions() {
            tree.add(tx.hash_as_entrypoint());
        }

        assert_eq!(tree.root(), block.header().merkle_root());
    }

    #[test]
    fn entrypoint_merkle_bottom_up_matches_incremental_root_shapes() {
        fn sample_leaf(idx: u8) -> HashOf<TransactionEntrypoint> {
            let mut bytes = [0_u8; Hash::LENGTH];
            bytes[0] = idx;
            bytes[Hash::LENGTH - 1] = idx.wrapping_mul(17);
            HashOf::from_untyped_unchecked(Hash::prehashed(bytes))
        }

        fn incremental_root(
            leaves: &[HashOf<TransactionEntrypoint>],
        ) -> Option<HashOf<MerkleTree<TransactionEntrypoint>>> {
            let mut tree = MerkleTree::default();
            for leaf in leaves {
                tree.add(*leaf);
            }
            tree.root()
        }

        fn bottom_up_root(
            leaves: Vec<HashOf<TransactionEntrypoint>>,
        ) -> Option<HashOf<MerkleTree<TransactionEntrypoint>>> {
            let tree = MerkleTree::from_typed_leaves_parallel(leaves);
            tree.root()
        }

        for count in [1_usize, 2, 3, 4, 5, 8] {
            let leaves = (0..count)
                .map(|idx| sample_leaf(u8::try_from(idx + 1).expect("small test index")))
                .collect::<Vec<_>>();
            assert_eq!(
                bottom_up_root(leaves.clone()),
                incremental_root(&leaves),
                "bottom-up Merkle root must match incremental insertion for {count} leaves"
            );
        }
    }

    #[test]
    fn lane_relay_helper_emits_pending_relay_and_rbc_bytes() {
        use iroha_crypto::{Hash, HashOf};
        use iroha_data_model::{
            block::consensus::{LaneBlockCommitment, LaneSettlementReceipt},
            da::commitment::DaCommitmentBundle,
            nexus::{DataSpaceId, LaneId},
        };

        let da_hash: Option<HashOf<DaCommitmentBundle>> = Some(HashOf::from_untyped_unchecked(
            Hash::prehashed([0xAB; Hash::LENGTH]),
        ));
        let mut block_header = BlockHeader::new(
            core::num::NonZeroU64::new(5).expect("non-zero height"),
            None,
            None,
            None,
            1_700_000_000_000,
            0,
        );
        block_header.set_da_commitments_hash(da_hash);

        let lane_id = LaneId::new(2);
        let dataspace_id = DataSpaceId::new(1);
        let receipt = LaneSettlementReceipt {
            source_id: [0x11; 32],
            local_amount_micro: 10,
            xor_due_micro: 20,
            xor_after_haircut_micro: 18,
            xor_variance_micro: 2,
            timestamp_ms: 1_700_000_100,
        };
        let settlement = LaneBlockCommitment {
            block_height: block_header.height().get(),
            lane_id,
            dataspace_id,
            tx_count: 1,
            total_local_micro: receipt.local_amount_micro,
            total_xor_due_micro: receipt.xor_due_micro,
            total_xor_after_haircut_micro: receipt.xor_after_haircut_micro,
            total_xor_variance_micro: receipt.xor_variance_micro,
            swap_metadata: None,
            receipts: vec![receipt],
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        };

        let mut lane_summaries = BTreeMap::new();
        lane_summaries.insert(
            lane_id,
            LaneSummary {
                rbc_bytes_total: 2048,
                ..LaneSummary::default()
            },
        );

        let relays = lane_relay_envelopes_for_block(
            &block_header,
            da_hash,
            std::slice::from_ref(&settlement),
            &lane_summaries,
        );
        assert_eq!(relays.len(), 1);
        let envelope = &relays[0];
        assert!(
            envelope.qc.is_none(),
            "block-level commit QC must not be copied into lane relay QC"
        );
        assert_eq!(envelope.rbc_bytes_total, 2048);
        envelope.verify().expect("envelope should validate");
    }

    #[test]
    fn lane_relay_envelopes_attach_manifest_roots() {
        use iroha_crypto::{Hash, HashOf};
        use iroha_data_model::{
            block::consensus::{LaneBlockCommitment, LaneSettlementReceipt},
            da::commitment::DaCommitmentBundle,
            nexus::{DataSpaceId, LaneId},
        };

        let da_hash: Option<HashOf<DaCommitmentBundle>> = Some(HashOf::from_untyped_unchecked(
            Hash::prehashed([0xAB; Hash::LENGTH]),
        ));
        let mut block_header = BlockHeader::new(
            core::num::NonZeroU64::new(5).expect("non-zero height"),
            None,
            None,
            None,
            1_700_000_000_000,
            0,
        );
        block_header.set_da_commitments_hash(da_hash);

        let lane_id = LaneId::new(2);
        let dataspace_id = DataSpaceId::new(1);
        let receipt = LaneSettlementReceipt {
            source_id: [0x11; 32],
            local_amount_micro: 10,
            xor_due_micro: 20,
            xor_after_haircut_micro: 18,
            xor_variance_micro: 2,
            timestamp_ms: 1_700_000_100,
        };
        let settlement = LaneBlockCommitment {
            block_height: block_header.height().get(),
            lane_id,
            dataspace_id,
            tx_count: 1,
            total_local_micro: receipt.local_amount_micro,
            total_xor_due_micro: receipt.xor_due_micro,
            total_xor_after_haircut_micro: receipt.xor_after_haircut_micro,
            total_xor_variance_micro: receipt.xor_variance_micro,
            swap_metadata: None,
            receipts: vec![receipt],
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        };

        let mut lane_summaries = BTreeMap::new();
        lane_summaries.insert(
            lane_id,
            LaneSummary {
                rbc_bytes_total: 512,
                ..LaneSummary::default()
            },
        );

        let mut envelopes = lane_relay_envelopes_for_block(
            &block_header,
            da_hash,
            std::slice::from_ref(&settlement),
            &lane_summaries,
        );
        let manifest_root = [0x44; 32];
        let manifest_roots: BTreeMap<DataSpaceId, [u8; 32]> =
            core::iter::once((dataspace_id, manifest_root)).collect();
        attach_manifest_roots_to_relays(&mut envelopes, &manifest_roots);

        assert_eq!(envelopes.len(), 1);
        envelopes[0].fastpq_proof = Some(iroha_data_model::nexus::LaneFastpqProofMaterial {
            proof_digest: Hash::new(b"test-fastpq-proof"),
            verified_at_height: envelopes[0].block_height,
        });
        assert_eq!(envelopes[0].manifest_root, Some(manifest_root));
        assert!(envelopes[0].fastpq_proof.is_some());
        envelopes[0]
            .verify_fastpq_proof_material()
            .expect("FastPQ proof material must validate");
    }

    #[test]
    fn dag_fingerprint_stability_smoke() {
        // Build a small world and a block with two independent txs to exercise access-set derivation
        let chain_id = ChainId::from("chain");
        let (alice_id, _) = iroha_test_samples::gen_account_in("wonderland");
        let (bob_id, _) = iroha_test_samples::gen_account_in("wonderland");
        let domain_id: DomainId =
            DomainId::try_new("wonderland", "universal").expect("wonderland domain");
        let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
        let ad: AssetDefinition = {
            let __asset_definition_id = iroha_data_model::asset::AssetDefinitionId::new(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "coin".parse().unwrap(),
            );
            AssetDefinition::new(__asset_definition_id.clone(), NumericSpec::default())
                .with_name(__asset_definition_id.name().to_string())
        }
        .build(&alice_id);
        let acc_a = Account::new(alice_id.clone()).build(&alice_id);
        let acc_b = Account::new(bob_id.clone()).build(&alice_id);
        let world = crate::state::World::with([domain], [acc_a, acc_b], [ad]);
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new(world, kura, query);

        let rose: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
        let a_coin = AssetId::of(rose.clone(), alice_id.clone());
        let tx1 = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions([Mint::asset_numeric(5_u32, a_coin.clone())])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
        let tx2 = TransactionBuilder::new(chain_id.clone(), bob_id.clone())
            .with_instructions([SetKeyValue::account(
                bob_id.clone(),
                "k".parse().unwrap(),
                iroha_primitives::json::Json::new("v"),
            )])
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key());
        let acc: Vec<_> = vec![tx1, tx2]
            .into_iter()
            .map(|t| crate::tx::AcceptedTransaction::new_unchecked(Cow::Owned(t)))
            .collect();

        // Run twice and ensure both runs succeed (determinism covered by other tests);
        // pipeline persistence is best-effort in tests without a store dir.
        let new_block = BlockBuilder::new(acc.clone())
            .chain(0, None)
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key())
            .unpack(|_| {});
        let mut sb = state.block(new_block.header());
        let vb = ValidBlock::validate_unchecked(new_block.into(), &mut sb).unpack(|_| {});
        let cb = vb.commit_unchecked().unpack(|_| {});
        let _ = sb.apply_without_execution(&cb, Vec::new());
        drop(sb);

        let new_block2 = BlockBuilder::new(acc)
            .chain(0, None)
            .sign(iroha_test_samples::ALICE_KEYPAIR.private_key())
            .unpack(|_| {});
        let mut sb2 = state.block(new_block2.header());
        let vb2 = ValidBlock::validate_unchecked(new_block2.into(), &mut sb2).unpack(|_| {});
        let cb2 = vb2.commit_unchecked().unpack(|_| {});
        let _ = sb2.apply_without_execution(&cb2, Vec::new());
    }

    fn state_with_transaction_policy(
        chain_id: &ChainId,
        authority: &AccountId,
        require_height_ttl: bool,
        require_sequence: bool,
    ) -> State {
        let domain_id = DomainId::try_new("wonderland", "universal").expect("valid domain");
        let domain = Domain::new(domain_id).build(authority);
        let account = Account::new(authority.clone()).build(authority);
        let mut world = World::with([domain], [account], []);
        let mut params = iroha_data_model::parameter::system::Parameters::default();
        params.transaction = params
            .transaction
            .with_ingress_enforcement(require_height_ttl, require_sequence);
        world.parameters = mv::cell::Cell::new(params);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        State::new_with_chain(world, kura, query_handle, chain_id.clone())
    }

    fn add_pipeline_metadata_trigger(
        world: &mut World,
        authority: &AccountId,
        trigger_id: &str,
        key: Name,
        filter: PipelineEventFilterBox,
    ) {
        let action = crate::smartcontracts::triggers::specialized::SpecializedAction::new(
            vec![InstructionBox::from(SetKeyValue::account(
                authority.clone(),
                key,
                Json::new("ok"),
            ))],
            Repeats::Exactly(1),
            authority.clone(),
            filter,
        );
        let mut trigger_block = world.triggers.block();
        let mut trigger_transaction = trigger_block.transaction();
        trigger_transaction
            .add_pipeline_trigger(
                crate::smartcontracts::triggers::specialized::SpecializedTrigger::new(
                    trigger_id.parse().expect("trigger id"),
                    action,
                ),
            )
            .expect("add pipeline trigger");
        trigger_transaction.apply();
        trigger_block.commit();
    }

    fn previous_block_at_height(height: u64) -> SignedBlock {
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(NonZeroU64::new(height).expect("non-zero height"));
        });
        latest_valid.into()
    }

    fn validation_error_message(block: &SignedBlock) -> String {
        block
            .errors()
            .next()
            .map(|(_, err)| format!("{err:?}"))
            .expect("block must contain a transaction error")
    }

    fn sealed_set_key_entrypoints(
        chain_id: &ChainId,
        authority: &AccountId,
        keypair: &KeyPair,
        reveal_after_height: u64,
        reveal_deadline_height: u64,
        metadata_key: Name,
    ) -> (TransactionEntrypoint, TransactionEntrypoint) {
        let mut builder = TransactionBuilder::new(chain_id.clone(), authority.clone());
        builder.set_creation_time(Duration::ZERO);
        let signed = builder
            .with_instructions([SetKeyValue::account(
                authority.clone(),
                metadata_key,
                Json::new("revealed"),
            )])
            .sign(keypair.private_key());
        let salt = [0x5A; 32];
        let commitment =
            compute_sealed_transaction_commitment(chain_id, &signed, salt, reveal_deadline_height);
        let payload = SealedTransactionCommitmentPayload::new(
            chain_id.clone(),
            authority.clone(),
            commitment,
            reveal_after_height,
            reveal_deadline_height,
            None,
        );
        let signed_commitment =
            SignedSealedTransactionCommitment::sign(payload, keypair.private_key());
        let reveal = SealedTransactionReveal::new(commitment, signed, salt);

        (
            TransactionEntrypoint::SealedCommitment(signed_commitment),
            TransactionEntrypoint::SealedReveal(reveal),
        )
    }

    #[test]
    fn block_validation_external_only_records_entrypoint_hash_without_fallback() {
        let chain_id = ChainId::from("external-only-borrowed-validation");
        let (authority, keypair) = gen_account_in("wonderland");
        let state = state_with_transaction_policy(&chain_id, &authority, false, false);
        let signed = TransactionBuilder::new(chain_id.clone(), authority.clone())
            .with_instructions([Log::new(Level::INFO, "external-only".to_owned())])
            .sign(keypair.private_key());
        let entrypoint_hash = TransactionEntrypoint::External(signed.clone()).hash();
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(signed));
        let block = BlockBuilder::new(vec![accepted])
            .chain(0, state.view().latest_block().as_deref())
            .sign(keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(block.header());

        let valid_block = block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let results: Vec<_> = valid_block.as_ref().entrypoint_results().collect();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.hash(), entrypoint_hash);
        assert!(
            results[0].2.0.is_ok(),
            "external-only transaction must execute successfully: {:?}",
            results[0].2
        );
    }

    #[test]
    fn block_validation_non_external_entrypoint_uses_sequential_fallback() {
        let chain_id = ChainId::from("non-external-sequential-fallback");
        let (authority, keypair) = gen_account_in("wonderland");
        let state = state_with_transaction_policy(&chain_id, &authority, false, false);
        let metadata_key = Name::from_str("sequential_fallback_marker").expect("metadata key");
        let (commitment_entrypoint, _reveal_entrypoint) =
            sealed_set_key_entrypoints(&chain_id, &authority, &keypair, 2, 4, metadata_key);
        let commitment_entrypoint_hash = commitment_entrypoint.hash();
        let accepted =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(commitment_entrypoint));
        let block = BlockBuilder::new(vec![accepted])
            .chain(0, state.view().latest_block().as_deref())
            .sign(keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(block.header());

        let valid_block = block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let results: Vec<_> = valid_block.as_ref().entrypoint_results().collect();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.hash(), commitment_entrypoint_hash);
        assert!(
            results[0].2.0.is_ok(),
            "non-external entrypoint fallback must preserve execution: {:?}",
            results[0].2
        );
    }

    #[test]
    fn block_validation_sequential_entrypoints_execute_pipeline_triggers() {
        let chain_id = ChainId::from("sequential-pipeline-triggers");
        let (authority, keypair) = gen_account_in("wonderland");
        let domain_id = DomainId::try_new("wonderland", "universal").expect("valid domain");
        let domain = Domain::new(domain_id).build(&authority);
        let account = Account::new(authority.clone()).build(&authority);
        let mut world = World::with([domain], [account], []);
        let block_key = Name::from_str("sequential_block_pipeline_trigger").expect("metadata key");
        let tx_key = Name::from_str("sequential_tx_pipeline_trigger").expect("metadata key");
        let external_signed = TransactionBuilder::new(chain_id.clone(), authority.clone())
            .with_instructions([Log::new(Level::INFO, "external".to_owned())])
            .sign(keypair.private_key());
        let external_hash = external_signed.hash();
        add_pipeline_metadata_trigger(
            &mut world,
            &authority,
            "sequential_block_approved",
            block_key.clone(),
            PipelineEventFilterBox::from(BlockEventFilter::new().for_status(BlockStatus::Approved)),
        );
        add_pipeline_metadata_trigger(
            &mut world,
            &authority,
            "sequential_external_approved",
            tx_key.clone(),
            PipelineEventFilterBox::from(
                TransactionEventFilter::new()
                    .for_hash(external_hash)
                    .for_status(TransactionStatus::Approved),
            ),
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain_id.clone());
        let metadata_key = Name::from_str("sequential_commitment_marker").expect("metadata key");
        let (commitment_entrypoint, _reveal_entrypoint) =
            sealed_set_key_entrypoints(&chain_id, &authority, &keypair, 2, 4, metadata_key);

        let accepted_external = AcceptedTransaction::new_unchecked(Cow::Owned(external_signed));
        let accepted_commitment =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(commitment_entrypoint));
        let block = BlockBuilder::new(vec![accepted_external, accepted_commitment])
            .chain(0, state.view().latest_block().as_deref())
            .sign(keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(block.header());

        let valid_block = block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        assert!(
            valid_block
                .as_ref()
                .entrypoint_results()
                .all(|(_, _, result)| result.0.is_ok()),
            "mixed sequential block should validate successfully"
        );
        let (block_value, tx_value) = state_block
            .world
            .map_account(&authority, |account| {
                (
                    account.value().metadata().get(&block_key).cloned(),
                    account.value().metadata().get(&tx_key).cloned(),
                )
            })
            .expect("authority account exists");
        assert_eq!(block_value, Some(Json::new("ok")));
        assert_eq!(tx_value, Some(Json::new("ok")));
    }

    #[test]
    fn block_validation_sealed_only_entrypoint_executes_only_block_pipeline_trigger() {
        let chain_id = ChainId::from("sealed-only-pipeline-triggers");
        let (authority, keypair) = gen_account_in("wonderland");
        let domain_id = DomainId::try_new("wonderland", "universal").expect("valid domain");
        let domain = Domain::new(domain_id).build(&authority);
        let account = Account::new(authority.clone()).build(&authority);
        let mut world = World::with([domain], [account], []);
        let block_key = Name::from_str("sealed_only_block_pipeline_trigger").expect("metadata key");
        let tx_key = Name::from_str("sealed_only_tx_pipeline_trigger").expect("metadata key");
        let dummy_signed = TransactionBuilder::new(chain_id.clone(), authority.clone())
            .with_instructions([Log::new(Level::INFO, "dummy".to_owned())])
            .sign(keypair.private_key());
        add_pipeline_metadata_trigger(
            &mut world,
            &authority,
            "sealed_only_block_approved",
            block_key.clone(),
            PipelineEventFilterBox::from(BlockEventFilter::new().for_status(BlockStatus::Approved)),
        );
        add_pipeline_metadata_trigger(
            &mut world,
            &authority,
            "sealed_only_dummy_tx_approved",
            tx_key.clone(),
            PipelineEventFilterBox::from(
                TransactionEventFilter::new()
                    .for_hash(dummy_signed.hash())
                    .for_status(TransactionStatus::Approved),
            ),
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain_id.clone());
        let metadata_key = Name::from_str("sealed_only_commitment_marker").expect("metadata key");
        let (commitment_entrypoint, _reveal_entrypoint) =
            sealed_set_key_entrypoints(&chain_id, &authority, &keypair, 2, 4, metadata_key);
        let accepted_commitment =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(commitment_entrypoint));
        let block = BlockBuilder::new(vec![accepted_commitment])
            .chain(0, state.view().latest_block().as_deref())
            .sign(keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(block.header());

        let valid_block = block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        assert!(
            valid_block
                .as_ref()
                .entrypoint_results()
                .all(|(_, _, result)| result.0.is_ok()),
            "sealed-only block should validate successfully"
        );
        let (block_value, tx_value) = state_block
            .world
            .map_account(&authority, |account| {
                (
                    account.value().metadata().get(&block_key).cloned(),
                    account.value().metadata().get(&tx_key).cloned(),
                )
            })
            .expect("authority account exists");
        assert_eq!(block_value, Some(Json::new("ok")));
        assert_eq!(
            tx_value, None,
            "sealed-only entrypoints must not synthesize transaction pipeline events"
        );
    }

    #[test]
    fn block_validation_sequential_entrypoints_execute_rejected_transaction_pipeline_trigger() {
        let chain_id = ChainId::from("sequential-rejected-pipeline-trigger");
        let (authority, keypair) = gen_account_in("wonderland");
        let domain_id = DomainId::try_new("wonderland", "universal").expect("valid domain");
        let domain = Domain::new(domain_id).build(&authority);
        let account = Account::new(authority.clone()).build(&authority);
        let mut world = World::with([domain], [account], []);
        let block_key =
            Name::from_str("sequential_rejected_block_pipeline_trigger").expect("metadata key");
        let rejected_key =
            Name::from_str("sequential_rejected_tx_pipeline_trigger").expect("metadata key");
        let approved_key =
            Name::from_str("sequential_wrong_approved_tx_pipeline_trigger").expect("metadata key");
        let external_signed = TransactionBuilder::new(chain_id.clone(), authority.clone())
            .with_instructions([Unregister::domain(
                DomainId::try_new("missing-domain", "universal").expect("valid domain id"),
            )])
            .sign(keypair.private_key());
        let external_hash = external_signed.hash();
        add_pipeline_metadata_trigger(
            &mut world,
            &authority,
            "sequential_rejected_block_approved",
            block_key.clone(),
            PipelineEventFilterBox::from(BlockEventFilter::new().for_status(BlockStatus::Approved)),
        );
        add_pipeline_metadata_trigger(
            &mut world,
            &authority,
            "sequential_external_rejected",
            rejected_key.clone(),
            PipelineEventFilterBox::from(
                TransactionEventFilter::new()
                    .for_hash(external_hash)
                    .for_status(TransactionStatus::Rejected(Box::new(
                        TransactionRejectionReason::Validation(ValidationFail::TooComplex),
                    ))),
            ),
        );
        add_pipeline_metadata_trigger(
            &mut world,
            &authority,
            "sequential_external_wrong_approved",
            approved_key.clone(),
            PipelineEventFilterBox::from(
                TransactionEventFilter::new()
                    .for_hash(external_hash)
                    .for_status(TransactionStatus::Approved),
            ),
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain_id.clone());
        let metadata_key =
            Name::from_str("sequential_rejected_commitment_marker").expect("metadata key");
        let (commitment_entrypoint, _reveal_entrypoint) =
            sealed_set_key_entrypoints(&chain_id, &authority, &keypair, 2, 4, metadata_key);
        let accepted_external = AcceptedTransaction::new_unchecked(Cow::Owned(external_signed));
        let accepted_commitment =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(commitment_entrypoint));
        let block = BlockBuilder::new(vec![accepted_external, accepted_commitment])
            .chain(0, state.view().latest_block().as_deref())
            .sign(keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(block.header());

        let valid_block = block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let results: Vec<_> = valid_block.as_ref().entrypoint_results().collect();
        assert!(
            results.iter().any(|(_, _, result)| result.0.is_err()),
            "mixed sequential block should record the failing external transaction"
        );
        let (block_value, rejected_value, approved_value) = state_block
            .world
            .map_account(&authority, |account| {
                (
                    account.value().metadata().get(&block_key).cloned(),
                    account.value().metadata().get(&rejected_key).cloned(),
                    account.value().metadata().get(&approved_key).cloned(),
                )
            })
            .expect("authority account exists");
        assert_eq!(block_value, Some(Json::new("ok")));
        assert_eq!(rejected_value, Some(Json::new("ok")));
        assert_eq!(
            approved_value, None,
            "rejected transaction must not match approved transaction filters"
        );
    }

    #[test]
    fn block_pipeline_executes_sealed_reveal_and_records_entrypoint_hash() {
        let chain_id = ChainId::from("sealed-block-pipeline");
        let (authority, keypair) = gen_account_in("wonderland");
        let state = state_with_transaction_policy(&chain_id, &authority, false, false);
        let metadata_key = Name::from_str("sealed_reveal_executed").expect("metadata key");
        let (commitment_entrypoint, reveal_entrypoint) =
            sealed_set_key_entrypoints(&chain_id, &authority, &keypair, 2, 4, metadata_key.clone());
        let commitment_entrypoint_hash = commitment_entrypoint.hash();
        let reveal_entrypoint_hash = reveal_entrypoint.hash();

        let accepted_commitment =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(commitment_entrypoint));
        let (_commit_clock, commit_time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let commitment_block =
            BlockBuilder::new_with_time_source(vec![accepted_commitment], commit_time_source)
                .chain(0, state.view().latest_block().as_deref())
                .sign(keypair.private_key())
                .unpack(|_| {});
        let mut commitment_state_block = state.block(commitment_block.header);
        let initial_smart_contract_state_len =
            commitment_state_block.world.smart_contract_state.len();
        let valid_commitment_block = commitment_block
            .validate_and_record_transactions(&mut commitment_state_block)
            .unpack(|_| {});

        let commitment_result = valid_commitment_block
            .as_ref()
            .entrypoint_results()
            .next()
            .expect("commitment result");
        assert_eq!(commitment_result.1.hash(), commitment_entrypoint_hash);
        assert!(
            commitment_result.2.0.is_ok(),
            "sealed commitment must execute successfully: {:?}",
            commitment_result.2
        );
        assert_eq!(
            commitment_state_block.world.smart_contract_state.len(),
            initial_smart_contract_state_len + 1,
            "commitment block should leave one pending sealed commitment"
        );
        commitment_state_block.commit().expect("commitment commit");
        let commitment_signed_block: SignedBlock = valid_commitment_block.into();

        let accepted_reveal =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(reveal_entrypoint));
        let (_reveal_clock, reveal_time_source) = TimeSource::new_mock(Duration::from_millis(2));
        let reveal_block =
            BlockBuilder::new_with_time_source(vec![accepted_reveal], reveal_time_source)
                .chain(0, Some(&commitment_signed_block))
                .sign(keypair.private_key())
                .unpack(|_| {});
        let mut reveal_state_block = state.block(reveal_block.header);
        let valid_reveal_block = reveal_block
            .validate_and_record_transactions(&mut reveal_state_block)
            .unpack(|_| {});

        let reveal_result = valid_reveal_block
            .as_ref()
            .entrypoint_results()
            .next()
            .expect("reveal result");
        assert_eq!(reveal_result.1.hash(), reveal_entrypoint_hash);
        assert!(
            reveal_result.2.0.is_ok(),
            "sealed reveal must execute the inner transaction: {:?}",
            reveal_result.2
        );
        assert_eq!(
            reveal_state_block.world.smart_contract_state.len(),
            initial_smart_contract_state_len,
            "successful reveal should consume the pending commitment"
        );
        let metadata_value = reveal_state_block
            .world
            .map_account(&authority, |account| {
                account.value().metadata().get(&metadata_key).cloned()
            })
            .expect("authority account exists");
        assert_eq!(metadata_value, Some(Json::new("revealed")));
    }

    #[test]
    fn prune_expired_sealed_commitments_removes_pending_state_after_deadline() {
        let chain_id = ChainId::from("sealed-prune-pipeline");
        let (authority, keypair) = gen_account_in("wonderland");
        let state = state_with_transaction_policy(&chain_id, &authority, false, false);
        let metadata_key = Name::from_str("sealed_prune_marker").expect("metadata key");
        let (commitment_entrypoint, _reveal_entrypoint) =
            sealed_set_key_entrypoints(&chain_id, &authority, &keypair, 2, 2, metadata_key);

        let accepted_commitment =
            AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(commitment_entrypoint));
        let (_commit_clock, commit_time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let commitment_block =
            BlockBuilder::new_with_time_source(vec![accepted_commitment], commit_time_source)
                .chain(0, state.view().latest_block().as_deref())
                .sign(keypair.private_key())
                .unpack(|_| {});
        let mut commitment_state_block = state.block(commitment_block.header);
        let initial_smart_contract_state_len =
            commitment_state_block.world.smart_contract_state.len();
        let valid_commitment_block = commitment_block
            .validate_and_record_transactions(&mut commitment_state_block)
            .unpack(|_| {});
        assert!(
            valid_commitment_block
                .as_ref()
                .entrypoint_results()
                .all(|(_, _, result)| result.0.is_ok()),
            "commitment block should not reject the sealed commitment"
        );
        assert_eq!(
            commitment_state_block.world.smart_contract_state.len(),
            initial_smart_contract_state_len + 1
        );
        commitment_state_block.commit().expect("commitment commit");

        let prune_header = BlockHeader::new(nonzero!(3_u64), None, None, None, 3, 0);
        let mut prune_state_block = state.block(prune_header);
        assert_eq!(
            prune_state_block.world.smart_contract_state.len(),
            initial_smart_contract_state_len + 1,
            "pending commitment should still be visible before pruning"
        );

        let pruned = crate::tx::prune_expired_sealed_commitments(&mut prune_state_block);

        assert_eq!(pruned, 1);
        assert_eq!(
            prune_state_block.world.smart_contract_state.len(),
            initial_smart_contract_state_len,
            "expired sealed commitment should be removed after its deadline"
        );
    }

    #[test]
    fn block_pipeline_rejects_expired_height_ttl() {
        let chain_id = ChainId::from("block-height-ttl-check");
        let (authority, keypair) = gen_account_in("wonderland");
        let state = state_with_transaction_policy(&chain_id, &authority, true, false);
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("expires_at_height").expect("metadata key"),
            Json::from(2_u64),
        );

        let mut builder = TransactionBuilder::new(chain_id.clone(), authority.clone());
        builder.set_creation_time(Duration::ZERO);
        let tx = builder
            .with_instructions([Log::new(Level::INFO, "expired".to_owned())])
            .with_metadata(metadata)
            .sign(keypair.private_key());
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let previous = previous_block_at_height(1);
        let unverified_block = BlockBuilder::new_with_time_source(vec![accepted], time_source)
            .chain(0, Some(&previous))
            .sign(keypair.private_key())
            .unpack(|_| {});

        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        let error = validation_error_message(valid_block.as_ref());
        assert!(error.contains("expired"), "unexpected rejection: {error}");
    }

    #[test]
    fn block_pipeline_rejects_non_increasing_tx_sequence() {
        let chain_id = ChainId::from("block-sequence-check");
        let (authority, keypair) = gen_account_in("wonderland");
        let state = state_with_transaction_policy(&chain_id, &authority, false, true);
        {
            let mut world = state.world.block();
            world.tx_sequences.insert(authority.clone(), 5);
            world.commit();
        }
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("tx_sequence").expect("metadata key"),
            Json::from(5_u64),
        );

        let mut builder = TransactionBuilder::new(chain_id.clone(), authority.clone());
        builder.set_creation_time(Duration::ZERO);
        let tx = builder
            .with_instructions([Log::new(Level::INFO, "sequence".to_owned())])
            .with_metadata(metadata)
            .sign(keypair.private_key());
        let accepted = AcceptedTransaction::new_unchecked(Cow::Owned(tx));
        let (_handle, time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let previous = previous_block_at_height(1);
        let unverified_block = BlockBuilder::new_with_time_source(vec![accepted], time_source)
            .chain(0, Some(&previous))
            .sign(keypair.private_key())
            .unpack(|_| {});

        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        let error = validation_error_message(valid_block.as_ref());
        assert!(error.contains("sequence"), "unexpected rejection: {error}");
    }

    #[tokio::test]
    async fn should_reject_due_to_repetition() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        // Predefined world state
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("Valid");
        let account = Account::new(alice_id.clone()).build(&alice_id);
        let domain = Domain::new(domain_id).build(&alice_id);
        let world = World::with([domain], [account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);
        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        // Creating an instruction
        let asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
            "xor".parse().expect("asset name"),
        );
        let create_asset_definition = Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id).with_name("xor".to_owned()),
        );

        // Making two transactions that have the same instruction
        let tx = TransactionBuilder::new(chain_id.clone(), alice_id)
            .with_instructions([create_asset_definition])
            .sign(alice_keypair.private_key());
        let crypto_cfg = state.crypto();
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
        )
        .expect("Valid");

        // Creating a block of two identical transactions and validating it
        let transactions = vec![tx.clone(), tx];
        let unverified_block = BlockBuilder::new(transactions)
            .chain(0, state.view().latest_block().as_deref())
            .sign(alice_keypair.private_key())
            .unpack(|_| {});

        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        state_block.commit().unwrap();

        // The 1st transaction should be confirmed and the 2nd rejected
        assert_eq!(valid_block.as_ref().errors().next().unwrap().0, 1);
    }

    #[tokio::test]
    async fn tx_order_same_in_validation_and_revalidation() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        // Predefined world state
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("Valid");
        let account = Account::new(alice_id.clone()).build(&alice_id);
        let domain = Domain::new(domain_id).build(&alice_id);
        let domain_a_id = DomainId::try_new("domain-a", "universal").unwrap();
        let domain_b_id = DomainId::try_new("domain-b", "universal").unwrap();
        let mut world = World::with([domain], [account], []);
        seed_domain_name_lease(&mut world, &alice_id, &domain_a_id);
        seed_domain_name_lease(&mut world, &alice_id, &domain_b_id);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);
        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        // Two independent register instructions (no ordering dependencies)
        let domain_a = Register::domain(Domain::new(domain_a_id));
        let domain_b = Register::domain(Domain::new(domain_b_id));

        let tx = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions::<InstructionBox>([domain_a.into()])
            .sign(alice_keypair.private_key());
        let crypto_cfg = state.crypto();
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
        )
        .expect("Valid");

        let fail_domain_id = DomainId::try_new("missing-domain", "universal").expect("valid id");
        let fail_instruction = Unregister::domain(fail_domain_id);
        let succeed_instruction = domain_b;

        let tx0 = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions::<InstructionBox>([fail_instruction.into()])
            .sign(alice_keypair.private_key());
        let tx0 = AcceptedTransaction::accept(
            tx0,
            &chain_id,
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
        )
        .expect("Valid");

        let tx2 = TransactionBuilder::new(chain_id.clone(), alice_id)
            .with_instructions::<InstructionBox>([succeed_instruction.into()])
            .sign(alice_keypair.private_key());
        let tx2 = AcceptedTransaction::accept(
            tx2,
            &chain_id,
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
        )
        .expect("Valid");

        let fail_hash = tx0.as_ref().hash_as_entrypoint();
        let register_hash = tx.as_ref().hash_as_entrypoint();
        let succeed_hash = tx2.as_ref().hash_as_entrypoint();

        // Creating a block of two identical transactions and validating it
        let transactions = vec![tx0, tx, tx2];
        let unverified_block = BlockBuilder::new(transactions)
            .chain(0, state.view().latest_block().as_deref())
            .sign(alice_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        state_block.commit().unwrap();

        // The 1st transaction should fail and 2nd succeed
        let block_ref = valid_block.as_ref();
        let outcomes: Vec<_> = block_ref
            .entrypoint_hashes()
            .zip(block_ref.results())
            .collect();

        let lookup = |hash: &_, label: &str| {
            outcomes
                .iter()
                .find(|(entry_hash, _)| entry_hash == hash)
                .unwrap_or_else(|| panic!("missing result for {label}"))
                .1
                .as_ref()
        };

        let fail_result = lookup(&fail_hash, "fail tx");
        assert!(fail_result.is_err(), "fail tx must be rejected");
        let register_result = lookup(&register_hash, "register tx");
        assert!(
            register_result.is_ok(),
            "register tx must succeed, got {register_result:?}"
        );
        let succeed_result = lookup(&succeed_hash, "succeed tx");
        assert!(
            succeed_result.is_ok(),
            "succeed tx must succeed, got {succeed_result:?}"
        );
    }

    #[tokio::test]
    async fn failed_transactions_revert() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        // Predefined world state
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("Valid");
        let account = Account::new(alice_id.clone()).build(&alice_id);
        let domain = Domain::new(domain_id).build(&alice_id);
        let created_domain_id = DomainId::try_new("domain", "universal").expect("Valid");
        let mut world = World::with([domain], [account], []);
        seed_domain_name_lease(&mut world, &alice_id, &created_domain_id);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);
        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let create_domain = Register::domain(Domain::new(created_domain_id));
        let asset_definition_id = iroha_data_model::asset::AssetDefinitionId::new(
            DomainId::try_new("domain", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
        let create_asset = Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id).with_name("coin".to_owned()),
        );
        let fail_isi = Unregister::domain(DomainId::try_new("dummy", "universal").unwrap());
        let tx_fail = TransactionBuilder::new(chain_id.clone(), alice_id.clone())
            .with_instructions::<InstructionBox>([create_domain.clone().into(), fail_isi.into()])
            .sign(alice_keypair.private_key());
        let crypto_cfg = state.crypto();
        let tx_fail = AcceptedTransaction::accept(
            tx_fail,
            &chain_id,
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
        )
        .expect("Valid");
        let tx_accept = TransactionBuilder::new(chain_id.clone(), alice_id)
            .with_instructions::<InstructionBox>([create_domain.into(), create_asset.into()])
            .sign(alice_keypair.private_key());
        let tx_accept = AcceptedTransaction::accept(
            tx_accept,
            &chain_id,
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
        )
        .expect("Valid");

        let fail_hash = tx_fail.as_ref().hash_as_entrypoint();
        let accept_hash = tx_accept.as_ref().hash_as_entrypoint();

        // Creating a block of where first transaction must fail and second one fully executed
        let transactions = vec![tx_fail, tx_accept];
        let unverified_block = BlockBuilder::new(transactions)
            .chain(0, state.view().latest_block().as_deref())
            .sign(alice_keypair.private_key())
            .unpack(|_| {});

        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        state_block.commit().unwrap();

        let block_ref = valid_block.as_ref();
        let outcomes: Vec<_> = block_ref
            .entrypoint_hashes()
            .zip(block_ref.results())
            .collect();

        let lookup = |target: &_, msg: &str| {
            outcomes
                .iter()
                .find(|(hash, _)| hash == target)
                .unwrap_or_else(|| panic!("missing result for {msg}"))
                .1
                .as_ref()
        };

        let fail_result = lookup(&fail_hash, "fail tx");
        assert!(fail_result.is_err(), "Failing tx must be rejected");
        let accept_result = lookup(&accept_hash, "accept tx");
        assert!(
            accept_result.is_ok(),
            "Second tx must succeed, got {accept_result:?}"
        );
    }

    #[test]
    fn rejected_business_execution_still_charges_nexus_fee() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let chain_id = ChainId::from("rejected-business-fee-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (sink_id, _sink_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&payer_id);
        let payer_asset = Asset::new(
            AssetId::of(asset_definition_id.clone(), payer_id.clone()),
            Numeric::from(10_u32),
        );
        let sink_asset = Asset::new(
            AssetId::of(asset_definition_id.clone(), sink_id.clone()),
            Numeric::zero(),
        );
        let world = World::with_assets(
            [domain],
            [payer, sink],
            [asset_definition],
            [payer_asset, sink_asset],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.fee_asset_id = asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }
        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let created_domain_id = DomainId::try_new("fee-created", "universal").unwrap();
        let create_domain = Register::domain(Domain::new(created_domain_id.clone()));
        let fail_instruction =
            Unregister::domain(DomainId::try_new("missing-domain", "universal").unwrap());
        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_instructions::<InstructionBox>([create_domain.into(), fail_instruction.into()])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert_eq!(
            valid_block.as_ref().errors().next().map(|(idx, _)| idx),
            Some(0)
        );
        let first_error = valid_block
            .as_ref()
            .errors()
            .next()
            .map(|(_, err)| format!("{err:?}"));
        let assets = state_block.world.assets();
        let payer_balance = assets
            .get(&AssetId::of(asset_definition_id.clone(), payer_id))
            .expect("payer balance exists")
            .0
            .to_string();
        let sink_balance = assets
            .get(&AssetId::of(asset_definition_id, sink_id))
            .expect("sink balance exists")
            .0
            .to_string();

        assert_eq!(payer_balance, "9", "tx error: {first_error:?}");
        assert_eq!(sink_balance, "0");
        assert!(
            state_block.world.domain(&created_domain_id).is_err(),
            "failed transaction state changes must still be rolled back"
        );
    }

    #[test]
    fn fee_enabled_single_transfer_uses_detached_merge_without_fee_fallback() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();

        let chain_id = ChainId::from("fee-detached-single-transfer-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
        let (sink_id, _sink_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let transfer_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("asset name"));
        let fee_asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let transfer_asset_definition =
            AssetDefinition::numeric(transfer_asset_definition_id.clone())
                .with_name("rose".to_owned())
                .build(&payer_id);
        let fee_asset_definition = AssetDefinition::numeric(fee_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&payer_id);
        let payer_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
        let recipient_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
        let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
        let world = World::with_assets(
            [domain],
            [payer, recipient, sink],
            [transfer_asset_definition, fee_asset_definition],
            [
                Asset::new(payer_transfer_asset.clone(), Numeric::from(5_u32)),
                Asset::new(recipient_transfer_asset.clone(), Numeric::zero()),
                Asset::new(payer_fee_asset.clone(), Numeric::from(10_u32)),
            ],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_instructions([Transfer::asset_numeric(
                payer_transfer_asset.clone(),
                1_u32,
                recipient_id.clone(),
            )])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert!(
            valid_block.as_ref().errors().next().is_none(),
            "fee-enabled transfer should be accepted"
        );
        let snapshot = crate::sumeragi::status::snapshot();
        assert_eq!(snapshot.pipeline_execution.detached_merged_total, 1);
        assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 0);
        assert_eq!(
            snapshot
                .pipeline_execution
                .detached_fallback_fee_postprocessing_total,
            0
        );

        let assets = state_block.world.assets();
        assert_eq!(
            assets.get(&payer_transfer_asset).expect("payer rose").0,
            Numeric::from(4_u32)
        );
        assert_eq!(
            assets
                .get(&recipient_transfer_asset)
                .expect("recipient rose")
                .0,
            Numeric::from(1_u32)
        );
        assert_eq!(
            assets.get(&payer_fee_asset).expect("payer xor").0,
            Numeric::from(9_u32)
        );
    }

    #[test]
    fn fee_enabled_supported_non_transfer_uses_fee_postprocessing_fallback() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();

        let chain_id = ChainId::from("fee-detached-non-transfer-fallback-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (sink_id, _sink_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let fee_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "xor".parse().expect("asset name"));
        let fee_asset_definition = AssetDefinition::numeric(fee_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&payer_id);
        let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
        let world = World::with_assets(
            [domain],
            [payer, sink],
            [fee_asset_definition],
            [Asset::new(payer_fee_asset.clone(), Numeric::from(10_u32))],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let marker_key: Name = "fee_fallback_marker".parse().expect("metadata key");
        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_instructions([SetKeyValue::account(
                payer_id.clone(),
                marker_key.clone(),
                Json::from(true),
            )])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert!(
            valid_block.as_ref().errors().next().is_none(),
            "supported non-transfer fee transaction should be accepted through sequential fallback"
        );
        let snapshot = crate::sumeragi::status::snapshot();
        assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
        assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 1);
        assert_eq!(
            snapshot
                .pipeline_execution
                .detached_fallback_fee_postprocessing_total,
            1
        );

        let assets = state_block.world.assets();
        assert_eq!(
            assets.get(&payer_fee_asset).expect("payer xor").0,
            Numeric::from(9_u32)
        );
        let marker_value = state_block
            .world
            .map_account(&payer_id, |account| {
                account.value().metadata().get(&marker_key).cloned()
            })
            .expect("payer account exists");
        assert_eq!(marker_value, Some(Json::from(true)));
    }

    #[test]
    fn fee_enabled_single_transfer_rejects_without_partial_state_when_fee_missing() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();

        let chain_id = ChainId::from("fee-detached-insufficient-fee-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
        let (sink_id, _sink_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let transfer_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("asset name"));
        let fee_asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let transfer_asset_definition =
            AssetDefinition::numeric(transfer_asset_definition_id.clone())
                .with_name("rose".to_owned())
                .build(&payer_id);
        let fee_asset_definition = AssetDefinition::numeric(fee_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&payer_id);
        let payer_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
        let recipient_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
        let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
        let world = World::with_assets(
            [domain],
            [payer, recipient, sink],
            [transfer_asset_definition, fee_asset_definition],
            [
                Asset::new(payer_transfer_asset.clone(), Numeric::from(5_u32)),
                Asset::new(recipient_transfer_asset.clone(), Numeric::zero()),
                Asset::new(payer_fee_asset.clone(), Numeric::zero()),
            ],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_instructions([Transfer::asset_numeric(
                payer_transfer_asset.clone(),
                1_u32,
                recipient_id,
            )])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert_eq!(
            valid_block.as_ref().errors().next().map(|(idx, _)| idx),
            Some(0),
            "insufficient fee must reject the transaction"
        );

        let assets = state_block.world.assets();
        assert_eq!(
            assets.get(&payer_transfer_asset).expect("payer rose").0,
            Numeric::from(5_u32),
            "business transfer must not leak when fee charging fails"
        );
        assert_eq!(
            assets
                .get(&recipient_transfer_asset)
                .expect("recipient rose")
                .0,
            Numeric::zero(),
            "recipient balance must remain unchanged when fee charging fails"
        );
        assert_eq!(
            assets.get(&payer_fee_asset).expect("payer xor").0,
            Numeric::zero(),
            "failed fee debit must not create a negative or partial fee state"
        );
    }

    #[test]
    fn fee_enabled_single_transfer_with_active_data_trigger_uses_fee_fallback() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();

        let chain_id = ChainId::from("fee-detached-data-trigger-fallback-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
        let (sink_id, _sink_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let transfer_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("asset name"));
        let fee_asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let transfer_asset_definition =
            AssetDefinition::numeric(transfer_asset_definition_id.clone())
                .with_name("rose".to_owned())
                .build(&payer_id);
        let fee_asset_definition = AssetDefinition::numeric(fee_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&payer_id);
        let payer_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
        let recipient_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
        let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
        let world = World::with_assets(
            [domain],
            [payer, recipient, sink],
            [transfer_asset_definition, fee_asset_definition],
            [
                Asset::new(payer_transfer_asset.clone(), Numeric::from(5_u32)),
                Asset::new(recipient_transfer_asset.clone(), Numeric::zero()),
                Asset::new(payer_fee_asset.clone(), Numeric::from(10_u32)),
            ],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let trigger_marker_key: Name = "fee_trigger_marker".parse().expect("metadata key");
        let trigger_id: TriggerId = "fee_transfer_trigger_guard".parse().unwrap();
        let trigger = Trigger::new(
            trigger_id,
            Action::new(
                vec![InstructionBox::from(SetKeyValue::account(
                    payer_id.clone(),
                    trigger_marker_key.clone(),
                    Json::from("triggered"),
                ))],
                Repeats::Exactly(1),
                payer_id.clone(),
                DataEventFilter::Asset(
                    AssetEventFilter::new().for_asset(payer_transfer_asset.clone()),
                ),
            ),
        );
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let setup_block = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let setup_signed: SignedBlock = setup_block.clone().into();
        {
            let mut setup_state_block = state.block(setup_block.as_ref().header());
            let mut setup_tx = setup_state_block.transaction();
            Register::trigger(trigger)
                .execute(&payer_id, &mut setup_tx)
                .expect("register data trigger");
            setup_tx.apply();
            setup_state_block.commit().expect("commit trigger setup");
        }

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_instructions([Transfer::asset_numeric(
                payer_transfer_asset.clone(),
                1_u32,
                recipient_id.clone(),
            )])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&setup_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert!(
            valid_block.as_ref().errors().next().is_none(),
            "fee-enabled transfer with an active data trigger should be accepted through fallback"
        );
        let snapshot = crate::sumeragi::status::snapshot();
        assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
        assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 1);
        assert_eq!(
            snapshot
                .pipeline_execution
                .detached_fallback_fee_postprocessing_total,
            1
        );

        let assets = state_block.world.assets();
        assert_eq!(
            assets.get(&payer_transfer_asset).expect("payer rose").0,
            Numeric::from(4_u32)
        );
        assert_eq!(
            assets
                .get(&recipient_transfer_asset)
                .expect("recipient rose")
                .0,
            Numeric::from(1_u32)
        );
        assert_eq!(
            assets.get(&payer_fee_asset).expect("payer xor").0,
            Numeric::from(9_u32)
        );
        let marker_value = state_block
            .world
            .map_account(&payer_id, |account| {
                account.value().metadata().get(&trigger_marker_key).cloned()
            })
            .expect("payer account exists");
        assert_eq!(marker_value, Some(Json::from("triggered")));
    }

    #[test]
    fn fee_enabled_single_transfer_rejects_without_partial_state_when_fee_asset_missing() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();

        let chain_id = ChainId::from("fee-detached-missing-fee-asset-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
        let (sink_id, _sink_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let transfer_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("asset name"));
        let fee_asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let transfer_asset_definition =
            AssetDefinition::numeric(transfer_asset_definition_id.clone())
                .with_name("rose".to_owned())
                .build(&payer_id);
        let fee_asset_definition = AssetDefinition::numeric(fee_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&payer_id);
        let payer_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
        let recipient_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
        let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
        let world = World::with_assets(
            [domain],
            [payer, recipient, sink],
            [transfer_asset_definition, fee_asset_definition],
            [
                Asset::new(payer_transfer_asset.clone(), Numeric::from(5_u32)),
                Asset::new(recipient_transfer_asset.clone(), Numeric::zero()),
            ],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_instructions([Transfer::asset_numeric(
                payer_transfer_asset.clone(),
                1_u32,
                recipient_id,
            )])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert_eq!(
            valid_block.as_ref().errors().next().map(|(idx, _)| idx),
            Some(0),
            "missing payer fee asset must reject the transaction"
        );

        let assets = state_block.world.assets();
        assert_eq!(
            assets.get(&payer_transfer_asset).expect("payer rose").0,
            Numeric::from(5_u32),
            "business transfer must not leak when fee asset lookup fails"
        );
        assert_eq!(
            assets
                .get(&recipient_transfer_asset)
                .expect("recipient rose")
                .0,
            Numeric::zero(),
            "recipient balance must remain unchanged when fee asset lookup fails"
        );
        assert!(
            assets.get(&payer_fee_asset).is_none(),
            "fee charging must not create the missing payer fee asset"
        );
    }

    #[test]
    fn fee_enabled_transfer_fee_same_asset_rejects_without_partial_state() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();

        let chain_id = ChainId::from("fee-detached-same-asset-fee-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
        let (sink_id, _sink_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let asset_definition_id =
            AssetDefinitionId::new(domain_id, "rose".parse().expect("asset name"));
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone())
            .with_name("rose".to_owned())
            .build(&payer_id);
        let payer_asset = AssetId::of(asset_definition_id.clone(), payer_id.clone());
        let recipient_asset = AssetId::of(asset_definition_id.clone(), recipient_id.clone());
        let world = World::with_assets(
            [domain],
            [payer, recipient, sink],
            [asset_definition],
            [
                Asset::new(payer_asset.clone(), Numeric::from(1_u32)),
                Asset::new(recipient_asset.clone(), Numeric::zero()),
            ],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.fee_asset_id = asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_instructions([Transfer::asset_numeric(
                payer_asset.clone(),
                1_u32,
                recipient_id,
            )])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert_eq!(
            valid_block.as_ref().errors().next().map(|(idx, _)| idx),
            Some(0),
            "fee debit must reject when the payer only has enough balance for the transfer itself"
        );
        let snapshot = crate::sumeragi::status::snapshot();
        assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
        assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 1);

        let assets = state_block.world.assets();
        assert_eq!(
            assets.get(&payer_asset).expect("payer rose").0,
            Numeric::from(1_u32),
            "transfer must not leak when post-transfer fee debit fails"
        );
        assert_eq!(
            assets.get(&recipient_asset).expect("recipient rose").0,
            Numeric::zero(),
            "recipient must not receive funds from a transaction rejected during fee charging"
        );
    }

    #[test]
    fn fee_enabled_shared_fee_balance_rejects_later_transfer_without_rolling_back_prior_success() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();

        let chain_id = ChainId::from("fee-detached-shared-fee-balance-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
        let (sink_id, _sink_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let transfer_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("asset name"));
        let fee_asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let transfer_asset_definition =
            AssetDefinition::numeric(transfer_asset_definition_id.clone())
                .with_name("rose".to_owned())
                .build(&payer_id);
        let fee_asset_definition = AssetDefinition::numeric(fee_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&payer_id);
        let payer_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
        let recipient_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
        let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
        let world = World::with_assets(
            [domain],
            [payer, recipient, sink],
            [transfer_asset_definition, fee_asset_definition],
            [
                Asset::new(payer_transfer_asset.clone(), Numeric::from(5_u32)),
                Asset::new(recipient_transfer_asset.clone(), Numeric::zero()),
                Asset::new(payer_fee_asset.clone(), Numeric::from(1_u32)),
            ],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let mut first_builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        first_builder.set_creation_time(Duration::from_millis(0));
        let first_tx = first_builder
            .with_instructions([Transfer::asset_numeric(
                payer_transfer_asset.clone(),
                1_u32,
                recipient_id.clone(),
            )])
            .sign(payer_keypair.private_key());
        let first_tx = AcceptedTransaction::accept(
            first_tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("first transaction should pass stateless admission");

        let mut second_builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        second_builder.set_creation_time(Duration::from_millis(1));
        let second_tx = second_builder
            .with_instructions([Transfer::asset_numeric(
                payer_transfer_asset.clone(),
                1_u32,
                recipient_id,
            )])
            .sign(payer_keypair.private_key());
        let second_tx = AcceptedTransaction::accept(
            second_tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("second transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block =
            BlockBuilder::new_with_time_source(vec![first_tx, second_tx], block_time_source)
                .chain(1, Some(&latest_signed))
                .sign(payer_keypair.private_key())
                .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert_eq!(
            valid_block.as_ref().errors().count(),
            1,
            "only one of the two transfers can pay the configured base fee"
        );
        let snapshot = crate::sumeragi::status::snapshot();
        assert_eq!(
            snapshot.pipeline_execution.detached_merged_total, 1,
            "one transfer should stay on the detached merge path"
        );
        assert_eq!(
            snapshot.pipeline_execution.detached_fallback_total, 1,
            "the fee-exhausted transfer should retry through sequential fallback before rejection"
        );

        let assets = state_block.world.assets();
        assert_eq!(
            assets
                .get(&payer_transfer_asset)
                .expect("payer rose after block")
                .0,
            Numeric::from(4_u32),
            "the accepted transfer must remain committed"
        );
        assert_eq!(
            assets
                .get(&recipient_transfer_asset)
                .expect("recipient rose after block")
                .0,
            Numeric::from(1_u32),
            "the rejected transfer must not leak after the first fee drains the payer"
        );
        assert_eq!(
            assets
                .get(&payer_fee_asset)
                .map(|asset| asset.0.clone())
                .unwrap_or_else(Numeric::zero),
            Numeric::zero(),
            "only the accepted transaction may consume the available fee balance"
        );
    }

    #[test]
    fn fee_enabled_transfer_then_failing_instruction_falls_back_without_leaking_transfer() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();

        let chain_id = ChainId::from("fee-detached-transfer-then-fail-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
        let (sink_id, _sink_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let transfer_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("asset name"));
        let fee_asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let transfer_asset_definition =
            AssetDefinition::numeric(transfer_asset_definition_id.clone())
                .with_name("rose".to_owned())
                .build(&payer_id);
        let fee_asset_definition = AssetDefinition::numeric(fee_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&payer_id);
        let payer_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
        let recipient_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
        let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
        let world = World::with_assets(
            [domain],
            [payer, recipient, sink],
            [transfer_asset_definition, fee_asset_definition],
            [
                Asset::new(payer_transfer_asset.clone(), Numeric::from(5_u32)),
                Asset::new(recipient_transfer_asset.clone(), Numeric::zero()),
                Asset::new(payer_fee_asset.clone(), Numeric::from(10_u32)),
            ],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let missing_domain_id = DomainId::try_new("missing-domain", "universal").unwrap();
        let tx = builder
            .with_instructions::<InstructionBox>([
                Transfer::asset_numeric(payer_transfer_asset.clone(), 1_u32, recipient_id).into(),
                Unregister::domain(missing_domain_id).into(),
            ])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert_eq!(
            valid_block.as_ref().errors().next().map(|(idx, _)| idx),
            Some(0),
            "the failing instruction after the transfer must reject the whole transaction"
        );
        let snapshot = crate::sumeragi::status::snapshot();
        assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
        assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 1);
        assert_eq!(
            snapshot
                .pipeline_execution
                .detached_fallback_unsupported_instruction_total,
            1,
            "multi-instruction transfer transactions must not use detached transfer merge"
        );

        let assets = state_block.world.assets();
        assert_eq!(
            assets
                .get(&payer_transfer_asset)
                .expect("payer rose after rejected transfer")
                .0,
            Numeric::from(5_u32),
            "payer balance must remain unchanged after rejected transfer"
        );
        assert_eq!(
            assets
                .get(&recipient_transfer_asset)
                .expect("recipient rose after rejected transfer")
                .0,
            Numeric::zero(),
            "recipient must not receive assets from a transaction rejected after the transfer"
        );
        assert_eq!(
            assets
                .get(&payer_fee_asset)
                .expect("payer xor after rejected transfer")
                .0,
            Numeric::from(9_u32),
            "rejected business execution must still charge the configured Nexus fee"
        );
    }

    #[test]
    fn fee_enabled_non_increasing_sequence_rejects_before_transfer_or_fee() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();

        let chain_id = ChainId::from("fee-detached-sequence-admission-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
        let (sink_id, _sink_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let transfer_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("asset name"));
        let fee_asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let transfer_asset_definition =
            AssetDefinition::numeric(transfer_asset_definition_id.clone())
                .with_name("rose".to_owned())
                .build(&payer_id);
        let fee_asset_definition = AssetDefinition::numeric(fee_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&payer_id);
        let payer_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
        let recipient_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
        let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
        let mut world = World::with_assets(
            [domain],
            [payer, recipient, sink],
            [transfer_asset_definition, fee_asset_definition],
            [
                Asset::new(payer_transfer_asset.clone(), Numeric::from(5_u32)),
                Asset::new(recipient_transfer_asset.clone(), Numeric::zero()),
                Asset::new(payer_fee_asset.clone(), Numeric::from(10_u32)),
            ],
            [],
        );
        let mut params = iroha_data_model::parameter::system::Parameters::default();
        params.transaction = params.transaction.with_ingress_enforcement(false, true);
        world.parameters = mv::cell::Cell::new(params);
        world.tx_sequences.insert(payer_id.clone(), 5);

        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("tx_sequence").expect("metadata key"),
            Json::from(5_u64),
        );
        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_metadata(metadata)
            .with_instructions([Transfer::asset_numeric(
                payer_transfer_asset.clone(),
                1_u32,
                recipient_id,
            )])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert_eq!(
            valid_block.as_ref().errors().next().map(|(idx, _)| idx),
            Some(0),
            "non-increasing tx_sequence must reject before transfer or fee application"
        );
        let snapshot = crate::sumeragi::status::snapshot();
        assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
        assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 0);

        let assets = state_block.world.assets();
        assert_eq!(
            assets
                .get(&payer_transfer_asset)
                .expect("payer rose after sequence rejection")
                .0,
            Numeric::from(5_u32)
        );
        assert_eq!(
            assets
                .get(&recipient_transfer_asset)
                .expect("recipient rose after sequence rejection")
                .0,
            Numeric::zero()
        );
        assert_eq!(
            assets
                .get(&payer_fee_asset)
                .expect("payer xor after sequence rejection")
                .0,
            Numeric::from(10_u32),
            "stateful admission failures must not charge Nexus fees"
        );
        assert_eq!(
            state_block.world.tx_sequences.get(&payer_id),
            Some(&5),
            "rejected sequence must not advance stored per-authority state"
        );
    }

    #[test]
    fn fee_enabled_invalid_sink_before_burn_rejects_without_partial_transfer_or_fee() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();

        let chain_id = ChainId::from("fee-detached-invalid-sink-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
        let transfer_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("asset name"));
        let fee_asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let transfer_asset_definition =
            AssetDefinition::numeric(transfer_asset_definition_id.clone())
                .with_name("rose".to_owned())
                .build(&payer_id);
        let fee_asset_definition = AssetDefinition::numeric(fee_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&payer_id);
        let payer_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
        let recipient_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
        let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
        let world = World::with_assets(
            [domain],
            [payer, recipient],
            [transfer_asset_definition, fee_asset_definition],
            [
                Asset::new(payer_transfer_asset.clone(), Numeric::from(5_u32)),
                Asset::new(recipient_transfer_asset.clone(), Numeric::zero()),
                Asset::new(payer_fee_asset.clone(), Numeric::from(10_u32)),
            ],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = "not-an-account-literal".to_owned();
            nexus.fees.burn_from_unix_timestamp_ms = 20;
        }

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_instructions([Transfer::asset_numeric(
                payer_transfer_asset.clone(),
                1_u32,
                recipient_id,
            )])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert_eq!(
            valid_block.as_ref().errors().next().map(|(idx, _)| idx),
            Some(0),
            "invalid pre-burn fee sink must reject the transaction"
        );
        let snapshot = crate::sumeragi::status::snapshot();
        assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
        assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 1);

        let assets = state_block.world.assets();
        assert_eq!(
            assets
                .get(&payer_transfer_asset)
                .expect("payer rose after invalid sink rejection")
                .0,
            Numeric::from(5_u32)
        );
        assert_eq!(
            assets
                .get(&recipient_transfer_asset)
                .expect("recipient rose after invalid sink rejection")
                .0,
            Numeric::zero()
        );
        assert_eq!(
            assets
                .get(&payer_fee_asset)
                .expect("payer xor after invalid sink rejection")
                .0,
            Numeric::from(10_u32),
            "fee routing config failures must not debit the payer"
        );
    }

    #[test]
    fn fee_enabled_unauthorized_sponsor_rejects_without_transfer_or_sponsor_debit() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();

        let chain_id = ChainId::from("fee-detached-unauthorized-sponsor-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (sponsor_id, _sponsor_keypair) = gen_account_in("wonderland");
        let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
        let (sink_id, _sink_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let sponsor = Account::new(sponsor_id.clone()).build(&sponsor_id);
        let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let transfer_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("asset name"));
        let fee_asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let transfer_asset_definition =
            AssetDefinition::numeric(transfer_asset_definition_id.clone())
                .with_name("rose".to_owned())
                .build(&payer_id);
        let fee_asset_definition = AssetDefinition::numeric(fee_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&payer_id);
        let payer_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
        let recipient_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
        let sponsor_fee_asset = AssetId::of(fee_asset_definition_id.clone(), sponsor_id.clone());
        let world = World::with_assets(
            [domain],
            [payer, sponsor, recipient, sink],
            [transfer_asset_definition, fee_asset_definition],
            [
                Asset::new(payer_transfer_asset.clone(), Numeric::from(5_u32)),
                Asset::new(recipient_transfer_asset.clone(), Numeric::zero()),
                Asset::new(sponsor_fee_asset.clone(), Numeric::from(10_u32)),
            ],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.sponsorship_enabled = true;
            nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("fee_sponsor").expect("metadata key"),
            Json::new(sponsor_id.to_string()),
        );
        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_metadata(metadata)
            .with_instructions([Transfer::asset_numeric(
                payer_transfer_asset.clone(),
                1_u32,
                recipient_id,
            )])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert_eq!(
            valid_block.as_ref().errors().next().map(|(idx, _)| idx),
            Some(0),
            "unauthorized fee sponsor metadata must reject the transaction"
        );
        let snapshot = crate::sumeragi::status::snapshot();
        assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
        assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 1);

        let assets = state_block.world.assets();
        assert_eq!(
            assets
                .get(&payer_transfer_asset)
                .expect("payer rose after sponsor rejection")
                .0,
            Numeric::from(5_u32)
        );
        assert_eq!(
            assets
                .get(&recipient_transfer_asset)
                .expect("recipient rose after sponsor rejection")
                .0,
            Numeric::zero()
        );
        assert_eq!(
            assets
                .get(&sponsor_fee_asset)
                .expect("sponsor xor after sponsor rejection")
                .0,
            Numeric::from(10_u32),
            "unauthorized sponsor rejection must not debit the sponsor"
        );
    }

    #[test]
    fn fee_enabled_disabled_sponsor_rejects_without_transfer_or_sponsor_debit() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();

        let chain_id = ChainId::from("fee-detached-disabled-sponsor-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (sponsor_id, _sponsor_keypair) = gen_account_in("wonderland");
        let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
        let (sink_id, _sink_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let sponsor = Account::new(sponsor_id.clone()).build(&sponsor_id);
        let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let transfer_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("asset name"));
        let fee_asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let transfer_asset_definition =
            AssetDefinition::numeric(transfer_asset_definition_id.clone())
                .with_name("rose".to_owned())
                .build(&payer_id);
        let fee_asset_definition = AssetDefinition::numeric(fee_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&payer_id);
        let payer_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
        let recipient_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
        let sponsor_fee_asset = AssetId::of(fee_asset_definition_id.clone(), sponsor_id.clone());
        let world = World::with_assets(
            [domain],
            [payer, sponsor, recipient, sink],
            [transfer_asset_definition, fee_asset_definition],
            [
                Asset::new(payer_transfer_asset.clone(), Numeric::from(5_u32)),
                Asset::new(recipient_transfer_asset.clone(), Numeric::zero()),
                Asset::new(sponsor_fee_asset.clone(), Numeric::from(10_u32)),
            ],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.sponsorship_enabled = false;
            nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("fee_sponsor").expect("metadata key"),
            Json::new(sponsor_id.to_string()),
        );
        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_metadata(metadata)
            .with_instructions([Transfer::asset_numeric(
                payer_transfer_asset.clone(),
                1_u32,
                recipient_id,
            )])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert_eq!(
            valid_block.as_ref().errors().next().map(|(idx, _)| idx),
            Some(0),
            "fee_sponsor metadata must reject when sponsorship is disabled"
        );
        let snapshot = crate::sumeragi::status::snapshot();
        assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
        assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 1);

        let assets = state_block.world.assets();
        assert_eq!(
            assets
                .get(&payer_transfer_asset)
                .expect("payer rose after disabled sponsor rejection")
                .0,
            Numeric::from(5_u32)
        );
        assert_eq!(
            assets
                .get(&recipient_transfer_asset)
                .expect("recipient rose after disabled sponsor rejection")
                .0,
            Numeric::zero()
        );
        assert_eq!(
            assets
                .get(&sponsor_fee_asset)
                .expect("sponsor xor after disabled sponsor rejection")
                .0,
            Numeric::from(10_u32),
            "disabled sponsorship must not debit the requested sponsor"
        );
    }

    #[test]
    fn fee_enabled_sponsor_cap_rejects_without_transfer_or_sponsor_debit() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();

        let chain_id = ChainId::from("fee-detached-sponsor-cap-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (sponsor_id, _sponsor_keypair) = gen_account_in("wonderland");
        let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
        let (sink_id, _sink_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let sponsor = Account::new(sponsor_id.clone()).build(&sponsor_id);
        let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let transfer_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("asset name"));
        let fee_asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let transfer_asset_definition =
            AssetDefinition::numeric(transfer_asset_definition_id.clone())
                .with_name("rose".to_owned())
                .build(&payer_id);
        let fee_asset_definition = AssetDefinition::numeric(fee_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&payer_id);
        let payer_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
        let recipient_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
        let sponsor_fee_asset = AssetId::of(fee_asset_definition_id.clone(), sponsor_id.clone());
        let mut world = World::with_assets(
            [domain],
            [payer, sponsor, recipient, sink],
            [transfer_asset_definition, fee_asset_definition],
            [
                Asset::new(payer_transfer_asset.clone(), Numeric::from(5_u32)),
                Asset::new(recipient_transfer_asset.clone(), Numeric::zero()),
                Asset::new(sponsor_fee_asset.clone(), Numeric::from(10_u32)),
            ],
            [],
        );
        let fee_permission: Permission =
            iroha_executor_data_model::permission::nexus::CanUseFeeSponsor {
                sponsor: sponsor_id.clone(),
            }
            .into();
        world.account_permissions.insert(
            payer_id.clone(),
            std::collections::BTreeSet::from([fee_permission]),
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(2_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.sponsorship_enabled = true;
            nexus.fees.sponsor_max_fee = Numeric::from(1_u32);
            nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("fee_sponsor").expect("metadata key"),
            Json::new(sponsor_id.to_string()),
        );
        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_metadata(metadata)
            .with_instructions([Transfer::asset_numeric(
                payer_transfer_asset.clone(),
                1_u32,
                recipient_id,
            )])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert_eq!(
            valid_block.as_ref().errors().next().map(|(idx, _)| idx),
            Some(0),
            "fee_sponsor metadata must reject when computed fee exceeds sponsor_max_fee"
        );
        let snapshot = crate::sumeragi::status::snapshot();
        assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
        assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 1);

        let assets = state_block.world.assets();
        assert_eq!(
            assets
                .get(&payer_transfer_asset)
                .expect("payer rose after sponsor cap rejection")
                .0,
            Numeric::from(5_u32)
        );
        assert_eq!(
            assets
                .get(&recipient_transfer_asset)
                .expect("recipient rose after sponsor cap rejection")
                .0,
            Numeric::zero()
        );
        assert_eq!(
            assets
                .get(&sponsor_fee_asset)
                .expect("sponsor xor after sponsor cap rejection")
                .0,
            Numeric::from(10_u32),
            "sponsor cap rejection must not debit the sponsor"
        );
    }

    #[test]
    fn fee_enabled_invalid_fee_asset_rejects_without_partial_transfer_or_fee() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();

        let chain_id = ChainId::from("fee-detached-invalid-fee-asset-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
        let (sink_id, _sink_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let transfer_asset_definition_id =
            AssetDefinitionId::new(domain_id, "rose".parse().expect("asset name"));
        let transfer_asset_definition =
            AssetDefinition::numeric(transfer_asset_definition_id.clone())
                .with_name("rose".to_owned())
                .build(&payer_id);
        let payer_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
        let recipient_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
        let world = World::with_assets(
            [domain],
            [payer, recipient, sink],
            [transfer_asset_definition],
            [
                Asset::new(payer_transfer_asset.clone(), Numeric::from(5_u32)),
                Asset::new(recipient_transfer_asset.clone(), Numeric::zero()),
            ],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.fee_asset_id = "not-an-asset-literal".to_owned();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_instructions([Transfer::asset_numeric(
                payer_transfer_asset.clone(),
                1_u32,
                recipient_id,
            )])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert_eq!(
            valid_block.as_ref().errors().next().map(|(idx, _)| idx),
            Some(0),
            "invalid configured fee asset must reject the transaction"
        );
        let snapshot = crate::sumeragi::status::snapshot();
        assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
        assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 1);

        let assets = state_block.world.assets();
        assert_eq!(
            assets
                .get(&payer_transfer_asset)
                .expect("payer rose after invalid fee asset rejection")
                .0,
            Numeric::from(5_u32)
        );
        assert_eq!(
            assets
                .get(&recipient_transfer_asset)
                .expect("recipient rose after invalid fee asset rejection")
                .0,
            Numeric::zero()
        );
    }

    #[test]
    fn fee_enabled_malformed_sponsor_metadata_rejects_without_transfer_or_fee() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();

        let chain_id = ChainId::from("fee-detached-malformed-sponsor-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
        let (sink_id, _sink_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let transfer_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("asset name"));
        let fee_asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let transfer_asset_definition =
            AssetDefinition::numeric(transfer_asset_definition_id.clone())
                .with_name("rose".to_owned())
                .build(&payer_id);
        let fee_asset_definition = AssetDefinition::numeric(fee_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&payer_id);
        let payer_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
        let recipient_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
        let payer_fee_asset = AssetId::of(fee_asset_definition_id.clone(), payer_id.clone());
        let world = World::with_assets(
            [domain],
            [payer, recipient, sink],
            [transfer_asset_definition, fee_asset_definition],
            [
                Asset::new(payer_transfer_asset.clone(), Numeric::from(5_u32)),
                Asset::new(recipient_transfer_asset.clone(), Numeric::zero()),
                Asset::new(payer_fee_asset.clone(), Numeric::from(10_u32)),
            ],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.sponsorship_enabled = true;
            nexus.fees.fee_asset_id = fee_asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("fee_sponsor").expect("metadata key"),
            Json::from(true),
        );
        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_metadata(metadata)
            .with_instructions([Transfer::asset_numeric(
                payer_transfer_asset.clone(),
                1_u32,
                recipient_id,
            )])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert_eq!(
            valid_block.as_ref().errors().next().map(|(idx, _)| idx),
            Some(0),
            "malformed fee_sponsor metadata must reject the transaction"
        );
        let snapshot = crate::sumeragi::status::snapshot();
        assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
        assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 1);

        let assets = state_block.world.assets();
        assert_eq!(
            assets
                .get(&payer_transfer_asset)
                .expect("payer rose after malformed sponsor rejection")
                .0,
            Numeric::from(5_u32)
        );
        assert_eq!(
            assets
                .get(&recipient_transfer_asset)
                .expect("recipient rose after malformed sponsor rejection")
                .0,
            Numeric::zero()
        );
        assert_eq!(
            assets
                .get(&payer_fee_asset)
                .expect("payer xor after malformed sponsor rejection")
                .0,
            Numeric::from(10_u32),
            "malformed sponsor metadata must not fall back to payer debit"
        );
    }

    #[test]
    fn fee_enabled_missing_gas_asset_metadata_rejects_without_partial_transfer() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();

        let chain_id = ChainId::from("fee-detached-missing-gas-asset-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
        let transfer_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("asset name"));
        let gas_asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let transfer_asset_definition =
            AssetDefinition::numeric(transfer_asset_definition_id.clone())
                .with_name("rose".to_owned())
                .build(&payer_id);
        let gas_asset_definition = AssetDefinition::numeric(gas_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&payer_id);
        let payer_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
        let recipient_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
        let payer_gas_asset = AssetId::of(gas_asset_definition_id.clone(), payer_id.clone());
        let world = World::with_assets(
            [domain],
            [payer, recipient],
            [transfer_asset_definition, gas_asset_definition],
            [
                Asset::new(payer_transfer_asset.clone(), Numeric::from(5_u32)),
                Asset::new(recipient_transfer_asset.clone(), Numeric::zero()),
                Asset::new(payer_gas_asset.clone(), Numeric::from(10_u32)),
            ],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        state.pipeline.gas.accepted_assets = vec![gas_asset_definition_id.to_string()];

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_instructions([Transfer::asset_numeric(
                payer_transfer_asset.clone(),
                1_u32,
                recipient_id,
            )])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert_eq!(
            valid_block.as_ref().errors().next().map(|(idx, _)| idx),
            Some(0),
            "gas policy must reject a transaction missing gas_asset_id metadata"
        );
        let snapshot = crate::sumeragi::status::snapshot();
        assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
        assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 1);

        let assets = state_block.world.assets();
        assert_eq!(
            assets
                .get(&payer_transfer_asset)
                .expect("payer rose after missing gas asset rejection")
                .0,
            Numeric::from(5_u32)
        );
        assert_eq!(
            assets
                .get(&recipient_transfer_asset)
                .expect("recipient rose after missing gas asset rejection")
                .0,
            Numeric::zero()
        );
        assert_eq!(
            assets
                .get(&payer_gas_asset)
                .expect("payer gas asset after missing gas metadata rejection")
                .0,
            Numeric::from(10_u32)
        );
    }

    #[test]
    fn fee_enabled_missing_gas_rate_mapping_rejects_without_partial_transfer() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();
        crate::sumeragi::status::reset_rbc_backlog_stats_for_tests();

        let chain_id = ChainId::from("fee-detached-missing-gas-rate-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (recipient_id, _recipient_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let recipient = Account::new(recipient_id.clone()).build(&recipient_id);
        let transfer_asset_definition_id =
            AssetDefinitionId::new(domain_id.clone(), "rose".parse().expect("asset name"));
        let gas_asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let transfer_asset_definition =
            AssetDefinition::numeric(transfer_asset_definition_id.clone())
                .with_name("rose".to_owned())
                .build(&payer_id);
        let gas_asset_definition = AssetDefinition::numeric(gas_asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&payer_id);
        let payer_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), payer_id.clone());
        let recipient_transfer_asset =
            AssetId::of(transfer_asset_definition_id.clone(), recipient_id.clone());
        let payer_gas_asset = AssetId::of(gas_asset_definition_id.clone(), payer_id.clone());
        let world = World::with_assets(
            [domain],
            [payer, recipient],
            [transfer_asset_definition, gas_asset_definition],
            [
                Asset::new(payer_transfer_asset.clone(), Numeric::from(5_u32)),
                Asset::new(recipient_transfer_asset.clone(), Numeric::zero()),
                Asset::new(payer_gas_asset.clone(), Numeric::from(10_u32)),
            ],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        state.pipeline.gas.accepted_assets = vec![gas_asset_definition_id.to_string()];
        state.pipeline.gas.units_per_gas.clear();

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("gas_asset_id").expect("metadata key"),
            Json::new(gas_asset_definition_id.to_string()),
        );
        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_metadata(metadata)
            .with_instructions([Transfer::asset_numeric(
                payer_transfer_asset.clone(),
                1_u32,
                recipient_id,
            )])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert_eq!(
            valid_block.as_ref().errors().next().map(|(idx, _)| idx),
            Some(0),
            "accepted gas_asset_id without units_per_gas mapping must reject"
        );
        let snapshot = crate::sumeragi::status::snapshot();
        assert_eq!(snapshot.pipeline_execution.detached_merged_total, 0);
        assert_eq!(snapshot.pipeline_execution.detached_fallback_total, 1);

        let assets = state_block.world.assets();
        assert_eq!(
            assets
                .get(&payer_transfer_asset)
                .expect("payer rose after missing gas rate rejection")
                .0,
            Numeric::from(5_u32)
        );
        assert_eq!(
            assets
                .get(&recipient_transfer_asset)
                .expect("recipient rose after missing gas rate rejection")
                .0,
            Numeric::zero()
        );
        assert_eq!(
            assets
                .get(&payer_gas_asset)
                .expect("payer gas asset after missing gas rate rejection")
                .0,
            Numeric::from(10_u32)
        );
    }

    #[test]
    fn signed_block_sponsored_fee_burns_when_sponsor_is_fee_sink() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let chain_id = ChainId::from("sponsored-block-fee-burn-test");
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let (sponsor_id, sponsor_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&authority_id);
        let authority = Account::new(authority_id.clone()).build(&authority_id);
        let sponsor = Account::new(sponsor_id.clone()).build(&sponsor_id);
        let asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&authority_id);
        let sponsor_asset_id = AssetId::of(asset_definition_id.clone(), sponsor_id.clone());
        let sponsor_asset = Asset::new(sponsor_asset_id.clone(), Numeric::from(10_u32));
        let world = World::with_assets(
            [domain],
            [authority, sponsor],
            [asset_definition],
            [sponsor_asset],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.sponsorship_enabled = true;
            nexus.fees.fee_asset_id = asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = sponsor_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 5;
        }

        {
            let fee_permission: Permission =
                iroha_executor_data_model::permission::nexus::CanUseFeeSponsor {
                    sponsor: sponsor_id.clone(),
                }
                .into();
            let mut world = state.world.block();
            world.account_permissions.insert(
                authority_id.clone(),
                std::collections::BTreeSet::from([fee_permission]),
            );
            world.commit();
        }

        let (_genesis_handle, genesis_time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let genesis_block = BlockBuilder::new_with_time_source(Vec::new(), genesis_time_source)
            .chain(0, None)
            .sign(sponsor_keypair.private_key())
            .unpack(|_| {});
        let genesis_signed: SignedBlock = genesis_block.clone().into();
        let mut genesis_state_block = state.block(genesis_block.header);
        let _valid_genesis = genesis_block
            .validate_and_record_transactions(&mut genesis_state_block)
            .unpack(|_| {});
        genesis_state_block.commit().expect("commit first block");

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("fee_sponsor").expect("static name"),
            iroha_primitives::json::Json::new(sponsor_id.to_string()),
        );
        let mut builder = TransactionBuilder::new(chain_id.clone(), authority_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_metadata(metadata)
            .with_instructions::<InstructionBox>([Log::new(Level::INFO, "fee".to_owned()).into()])
            .sign(authority_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&genesis_signed))
            .sign(sponsor_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert!(
            valid_block.as_ref().errors().next().is_none(),
            "sponsored transaction should be approved"
        );
        state_block.commit().expect("commit sponsored fee block");

        let committed_balance_after = state
            .view()
            .world()
            .assets()
            .get(&sponsor_asset_id)
            .expect("sponsor asset exists after block commit")
            .0
            .try_mantissa_u128()
            .unwrap();
        assert_eq!(committed_balance_after, 9);
    }

    #[test]
    fn routed_signed_block_sponsored_fee_burns_global_sponsor_asset() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let chain_id = ChainId::from("routed-sponsored-block-fee-burn-test");
        let (authority_id, authority_keypair) = gen_account_in("wonderland");
        let (sponsor_id, sponsor_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&authority_id);
        let authority = Account::new(authority_id.clone()).build(&authority_id);
        let sponsor = Account::new(sponsor_id.clone()).build(&sponsor_id);
        let asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&authority_id);
        let sponsor_asset_id = AssetId::of(asset_definition_id.clone(), sponsor_id.clone());
        let sponsor_asset = Asset::new(sponsor_asset_id.clone(), Numeric::from(10_u32));
        let world = World::with_assets(
            [domain],
            [authority, sponsor],
            [asset_definition],
            [sponsor_asset],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        let paynet_lane = LaneId::new(3);
        let paynet_dataspace = DataSpaceId::new(10);
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.sponsorship_enabled = true;
            nexus.fees.fee_asset_id = asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = sponsor_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 5;
            nexus.lane_catalog = LaneCatalog::new(
                nonzero!(4_u32),
                vec![
                    LaneConfig::default(),
                    LaneConfig {
                        id: paynet_lane,
                        dataspace_id: paynet_dataspace,
                        alias: "paynet".to_string(),
                        ..LaneConfig::default()
                    },
                ],
            )
            .expect("lane catalog");
            nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
                iroha_data_model::nexus::DataSpaceMetadata::default(),
                iroha_data_model::nexus::DataSpaceMetadata {
                    id: paynet_dataspace,
                    alias: "paynet".to_string(),
                    description: None,
                    fault_tolerance: 1,
                },
            ])
            .expect("dataspace catalog");
            nexus.routing_policy.default_lane = paynet_lane;
            nexus.routing_policy.default_dataspace = paynet_dataspace;
        }

        {
            let fee_permission: Permission =
                iroha_executor_data_model::permission::nexus::CanUseFeeSponsor {
                    sponsor: sponsor_id.clone(),
                }
                .into();
            let mut world = state.world.block();
            world.account_permissions.insert(
                authority_id.clone(),
                std::collections::BTreeSet::from([fee_permission]),
            );
            world.commit();
        }

        let (_genesis_handle, genesis_time_source) = TimeSource::new_mock(Duration::from_millis(1));
        let genesis_block = BlockBuilder::new_with_time_source(Vec::new(), genesis_time_source)
            .chain(0, None)
            .sign(sponsor_keypair.private_key())
            .unpack(|_| {});
        let genesis_signed: SignedBlock = genesis_block.clone().into();
        let mut genesis_state_block = state.block(genesis_block.header);
        let _valid_genesis = genesis_block
            .validate_and_record_transactions(&mut genesis_state_block)
            .unpack(|_| {});
        genesis_state_block.commit().expect("commit first block");

        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("fee_sponsor").expect("static name"),
            iroha_primitives::json::Json::new(sponsor_id.to_string()),
        );
        let mut builder = TransactionBuilder::new(chain_id.clone(), authority_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_metadata(metadata)
            .with_instructions::<InstructionBox>([Log::new(Level::INFO, "fee".to_owned()).into()])
            .sign(authority_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&genesis_signed))
            .sign(sponsor_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert!(
            valid_block.as_ref().errors().next().is_none(),
            "routed sponsored transaction should be approved"
        );
        state_block
            .commit()
            .expect("commit routed sponsored fee block");

        let committed_balance_after = state
            .view()
            .world()
            .assets()
            .get(&sponsor_asset_id)
            .expect("sponsor asset exists after block commit")
            .0
            .try_mantissa_u128()
            .unwrap();
        assert_eq!(committed_balance_after, 9);
    }

    #[test]
    fn rejected_data_trigger_execution_still_charges_nexus_fee() {
        let _guard = crate::sumeragi::status::nexus_fee_test_lock()
            .lock()
            .expect("nexus fee test lock");
        crate::sumeragi::status::reset_nexus_economics_for_tests();

        let chain_id = ChainId::from("rejected-trigger-fee-test");
        let (payer_id, payer_keypair) = gen_account_in("wonderland");
        let (sink_id, _sink_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").unwrap();
        let domain = Domain::new(domain_id.clone()).build(&payer_id);
        let payer = Account::new(payer_id.clone()).build(&payer_id);
        let sink = Account::new(sink_id.clone()).build(&sink_id);
        let asset_definition_id =
            AssetDefinitionId::new(domain_id, "xor".parse().expect("asset name"));
        let asset_definition = AssetDefinition::numeric(asset_definition_id.clone())
            .with_name("xor".to_owned())
            .build(&payer_id);
        let payer_asset = Asset::new(
            AssetId::of(asset_definition_id.clone(), payer_id.clone()),
            Numeric::from(10_u32),
        );
        let sink_asset = Asset::new(
            AssetId::of(asset_definition_id.clone(), sink_id.clone()),
            Numeric::zero(),
        );
        let world = World::with_assets(
            [domain],
            [payer, sink],
            [asset_definition],
            [payer_asset, sink_asset],
            [],
        );
        let kura = Arc::new(Kura::blank_kura_for_testing());
        let query_handle = LiveQueryStore::start_test();
        let mut state =
            State::new_with_chain(world, Arc::clone(&kura), query_handle, chain_id.clone());
        {
            let nexus = state.nexus.get_mut();
            nexus.enabled = true;
            nexus.fees.base_fee = Numeric::from(1_u32);
            nexus.fees.per_byte_fee = Numeric::zero();
            nexus.fees.per_instruction_fee = Numeric::zero();
            nexus.fees.per_gas_unit_fee = Numeric::zero();
            nexus.fees.fee_asset_id = asset_definition_id.to_string();
            nexus.fees.fee_sink_account_id = sink_id.to_string();
            nexus.fees.burn_from_unix_timestamp_ms = 0;
        }
        {
            let mut world = state.world.block();
            world
                .parameters
                .set_parameter(iroha_data_model::parameter::Parameter::SmartContract(
                    iroha_data_model::parameter::SmartContractParameter::ExecutionDepth(0),
                ));
            world.commit();
        }
        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };
        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_leader_public, leader_private) = leader.into_parts();
        let latest_valid = ValidBlock::new_dummy_and_modify_header(&leader_private, |header| {
            header.set_height(nonzero!(1_u64));
        });
        let latest_signed: SignedBlock = latest_valid.into();

        let trigger_id: TriggerId = "fee_depth_limit_trigger".parse().unwrap();
        let flag_key: Name = "fee_trigger_flag".parse().unwrap();
        let event_key: Name = "fee_trigger_event".parse().unwrap();
        let trigger = Trigger::new(
            trigger_id,
            Action::new(
                vec![InstructionBox::from(SetKeyValue::account(
                    payer_id.clone(),
                    flag_key,
                    Json::from(true),
                ))],
                Repeats::Indefinitely,
                payer_id.clone(),
                DataEventFilter::Any,
            ),
        );
        let mut builder = TransactionBuilder::new(chain_id.clone(), payer_id.clone());
        builder.set_creation_time(Duration::from_millis(0));
        let tx = builder
            .with_instructions::<InstructionBox>([
                Grant::account_permission(
                    iroha_executor_data_model::permission::trigger::CanRegisterTrigger {
                        authority: payer_id.clone(),
                    },
                    payer_id.clone(),
                )
                .into(),
                Register::trigger(trigger).into(),
                SetKeyValue::account(payer_id.clone(), event_key.clone(), Json::from(true)).into(),
            ])
            .sign(payer_keypair.private_key());
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            state.crypto().as_ref(),
        )
        .expect("transaction should pass stateless admission");

        let (_block_handle, block_time_source) = TimeSource::new_mock(Duration::from_millis(10));
        let unverified_block = BlockBuilder::new_with_time_source(vec![tx], block_time_source)
            .chain(1, Some(&latest_signed))
            .sign(payer_keypair.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert_eq!(
            valid_block.as_ref().errors().next().map(|(idx, _)| idx),
            Some(0)
        );
        let first_error = valid_block.as_ref().errors().next().map(|(_, err)| err);
        assert!(
            matches!(
                first_error,
                Some(TransactionRejectionReason::TriggerExecution(
                    iroha_data_model::transaction::error::TriggerExecutionFail::MaxDepthExceeded
                ))
            ),
            "unexpected trigger rejection: {first_error:?}"
        );

        let assets = state_block.world.assets();
        let payer_balance = assets
            .get(&AssetId::of(asset_definition_id.clone(), payer_id.clone()))
            .expect("payer balance exists")
            .0
            .to_string();
        let sink_balance = assets
            .get(&AssetId::of(asset_definition_id, sink_id))
            .expect("sink balance exists")
            .0
            .to_string();

        assert_eq!(payer_balance, "9", "tx error: {first_error:?}");
        assert_eq!(sink_balance, "0");
        let event_value = state_block
            .world
            .map_account(&payer_id, |account| {
                account.value().metadata().get(&event_key).cloned()
            })
            .expect("payer account exists");
        assert!(
            event_value.is_none(),
            "trigger-rejected transaction state changes must still be rolled back"
        );
    }

    #[tokio::test]
    async fn validate_and_record_transactions_allows_missing_authority_self_register() {
        let chain_id = ChainId::from("missing-authority-self-register-block");

        let (authority, keypair) = gen_account_in("wonderland");
        let world = World::new();
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new_with_chain(world, kura, query_handle, chain_id.clone());
        let (max_clock_drift, tx_limits) = {
            let state_view = state.world.view();
            let params = state_view.parameters();
            (params.sumeragi().max_clock_drift(), params.transaction())
        };

        let tx = TransactionBuilder::new(chain_id.clone(), authority.clone())
            .with_instructions([
                InstructionBox::from(Register::account(Account::new(authority.clone()))),
                InstructionBox::from(Log::new(Level::INFO, "self-register".into())),
            ])
            .sign(keypair.private_key());
        let crypto_cfg = state.crypto();
        let tx = AcceptedTransaction::accept(
            tx,
            &chain_id,
            max_clock_drift,
            tx_limits,
            crypto_cfg.as_ref(),
        )
        .expect("admission should accept transaction shape");

        let unverified_block = BlockBuilder::new(vec![tx])
            .chain(0, state.view().latest_block().as_deref())
            .sign(keypair.private_key())
            .unpack(|_| {});

        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});

        assert!(
            valid_block.as_ref().errors().next().is_none(),
            "self-register block path should not produce transaction errors"
        );
        assert!(
            state_block.world.accounts.get(&authority).is_some(),
            "authority account should be materialized during block execution"
        );
    }

    #[tokio::test]
    async fn genesis_public_key_is_checked() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        // Predefined world state
        let genesis_correct_key = KeyPair::random();
        let genesis_wrong_key = KeyPair::random();
        let genesis_correct_account_id = AccountId::new(genesis_correct_key.public_key().clone());
        let genesis_wrong_account_id = AccountId::new(genesis_wrong_key.public_key().clone());
        let genesis_domain =
            Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_correct_account_id);
        let genesis_wrong_account =
            Account::new(genesis_wrong_account_id.clone()).build(&genesis_wrong_account_id);
        let world = World::with([genesis_domain], [genesis_wrong_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);

        // Creating an instruction
        let isi = Log::new(
            iroha_data_model::Level::DEBUG,
            "instruction itself doesn't matter here".to_string(),
        );

        // Create genesis transaction
        // Sign with `genesis_wrong_key` as peer which has incorrect genesis key pair
        // Bypass `accept_genesis` check to allow signing with wrong key
        let tx = TransactionBuilder::new(chain_id.clone(), genesis_wrong_account_id.clone())
            .with_instructions([isi])
            .sign(genesis_wrong_key.private_key());
        let tx = AcceptedTransaction::new_unchecked(Cow::Owned(tx));

        // Create genesis block
        let transactions = vec![tx];
        let topology = test_topology(1);
        let unverified_block = BlockBuilder::new(transactions)
            .chain(0, state.view().latest_block().as_deref())
            .sign(genesis_correct_key.private_key())
            .unpack(|_| {});

        let mut state_block = state.block(unverified_block.header);
        let valid_block = unverified_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        state_block.commit().unwrap();

        // Validate genesis block
        // Use correct genesis key and check if transaction is rejected
        let block: SignedBlock = valid_block.into();
        let mut state_block = state.block(block.header());
        let (_handle, time_source) = TimeSource::new_mock(block.header().creation_time());
        let (_, error) = ValidBlock::validate(
            block,
            &topology,
            &chain_id,
            &genesis_correct_account_id,
            &time_source,
            &mut state_block,
        )
        .unpack(|_| {})
        .unwrap_err();
        state_block.commit().unwrap();

        // The first transaction should be rejected
        assert_eq!(
            error.as_ref(),
            &BlockValidationError::InvalidGenesis(InvalidGenesisError::UnexpectedAuthority)
        );
    }

    #[tokio::test]
    async fn genesis_asset_definition_registration_is_not_domain_gated() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        let genesis_key_pair = KeyPair::random();
        let genesis_account_id = AccountId::new(genesis_key_pair.public_key().clone());
        let alice_key_pair = KeyPair::random();
        let wonderland_domain_id: DomainId =
            DomainId::try_new("wonderland", "universal").expect("Valid domain id");
        let alice_account_id = AccountId::new(alice_key_pair.public_key().clone());

        let genesis_domain = Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_account_id);
        let wonderland_domain = Domain::new(wonderland_domain_id.clone()).build(&alice_account_id);
        let genesis_account = Account::new(genesis_account_id.clone()).build(&genesis_account_id);
        let alice_account = Account::new(alice_account_id.clone()).build(&alice_account_id);

        let world = World::with(
            [genesis_domain, wonderland_domain],
            [genesis_account, alice_account],
            [],
        );
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);

        let asset_definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("valid domain id"),
            "xor".parse().expect("valid asset name"),
        );
        let instruction = Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id).with_name("xor".to_owned()),
        );

        let tx = TransactionBuilder::new(chain_id.clone(), genesis_account_id.clone())
            .with_instructions([instruction])
            .sign(genesis_key_pair.private_key());
        let block = SignedBlock::genesis(vec![tx], genesis_key_pair.private_key(), None, None);

        let topology = test_topology(1);
        let mut state_block = state.block(block.header());
        let (_handle, time_source) = TimeSource::new_mock(block.header().creation_time());
        let _valid = ValidBlock::validate(
            block,
            &topology,
            &chain_id,
            &genesis_account_id,
            &time_source,
            &mut state_block,
        )
        .unpack(|_| {})
        .expect(
            "genesis asset-definition registration should not require domain-owner authorization",
        );
        state_block.commit().unwrap();
    }

    #[tokio::test]
    async fn genesis_domain_registration_bootstraps_domain_name_lease() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        let genesis_key_pair = KeyPair::random();
        let genesis_account_id = AccountId::new(genesis_key_pair.public_key().clone());
        let wonderland_domain_id: DomainId =
            DomainId::try_new("wonderland", "universal").expect("valid domain id");

        let genesis_domain = Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_account_id);
        let genesis_account = Account::new(genesis_account_id.clone()).build(&genesis_account_id);

        let world = World::with([genesis_domain], [genesis_account], []);
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state = State::new(world, kura, query_handle);

        let instruction = Register::domain(Domain::new(wonderland_domain_id.clone()));

        let tx = TransactionBuilder::new(chain_id.clone(), genesis_account_id.clone())
            .with_instructions([instruction])
            .sign(genesis_key_pair.private_key());
        let block = SignedBlock::genesis(vec![tx], genesis_key_pair.private_key(), None, None);

        let topology = test_topology(1);
        let mut state_block = state.block(block.header());
        let (_handle, time_source) = TimeSource::new_mock(block.header().creation_time());
        let _valid = ValidBlock::validate(
            block,
            &topology,
            &chain_id,
            &genesis_account_id,
            &time_source,
            &mut state_block,
        )
        .unpack(|_| {})
        .expect("genesis domain registration should bootstrap the SNS lease");
        state_block.commit().unwrap();

        let view = state.view();
        assert_eq!(
            crate::sns::active_domain_owner(view.world(), &wonderland_domain_id, 0),
            Some(genesis_account_id),
            "genesis registration should leave an active domain-name record behind"
        );
    }

    #[test]
    fn sumeragi_parameters_are_accessible() {
        let params = iroha_data_model::parameter::Parameters::default();
        let _ = params.sumeragi().max_clock_drift();
    }

    #[cfg(feature = "bls")]
    #[test]
    fn verify_validator_signatures_accepts_bls_normal() {
        use iroha_crypto::{Algorithm, KeyPair};
        use iroha_data_model::prelude::PeerId;

        use crate::sumeragi::network_topology::Topology;

        // 3 BLS peers
        let kp0 = KeyPair::from_seed(b"seed0".to_vec(), Algorithm::BlsNormal);
        let kp1 = KeyPair::from_seed(b"seed1".to_vec(), Algorithm::BlsNormal);
        let kp2 = KeyPair::from_seed(b"seed2".to_vec(), Algorithm::BlsNormal);
        let peers = vec![
            PeerId::new(kp0.public_key().clone()),
            PeerId::new(kp1.public_key().clone()),
            PeerId::new(kp2.public_key().clone()),
        ];
        let topology = Topology::new(peers);

        // Build SignedBlock signed by all
        let unverified_block = BlockBuilder::new(vec![dummy_accepted_transaction()])
            .chain(0, None)
            .sign(kp0.private_key())
            .unpack(|_| {});
        let mut vb = ValidBlock::new_unverified_for_tests(unverified_block.into());
        vb.sign(&kp1, &topology);
        vb.sign(&kp2, &topology);
        // Commit succeeds under BLS-normal uniform validators
        assert!(vb.commit(&topology).unpack(|_| {}).is_ok());
    }

    #[test]
    fn signature_error_maps_inactive_consensus_key_reason() {
        assert_eq!(
            map_sig_err_to_reason(&SignatureVerificationError::InactiveConsensusKey),
            error::BlockRejectionReason::InactiveConsensusKey
        );
    }
}

#[cfg(test)]
mod commit_signature_tally_tests {
    use std::collections::BTreeSet;

    use iroha_crypto::{Algorithm, KeyPair, SignatureOf};
    use iroha_data_model::block::builder::BlockBuilder as DataBlockBuilder;
    use nonzero_ext::nonzero;

    use super::*;
    use crate::{
        block::valid::commit_signature_tally,
        sumeragi::{consensus::ValidatorIndex, network_topology::Topology},
    };

    #[cfg(feature = "bls")]
    #[test]
    fn commit_signature_tally_dedups_and_counts_set_b() {
        let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_proxy = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_set_b = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let topology = Topology::new(vec![
            PeerId::new(kp_leader.public_key().clone()),
            PeerId::new(kp_validator.public_key().clone()),
            PeerId::new(kp_proxy.public_key().clone()),
            PeerId::new(kp_set_b.public_key().clone()),
        ]);

        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let hash = header.hash();
        let signatures = BTreeSet::from([
            BlockSignature::new(0, SignatureOf::from_hash(kp_leader.private_key(), hash)),
            BlockSignature::new(1, SignatureOf::from_hash(kp_validator.private_key(), hash)),
            BlockSignature::new(2, SignatureOf::from_hash(kp_proxy.private_key(), hash)),
            BlockSignature::new(3, SignatureOf::from_hash(kp_set_b.private_key(), hash)),
        ]);
        let block = DataBlockBuilder::new(header).build(signatures);

        let tally = commit_signature_tally(&block, &topology);
        assert_eq!(tally.present, 4);
        assert_eq!(tally.counted, 4);
        assert_eq!(tally.set_b_signatures, 1);
    }

    #[cfg(feature = "bls")]
    #[test]
    fn is_commit_rejects_duplicate_signer_index() {
        let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_proxy = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_dup = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let topology = Topology::new(vec![
            PeerId::new(kp_leader.public_key().clone()),
            PeerId::new(kp_proxy.public_key().clone()),
        ]);

        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let hash = header.hash();
        let signatures = BTreeSet::from([
            BlockSignature::new(0, SignatureOf::from_hash(kp_leader.private_key(), hash)),
            BlockSignature::new(1, SignatureOf::from_hash(kp_proxy.private_key(), hash)),
            BlockSignature::new(1, SignatureOf::from_hash(kp_dup.private_key(), hash)),
        ]);
        let block = DataBlockBuilder::new(header).build(signatures);

        let err = ValidBlock::is_commit(&block, &topology).unwrap_err();
        assert!(matches!(
            err,
            SignatureVerificationError::DuplicateSignature { signer } if signer == 1
        ));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn is_commit_rejects_proxy_tail_spoof() {
        let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_proxy = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_spoof = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let topology = Topology::new(vec![
            PeerId::new(kp_leader.public_key().clone()),
            PeerId::new(kp_proxy.public_key().clone()),
        ]);

        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let hash = header.hash();
        let signatures = BTreeSet::from([
            BlockSignature::new(0, SignatureOf::from_hash(kp_leader.private_key(), hash)),
            BlockSignature::new(1, SignatureOf::from_hash(kp_spoof.private_key(), hash)),
        ]);
        let block = DataBlockBuilder::new(header).build(signatures);

        let err = ValidBlock::is_commit(&block, &topology).unwrap_err();
        assert!(
            matches!(err, SignatureVerificationError::UnknownSignature),
            "expected proxy tail spoof rejection, got {err:?}"
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn is_commit_rejects_leader_spoof() {
        let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_proxy = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_spoof = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let topology = Topology::new(vec![
            PeerId::new(kp_leader.public_key().clone()),
            PeerId::new(kp_proxy.public_key().clone()),
        ]);

        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let hash = header.hash();
        let signatures = BTreeSet::from([
            BlockSignature::new(0, SignatureOf::from_hash(kp_spoof.private_key(), hash)),
            BlockSignature::new(1, SignatureOf::from_hash(kp_proxy.private_key(), hash)),
        ]);
        let block = DataBlockBuilder::new(header).build(signatures);

        let err = ValidBlock::is_commit(&block, &topology).unwrap_err();
        assert!(matches!(err, SignatureVerificationError::UnknownSignature));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn is_commit_rejects_set_b_spoof() {
        let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_proxy = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_set_b = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_spoof = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let topology = Topology::new(vec![
            PeerId::new(kp_leader.public_key().clone()),
            PeerId::new(kp_validator.public_key().clone()),
            PeerId::new(kp_proxy.public_key().clone()),
            PeerId::new(kp_set_b.public_key().clone()),
        ]);

        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let hash = header.hash();
        let signatures = BTreeSet::from([
            BlockSignature::new(0, SignatureOf::from_hash(kp_leader.private_key(), hash)),
            BlockSignature::new(1, SignatureOf::from_hash(kp_validator.private_key(), hash)),
            BlockSignature::new(2, SignatureOf::from_hash(kp_proxy.private_key(), hash)),
            BlockSignature::new(3, SignatureOf::from_hash(kp_spoof.private_key(), hash)),
        ]);
        let block = DataBlockBuilder::new(header).build(signatures);

        let err = ValidBlock::is_commit(&block, &topology).unwrap_err();
        assert!(matches!(err, SignatureVerificationError::UnknownSignature));
    }

    #[cfg(feature = "bls")]
    #[test]
    fn commit_with_signers_rejects_invalid_block_signature() {
        let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_proxy = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let topology = Topology::new(vec![
            PeerId::new(kp_leader.public_key().clone()),
            PeerId::new(kp_proxy.public_key().clone()),
        ]);

        // Corrupt the leader signature so the block signatures are no longer trustworthy.
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let hash = header.hash();
        let signatures = BTreeSet::from([
            BlockSignature::new(0, SignatureOf::from_hash(kp_proxy.private_key(), hash)),
            BlockSignature::new(1, SignatureOf::from_hash(kp_proxy.private_key(), hash)),
        ]);
        let block =
            ValidBlock::new_unverified_for_tests(DataBlockBuilder::new(header).build(signatures));
        let signers = BTreeSet::from([
            ValidatorIndex::try_from(0).expect("validator index parses"),
            ValidatorIndex::try_from(1).expect("validator index parses"),
        ]);

        let result = block
            .commit_with_signers(&topology, &signers, false)
            .unpack(|_| {});
        assert!(
            result.is_err(),
            "invalid block signatures must still be rejected even when a QC signer set is present"
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn commit_with_signers_succeeds_with_quorum_and_signatures() {
        let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_proxy = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let topology = Topology::new(vec![
            PeerId::new(kp_leader.public_key().clone()),
            PeerId::new(kp_proxy.public_key().clone()),
        ]);

        let mut block = ValidBlock::new_dummy(kp_leader.private_key());
        block.sign(&kp_proxy, &topology);
        let signers = BTreeSet::from([
            ValidatorIndex::try_from(0).expect("validator index parses"),
            ValidatorIndex::try_from(1).expect("validator index parses"),
        ]);

        let result = block
            .commit_with_signers(&topology, &signers, false)
            .unpack(|_| {});
        assert!(
            result.is_ok(),
            "quorum signatures should commit via QC signer set"
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn commit_with_signers_accepts_quorum_without_proxy_tail_signature() {
        let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_proxy = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_set_b = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let topology = Topology::new(vec![
            PeerId::new(kp_leader.public_key().clone()),
            PeerId::new(kp_validator.public_key().clone()),
            PeerId::new(kp_proxy.public_key().clone()),
            PeerId::new(kp_set_b.public_key().clone()),
        ]);

        // Sign with leader + validator but omit proxy-tail signature to mirror a QC with trimmed
        // block signatures.
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let hash = header.hash();
        let mut signatures = BTreeSet::new();
        signatures.insert(BlockSignature::new(
            0,
            SignatureOf::from_hash(kp_leader.private_key(), hash),
        ));
        signatures.insert(BlockSignature::new(
            1,
            SignatureOf::from_hash(kp_validator.private_key(), hash),
        ));
        signatures.insert(BlockSignature::new(
            3,
            SignatureOf::from_hash(kp_set_b.private_key(), hash),
        ));
        let block =
            ValidBlock::new_unverified_for_tests(DataBlockBuilder::new(header).build(signatures));
        let signers = BTreeSet::from([
            ValidatorIndex::try_from(0).expect("validator index parses"),
            ValidatorIndex::try_from(1).expect("validator index parses"),
            ValidatorIndex::try_from(2).expect("validator index parses"),
        ]);

        let result = block
            .commit_with_signers(&topology, &signers, false)
            .unpack(|_| {});
        assert!(
            result.is_ok(),
            "QC quorum should commit even when block signatures are trimmed"
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn commit_with_signers_allows_block_signer_not_in_qc() {
        let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_extra_validator = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_proxy = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let topology = Topology::new(vec![
            PeerId::new(kp_leader.public_key().clone()),
            PeerId::new(kp_validator.public_key().clone()),
            PeerId::new(kp_extra_validator.public_key().clone()),
            PeerId::new(kp_proxy.public_key().clone()),
        ]);

        // QC captured votes from leader + validator + proxy; block also carries a signature
        // from a validator that is not part of the QC signer set.
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let hash = header.hash();
        let mut signatures = BTreeSet::new();
        signatures.insert(BlockSignature::new(
            0,
            SignatureOf::from_hash(kp_leader.private_key(), hash),
        ));
        signatures.insert(BlockSignature::new(
            1,
            SignatureOf::from_hash(kp_validator.private_key(), hash),
        ));
        signatures.insert(BlockSignature::new(
            2,
            SignatureOf::from_hash(kp_extra_validator.private_key(), hash),
        ));
        signatures.insert(BlockSignature::new(
            3,
            SignatureOf::from_hash(kp_proxy.private_key(), hash),
        ));
        let block =
            ValidBlock::new_unverified_for_tests(DataBlockBuilder::new(header).build(signatures));
        let signers = BTreeSet::from([
            ValidatorIndex::try_from(0).expect("validator index parses"),
            ValidatorIndex::try_from(1).expect("validator index parses"),
            ValidatorIndex::try_from(3).expect("validator index parses"),
        ]);

        let result = block
            .commit_with_signers(&topology, &signers, false)
            .unpack(|_| {});
        assert!(
            result.is_ok(),
            "extra commit-role signatures outside the QC set should not block commit"
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn replace_signatures_restores_previous_on_failure() {
        let kp_leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let kp_proxy = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let topology = Topology::new(vec![
            PeerId::new(kp_leader.public_key().clone()),
            PeerId::new(kp_proxy.public_key().clone()),
        ]);

        let mut vb = ValidBlock::new_dummy(kp_leader.private_key());
        vb.sign(&kp_proxy, &topology);
        let original: BTreeSet<_> = vb.as_ref().signatures().cloned().collect();
        let hash = vb.as_ref().hash();
        let mut invalid = BTreeSet::new();
        invalid.insert(BlockSignature::new(
            1,
            SignatureOf::from_hash(kp_proxy.private_key(), hash),
        ));

        let result = vb.replace_signatures(invalid, &topology).unpack(|_| {});
        assert!(matches!(
            result,
            Err(SignatureVerificationError::LeaderMissing)
        ));
        let restored: BTreeSet<_> = vb.as_ref().signatures().cloned().collect();
        assert_eq!(restored, original);
    }
}
#[cfg(feature = "telemetry")]
fn estimate_transaction_teu(tx: &SignedTransaction) -> u64 {
    use iroha_data_model::transaction::Executable;
    const IVM_TEU_FALLBACK: u64 = 5_000;

    match tx.instructions() {
        Executable::Instructions(batch) => {
            let instructions: Vec<_> = batch.iter().cloned().collect();
            crate::gas::meter_instructions(&instructions)
        }
        Executable::ContractCall(_) => crate::executor::parse_gas_limit(tx.metadata())
            .ok()
            .flatten()
            .unwrap_or(IVM_TEU_FALLBACK),
        Executable::Ivm(bytecode) => match ProgramMetadata::parse(bytecode.as_ref()) {
            Ok(parsed) => {
                let max_cycles = parsed.metadata.max_cycles;
                if max_cycles == 0 {
                    IVM_TEU_FALLBACK
                } else {
                    max_cycles
                }
            }
            Err(err) => {
                iroha_logger::warn!(
                    ?err,
                    "Failed to parse IVM metadata while deriving TEU weight; using fallback"
                );
                IVM_TEU_FALLBACK
            }
        },
        Executable::IvmProved(proved) => crate::gas::meter_instructions(proved.overlay.as_ref()),
    }
}
