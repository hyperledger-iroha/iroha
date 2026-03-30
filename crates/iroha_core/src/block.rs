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
    collections::{BTreeMap, BTreeSet},
    hint::black_box,
    str::FromStr,
    time::Duration,
};

use iroha_crypto::{HashOf, KeyPair, MerkleTree, PublicKey};
#[cfg(feature = "bls")]
use iroha_data_model::metadata::Metadata;
use iroha_data_model::{
    ChainId,
    account::{AccountController, AccountId, rekey::AccountAlias},
    asset::{AssetDefinitionAlias, AssetDefinitionId, AssetId},
    block::{
        consensus::{LaneBlockCommitment, LaneSettlementReceipt},
        *,
    },
    confidential::ConfidentialFeatureDigest,
    consensus::{ConsensusKeyRole, PreviousRosterEvidence, Qc, VALIDATOR_SET_HASH_VERSION_V1},
    da::{
        commitment::{DaCommitmentBundle, DaProofPolicyBundle},
        pin_intent::DaPinIntentBundle,
    },
    domain::DomainId,
    events::prelude::*,
    isi::{InstructionBox, RemoveKeyValueBox, SetKeyValueBox, transfer::TransferBox},
    nexus::{
        AssetHandle, AxtHandleReplayKey, AxtPolicyEntry, AxtProofEnvelope, AxtRejectReason,
        DataSpaceCatalog, DataSpaceId, LaneConfig, LaneFastpqProofMaterial, LaneId,
        LaneRelayEnvelope, ProofBlob, proof_matches_manifest,
    },
    peer::PeerId,
    transaction::{
        SignedTransaction, TransactionEntrypoint, error::TransactionRejectionReason,
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

#[cfg(feature = "bls")]
fn bls_small_pop_from_metadata(
    metadata: &Metadata,
    key: &iroha_data_model::name::Name,
) -> Option<Vec<u8>> {
    bls_pop_from_metadata(metadata, key)
}

#[cfg(feature = "telemetry")]
const PIPELINE_LAYER_WIDTH_THRESHOLDS: [u64; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
const EMPTY_CONFIDENTIAL_FEATURE_DIGEST: ConfidentialFeatureDigest =
    iroha_data_model::confidential::DEFAULT_CONFIDENTIAL_FEATURE_DIGEST;
#[cfg(feature = "telemetry")]
use settlement_router::haircut::LiquidityProfile;
use settlement_router::{MicroXor, policy::BufferStatus};
use sha2::Digest as _;
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
    quarantine_executed: u64,
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
    buffer_snapshot: Option<SettlementBufferSnapshot>,
    source_counts: BTreeMap<AssetDefinitionId, u64>,
}

fn lane_relay_envelopes_for_block(
    block_header: &BlockHeader,
    da_commitment_hash: Option<HashOf<DaCommitmentBundle>>,
    lane_settlement_commitments: &[LaneBlockCommitment],
    lane_summaries: &BTreeMap<LaneId, LaneSummary>,
    commit_qc: Option<&Qc>,
) -> Vec<LaneRelayEnvelope> {
    lane_settlement_commitments
        .iter()
        .map(|commitment| {
            let rbc_bytes_total = lane_summaries
                .get(&commitment.lane_id)
                .map_or(0, |summary| summary.rbc_bytes_total);

            LaneRelayEnvelope::new(
                *block_header,
                commit_qc.cloned(),
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

fn attach_fastpq_proof_material_to_relays(envelopes: &mut [LaneRelayEnvelope]) {
    for envelope in envelopes {
        let verified_at_height = Some(envelope.block_height);
        envelope.fastpq_proof = Some(LaneFastpqProofMaterial {
            proof_digest: envelope.expected_fastpq_proof_digest(verified_at_height),
            verified_at_height,
        });
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

use iroha_data_model::Encode as _;

#[cfg(feature = "telemetry")]
use crate::queue::{LaneSchedulingLimits, QueueLimits};
use crate::{
    da::proof_policy_bundle_hash,
    executor::{charge_fees_for_applied_overlay, configure_executor_fuel_budget},
    kura::{PipelineDagSnapshot, PipelineRecoverySidecar, PipelineTxSnapshot},
    pipeline::{
        gpu::{self, AccessTriplet},
        overlay::TxOverlay,
        smallset::sort_dedup_u32_in_place,
    },
    prelude::*,
    queue::{evaluate_policy_with_catalog, routing_ledger},
    state::{
        State, StateBlock, StatelessValidationContext, WorldReadOnly,
        compute_confidential_feature_digest,
    },
    sumeragi::{VotingBlock, network_topology::Topology, status},
    tx::{AcceptTransactionFail, LaneAssignment, enforce_fraud_policy},
};
type QuarantineClassifier = fn(&iroha_data_model::transaction::SignedTransaction) -> bool;
type CommittedBlockEval = Result<CommittedBlock, (Box<ValidBlock>, Box<BlockValidationError>)>;
type WithCommittedBlockEvents = WithEvents<CommittedBlockEval>;

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
        let wonderland: DomainId = "wonderland".parse().expect("wonderland domain");
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
        let wonderland: DomainId = "wonderland".parse().expect("wonderland domain");
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
        let wonderland: DomainId = "wonderland".parse().expect("wonderland domain");
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
        let alpha: DomainId = "alpha".parse().expect("alpha domain");
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
        let domain_id: DomainId = "ivm".parse().expect("domain");
        let account_id = (*ALICE_ID).clone();
        let alias = AccountAlias::new(
            Name::from_str("gas").expect("alias name"),
            Some(AccountAliasDomain::new(domain_id.name().clone())),
            DataSpaceId::GLOBAL,
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
        let domain_id: DomainId = "ops".parse().expect("domain");
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
        let wonderland: DomainId = "wonderland".parse().expect("wonderland domain");
        let mut world = World::new();
        let selector =
            AccountDomainSelector::from_domain(&wonderland).expect("selector from domain");
        world.domain_selectors.insert(selector, wonderland);
        let world_view = world.view();
        let mut lane = LaneConfig::default();
        lane.metadata
            .insert("settlement.buffer_account".to_owned(), alice.to_string());
        let expected_asset_definition_id = AssetDefinitionId::new(
            "wonderland".parse().expect("domain"),
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

    let transactions = block.transactions_vec().as_slice();
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
                .map(|(idx, tx)| (tx.as_ref().hash_as_entrypoint(), idx, tx))
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
        #[inline]
        pub fn new_preserve_order(transactions: Vec<AcceptedTransaction<'static>>) -> Self {
            Self::new_preserve_order_with_time_source(transactions, TimeSource::new_system())
        }

        /// Create with provided [`TimeSource`] while preserving transaction order.
        pub fn new_preserve_order_with_time_source(
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
                .map(AsRef::as_ref)
                .map(SignedTransaction::creation_time)
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
                .map(AsRef::as_ref)
                .map(SignedTransaction::hash_as_entrypoint)
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
    use iroha_data_model::{
        ChainId,
        events::pipeline::PipelineEventBox,
        nexus::{AxtPolicySnapshot, GroupBinding, HandleBudget, HandleSubject},
        soracloud::{
            SoraRuntimeReceiptV1, SoraServiceHandlerClassV1, SoraServiceHealthStatusV1,
            SoraServiceMailboxMessageV1, SoraServiceRuntimeStateV1,
        },
    };
    use iroha_logger::warn;
    use iroha_primitives::time::TimeSource;

    use super::{
        event::{map_block_err_to_reason, map_sig_err_to_reason},
        *,
    };
    use crate::{
        smartcontracts::ivm::cache::IvmCache,
        soracloud_runtime::{
            SoracloudOrderedMailboxExecutionRequest, SoracloudOrderedMailboxExecutionResult,
            SoracloudRuntimeExecutionError,
        },
        state::{
            StateBlock, StateReadOnly, StateReadOnlyWithTransactions, StateTransaction,
            storage_transactions::TransactionsReadOnly,
        },
        sumeragi::network_topology::Role,
    };

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
        /// Elapsed milliseconds spent executing time triggers.
        pub(crate) execution_tx_time_triggers_ms: u64,
        /// Elapsed milliseconds spent finalizing block results after time triggers.
        pub(crate) execution_tx_finalize_ms: u64,
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
                        mutation.payload_bytes,
                        mutation.payload_commitment,
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

    struct StaticValidationData {
        expected_block_height: usize,
        max_clock_drift: Duration,
        tx_params: iroha_data_model::parameter::TransactionParameters,
        crypto_cfg: Arc<iroha_config::parameters::actual::Crypto>,
        pipeline_cfg: iroha_config::parameters::actual::Pipeline,
        aggregate_lane: LaneId,
    }

    #[allow(clippy::too_many_lines)]
    pub fn validate_axt_envelopes(
        block: &SignedBlock,
        state_block: &StateBlock<'_>,
    ) -> Result<(), BlockValidationError> {
        let snapshot = state_block.axt_policy_snapshot();
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
                if !proof_matches_manifest(proof, dsid, policy.manifest_root) {
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
                }
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
                    if !touch_dsids.contains(&spec.dsid) {
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
                    if !touch_dsids.contains(&fragment.intent.asset_dsid) {
                        return Err(make_env_error(
                            envelope_lane,
                            AxtRejectReason::Descriptor,
                            "missing touch manifest for handle dataspace",
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
            Self::validate_and_record_transactions(&mut block, state_block, None, true);
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
            Self::validate_and_record_transactions(&mut block, state_block, None, true);
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
            skip_tx_signature_validation: bool,
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
                skip_tx_signature_validation,
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
            skip_tx_signature_validation: bool,
        ) -> WithEvents<Result<(ValidBlock, StateBlock<'state>), Error>> {
            let total_start = Instant::now();
            let stateless_start = Instant::now();
            let to_ms = |duration: Duration| -> u64 {
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
            };
            let mut timings = timings;
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
                        let ev = PipelineEventBox::from(BlockEvent {
                            header: block.header(),
                            status: BlockStatus::Rejected(map_block_err_to_reason(&error)),
                        });
                        // No callback available here; rejection is emitted by the caller in
                        // `_with_events` variants. Keep behavior unchanged.
                        let _ = ev; // avoid unused warning if optimized out
                        return WithEvents::new(Err((Box::new(block), Box::new(error))));
                    }
                }
            };
            let committed_heights = {
                let transactions_view = state.transactions.view();
                Self::committed_heights_for_block(&block, &transactions_view)
            };
            let txs_for_cache: Vec<&SignedTransaction> = block.external_transactions().collect();
            let cache_cap = static_data.pipeline_cfg.stateless_cache_cap;
            let cache_enabled = cache_cap > 0 && !block.header().is_genesis();
            let max_clock_drift_ms = static_data.max_clock_drift.as_millis();
            let now_ms = block.header().creation_time().as_millis();
            let mut cached_ok = vec![false; txs_for_cache.len()];
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
            if let Some(context) = cache_context.clone() {
                let mut cache = state.stateless_validation_cache().lock();
                cache.set_cap(cache_cap);
                cache.ensure_context(context);
                for (idx, tx) in txs_for_cache.iter().enumerate() {
                    if cache.get_ok(&tx.hash(), now_ms) {
                        cached_ok[idx] = true;
                    }
                }
            }
            #[cfg(feature = "telemetry")]
            let metrics = Some(&state.telemetry);
            #[cfg(not(feature = "telemetry"))]
            let metrics = ();
            let static_snapshot_start = Instant::now();
            if let Err(error) = Self::validate_static_with_snapshot(
                &block,
                expected_chain_id,
                genesis_account,
                &static_data,
                &committed_heights,
                cache_enabled.then_some(cached_ok.as_slice()),
                skip_tx_signature_validation,
                metrics,
            ) {
                let stateless_elapsed = stateless_start.elapsed();
                if let Some(timings) = timings.as_deref_mut() {
                    timings.stateless_snapshot_ms = to_ms(static_snapshot_start.elapsed());
                }
                record_timings(&mut timings, stateless_elapsed, None);
                let ev = PipelineEventBox::from(BlockEvent {
                    header: block.header(),
                    status: BlockStatus::Rejected(map_block_err_to_reason(&error)),
                });
                // No callback available here; rejection is emitted by the caller in
                // `_with_events` variants. Keep behavior unchanged.
                let _ = ev; // avoid unused warning if optimized out
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Some(context) = cache_context {
                let mut cache = state.stateless_validation_cache().lock();
                cache.set_cap(cache_cap);
                cache.ensure_context(context);
                for (idx, tx) in txs_for_cache.iter().enumerate() {
                    if cached_ok[idx] {
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
                    cache.insert_ok(tx.hash(), expires_at_ms, not_before_ms);
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
                let ev = PipelineEventBox::from(BlockEvent {
                    header: block.header(),
                    status: BlockStatus::Rejected(map_block_err_to_reason(&error)),
                });
                let _ = ev;
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
            if let Some(timings) = timings.as_deref_mut() {
                timings.execution_state_block_ms = to_ms(state_block_start.elapsed());
            }
            let exec_witness_guard = crate::sumeragi::witness::exec_witness_guard();
            let tx_start = Instant::now();
            Self::validate_and_record_transactions(
                &mut block,
                &mut state_block,
                timings.as_deref_mut(),
                true,
            );
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
            mut block: SignedBlock,
            topology: &Topology,
            expected_chain_id: &ChainId,
            genesis_account: &AccountId,
            time_source: &TimeSource,
            state: &'state State,
            voting_block: &mut Option<VotingBlock>,
            soft_fork: bool,
            mut send_events: F,
        ) -> WithEvents<Result<(ValidBlock, StateBlock<'state>), Error>> {
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
                    false,
                ) {
                    Ok(data) => data,
                    Err(error) => {
                        let ev = PipelineEventBox::from(BlockEvent {
                            header: block.header(),
                            status: BlockStatus::Rejected(map_block_err_to_reason(&error)),
                        });
                        send_events(ev);
                        return WithEvents::new(Err((Box::new(block), Box::new(error))));
                    }
                }
            };
            let committed_heights = {
                let transactions_view = state.transactions.view();
                Self::committed_heights_for_block(&block, &transactions_view)
            };
            let txs_for_cache: Vec<&SignedTransaction> = block.external_transactions().collect();
            let cache_cap = static_data.pipeline_cfg.stateless_cache_cap;
            let cache_enabled = cache_cap > 0 && !block.header().is_genesis();
            let max_clock_drift_ms = static_data.max_clock_drift.as_millis();
            let now_ms = block.header().creation_time().as_millis();
            let mut cached_ok = vec![false; txs_for_cache.len()];
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
            if let Some(context) = cache_context.clone() {
                let mut cache = state.stateless_validation_cache().lock();
                cache.set_cap(cache_cap);
                cache.ensure_context(context);
                for (idx, tx) in txs_for_cache.iter().enumerate() {
                    if cache.get_ok(&tx.hash(), now_ms) {
                        cached_ok[idx] = true;
                    }
                }
            }
            #[cfg(feature = "telemetry")]
            let metrics = Some(&state.telemetry);
            #[cfg(not(feature = "telemetry"))]
            let metrics = ();
            if let Err(error) = Self::validate_static_with_snapshot(
                &block,
                expected_chain_id,
                genesis_account,
                &static_data,
                &committed_heights,
                cache_enabled.then_some(cached_ok.as_slice()),
                false,
                metrics,
            ) {
                let ev = PipelineEventBox::from(BlockEvent {
                    header: block.header(),
                    status: BlockStatus::Rejected(map_block_err_to_reason(&error)),
                });
                send_events(ev);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Some(context) = cache_context {
                let mut cache = state.stateless_validation_cache().lock();
                cache.set_cap(cache_cap);
                cache.ensure_context(context);
                for (idx, tx) in txs_for_cache.iter().enumerate() {
                    if cached_ok[idx] {
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
                    cache.insert_ok(tx.hash(), expires_at_ms, not_before_ms);
                }
            }
            // Release block writer before creating new one
            let _ = voting_block.take();
            if let Err(error) = state.ensure_da_indexes_hydrated() {
                let error = BlockValidationError::DaShardCursor(error);
                let ev = PipelineEventBox::from(BlockEvent {
                    header: block.header(),
                    status: BlockStatus::Rejected(map_block_err_to_reason(&error)),
                });
                send_events(ev);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            let mut state_block = if soft_fork {
                state.block_and_revert(block.header())
            } else {
                state.block(block.header())
            };
            let exec_witness_guard = crate::sumeragi::witness::exec_witness_guard();
            Self::validate_and_record_transactions(&mut block, &mut state_block, None, true);
            if let Err(error) = validate_axt_envelopes(&block, &state_block) {
                let ev = PipelineEventBox::from(BlockEvent {
                    header: block.header(),
                    status: BlockStatus::Rejected(map_block_err_to_reason(&error)),
                });
                send_events(ev);
                drop(state_block);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            if let Err(error) = state_block.validate_da_shard_cursors(&block) {
                let ev = PipelineEventBox::from(BlockEvent {
                    header: block.header(),
                    status: BlockStatus::Rejected(map_block_err_to_reason(&error)),
                });
                send_events(ev);
                drop(state_block);
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
                drop(state_block);
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
                drop(state_block);
                return WithEvents::new(Err((Box::new(block), Box::new(error))));
            }
            WithEvents::new(Ok((
                ValidBlock::new_signatures_verified(block),
                state_block,
            )))
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
            let aggregate_lane = nexus.routing_policy.default_lane;

            Ok(StaticValidationData {
                expected_block_height,
                max_clock_drift,
                tx_params,
                crypto_cfg,
                pipeline_cfg,
                aggregate_lane,
            })
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

        fn committed_heights_for_block(
            block: &SignedBlock,
            transactions: &impl TransactionsReadOnly,
        ) -> Vec<Option<NonZeroUsize>> {
            block
                .external_transactions()
                .map(|tx| transactions.get(&tx.hash()))
                .collect()
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
            cached_ok: Option<&[bool]>,
            skip_tx_signature_validation: bool,
            metrics: MetricsRef<'_>,
        ) -> Result<(), BlockValidationError> {
            #[cfg(not(feature = "telemetry"))]
            let () = metrics;
            let _ = static_data.aggregate_lane;
            #[cfg(feature = "telemetry")]
            let metrics = metrics.expect("telemetry enabled");
            #[cfg(all(feature = "telemetry", not(feature = "bls")))]
            let _ = metrics;

            let max_clock_drift = static_data.max_clock_drift;
            let tx_params = static_data.tx_params;
            let expected_block_height = static_data.expected_block_height;
            let pipeline_cfg = &static_data.pipeline_cfg;
            let crypto_cfg = &static_data.crypto_cfg;
            let block_creation_time = block.header().creation_time();
            let txs: Vec<&SignedTransaction> = block.external_transactions().collect();
            debug_assert_eq!(
                committed_heights.len(),
                txs.len(),
                "committed-height snapshot must align with block transaction list",
            );
            if let Some(cached_ok) = cached_ok {
                debug_assert_eq!(
                    cached_ok.len(),
                    txs.len(),
                    "stateless-cache snapshot must align with block transaction list",
                );
            }
            let cached = |idx: usize| {
                cached_ok
                    .and_then(|snapshot| snapshot.get(idx))
                    .copied()
                    .unwrap_or(false)
            };

            // Deterministic pre-verification of transaction signatures by scheme, using
            // runtime pipeline configuration caps. Successful pre-verification allows
            // skipping per-transaction signature checks further below for those schemes.

            // Ed25519 deterministic micro-batching with transcript-derived seed
            let mut ed_preverified = false;
            {
                #[derive(Clone)]
                struct EdItem {
                    idx: usize,
                    pk: [u8; 32],
                    msg: [u8; 32],
                    sig: [u8; 64],
                }
                let mut items: Vec<EdItem> = Vec::new();
                for (i, tx) in txs.iter().enumerate() {
                    if cached(i) {
                        continue;
                    }
                    let AccountController::Single(signatory) = tx.authority().controller() else {
                        continue;
                    };
                    let (algo, pk_bytes) = signatory.to_bytes();
                    if algo != iroha_crypto::Algorithm::Ed25519 {
                        continue;
                    }
                    // message is the 32-byte hash of TransactionPayload
                    let h = iroha_crypto::HashOf::new(tx.payload());
                    let mut msg = [0u8; 32];
                    msg.copy_from_slice(h.as_ref());
                    // signature bytes (64)
                    let sig_bytes = tx.signature().payload().payload();
                    if sig_bytes.len() != 64 || pk_bytes.len() != 32 {
                        return Err(BlockValidationError::TransactionAccept(
                            AcceptTransactionFail::SignatureVerification(
                                crate::tx::SignatureVerificationFail::new(
                                    tx.signature().clone(),
                                    crate::tx::SignatureRejectionCode::MalformedSignature,
                                    "bad signature or key length",
                                ),
                            ),
                        ));
                    }
                    let mut pk = [0u8; 32];
                    pk.copy_from_slice(pk_bytes);
                    let mut sig = [0u8; 64];
                    sig.copy_from_slice(sig_bytes);
                    items.push(EdItem {
                        idx: i,
                        pk,
                        msg,
                        sig,
                    });
                }
                let cap = if pipeline_cfg.signature_batch_max_ed25519 > 0 {
                    pipeline_cfg.signature_batch_max_ed25519
                } else {
                    pipeline_cfg.signature_batch_max
                };
                if cap > 0 && !items.is_empty() {
                    let derive_seed = |slice: &[&EdItem]| -> [u8; 32] {
                        let mut tuples: Vec<Vec<u8>> = slice
                            .iter()
                            .map(|it| {
                                let mut v = Vec::with_capacity(32 + 32 + 64);
                                v.extend_from_slice(&it.pk);
                                v.extend_from_slice(&it.msg);
                                v.extend_from_slice(&it.sig);
                                v
                            })
                            .collect();
                        tuples.sort_unstable();
                        let mut hasher = sha2::Sha256::new();
                        hasher.update(b"iroha:ecc_batch:v1:ed25519");
                        for t in tuples.iter() {
                            hasher.update(t);
                        }
                        let out = hasher.finalize();
                        let mut seed = [0u8; 32];
                        seed.copy_from_slice(&out);
                        seed
                    };
                    let verify_batch_slice = |slice: &[&EdItem]| -> bool {
                        let seed = derive_seed(slice);
                        let msgs: Vec<&[u8]> = slice.iter().map(|it| it.msg.as_slice()).collect();
                        let sigs: Vec<&[u8]> = slice.iter().map(|it| it.sig.as_slice()).collect();
                        let pks: Vec<&[u8]> = slice.iter().map(|it| it.pk.as_slice()).collect();
                        iroha_crypto::ed25519_verify_batch_deterministic(&msgs, &sigs, &pks, seed)
                            .is_ok()
                    };
                    let mut start = 0;
                    while start < items.len() {
                        let end = usize::min(start + cap, items.len());
                        let batch = &items[start..end];
                        let refs: Vec<&EdItem> = batch.iter().collect();
                        if !verify_batch_slice(&refs) {
                            use std::collections::VecDeque;
                            let mut q: VecDeque<Vec<&EdItem>> = VecDeque::new();
                            q.push_back(refs.clone());
                            let mut offending: Option<usize> = None;
                            while let Some(slc) = q.pop_front() {
                                if verify_batch_slice(&slc) {
                                    continue;
                                }
                                if slc.len() == 1 {
                                    offending = Some(slc[0].idx);
                                    break;
                                }
                                let mid = slc.len() / 2;
                                q.push_back(slc[..mid].to_vec());
                                q.push_back(slc[mid..].to_vec());
                            }
                            if let Some(idx) = offending {
                                if let Some(tx) = txs.get(idx) {
                                    return Err(BlockValidationError::TransactionAccept(
                                        AcceptTransactionFail::SignatureVerification(
                                            crate::tx::SignatureVerificationFail::new(
                                                tx.signature().clone(),
                                                crate::tx::SignatureRejectionCode::InvalidSignature,
                                                "ed25519 batch verification failed",
                                            ),
                                        ),
                                    ));
                                }
                            } else {
                                return Err(BlockValidationError::TransactionAccept(
                                    AcceptTransactionFail::SignatureVerification(
                                        crate::tx::SignatureVerificationFail::new(
                                            txs.first().expect("non-empty").signature().clone(),
                                            crate::tx::SignatureRejectionCode::InvalidSignature,
                                            "batch verification failed",
                                        ),
                                    ),
                                ));
                            }
                        }
                        start = end;
                    }
                    ed_preverified = true;
                }
            }

            // secp256k1 deterministic micro-batching (current impl: per-sig verify in batch order)
            let mut secp_preverified = false;
            {
                #[derive(Clone)]
                struct SecpItem {
                    idx: usize,
                    pk: Vec<u8>,
                    msg: [u8; 32],
                    sig: [u8; 64],
                }
                let mut items: Vec<SecpItem> = Vec::new();
                for (i, tx) in txs.iter().enumerate() {
                    if cached(i) {
                        continue;
                    }
                    let AccountController::Single(signatory) = tx.authority().controller() else {
                        continue;
                    };
                    let (algo, pk_bytes) = signatory.to_bytes();
                    if algo != iroha_crypto::Algorithm::Secp256k1 {
                        continue;
                    }
                    let h = iroha_crypto::HashOf::new(tx.payload());
                    let mut msg = [0u8; 32];
                    msg.copy_from_slice(h.as_ref());
                    let sig_bytes = tx.signature().payload().payload();
                    if sig_bytes.len() != 64 {
                        return Err(BlockValidationError::TransactionAccept(
                            AcceptTransactionFail::SignatureVerification(
                                crate::tx::SignatureVerificationFail::new(
                                    tx.signature().clone(),
                                    crate::tx::SignatureRejectionCode::MalformedSignature,
                                    "bad secp256k1 signature length",
                                ),
                            ),
                        ));
                    }
                    let mut sig = [0u8; 64];
                    sig.copy_from_slice(sig_bytes);
                    items.push(SecpItem {
                        idx: i,
                        pk: pk_bytes.to_vec(),
                        msg,
                        sig,
                    });
                }
                let cap = pipeline_cfg.signature_batch_max_secp256k1;
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
                        let msgs: Vec<&[u8]> = slice.iter().map(|it| it.msg.as_slice()).collect();
                        let sigs: Vec<&[u8]> = slice.iter().map(|it| it.sig.as_slice()).collect();
                        let pks: Vec<&[u8]> = slice.iter().map(|it| it.pk.as_slice()).collect();
                        iroha_crypto::secp256k1_verify_batch_deterministic(&msgs, &sigs, &pks, seed)
                            .is_ok()
                    };
                    let mut start = 0;
                    while start < items.len() {
                        let end = usize::min(start + cap, items.len());
                        let batch = &items[start..end];
                        let refs: Vec<&SecpItem> = batch.iter().collect();
                        if !verify_batch_slice(&refs) {
                            use std::collections::VecDeque;
                            let mut q: VecDeque<Vec<&SecpItem>> = VecDeque::new();
                            q.push_back(refs.clone());
                            let mut offending: Option<usize> = None;
                            while let Some(slc) = q.pop_front() {
                                if verify_batch_slice(&slc) {
                                    continue;
                                }
                                if slc.len() == 1 {
                                    offending = Some(slc[0].idx);
                                    break;
                                }
                                let mid = slc.len() / 2;
                                q.push_back(slc[..mid].to_vec());
                                q.push_back(slc[mid..].to_vec());
                            }
                            if let Some(idx) = offending {
                                if let Some(tx) = txs.get(idx) {
                                    return Err(BlockValidationError::TransactionAccept(
                                        AcceptTransactionFail::SignatureVerification(
                                            crate::tx::SignatureVerificationFail::new(
                                                tx.signature().clone(),
                                                crate::tx::SignatureRejectionCode::InvalidSignature,
                                                "secp256k1 batch verification failed",
                                            ),
                                        ),
                                    ));
                                }
                            } else {
                                return Err(BlockValidationError::TransactionAccept(
                                    AcceptTransactionFail::SignatureVerification(
                                        crate::tx::SignatureVerificationFail::new(
                                            txs.first().expect("non-empty").signature().clone(),
                                            crate::tx::SignatureRejectionCode::InvalidSignature,
                                            "batch verification failed",
                                        ),
                                    ),
                                ));
                            }
                        }
                        start = end;
                    }
                    secp_preverified = true;
                }
            }

            // BLS deterministic grouping (feature-gated). Verify per‑signature for now.
            #[cfg(feature = "bls")]
            #[allow(unused_assignments)]
            let mut bls_preverified = false;
            #[cfg(all(feature = "bls", feature = "telemetry"))]
            let (mut bls_agg_same_batches, mut bls_agg_multi_batches, bls_deterministic_batches) =
                (0u64, 0u64, 0u64);
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
                    small: bool, // true => BlsSmall, false => BlsNormal
                }
                static BLS_POP_KEY: LazyLock<iroha_data_model::name::Name> =
                    LazyLock::new(|| "bls_pop".parse().expect("valid metadata key"));
                static BLS_POP_SMALL_KEY: LazyLock<iroha_data_model::name::Name> =
                    LazyLock::new(|| "bls_pop_small".parse().expect("valid metadata key"));
                let mut all_normal_have_pop = true;
                let mut all_small_have_pop = true;
                let mut items_normal: Vec<BlsItem> = Vec::new();
                let mut items_small: Vec<BlsItem> = Vec::new();
                for (i, tx) in txs.iter().enumerate() {
                    if cached(i) {
                        continue;
                    }
                    let AccountController::Single(signatory) = tx.authority().controller() else {
                        continue;
                    };
                    let (algo, pk_bytes) = signatory.to_bytes();
                    let small = match algo {
                        iroha_crypto::Algorithm::BlsNormal => false,
                        iroha_crypto::Algorithm::BlsSmall => true,
                        _ => continue,
                    };
                    let h = iroha_crypto::HashOf::new(tx.payload());
                    let mut msg = [0u8; 32];
                    msg.copy_from_slice(h.as_ref());
                    let sig_bytes = tx.signature().payload().payload().to_vec();
                    let mut pop = None;
                    if small {
                        // Require PoP metadata for BLS-small keys to enable batching
                        if let Some(pop_hex) =
                            bls_small_pop_from_metadata(tx.metadata(), &BLS_POP_SMALL_KEY)
                        {
                            if iroha_crypto::bls_small_pop_verify(signatory, &pop_hex).is_err() {
                                all_small_have_pop = false;
                            } else {
                                pop = Some(pop_hex);
                            }
                        } else {
                            all_small_have_pop = false;
                        }
                    } else {
                        // Require a PoP in transaction metadata to harden against rogue-key
                        // aggregation. Missing/invalid PoP disables batching for normal BLS.
                        if let Some(pop_hex) = bls_pop_from_metadata(tx.metadata(), &BLS_POP_KEY) {
                            if iroha_crypto::bls_normal_pop_verify(signatory, &pop_hex).is_err() {
                                all_normal_have_pop = false;
                            } else {
                                pop = Some(pop_hex);
                            }
                        } else {
                            all_normal_have_pop = false;
                        }
                    }
                    let item = BlsItem {
                        idx: i,
                        pk: signatory.clone(),
                        pk_bytes: pk_bytes.to_vec(),
                        pop,
                        msg,
                        sig: sig_bytes,
                        small,
                    };
                    if small {
                        items_small.push(item);
                    } else {
                        items_normal.push(item);
                    }
                }
                #[cfg(feature = "telemetry")]
                let aggregate_lane = static_data.aggregate_lane;
                let cap = pipeline_cfg.signature_batch_max_bls;
                #[allow(unused_mut)]
                let mut verify_set = |set: &Vec<BlsItem>| -> Result<(), BlockValidationError> {
                    if set.is_empty() {
                        return Ok(());
                    }
                    let mut start = 0usize;
                    while start < set.len() {
                        let end = usize::min(start + cap.max(1), set.len());
                        let batch = &set[start..end];
                        let refs: Vec<&BlsItem> = batch.iter().collect();

                        // Hybrid grouping: same-message groups (size > 1) use fast-aggregate;
                        // singletons (pairwise-distinct messages) use multi-message aggregate.
                        // Deterministic ordering via BTreeMap on message bytes.
                        let mut groups: std::collections::BTreeMap<[u8; 32], Vec<&BlsItem>> =
                            std::collections::BTreeMap::new();
                        for it in &refs {
                            groups.entry(it.msg).or_default().push(*it);
                        }

                        // Helper: verify a same-message slice; return true on success
                        let verify_same = |slc: &[&BlsItem]| -> bool {
                            debug_assert!(!slc.is_empty());
                            let msg = slc[0].msg.as_slice();
                            let sigs: Vec<&[u8]> = slc.iter().map(|it| it.sig.as_slice()).collect();
                            let pks: Vec<&iroha_crypto::PublicKey> =
                                slc.iter().map(|it| &it.pk).collect();
                            let mut pops = Vec::with_capacity(slc.len());
                            for it in slc {
                                let Some(pop) = it.pop.as_ref() else {
                                    return false;
                                };
                                pops.push(pop.as_slice());
                            }
                            if slc[0].small {
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

                        // Helper: bisection for same-message group
                        let bisect_same = |slc: Vec<&BlsItem>| -> Option<usize> {
                            use std::collections::VecDeque;
                            let mut q: VecDeque<Vec<&BlsItem>> = VecDeque::new();
                            q.push_back(slc);
                            let mut offending: Option<usize> = None;
                            while let Some(s) = q.pop_front() {
                                if verify_same(&s) {
                                    continue;
                                }
                                if s.len() == 1 {
                                    offending = Some(s[0].idx);
                                    break;
                                }
                                let mid = s.len() / 2;
                                q.push_back(s[..mid].to_vec());
                                q.push_back(s[mid..].to_vec());
                            }
                            offending
                        };

                        // Collect singletons for multi-message aggregate
                        let mut singletons: Vec<&BlsItem> = Vec::new();
                        let mut group_fail: Option<usize> = None;
                        for (_k, slc) in groups.iter() {
                            if slc.len() == 1 {
                                singletons.push(slc[0]);
                            } else {
                                let ok = verify_same(slc);
                                #[cfg(feature = "telemetry")]
                                {
                                    bls_agg_same_batches = bls_agg_same_batches.saturating_add(1);
                                    metrics.inc_pipeline_sig_bls_result(aggregate_lane, true, ok);
                                }
                                if !ok {
                                    // Bisection within the same-message group
                                    group_fail = bisect_same(slc.clone());
                                    break;
                                }
                            }
                        }
                        if let Some(idx) = group_fail {
                            if let Some(tx) = txs.get(idx) {
                                #[cfg(feature = "telemetry")]
                                {
                                    // Record attempted aggregates even on failure
                                    metrics.set_pipeline_sig_bls_counts(
                                        aggregate_lane,
                                        bls_agg_same_batches,
                                        bls_agg_multi_batches,
                                        bls_deterministic_batches,
                                    );
                                }
                                return Err(BlockValidationError::TransactionAccept(
                                    AcceptTransactionFail::SignatureVerification(
                                        crate::tx::SignatureVerificationFail::new(
                                            tx.signature().clone(),
                                            crate::tx::SignatureRejectionCode::InvalidSignature,
                                            "bls batch verification failed",
                                        ),
                                    ),
                                ));
                            }
                            #[cfg(feature = "telemetry")]
                            {
                                // Record attempted aggregates even on failure
                                metrics.set_pipeline_sig_bls_counts(
                                    aggregate_lane,
                                    bls_agg_same_batches,
                                    bls_agg_multi_batches,
                                    bls_deterministic_batches,
                                );
                            }
                            return Err(BlockValidationError::TransactionAccept(
                                AcceptTransactionFail::SignatureVerification(
                                    crate::tx::SignatureVerificationFail::new(
                                        txs.first().expect("non-empty").signature().clone(),
                                        crate::tx::SignatureRejectionCode::InvalidSignature,
                                        "batch verification failed",
                                    ),
                                ),
                            ));
                        }

                        // Verify multi-message aggregate across singletons (pairwise distinct by construction)
                        if !singletons.is_empty() {
                            let msgs: Vec<&[u8]> =
                                singletons.iter().map(|it| it.msg.as_slice()).collect();
                            let sigs: Vec<&[u8]> =
                                singletons.iter().map(|it| it.sig.as_slice()).collect();
                            let pks: Vec<&[u8]> =
                                singletons.iter().map(|it| it.pk_bytes.as_slice()).collect();
                            let ok = if singletons[0].small {
                                #[cfg(feature = "telemetry")]
                                {
                                    bls_agg_multi_batches = bls_agg_multi_batches.saturating_add(1);
                                }
                                iroha_crypto::bls_small_verify_aggregate_multi_message(
                                    &msgs, &sigs, &pks,
                                )
                                .is_ok()
                            } else {
                                #[cfg(feature = "telemetry")]
                                {
                                    bls_agg_multi_batches = bls_agg_multi_batches.saturating_add(1);
                                }
                                iroha_crypto::bls_normal_verify_aggregate_multi_message(
                                    &msgs, &sigs, &pks,
                                )
                                .is_ok()
                            };
                            #[cfg(feature = "telemetry")]
                            metrics.inc_pipeline_sig_bls_result(aggregate_lane, false, ok);
                            if !ok {
                                // Bisection over singleton set only
                                use std::collections::VecDeque;
                                let mut q: VecDeque<Vec<&BlsItem>> = VecDeque::new();
                                q.push_back(singletons.clone());
                                let mut offending: Option<usize> = None;
                                while let Some(slc) = q.pop_front() {
                                    let msgs: Vec<&[u8]> =
                                        slc.iter().map(|it| it.msg.as_slice()).collect();
                                    let sigs: Vec<&[u8]> =
                                        slc.iter().map(|it| it.sig.as_slice()).collect();
                                    let pks: Vec<&[u8]> =
                                        slc.iter().map(|it| it.pk_bytes.as_slice()).collect();
                                    let ok = if slc[0].small {
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
                                        continue;
                                    }
                                    if slc.len() == 1 {
                                        offending = Some(slc[0].idx);
                                        break;
                                    }
                                    let mid = slc.len() / 2;
                                    q.push_back(slc[..mid].to_vec());
                                    q.push_back(slc[mid..].to_vec());
                                }
                                if let Some(idx) = offending {
                                    if let Some(tx) = txs.get(idx) {
                                        #[cfg(feature = "telemetry")]
                                        {
                                            // Record attempted aggregates even on failure
                                            metrics.set_pipeline_sig_bls_counts(
                                                aggregate_lane,
                                                bls_agg_same_batches,
                                                bls_agg_multi_batches,
                                                bls_deterministic_batches,
                                            );
                                        }
                                        return Err(BlockValidationError::TransactionAccept(
                                            AcceptTransactionFail::SignatureVerification(
                                                crate::tx::SignatureVerificationFail::new(
                                                    tx.signature().clone(),
                                                    crate::tx::SignatureRejectionCode::InvalidSignature,
                                                    "bls batch verification failed",
                                                ),
                                            ),
                                        ));
                                    }
                                } else {
                                    #[cfg(feature = "telemetry")]
                                    {
                                        // Record attempted aggregates even on failure
                                        metrics.set_pipeline_sig_bls_counts(
                                            aggregate_lane,
                                            bls_agg_same_batches,
                                            bls_agg_multi_batches,
                                            bls_deterministic_batches,
                                        );
                                    }
                                    return Err(BlockValidationError::TransactionAccept(
                                        AcceptTransactionFail::SignatureVerification(
                                            crate::tx::SignatureVerificationFail::new(
                                                txs.first().expect("non-empty").signature().clone(),
                                                crate::tx::SignatureRejectionCode::InvalidSignature,
                                                "batch verification failed",
                                            ),
                                        ),
                                    ));
                                }
                            }
                        }
                        start = end;
                    }
                    Ok(())
                };
                let enable_normal_batch = cap > 0 && all_normal_have_pop;
                let enable_small_batch = cap > 0 && all_small_have_pop;
                if enable_normal_batch {
                    verify_set(&items_normal)?;
                }
                if enable_small_batch {
                    verify_set(&items_small)?;
                }
                bls_preverified = (enable_normal_batch && !items_normal.is_empty())
                    || (enable_small_batch && !items_small.is_empty());
                #[cfg(feature = "telemetry")]
                metrics.set_pipeline_sig_bls_counts(
                    aggregate_lane,
                    bls_agg_same_batches,
                    bls_agg_multi_batches,
                    if bls_preverified {
                        0
                    } else {
                        bls_deterministic_batches
                    },
                );
            }

            // PQC (e.g., ML‑DSA Dilithium) deterministic grouping.
            let mut pqc_preverified = false;
            {
                #[derive(Clone)]
                struct PqcItem {
                    idx: usize,
                    pk: Vec<u8>,
                    msg: [u8; 32],
                    sig: Vec<u8>,
                }
                let mut items: Vec<PqcItem> = Vec::new();
                for (i, tx) in txs.iter().enumerate() {
                    if cached(i) {
                        continue;
                    }
                    let AccountController::Single(signatory) = tx.authority().controller() else {
                        continue;
                    };
                    let (algo, pk_bytes) = signatory.to_bytes();
                    if algo != iroha_crypto::Algorithm::MlDsa {
                        continue;
                    }
                    // Compute message hash
                    let h = iroha_crypto::HashOf::new(tx.payload());
                    let mut msg = [0u8; 32];
                    msg.copy_from_slice(h.as_ref());
                    // signature bytes are algorithm-specific length; keep as Vec
                    let sig_bytes = tx.signature().payload().payload().to_vec();
                    items.push(PqcItem {
                        idx: i,
                        pk: pk_bytes.to_vec(),
                        msg,
                        sig: sig_bytes,
                    });
                }
                let cap = pipeline_cfg.signature_batch_max_pqc;
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
                        let msgs: Vec<&[u8]> = slice.iter().map(|it| it.msg.as_slice()).collect();
                        let sigs: Vec<&[u8]> = slice.iter().map(|it| it.sig.as_slice()).collect();
                        let pks: Vec<&[u8]> = slice.iter().map(|it| it.pk.as_slice()).collect();
                        iroha_crypto::pqc_verify_batch_deterministic(&msgs, &sigs, &pks, seed)
                            .is_ok()
                    };
                    let mut start = 0;
                    while start < items.len() {
                        let end = usize::min(start + cap, items.len());
                        let batch = &items[start..end];
                        let refs: Vec<&PqcItem> = batch.iter().collect();
                        if !verify_batch_slice(&refs) {
                            use std::collections::VecDeque;
                            let mut q: VecDeque<Vec<&PqcItem>> = VecDeque::new();
                            q.push_back(refs.clone());
                            let mut offending: Option<usize> = None;
                            while let Some(slc) = q.pop_front() {
                                if verify_batch_slice(&slc) {
                                    continue;
                                }
                                if slc.len() == 1 {
                                    offending = Some(slc[0].idx);
                                    break;
                                }
                                let mid = slc.len() / 2;
                                q.push_back(slc[..mid].to_vec());
                                q.push_back(slc[mid..].to_vec());
                            }
                            if let Some(idx) = offending {
                                if let Some(tx) = txs.get(idx) {
                                    return Err(BlockValidationError::TransactionAccept(
                                        AcceptTransactionFail::SignatureVerification(
                                            crate::tx::SignatureVerificationFail::new(
                                                tx.signature().clone(),
                                                crate::tx::SignatureRejectionCode::InvalidSignature,
                                                "pqc batch verification failed",
                                            ),
                                        ),
                                    ));
                                }
                            } else {
                                return Err(BlockValidationError::TransactionAccept(
                                    AcceptTransactionFail::SignatureVerification(
                                        crate::tx::SignatureVerificationFail::new(
                                            txs.first().expect("non-empty").signature().clone(),
                                            crate::tx::SignatureRejectionCode::InvalidSignature,
                                            "batch verification failed",
                                        ),
                                    ),
                                ));
                            }
                        }
                        start = end;
                    }
                    pqc_preverified = true;
                }
            }

            let mut seen_hashes: std::collections::BTreeSet<HashOf<SignedTransaction>> =
                std::collections::BTreeSet::new();
            let mut entrypoints: Vec<HashOf<TransactionEntrypoint>> = Vec::with_capacity(txs.len());

            for (tx, committed_height) in txs.iter().zip(committed_heights.iter()) {
                let tx_hash = (*tx).hash();
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

                entrypoints.push(tx.hash_as_entrypoint());
            }

            use rayon::prelude::*;

            let is_genesis_block = block.header().is_genesis();
            let validate_tx =
                |(idx, tx): (usize, &&SignedTransaction)| -> Option<BlockValidationError> {
                    if cached(idx) {
                        return None;
                    }
                    if is_genesis_block {
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

                    let signature_override = if skip_tx_signature_validation {
                        Some(Ok(()))
                    } else {
                        match tx.authority().controller() {
                            AccountController::Single(signatory) => {
                                let algo = signatory.algorithm();
                                let skip = (algo == iroha_crypto::Algorithm::Ed25519
                                    && ed_preverified)
                                    || (algo == iroha_crypto::Algorithm::Secp256k1
                                        && secp_preverified)
                                    || ({
                                        #[cfg(feature = "bls")]
                                        {
                                            matches!(
                                                algo,
                                                iroha_crypto::Algorithm::BlsNormal
                                                    | iroha_crypto::Algorithm::BlsSmall
                                            ) && bls_preverified
                                        }
                                        #[cfg(not(feature = "bls"))]
                                        {
                                            false
                                        }
                                    })
                                    || (algo == iroha_crypto::Algorithm::MlDsa && pqc_preverified);
                                skip.then_some(Ok(()))
                            }
                            AccountController::Multisig(_) => None,
                        }
                    };

                    let stateless = if crate::tx::is_heartbeat_transaction(tx) {
                        match signature_override {
                        Some(override_result) => {
                            AcceptedTransaction::validate_heartbeat_with_now_with_signature_result(
                                tx,
                                chain_id,
                                max_clock_drift,
                                tx_params,
                                crypto_cfg.as_ref(),
                                block_creation_time,
                                Some(override_result),
                            )
                        }
                        None => AcceptedTransaction::validate_heartbeat_with_now(
                            tx,
                            chain_id,
                            max_clock_drift,
                            tx_params,
                            crypto_cfg.as_ref(),
                            block_creation_time,
                        ),
                    }
                    } else {
                        AcceptedTransaction::validate_with_now_with_signature_result(
                            tx,
                            chain_id,
                            max_clock_drift,
                            tx_params,
                            crypto_cfg.as_ref(),
                            block_creation_time,
                            signature_override,
                        )
                    };
                    stateless.err().map(BlockValidationError::TransactionAccept)
                };

            let use_parallel = pipeline_cfg.workers != 1 && txs.len() > 1;
            let tx_errors: Vec<Option<BlockValidationError>> = if use_parallel {
                txs.par_iter().enumerate().map(validate_tx).collect()
            } else {
                txs.iter().enumerate().map(validate_tx).collect()
            };
            for maybe_err in tx_errors {
                if let Some(err) = maybe_err {
                    return Err(err);
                }
            }

            let mut merkle_tree: MerkleTree<TransactionEntrypoint> =
                core::iter::empty::<HashOf<TransactionEntrypoint>>().collect();
            for entry in entrypoints {
                merkle_tree.add(entry);
            }

            let expected_merkle_root = merkle_tree.root();
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
            )?;
            let committed_heights = Self::committed_heights_for_block(block, state.transactions());
            let txs_for_cache: Vec<&SignedTransaction> = block.external_transactions().collect();
            let cache_cap = static_data.pipeline_cfg.stateless_cache_cap;
            let cache_enabled = cache_cap > 0 && !block.header().is_genesis();
            let max_clock_drift_ms = static_data.max_clock_drift.as_millis();
            let now_ms = block.header().creation_time().as_millis();
            let mut cached_ok = vec![false; txs_for_cache.len()];
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
            if let Some(context) = cache_context.clone() {
                let mut cache = state.stateless_validation_cache().lock();
                cache.set_cap(cache_cap);
                cache.ensure_context(context);
                for (idx, tx) in txs_for_cache.iter().enumerate() {
                    if cache.get_ok(&tx.hash(), now_ms) {
                        cached_ok[idx] = true;
                    }
                }
            }
            #[cfg(feature = "telemetry")]
            let metrics = Some(state.metrics());
            #[cfg(not(feature = "telemetry"))]
            let metrics = ();
            Self::validate_static_with_snapshot(
                block,
                chain_id,
                genesis_account,
                &static_data,
                &committed_heights,
                cache_enabled.then_some(cached_ok.as_slice()),
                false,
                metrics,
            )?;
            if let Some(context) = cache_context {
                let mut cache = state.stateless_validation_cache().lock();
                cache.set_cap(cache_cap);
                cache.ensure_context(context);
                for (idx, tx) in txs_for_cache.iter().enumerate() {
                    if cached_ok[idx] {
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
                    cache.insert_ok(tx.hash(), expires_at_ms, not_before_ms);
                }
            }
            Ok(())
        }

        /// Validate each transaction in the block, apply resulting state changes,
        /// and record results back into the block.
        ///
        /// Must be called with a **block that is _assumed_ to be valid**.
        /// When `skip_stateless_checks` is true, signature/limit validation is skipped under the
        /// assumption that the static snapshot validation already passed.
        #[allow(
            clippy::too_many_lines,
            clippy::explicit_iter_loop,
            clippy::option_if_let_else,
            clippy::manual_flatten,
            clippy::option_as_ref_cloned,
            clippy::needless_option_as_deref
        )]
        fn validate_and_record_transactions(
            block: &mut SignedBlock,
            state_block: &mut StateBlock<'_>,
            timings: Option<&mut ValidationTimings>,
            skip_stateless_checks: bool,
        ) {
            use rayon::prelude::*;

            use crate::pipeline::{
                access::{AccessSetSource, IvmStrategy, derive_for_transaction_with_source},
                overlay::{TxOverlay, build_overlay_for_transaction_with_accounts_zk},
            };

            let to_ms = |duration: Duration| -> u64 {
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
            };
            let mut timings = timings;
            let _ivm_cache = IvmCache::new();
            if block.has_results() {
                if let Some(snapshot) = block.axt_policy_snapshot() {
                    state_block.install_axt_policy_snapshot(snapshot);
                }
            }

            let height = block.header().height().get();

            // Start a new witness window for this block (SBV‑AM prototype)
            crate::sumeragi::witness::start_block();

            // Prepare scheduling: collect transactions, their access sets, and hashes
            let txs: Vec<&SignedTransaction> = block.external_transactions().collect();
            #[allow(clippy::disallowed_types)]
            let tx_hashes: std::collections::HashSet<_> = txs.iter().map(|tx| tx.hash()).collect();
            let height_u64 = block.header().height().get();
            let height_usize = height_u64.try_into().expect("block height fits usize");
            let block_height =
                std::num::NonZeroUsize::new(height_usize).expect("block height greater than zero");
            state_block
                .transactions
                .insert_block(tx_hashes, block_height);
            // Strategy controlled by configuration (no env reliance)
            let dynamic_prepass = state_block.pipeline.dynamic_prepass;
            let strategy = if dynamic_prepass {
                IvmStrategy::DynamicThenConservative
            } else {
                IvmStrategy::Conservative
            };
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
            let dataspace_catalog = &state_block.nexus.dataspace_catalog;
            let lane_catalog = &state_block.nexus.lane_catalog;
            let routing_policy = &state_block.nexus.routing_policy;
            let fraud_cfg = &state_block.fraud_monitoring;
            let cache_cap = state_block.pipeline.stateless_cache_cap;
            let cache_enabled = cache_cap > 0 && !is_genesis_block;
            let now_ms = block_creation_time.as_millis();
            let max_clock_drift_ms = max_clock_drift.as_millis();
            let mut cached_ok = vec![false; txs.len()];
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
                for (idx, tx) in txs.iter().enumerate() {
                    if cache.get_ok(&tx.hash(), now_ms) {
                        cached_ok[idx] = true;
                    }
                }
            }
            let routing_results: Vec<_> = if workers > 1 {
                if let Some(pool) = pool.as_ref() {
                    pool.install(|| {
                        txs.par_iter()
                            .map(|tx| {
                                let accepted = crate::tx::AcceptedTransaction::new_unchecked(
                                    Cow::Borrowed(*tx),
                                );
                                evaluate_policy_with_catalog(
                                    routing_policy,
                                    lane_catalog,
                                    dataspace_catalog,
                                    &accepted,
                                )
                            })
                            .collect()
                    })
                } else {
                    txs.par_iter()
                        .map(|tx| {
                            let accepted =
                                crate::tx::AcceptedTransaction::new_unchecked(Cow::Borrowed(*tx));
                            evaluate_policy_with_catalog(
                                routing_policy,
                                lane_catalog,
                                dataspace_catalog,
                                &accepted,
                            )
                        })
                        .collect()
                }
            } else {
                txs.iter()
                    .map(|tx| {
                        let accepted =
                            crate::tx::AcceptedTransaction::new_unchecked(Cow::Borrowed(*tx));
                        evaluate_policy_with_catalog(
                            routing_policy,
                            lane_catalog,
                            dataspace_catalog,
                            &accepted,
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
            let mut signature_overrides: Vec<
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
                    #[derive(Clone)]
                    struct EdItem {
                        idx: usize,
                        pk: [u8; 32],
                        msg: [u8; 32],
                        sig: [u8; 64],
                    }
                    let mut items: Vec<EdItem> = Vec::new();
                    for (idx, tx) in txs.iter().enumerate() {
                        if cached_ok[idx] {
                            continue;
                        }
                        let AccountController::Single(signatory) = tx.authority().controller()
                        else {
                            continue;
                        };
                        if signatory.algorithm() != iroha_crypto::Algorithm::Ed25519 {
                            continue;
                        }
                        let (_algo, pk_bytes) = signatory.to_bytes();
                        if pk_bytes.len() != 32 {
                            signature_overrides[idx] =
                                Some(malformed_signature(tx, "bad signature or key length"));
                            continue;
                        }
                        let sig_bytes = tx.signature().payload().payload();
                        if sig_bytes.len() != 64 {
                            signature_overrides[idx] =
                                Some(malformed_signature(tx, "bad signature or key length"));
                            continue;
                        }
                        let h = iroha_crypto::HashOf::new(tx.payload());
                        let mut msg = [0u8; 32];
                        msg.copy_from_slice(h.as_ref());
                        let mut pk = [0u8; 32];
                        pk.copy_from_slice(pk_bytes);
                        let mut sig = [0u8; 64];
                        sig.copy_from_slice(sig_bytes);
                        items.push(EdItem { idx, pk, msg, sig });
                    }
                    let cap = if state_block.pipeline.signature_batch_max_ed25519 > 0 {
                        state_block.pipeline.signature_batch_max_ed25519
                    } else {
                        state_block.pipeline.signature_batch_max
                    };
                    if cap > 0 && !items.is_empty() {
                        let derive_seed = |slice: &[&EdItem]| -> [u8; 32] {
                            let mut tuples: Vec<Vec<u8>> = slice
                                .iter()
                                .map(|it| {
                                    let mut v = Vec::with_capacity(32 + 32 + 64);
                                    v.extend_from_slice(&it.pk);
                                    v.extend_from_slice(&it.msg);
                                    v.extend_from_slice(&it.sig);
                                    v
                                })
                                .collect();
                            tuples.sort_unstable();
                            let mut hasher = sha2::Sha256::new();
                            hasher.update(b"iroha:ecc_batch:v1:ed25519");
                            for t in tuples.iter() {
                                hasher.update(t);
                            }
                            let out = hasher.finalize();
                            let mut seed = [0u8; 32];
                            seed.copy_from_slice(&out);
                            seed
                        };
                        let verify_batch_slice = |slice: &[&EdItem]| -> bool {
                            let seed = derive_seed(slice);
                            let msgs: Vec<&[u8]> =
                                slice.iter().map(|it| it.msg.as_slice()).collect();
                            let sigs: Vec<&[u8]> =
                                slice.iter().map(|it| it.sig.as_slice()).collect();
                            let pks: Vec<&[u8]> = slice.iter().map(|it| it.pk.as_slice()).collect();
                            iroha_crypto::ed25519_verify_batch_deterministic(
                                &msgs, &sigs, &pks, seed,
                            )
                            .is_ok()
                        };
                        let mut start = 0;
                        while start < items.len() {
                            let end = usize::min(start + cap, items.len());
                            let batch = &items[start..end];
                            let refs: Vec<&EdItem> = batch.iter().collect();
                            if verify_batch_slice(&refs) {
                                for it in batch {
                                    signature_overrides[it.idx] = Some(Ok(()));
                                }
                            } else {
                                for it in batch {
                                    signature_overrides[it.idx] =
                                        Some(signature_result_for_tx(txs[it.idx]));
                                }
                            }
                            start = end;
                        }
                    }
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
                        if cached_ok[idx] {
                            continue;
                        }
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
                            signature_overrides[idx] =
                                Some(malformed_signature(tx, "bad secp256k1 signature length"));
                            continue;
                        }
                        let h = iroha_crypto::HashOf::new(tx.payload());
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
                                    signature_overrides[it.idx] = Some(Ok(()));
                                }
                            } else {
                                for it in batch {
                                    signature_overrides[it.idx] =
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
                        if cached_ok[idx] {
                            continue;
                        }
                        let AccountController::Single(signatory) = tx.authority().controller()
                        else {
                            continue;
                        };
                        if signatory.algorithm() != iroha_crypto::Algorithm::MlDsa {
                            continue;
                        }
                        let (_algo, pk_bytes) = signatory.to_bytes();
                        let h = iroha_crypto::HashOf::new(tx.payload());
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
                                    signature_overrides[it.idx] = Some(Ok(()));
                                }
                            } else {
                                for it in batch {
                                    signature_overrides[it.idx] =
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
                        if cached_ok[idx] {
                            continue;
                        }
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
                        let h = iroha_crypto::HashOf::new(tx.payload());
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
                                    signature_overrides[it.idx] = Some(Ok(()));
                                }
                            } else {
                                for it in group {
                                    signature_overrides[it.idx] =
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
                                    signature_overrides[it.idx] = Some(Ok(()));
                                }
                            } else {
                                for it in singletons {
                                    signature_overrides[it.idx] =
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
                            dataspace_catalog,
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
                    if cached_ok[idx] {
                        return None;
                    }
                    if skip_stateless_checks {
                        return None;
                    }
                    let signature_override = signature_overrides
                        .get(idx)
                        .and_then(|override_result| override_result.as_ref().cloned());
                    let stateless = if is_heartbeat {
                        match signature_override {
                            Some(override_result) => {
                                AcceptedTransaction::validate_heartbeat_with_now_with_signature_result(
                                    tx,
                                    &chain_id,
                                    max_clock_drift,
                                    tx_params,
                                    crypto_cfg.as_ref(),
                                    block_creation_time,
                                    Some(override_result),
                                )
                            }
                            None => AcceptedTransaction::validate_heartbeat_with_now(
                                tx,
                                &chain_id,
                                max_clock_drift,
                                tx_params,
                                crypto_cfg.as_ref(),
                                block_creation_time,
                            ),
                        }
                    } else {
                        AcceptedTransaction::validate_with_now_with_signature_result(
                            tx,
                            &chain_id,
                            max_clock_drift,
                            tx_params,
                            crypto_cfg.as_ref(),
                            block_creation_time,
                            signature_override,
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
                    if cached_ok[idx] || stateless_rejections[idx].is_some() {
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
                    cache.insert_ok(tx.hash(), expires_at_ms, not_before_ms);
                }
            }
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), stateless_start) {
                timings.execution_tx_stateless_ms = to_ms(start.elapsed());
            }
            #[cfg(feature = "telemetry")]
            let t_access_start = Instant::now();
            let access_start = timings.as_ref().map(|_| Instant::now());
            let derived: Vec<_> = if dynamic_prepass {
                if workers > 1 {
                    if let Some(pool) = pool.as_ref() {
                        pool.install(|| {
                            txs.par_iter()
                                .enumerate()
                                .map(|(idx, tx)| {
                                    if stateless_rejections[idx].is_some() {
                                        (crate::pipeline::access::AccessSet::new(), None)
                                    } else {
                                        derive_for_transaction_with_source(
                                            tx,
                                            Some(&*state_block),
                                            strategy,
                                        )
                                    }
                                })
                                .collect()
                        })
                    } else {
                        txs.par_iter()
                            .enumerate()
                            .map(|(idx, tx)| {
                                if stateless_rejections[idx].is_some() {
                                    (crate::pipeline::access::AccessSet::new(), None)
                                } else {
                                    derive_for_transaction_with_source(
                                        tx,
                                        Some(&*state_block),
                                        strategy,
                                    )
                                }
                            })
                            .collect()
                    }
                } else {
                    txs.iter()
                        .enumerate()
                        .map(|(idx, tx)| {
                            if stateless_rejections[idx].is_some() {
                                (crate::pipeline::access::AccessSet::new(), None)
                            } else {
                                derive_for_transaction_with_source(
                                    tx,
                                    Some(&*state_block),
                                    strategy,
                                )
                            }
                        })
                        .collect()
                }
            } else {
                txs.iter()
                    .enumerate()
                    .map(|(idx, tx)| {
                        if stateless_rejections[idx].is_some() {
                            (crate::pipeline::access::AccessSet::new(), None)
                        } else {
                            derive_for_transaction_with_source(tx, Some(&*state_block), strategy)
                        }
                    })
                    .collect()
            };
            let mut access_sources: Vec<Option<AccessSetSource>> =
                Vec::with_capacity(derived.len());
            let mut access: Vec<crate::pipeline::access::AccessSet> =
                Vec::with_capacity(derived.len());
            for (set, source) in derived {
                access.push(set);
                access_sources.push(source);
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
            let call_hashes: Vec<_> = txs.iter().map(|tx| tx.hash_as_entrypoint()).collect();
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

            // Parallel overlay construction from configuration
            let build_parallel = state_block.pipeline.parallel_overlay;
            let overlay_start = timings.as_ref().map(|_| Instant::now());
            // Quarantine lane: overlays for quarantined transactions will be built
            // sequentially with per-tx caps below. Normal lane follows configured parallelism.
            #[cfg(feature = "telemetry")]
            let t_overlay_start = Instant::now();
            let mut overlays: Vec<
                Result<Arc<TxOverlay>, crate::pipeline::overlay::OverlayBuildError>,
            > = vec![Err(crate::pipeline::overlay::OverlayBuildError::IvmHeaderParse); txs.len()];
            // Normal lane overlays
            if build_parallel && workers > 1 {
                if let Some(pool) = pool.as_ref() {
                    pool.install(|| {
                        overlays.par_iter_mut().enumerate().for_each(|(i, slot)| {
                            if !is_quarantine[i] && stateless_rejections[i].is_none() {
                                let tx = &txs[i];
                                let metadata = crate::pipeline::overlay::resolve_streaming_metadata(
                                    state_block,
                                    tx.authority(),
                                );
                                *slot = build_overlay_for_transaction_with_accounts_zk(
                                    tx,
                                    accounts_snapshot.as_ref(),
                                    state_block,
                                    state_block.zk().halo2.enabled
                                        || state_block.zk().stark.enabled,
                                    &block.header(),
                                    metadata,
                                )
                                .map(Arc::new);
                            }
                        })
                    });
                } else {
                    overlays.par_iter_mut().enumerate().for_each(|(i, slot)| {
                        if !is_quarantine[i] && stateless_rejections[i].is_none() {
                            let tx = &txs[i];
                            let metadata = crate::pipeline::overlay::resolve_streaming_metadata(
                                state_block,
                                tx.authority(),
                            );
                            *slot = build_overlay_for_transaction_with_accounts_zk(
                                tx,
                                accounts_snapshot.as_ref(),
                                state_block,
                                state_block.zk().halo2.enabled || state_block.zk().stark.enabled,
                                &block.header(),
                                metadata,
                            )
                            .map(Arc::new);
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
                        overlays[i] = build_overlay_for_transaction_with_accounts_zk(
                            tx,
                            accounts_snapshot.as_ref(),
                            state_block,
                            state_block.zk().halo2.enabled || state_block.zk().stark.enabled,
                            &block.header(),
                            metadata,
                        )
                        .map(Arc::new);
                    }
                }
            }
            // Quarantine lane overlays (caps): build only for allowed; mark overflow as error
            for (i, tx) in txs.iter().enumerate() {
                if is_quarantine[i] {
                    if quarantine_overflow.contains(&i) {
                        overlays[i] =
                            Err(crate::pipeline::overlay::OverlayBuildError::QuarantineOverflow);
                    } else if quarantine_allowed.contains(&i) && stateless_rejections[i].is_none() {
                        let metadata = crate::pipeline::overlay::resolve_streaming_metadata(
                            state_block,
                            tx.authority(),
                        );
                        overlays[i] =
                            crate::pipeline::overlay::build_overlay_for_transaction_quarantine(
                                tx,
                                accounts_snapshot.as_ref(),
                                q_cycle_cap,
                                q_time_cap,
                                upper_cycle_cap,
                                metadata,
                            )
                            .map(Arc::new);
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
                let txs_sidecar: Vec<PipelineTxSnapshot> = txs
                    .iter()
                    .zip(access.iter())
                    .map(|(tx, aset)| PipelineTxSnapshot {
                        hash: tx.hash_as_entrypoint(),
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
            let mut apply_prep_ms = 0u64;
            let mut apply_detached_ms = 0u64;
            let mut apply_merge_ms = 0u64;
            let mut apply_fallback_ms = 0u64;
            let mut apply_quarantine_ms = 0u64;
            let mut apply_sequential_ms = 0u64;
            let mut lane_summaries: BTreeMap<LaneId, LaneSummary> = BTreeMap::new();
            let mut dataspace_summaries: BTreeMap<(LaneId, DataSpaceId), u64> = BTreeMap::new();
            let mut pending_settlements = state_block.drain_settlement_records();

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

                    let bytes = txs[idx].encode();
                    summary.rbc_bytes_total =
                        summary.rbc_bytes_total.saturating_add(bytes.len() as u64);

                    if let Some(record) = pending_settlements.remove(&txs[idx].hash()) {
                        let builder = lane_settlement_builders
                            .entry((decision.lane_id, decision.dataspace_id))
                            .or_default();
                        builder.tx_count = builder.tx_count.saturating_add(1);
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
                    for tx in &txs {
                        let hash = tx.hash();
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

                let lane_activity_snapshot: Vec<status::LaneActivitySnapshot> = lane_summaries
                    .iter()
                    .map(|(lane_id, summary)| status::LaneActivitySnapshot {
                        lane_id: lane_id.as_u32(),
                        tx_vertices: summary.tx_vertices,
                        tx_edges: summary.tx_edges,
                        overlay_count: summary.overlay_count,
                        overlay_instr_total: summary.overlay_instr_total,
                        overlay_bytes_total: summary.overlay_bytes_total,
                        rbc_chunks: summary.rbc_chunks,
                        rbc_bytes_total: summary.rbc_bytes_total,
                    })
                    .collect();
                status::set_lane_activity_snapshot(lane_activity_snapshot);

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
                let commit_qc = state_block
                    .world
                    .commit_qcs()
                    .get(&block_header.hash())
                    .cloned();
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
                    commit_qc.as_ref(),
                );
                attach_manifest_roots_to_relays(&mut lane_relay_envelopes, &manifest_roots);
                attach_fastpq_proof_material_to_relays(&mut lane_relay_envelopes);
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

                use crate::state::DetachedStateTransactionDelta;

                #[derive(Clone)]
                struct PreparedEntry {
                    idx: usize,
                    authority: AccountId,
                    chunk_size: usize,
                    _log_only: bool,
                }

                // Compute conflict-free layers
                // Compute conflict-free layers per DSF component and merge deterministically
                let mut per_comp_layers: Vec<Vec<Vec<usize>>> =
                    Vec::with_capacity(components.len());
                let mut max_depth = 0usize;
                for comp in components.iter() {
                    let mut indeg_c = indeg.clone();
                    let mut ready_c: Vec<usize> = Vec::new();
                    for &i in comp.iter() {
                        if indeg_c[i] == 0 {
                            ready_c.push(i);
                        }
                    }
                    let mut comp_layers: Vec<Vec<usize>> = Vec::new();
                    while !ready_c.is_empty() {
                        ready_c.sort_unstable_by(|&a, &b| {
                            call_hashes[a].cmp(&call_hashes[b]).then_with(|| a.cmp(&b))
                        });
                        let layer_c: Vec<usize> = ready_c.split_off(0);
                        let mut wave: Vec<usize> = Vec::with_capacity(layer_c.len());
                        for &i in &layer_c {
                            let (start, end) = (row_offsets[i], row_offsets[i + 1]);
                            for &v in &cols[start..end] {
                                indeg_c[v] = indeg_c[v].saturating_sub(1);
                                if indeg_c[v] == 0 && comp.binary_search(&v).is_ok() {
                                    ready_c.push(v);
                                }
                            }
                            wave.push(i);
                        }
                        comp_layers.push(wave);
                    }
                    max_depth = max_depth.max(comp_layers.len());
                    per_comp_layers.push(comp_layers);
                }
                let mut layers: Vec<Vec<usize>> = Vec::with_capacity(max_depth);
                for d in 0..max_depth {
                    let mut wave: Vec<usize> = Vec::new();
                    for comp_layers in per_comp_layers.iter() {
                        if d < comp_layers.len() {
                            wave.extend_from_slice(&comp_layers[d]);
                        }
                    }
                    if !wave.is_empty() {
                        wave.sort_by_key(|&i| (call_hashes[i], i));
                        layers.push(wave);
                    }
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
                                        if max_bytes > 0 && overlay.byte_size() as u64 > max_bytes {
                                            return Err((
                                                idx,
                                                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                                    iroha_data_model::ValidationFail::NotPermitted(format!(
                                                        "overlay exceeds max bytes: {} > {max_bytes}",
                                                        overlay.byte_size()
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
                                    if max_bytes > 0 && overlay.byte_size() as u64 > max_bytes {
                                        return Err((
                                            idx,
                                            iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                                iroha_data_model::ValidationFail::NotPermitted(format!(
                                                    "overlay exceeds max bytes: {} > {max_bytes}",
                                                    overlay.byte_size()
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
                            if max_bytes > 0 && overlay.byte_size() as u64 > max_bytes {
                                return Err((idx, iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                    iroha_data_model::ValidationFail::NotPermitted(format!("overlay exceeds max bytes: {} > {max_bytes}", overlay.byte_size())),
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
                    let requires_fee_postprocessing =
                        |tx: &iroha_data_model::transaction::SignedTransaction| {
                            if !state_block.pipeline.gas.accepted_assets.is_empty() {
                                return true;
                            }
                            if tx.metadata().get("gas_asset_id").is_some() {
                                return true;
                            }
                            if state_block.nexus.enabled {
                                let fees = &state_block.nexus.fees;
                                if fees.base_fee > Numeric::zero()
                                    || fees.per_byte_fee > Numeric::zero()
                                    || fees.per_instruction_fee > Numeric::zero()
                                    || fees.per_gas_unit_fee > Numeric::zero()
                                {
                                    return true;
                                }
                            }
                            false
                        };
                    let eval_detached = |p: &PreparedEntry| {
                        if let Some(Ok(ovl)) = overlays.get(p.idx) {
                            let tx = txs[p.idx];
                            if matches!(
                                &*state_block.world.executor,
                                crate::executor::Executor::UserProvided(_)
                            ) {
                                return (p.idx, None);
                            }
                            if requires_fee_postprocessing(tx) {
                                return (p.idx, None);
                            }
                            if ovl.has_durable_state_changes() {
                                return (p.idx, None);
                            }
                            let mut delta = DetachedStateTransactionDelta::default();
                            let mut unsupported = false;
                            let mut reject: Option<TransactionRejectionReason> = None;
                            for instr in ovl.instructions() {
                                if asset_transfer_target(instr).is_some() {
                                    unsupported = true;
                                    break;
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
                                        (p.idx, None)
                                    } else {
                                        (p.idx, Some(Ok(delta)))
                                    }
                                },
                                |r| (p.idx, Some(Err(r))),
                            )
                        } else {
                            (p.idx, None)
                        }
                    };
                    let deltas_vec: Vec<(
                        usize,
                        Option<Result<DetachedStateTransactionDelta, TransactionRejectionReason>>,
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
                    for (idx, maybe) in deltas_vec {
                        if idx < n {
                            deltas[idx] = maybe;
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
                            }
                            let tx = txs[idx];
                            let hash = tx.hash_as_entrypoint();
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
                            if max_bytes > 0 && overlay.byte_size() as u64 > max_bytes {
                                record_amx_abort(state_block_mut, idx, "prepare");
                                return Err(TransactionRejectionReason::Validation(
                                    iroha_data_model::ValidationFail::NotPermitted(format!(
                                        "overlay exceeds max bytes: {} > {}",
                                        overlay.byte_size(),
                                        max_bytes
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
                                Err(e) => Err(TransactionRejectionReason::Validation(e)),
                                Ok(()) => {
                                    if let Err(err) = charge_fees_for_applied_overlay(
                                        &mut state_tx,
                                        &authority,
                                        tx,
                                        overlay.as_ref(),
                                    ) {
                                        Err(TransactionRejectionReason::Validation(err))
                                    } else {
                                        match state_tx.execute_data_triggers_dfs(&authority) {
                                            Err(err) => Err(err),
                                            Ok(trigger_sequence) => {
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
                                // ensure authority exists
                                let missing_authority = {
                                    let tx = txs[p.idx];
                                    let st = state_block.transaction();
                                    missing_authority_requires_rejection(
                                        &st,
                                        tx,
                                        &p.authority,
                                        tx.instructions().instruction_count() as usize,
                                        block.header().is_genesis(),
                                    )
                                };
                                if missing_authority {
                                    record_amx_abort(state_block, p.idx, "commit");
                                    let tx = txs[p.idx];
                                    let hash = tx.hash_as_entrypoint();
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
                                let tx = txs[p.idx];
                                let hash = tx.hash_as_entrypoint();
                                match delta.merge_into(state_block, &p.authority) {
                                    Ok(trigger_sequence) => {
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
                                        let hash = tx.hash_as_entrypoint();
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
                        let hash = tx.hash_as_entrypoint();
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
                        if max_bytes > 0 && overlay.byte_size() as u64 > max_bytes {
                            record_result(
                                idx,
                                Err(
                                    iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                        iroha_data_model::ValidationFail::NotPermitted(format!(
                                            "overlay exceeds max bytes: {} > {}",
                                            overlay.byte_size(), max_bytes
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
                                        Err(e) => Err(
                                            iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                                e,
                                            ),
                                        ),
                                        Ok(()) => {
                                            if let Err(err) = charge_fees_for_applied_overlay(
                                                &mut state_tx,
                                                &authority,
                                                tx,
                                                overlay.as_ref(),
                                            ) {
                                                Err(
                                                    iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                                        err,
                                                    ),
                                                )
                                            } else {
                                                match state_tx.execute_data_triggers_dfs(&authority)
                                                {
                                                    Err(err) => Err(err),
                                                    Ok(trigger_sequence) => {
                                                        state_tx.apply();
                                                        Ok(trigger_sequence)
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
                    let hash = tx.hash_as_entrypoint();
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
                    if max_bytes > 0 && overlay.byte_size() as u64 > max_bytes {
                        record_amx_abort(state_block, idx, "prepare");
                        record_result(
                            idx,
                            Err(
                                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                    iroha_data_model::ValidationFail::NotPermitted(format!(
                                        "overlay exceeds max bytes: {} > {}",
                                        overlay.byte_size(), max_bytes
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
                                match overlay.apply_with_chunk(&mut state_tx, &authority, chunk_size) {
                                    Err(e) => Err(
                                        iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                            e,
                                        ),
                                    ),
                                    Ok(()) => {
                                        if let Err(err) = charge_fees_for_applied_overlay(
                                            &mut state_tx,
                                            &authority,
                                            tx,
                                            overlay.as_ref(),
                                        ) {
                                            Err(
                                                iroha_data_model::transaction::error::TransactionRejectionReason::Validation(
                                                    err,
                                                ),
                                            )
                                        } else {
                                            match state_tx.execute_data_triggers_dfs(&authority) {
                                                Err(err) => Err(err),
                                                Ok(trigger_sequence) => {
                                                    state_tx.apply();
                                                    Ok(trigger_sequence)
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
                let quarantine_total: u64 =
                    lane_summaries.values().map(|s| s.quarantine_executed).sum();

                telemetry.set_pipeline_detached_prepared(aggregate_lane, det_prepared_total);
                telemetry.set_pipeline_detached_merged(aggregate_lane, det_merged_total);
                telemetry.set_pipeline_detached_fallback(aggregate_lane, det_fallback_total);
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

            let time_triggers_start = timings.as_ref().map(|_| Instant::now());
            let (time_trgs, mut time_trg_hashes, mut time_trg_results) =
                state_block.execute_time_triggers(&block.header());
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), time_triggers_start) {
                timings.execution_tx_time_triggers_ms = to_ms(start.elapsed());
            }
            execute_soracloud_mailbox_runtime(state_block);
            let finalize_start = timings.as_ref().map(|_| Instant::now());
            let mut fastpq_entry_dataspaces = std::collections::BTreeMap::new();
            for (idx, entry_hash) in call_hashes.iter().enumerate() {
                fastpq_entry_dataspaces.insert(
                    iroha_crypto::Hash::from(*entry_hash),
                    routing_decisions[idx].dataspace_id,
                );
            }
            for entry_hash in &time_trg_hashes {
                fastpq_entry_dataspaces
                    .insert(iroha_crypto::Hash::from(*entry_hash), DataSpaceId::GLOBAL);
            }
            hashes.append(&mut time_trg_hashes);
            ordered_results.append(&mut time_trg_results);

            let mut tx_set_hashes = hashes.clone();
            tx_set_hashes.sort_unstable();
            let tx_set_hash =
                crate::fastpq::tx_set_hash_from_ordered_hashes(tx_set_hashes.iter().copied());
            state_block.set_fastpq_tx_set_hash(tx_set_hash);
            state_block.set_fastpq_entry_dataspaces(fastpq_entry_dataspaces);

            let fastpq_transcripts = state_block.drain_transfer_transcripts();
            let axt_envelopes = state_block.drain_axt_envelopes();
            let axt_policy_snapshot = Some(state_block.axt_policy_snapshot());
            block.set_transaction_results_with_transcripts(
                time_trgs,
                hashes.as_slice(),
                ordered_results,
                fastpq_transcripts,
                axt_envelopes,
                axt_policy_snapshot,
            );
            if let (Some(timings), Some(start)) = (timings.as_deref_mut(), finalize_start) {
                timings.execution_tx_finalize_ms = to_ms(start.elapsed());
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
                timings.execution_tx_apply_ms = to_ms(start.elapsed());
                timings.execution_tx_apply_prep_ms = apply_prep_ms;
                timings.execution_tx_apply_detached_ms = apply_detached_ms;
                timings.execution_tx_apply_merge_ms = apply_merge_ms;
                timings.execution_tx_apply_fallback_ms = apply_fallback_ms;
                timings.execution_tx_apply_quarantine_ms = apply_quarantine_ms;
                timings.execution_tx_apply_sequential_ms = apply_sequential_ms;
            }
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
            let exec_witness_guard = crate::sumeragi::witness::exec_witness_guard();
            Self::validate_and_record_transactions(&mut block, state_block, None, false);
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

    #[cfg(test)]
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
            consensus::{ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus},
            da::{
                commitment::{
                    DaCommitmentBundle, DaCommitmentRecord, DaProofScheme, KzgCommitment,
                    RetentionClass,
                },
                types::{BlobDigest, StorageTicketId},
            },
            isi::{Log, error::Mismatch},
            nexus::LaneId,
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
                SoracloudLocalReadResponse, SoracloudPrivateInferenceExecutionRequest,
                SoracloudPrivateInferenceExecutionResult, SoracloudRuntime,
                SoracloudRuntimeExecutionError, SoracloudRuntimeExecutionErrorKind,
                SoracloudRuntimeReadHandle, SoracloudRuntimeSnapshot,
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

            fn execute_private_inference(
                &self,
                _request: SoracloudPrivateInferenceExecutionRequest,
            ) -> Result<SoracloudPrivateInferenceExecutionResult, SoracloudRuntimeExecutionError>
            {
                Err(SoracloudRuntimeExecutionError::new(
                    SoracloudRuntimeExecutionErrorKind::Unavailable,
                    "private inference is not used in this test runtime",
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
                    state_bindings,
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
            let payload_commitment = Hash::new(b"portal-runtime-state-payload");
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
                    payload_bytes: Some(256),
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
                assert_eq!(entry.payload_bytes.get(), 256);
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
                )
                .expect("static state-dependent validation should succeed")
            };
            let committed_heights = {
                let transactions_view = state.transactions.view();
                ValidBlock::committed_heights_for_block(&signed, &transactions_view)
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
                None,
                false,
                metrics,
            )
            .expect("static snapshot validation should succeed");
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
                )
                .expect("static state-dependent validation should succeed")
            };
            let committed_heights = {
                let transactions_view = state.transactions.view();
                ValidBlock::committed_heights_for_block(&signed, &transactions_view)
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
            let domain_id: DomainId = "wonderland".parse().expect("valid domain");
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
            );
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
            );
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
            let block_creation_ms = signed_block.header().creation_time().as_millis();

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

            let mut cache = state.stateless_validation_cache().lock();
            assert!(
                cache.get_ok(&tx_hash, block_creation_ms),
                "successful static validation should populate stateless cache",
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
            let block_creation_ms = signed_block.header().creation_time().as_millis();

            let mut voting_block: Option<super::super::VotingBlock> = None;
            let mut events = Vec::new();
            let result = ValidBlock::validate_keep_voting_block_with_events(
                signed_block,
                &topology,
                &state.chain_id.clone(),
                &ALICE_ID,
                &block_time_source,
                &state,
                &mut voting_block,
                false,
                |event| events.push(event),
            )
            .unpack(|_| {});
            assert!(
                result.is_ok(),
                "validation with events should succeed and warm stateless cache"
            );
            assert!(events.is_empty(), "no rejection events expected");

            let mut cache = state.stateless_validation_cache().lock();
            assert!(
                cache.get_ok(&tx_hash, block_creation_ms),
                "successful static validation with events should populate stateless cache",
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
            let block_creation_ms = signed_block.header().creation_time().as_millis();

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

            let mut cache = state.stateless_validation_cache().lock();
            assert!(
                cache.get_ok(&tx_hash, block_creation_ms),
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
            let block_creation_ms = signed_block.header().creation_time().as_millis();

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

            let mut cache = state.stateless_validation_cache().lock();
            assert!(
                cache.get_ok(&tx_hash, block_creation_ms),
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
            let domain_id: DomainId = "fraud-cache-test".parse().expect("fraud-cache-test domain");
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
                "genesis".parse().expect("valid domain id"),
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

            let kura = Kura::blank_kura_for_testing();
            let query_handle = LiveQueryStore::start_test();
            let state = State::new(World::default(), kura, query_handle);
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

    #[cfg(test)]
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

        const ACCOUNT_FROM_LITERAL: &str =
            "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB";
        const ACCOUNT_TO_LITERAL: &str =
            "sorauロ1NfキgノモノBヲKフリメoヌツロrG81ヒjWホユVncwフSア3pリヒノhUS9Q76";

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
            block.set_transaction_results_with_transcripts(
                Vec::new(),
                &entry_hashes,
                results,
                BTreeMap::new(),
                vec![envelope],
                Some(snapshot),
            );
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
                proof: Some(ProofBlob {
                    payload: policy.manifest_root.to_vec(),
                    expiry_slot: Some(50),
                }),
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
                    proof: ProofBlob {
                        payload: policy.manifest_root.to_vec(),
                        expiry_slot: Some(12),
                    },
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
                        proof: ProofBlob {
                            payload: policy_a.manifest_root.to_vec(),
                            expiry_slot: Some(25),
                        },
                    },
                    AxtProofFragment {
                        dsid: dsid_b,
                        proof: ProofBlob {
                            payload: policy_b.manifest_root.to_vec(),
                            expiry_slot: Some(25),
                        },
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
                    proof: ProofBlob {
                        payload: policy.manifest_root.to_vec(),
                        expiry_slot: Some(12),
                    },
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
                    proof: ProofBlob {
                        payload: policy.manifest_root.to_vec(),
                        expiry_slot: Some(12),
                    },
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
                    proof: ProofBlob {
                        payload: policy.manifest_root.to_vec(),
                        expiry_slot: Some(12),
                    },
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
                "missing touch manifest for handle dataspace",
            );
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
                    proof: ProofBlob {
                        payload: policy.manifest_root.to_vec(),
                        expiry_slot: Some(12),
                    },
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
                    proof: ProofBlob {
                        payload: policy.manifest_root.to_vec(),
                        expiry_slot: Some(12),
                    },
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
                    proof: ProofBlob {
                        payload: policy.manifest_root.to_vec(),
                        expiry_slot: Some(12),
                    },
                },
                AxtProofFragment {
                    dsid: dsid_b,
                    proof: ProofBlob {
                        payload: policy.manifest_root.to_vec(),
                        expiry_slot: Some(12),
                    },
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
            first.amount = 7;
            let mut second = first.clone();
            second.handle.sub_nonce = 4;
            second.amount = 7;

            let proof = AxtProofFragment {
                dsid,
                proof: ProofBlob {
                    payload: policy.manifest_root.to_vec(),
                    expiry_slot: Some(15),
                },
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
        fn axt_validation_rejects_expired_proof() {
            let kura = Kura::blank_kura_for_testing();
            let query = LiveQueryStore::start_test();
            let mut state = State::new_for_testing(World::new(), kura, query);
            let dsid = DataSpaceId::new(22);
            let lane = LaneId::new(9);
            let policy = AxtPolicyEntry {
                manifest_root: [0x45; 32],
                target_lane: lane,
                min_handle_era: 1,
                min_sub_nonce: 1,
                current_slot: 5,
            };
            state.set_axt_policy(dsid, policy);

            let descriptor = AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            };
            let binding = binding_for_descriptor(&descriptor);
            let mut handle = sample_handle(binding, lane, dsid, 6, policy.manifest_root);
            handle.proof = Some(ProofBlob {
                payload: policy.manifest_root.to_vec(),
                expiry_slot: Some(4),
            });
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
                    proof: ProofBlob {
                        payload: policy.manifest_root.to_vec(),
                        expiry_slot: Some(0),
                    },
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
            handle.proof = Some(ProofBlob {
                payload: policy.manifest_root.to_vec(),
                expiry_slot: Some(8),
            });
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
            let envelope = AxtEnvelopeRecord {
                binding,
                lane,
                descriptor,
                touches: Vec::new(),
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: ProofBlob {
                        payload: [0x99; 32].to_vec(),
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
                    proof: ProofBlob {
                        payload: policy.manifest_root.to_vec(),
                        expiry_slot: Some(10),
                    },
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
                    proof: ProofBlob {
                        payload: policy.manifest_root.to_vec(),
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
                    proof: ProofBlob {
                        payload: policy.manifest_root.to_vec(),
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
                    proof: ProofBlob {
                        payload: policy.manifest_root.to_vec(),
                        expiry_slot: Some(8),
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
                    proof: ProofBlob {
                        payload: policy.manifest_root.to_vec(),
                        expiry_slot: Some(9),
                    },
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
                    proof: ProofBlob {
                        payload: policy_b.manifest_root.to_vec(),
                        expiry_slot: Some(10),
                    },
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
                    proof: ProofBlob {
                        payload: manifest_root.to_vec(),
                        expiry_slot: Some(15),
                    },
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
            block.set_transaction_results_with_transcripts(
                Vec::new(),
                &entry_hashes,
                results,
                BTreeMap::new(),
                vec![envelope],
                None,
            );

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
            let proof_envelope = AxtProofEnvelope {
                dsid,
                manifest_root: policy.manifest_root,
                da_commitment: None,
                proof: vec![0xAB],
                fastpq_binding: None,
                committed_amount: Some(5),
                amount_commitment: None,
            };
            let proof_payload = norito::to_bytes(&proof_envelope).expect("encode proof envelope");
            let expected_commitment =
                ivm::axt::derive_amount_commitment(dsid, 5, Some(proof_payload.as_slice()));

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
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: ProofBlob {
                        payload: proof_payload,
                        expiry_slot: Some(9),
                    },
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
            let proof_envelope = AxtProofEnvelope {
                dsid,
                manifest_root: policy.manifest_root,
                da_commitment: None,
                proof: vec![0xCD],
                fastpq_binding: None,
                committed_amount: Some(5),
                amount_commitment: None,
            };
            let proof_payload = norito::to_bytes(&proof_envelope).expect("encode proof envelope");

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
                proofs: vec![AxtProofFragment {
                    dsid,
                    proof: ProofBlob {
                        payload: proof_payload,
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
}

#[cfg(test)]
mod tests {
    use core::time::Duration;
    use std::borrow::Cow;

    use iroha_data_model::{errors::AmxStage, prelude::*};
    use iroha_genesis::GENESIS_DOMAIN_ID;
    #[cfg(feature = "bls")]
    use iroha_primitives::json::Json;
    use iroha_primitives::time::TimeSource;
    use iroha_test_samples::gen_account_in;

    use super::*;
    use crate::{
        block::event::map_sig_err_to_reason,
        kura::Kura,
        query::store::LiveQueryStore,
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

    #[test]
    #[cfg(feature = "bls")]
    fn bls_pop_metadata_extracts_hex_string() {
        let mut metadata = Metadata::default();
        let key: iroha_data_model::name::Name = "bls_pop".parse().expect("valid name");
        metadata.insert(key.clone(), Json::new("A1B2"));
        let out = bls_pop_from_metadata(&metadata, &key).expect("pop should parse");
        assert_eq!(out, hex::decode("A1B2").unwrap());
    }

    #[test]
    #[cfg(feature = "bls")]
    fn bls_pop_metadata_rejects_non_string() {
        let mut metadata = Metadata::default();
        let key: iroha_data_model::name::Name = "bls_pop".parse().expect("valid name");
        metadata.insert(key.clone(), Json::new(123u64));
        assert!(bls_pop_from_metadata(&metadata, &key).is_none());
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
    fn lane_relay_helper_threads_commit_qc_and_rbc_bytes() {
        use iroha_crypto::{Hash, HashOf};
        use iroha_data_model::consensus::{Qc, QcAggregate, VALIDATOR_SET_HASH_VERSION_V1};
        use iroha_data_model::{
            block::consensus::{LaneBlockCommitment, LaneSettlementReceipt},
            da::commitment::DaCommitmentBundle,
            nexus::{DataSpaceId, LaneId},
            peer::PeerId,
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
        };

        let mut lane_summaries = BTreeMap::new();
        lane_summaries.insert(
            lane_id,
            LaneSummary {
                rbc_bytes_total: 2048,
                ..LaneSummary::default()
            },
        );

        let validator_set: Vec<PeerId> = Vec::new();
        let commit_qc = Qc {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: block_header.hash(),
            parent_state_root: Hash::prehashed([0xCE; Hash::LENGTH]),
            post_state_root: Hash::prehashed([0xCD; Hash::LENGTH]),
            height: block_header.height().get(),
            view: 3,
            epoch: 0,
            mode_tag: crate::sumeragi::consensus::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: vec![0x01],
                bls_aggregate_signature: vec![0x02],
            },
        };

        let with_qc = lane_relay_envelopes_for_block(
            &block_header,
            da_hash,
            std::slice::from_ref(&settlement),
            &lane_summaries,
            Some(&commit_qc),
        );
        assert_eq!(with_qc.len(), 1);
        let envelope = &with_qc[0];
        assert_eq!(envelope.qc.as_ref(), Some(&commit_qc));
        assert_eq!(envelope.rbc_bytes_total, 2048);
        envelope.verify().expect("envelope should validate");

        let without_qc = lane_relay_envelopes_for_block(
            &block_header,
            da_hash,
            &[settlement],
            &lane_summaries,
            None,
        );
        assert_eq!(without_qc.len(), 1);
        assert!(without_qc[0].qc.is_none());
        assert_eq!(without_qc[0].rbc_bytes_total, 2048);
        without_qc[0].verify().expect("envelope should validate");
    }

    #[test]
    fn lane_relay_envelopes_attach_manifest_roots() {
        use iroha_crypto::{Hash, HashOf};
        use iroha_data_model::consensus::{Qc, QcAggregate, VALIDATOR_SET_HASH_VERSION_V1};
        use iroha_data_model::{
            block::consensus::{LaneBlockCommitment, LaneSettlementReceipt},
            da::commitment::DaCommitmentBundle,
            nexus::{DataSpaceId, LaneId},
            peer::PeerId,
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
        };

        let mut lane_summaries = BTreeMap::new();
        lane_summaries.insert(
            lane_id,
            LaneSummary {
                rbc_bytes_total: 512,
                ..LaneSummary::default()
            },
        );

        let validator_set: Vec<PeerId> = Vec::new();
        let commit_qc = Qc {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: block_header.hash(),
            parent_state_root: Hash::prehashed([0xCE; Hash::LENGTH]),
            post_state_root: Hash::prehashed([0xCD; Hash::LENGTH]),
            height: block_header.height().get(),
            view: 3,
            epoch: 0,
            mode_tag: crate::sumeragi::consensus::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: vec![0x01],
                bls_aggregate_signature: vec![0x02],
            },
        };

        let mut envelopes = lane_relay_envelopes_for_block(
            &block_header,
            da_hash,
            std::slice::from_ref(&settlement),
            &lane_summaries,
            Some(&commit_qc),
        );
        let manifest_root = [0x44; 32];
        let manifest_roots: BTreeMap<DataSpaceId, [u8; 32]> =
            core::iter::once((dataspace_id, manifest_root)).collect();
        attach_manifest_roots_to_relays(&mut envelopes, &manifest_roots);
        attach_fastpq_proof_material_to_relays(&mut envelopes);

        assert_eq!(envelopes.len(), 1);
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
        let domain_id: DomainId = "wonderland".parse().expect("wonderland domain");
        let domain: Domain = Domain::new(domain_id.clone()).build(&alice_id);
        let ad: AssetDefinition = {
            let __asset_definition_id = iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
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
            "wonderland".parse().unwrap(),
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

    #[tokio::test]
    async fn should_reject_due_to_repetition() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        // Predefined world state
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = "wonderland".parse().expect("Valid");
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
            "wonderland".parse().expect("domain id"),
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
        let domain_id: DomainId = "wonderland".parse().expect("Valid");
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
        // Two independent register instructions (no ordering dependencies)
        let domain_a = Register::domain(Domain::new("domain-a".parse().unwrap()));
        let domain_b = Register::domain(Domain::new("domain-b".parse().unwrap()));

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

        let fail_domain_id = "missing-domain".parse().expect("valid id");
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

        let is_ok = |hash: &_, label: &str| {
            outcomes
                .iter()
                .find(|(entry_hash, _)| entry_hash == hash)
                .unwrap_or_else(|| panic!("missing result for {label}"))
                .1
                .as_ref()
                .is_ok()
        };

        assert!(!is_ok(&fail_hash, "fail tx"));
        assert!(is_ok(&register_hash, "register tx"));
        assert!(is_ok(&succeed_hash, "succeed tx"));
    }

    #[tokio::test]
    async fn failed_transactions_revert() {
        let chain_id = ChainId::from("00000000-0000-0000-0000-000000000000");

        // Predefined world state
        let (alice_id, alice_keypair) = gen_account_in("wonderland");
        let domain_id: DomainId = "wonderland".parse().expect("Valid");
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
        let domain_id = "domain".parse().expect("Valid");
        let create_domain = Register::domain(Domain::new(domain_id));
        let asset_definition_id = iroha_data_model::asset::AssetDefinitionId::new(
            "domain".parse().unwrap(),
            "coin".parse().unwrap(),
        );
        let create_asset = Register::asset_definition(
            AssetDefinition::numeric(asset_definition_id).with_name("coin".to_owned()),
        );
        let fail_isi = Unregister::domain("dummy".parse().unwrap());
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

        assert!(
            lookup(&fail_hash, "fail tx").is_err(),
            "Failing tx must be rejected"
        );
        assert!(
            lookup(&accept_hash, "accept tx").is_ok(),
            "Second tx must succeed"
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
        let wonderland_domain_id: DomainId = "wonderland".parse().expect("Valid domain id");
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
            "wonderland".parse().expect("valid domain id"),
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
        let wonderland_domain_id: DomainId = "wonderland".parse().expect("valid domain id");

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
