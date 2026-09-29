//! Norito-encoded consensus types shared across Sumeragi implementations.
//!
//! These types cover signed consensus genesis parameters, operator diagnostics, Nexus fee
//! and settlement receipts, and execution witnesses. Global consensus messages and signed
//! RS16 data availability live in [`super::consensus_v2`]; there is no global-v1 message
//! family.
use super::Header as BlockHeader;

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    asset::AssetDefinitionId,
    fastpq::{FastpqTransitionBatch, TransferTranscriptBundle},
    nexus::FeeDebitSource,
};
use core::num::NonZeroU64;
use iroha_crypto::{Hash, HashOf};
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
use iroha_primitives::numeric::{Numeric, Quantity};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, DecodeAll, Encode};
use std::{string::String, vec::Vec};
/// Height alias for consensus.
pub type Height = u64;
/// View/round number alias.
pub type View = u64;
/// Validator index within the active set.
pub type ValidatorIndex = u32;
/// Canonical consensus parameters included in the genesis fingerprint.
///
/// These parameters are encoded with Norito (binary) in a fixed order to
/// guarantee determinism across peers and platforms.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus::ConsensusGenesisParams")]
pub struct ConsensusGenesisParams {
    /// Signed, immutable interval between block-production opportunities.
    pub block_cadence_ms: NonZeroU64,
    /// Block sizing: max transactions per block.
    pub block_max_transactions: NonZeroU64,
    /// Type-safe mode-specific signed consensus parameters.
    pub mode: ConsensusGenesisModeParams,
    /// Explicit global consensus protocol revision.
    pub protocol_version: u32,
    /// Required signed inputs for constructing Sumeragi v2 height contexts.
    pub v2_context: super::consensus_v2::SumeragiV2GenesisContextParameters,
}
/// Type-safe first-release consensus mode carrier.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus::ConsensusGenesisModeParams")]
pub enum ConsensusGenesisModeParams {
    /// Permissioned consensus has no election parameters.
    Permissioned,
    /// Nominated proof-of-stake consensus and its signed election inputs.
    Npos(NposGenesisParams),
}
impl ConsensusGenesisParams {
    /// Validate every frozen first-release consensus input before fingerprinting or use.
    ///
    /// # Errors
    /// Returns a diagnostic for unsupported protocol revisions, invalid v2
    /// context geometry, or invalid `NPoS` election parameters.
    pub fn validate(&self) -> Result<(), String> {
        if self.protocol_version != u32::from(crate::sumeragi::PROTOCOL_VERSION) {
            return Err(format!(
                "unsupported consensus protocol version {}",
                self.protocol_version
            ));
        }
        self.v2_context
            .validate()
            .map_err(|error| format!("invalid Sumeragi v2 genesis context: {error}"))?;
        if let ConsensusGenesisModeParams::Npos(npos) = &self.mode {
            npos.validate().map_err(str::to_owned)?;
        }
        Ok(())
    }
}
/// `NPoS`-specific consensus parameters hashed into the genesis fingerprint.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus::NposGenesisParams")]
pub struct NposGenesisParams {
    /// Non-zero epoch length in blocks.
    pub epoch_length_blocks: NonZeroU64,
    /// Deterministic epoch seed for PRF-based leader and validator selection.
    pub epoch_seed: [u8; 32],
    /// Exact bounded `3f + 1` ceiling for the next epoch committee.
    pub max_validators: u32,
    /// Minimum self-bond required for validator eligibility.
    pub min_self_bond: Quantity,
    /// Minimum nomination bond required for delegators.
    pub min_nomination_bond: Quantity,
    /// Finality margin in blocks before activating a newly elected set.
    pub finality_margin_blocks: u64,
    /// Evidence retention horizon in blocks.
    pub evidence_horizon_blocks: u64,
    /// Activation lag in blocks for newly scheduled validator sets.
    pub activation_lag_blocks: u64,
    /// Slashing delay in blocks before evidence penalties apply.
    pub slashing_delay_blocks: u64,
}
impl NposGenesisParams {
    /// Validate signed `NPoS` election and reconfiguration inputs.
    ///
    /// # Errors
    /// Returns a stable diagnostic when a seed, bond, or
    /// reconfiguration bound is invalid.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.epoch_seed == [0; 32] {
            return Err("epoch_seed must not be all zero");
        }
        if usize::try_from(self.max_validators)
            .ok()
            .is_none_or(|count| !super::consensus_v2::is_valid_committee_size(count))
        {
            return Err("max_validators must be a bounded 3f + 1 committee size (4..=31)");
        }
        if self.min_self_bond.is_zero() || self.min_nomination_bond.is_zero() {
            return Err("NPoS minimum bond values must be greater than zero");
        }
        if self.finality_margin_blocks == 0
            || self.evidence_horizon_blocks == 0
            || self.activation_lag_blocks == 0
            || self.slashing_delay_blocks == 0
        {
            return Err("NPoS finality and reconfiguration bounds must be greater than zero");
        }
        let accountability_window = self
            .evidence_horizon_blocks
            .checked_add(self.slashing_delay_blocks)
            .ok_or("NPoS evidence-and-slashing window overflows u64")?;
        let retained_roster_window = self
            .epoch_length_blocks
            .get()
            .checked_mul(3)
            .ok_or("NPoS three-epoch evidence capacity window overflows u64")?;
        if accountability_window > retained_roster_window {
            return Err(
                "evidence_horizon_blocks + slashing_delay_blocks must not exceed three epoch lengths",
            );
        }
        Ok(())
    }
}
/// Aggregated per-lane commitment summary reported by Sumeragi status.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::block::consensus::SumeragiLaneCommitment")]
pub struct SumeragiLaneCommitment {
    /// Block height associated with the commitment.
    pub block_height: u64,
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Number of transactions attributed to the lane.
    pub tx_count: u64,
    /// Total RBC chunks allocated to the lane.
    pub total_chunks: u64,
    /// Total RBC payload bytes allocated to the lane.
    pub rbc_bytes_total: u64,
    /// Total TEU allocated to the lane.
    pub teu_total: u64,
    /// Block hash anchoring the commitment.
    pub block_hash: HashOf<BlockHeader>,
}
/// Aggregated per-dataspace commitment summary reported by Sumeragi status.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::block::consensus::SumeragiDataspaceCommitment")]
pub struct SumeragiDataspaceCommitment {
    /// Block height associated with the commitment.
    pub block_height: u64,
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Numeric dataspace identifier.
    pub dataspace_id: DataSpaceId,
    /// Number of transactions attributed to the dataspace.
    pub tx_count: u64,
    /// Total RBC chunks allocated to the dataspace.
    pub total_chunks: u64,
    /// Total RBC payload bytes allocated to the dataspace.
    pub rbc_bytes_total: u64,
    /// Total TEU allocated to the dataspace.
    pub teu_total: u64,
    /// Block hash anchoring the commitment.
    pub block_hash: HashOf<BlockHeader>,
}
/// Deterministic settlement receipt emitted for audit and reconciliation.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus::LaneSettlementReceipt")]
pub struct LaneSettlementReceipt {
    /// Caller-specified identifier linking the receipt to the originating transaction.
    pub source_id: [u8; 32],
    /// Exact local gas-token amount debited from the payer.
    pub local_amount: Quantity,
    /// Exact XOR amount booked immediately after inclusion.
    pub xor_due: Quantity,
    /// Exact XOR amount expected post-haircut.
    pub xor_after_haircut: Quantity,
    /// Safety margin consumed by this receipt (`xor_due - xor_after_haircut`).
    pub xor_variance: Quantity,
    /// UTC timestamp in milliseconds when the receipt was generated.
    pub timestamp_ms: u64,
}
/// Deterministic Nexus fee schedule inputs captured for asynchronous settlement.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus::NexusFeeScheduleInputs")]
pub struct NexusFeeScheduleInputs {
    /// Serialized signed transaction payload length used for fee metering.
    pub tx_bytes_len: u64,
    /// Number of native instructions included in the transaction fee calculation.
    pub instruction_count: u64,
    /// Gas units used by the transaction.
    pub gas_used: u64,
    /// Base fee from `nexus.fees.base_fee`.
    pub base_fee: Quantity,
    /// Per-byte fee from `nexus.fees.per_byte_fee`.
    pub per_byte_fee: Quantity,
    /// Per-instruction fee from `nexus.fees.per_instruction_fee`.
    pub per_instruction_fee: Quantity,
    /// Per-gas-unit fee from `nexus.fees.per_gas_unit_fee`.
    pub per_gas_unit_fee: Quantity,
}
/// Versioned Nexus fee receipt committed by a finalized lane block.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus::NexusFeeReceipt")]
pub struct NexusFeeReceipt {
    /// Receipt format version.
    pub version: u16,
    /// Source transaction hash/id.
    pub source_id: [u8; 32],
    /// DPN dataspace that finalized the source transaction.
    pub dataspace_id: DataSpaceId,
    /// DPN lane that finalized the source transaction.
    pub lane_id: LaneId,
    /// DPN block height that finalized the source transaction.
    pub block_height: u64,
    /// Exact account or sponsor-program vault charged by settlement.
    pub debit_source: FeeDebitSource,
    /// Canonical fee asset definition charged by settlement.
    pub fee_asset_id: AssetDefinitionId,
    /// Immutable sponsor-program revision charged by this receipt, when sponsored.
    #[norito(required)]
    pub program_revision: Option<u64>,
    /// Proof-bound cross-lane spend lease, when relay settlement is used.
    #[norito(required)]
    pub lease_id: Option<Hash>,
    /// Computed fee amount to burn on Nexus.
    pub fee_amount: Quantity,
    /// Fee schedule inputs needed to recompute [`Self::fee_amount`].
    pub schedule: NexusFeeScheduleInputs,
}
impl NexusFeeReceipt {
    /// Clean-break receipt version carrying typed debit sources and canonical assets.
    pub const VERSION: u16 = 2;
}
/// Liquidity profile applied when computing XOR conversions.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(tag = "profile", content = "state")]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus::LaneLiquidityProfile")]
pub enum LaneLiquidityProfile {
    /// Deep pools with negligible slippage.
    Tier1,
    /// Medium depth pools with moderate slippage.
    Tier2,
    /// Thin pools or credit-constrained venues.
    Tier3,
}
/// Volatility bucket applied when computing the safety margin.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    Default,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(tag = "bucket", content = "state")]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus::LaneVolatilityClass")]
pub enum LaneVolatilityClass {
    /// Normal operating conditions.
    #[default]
    Stable,
    /// Elevated but healthy volatility.
    Elevated,
    /// Dislocated markets requiring maximal margin.
    Dislocated,
}
/// Swap metadata describing the deterministic conversion parameters.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus::LaneSwapMetadata")]
pub struct LaneSwapMetadata {
    /// Basis-point safety margin applied on top of the TWAP.
    pub epsilon_bps: u16,
    /// TWAP window length in seconds.
    pub twap_window_seconds: u32,
    /// Liquidity profile guiding haircut selection.
    pub liquidity_profile: LaneLiquidityProfile,
    /// Canonical exact TWAP value (`local_token / XOR`).
    pub twap_local_per_xor: Numeric,
    /// Volatility bucket recorded when applying the epsilon.
    pub volatility_class: LaneVolatilityClass,
}
impl<'a> norito::core::DecodeFromSlice<'a> for LaneSwapMetadata {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        decode_from_slice_canonical(bytes)
    }
}
/// Runtime-upgrade governance hook snapshot.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::block::consensus::SumeragiRuntimeUpgradeHook")]
pub struct SumeragiRuntimeUpgradeHook {
    /// Whether runtime-upgrade instructions are allowed.
    pub allow: bool,
    /// Whether runtime-upgrade instructions must include metadata.
    pub require_metadata: bool,
    /// Metadata key enforced by the manifest, if specified.
    #[norito(default)]
    pub metadata_key: Option<String>,
    /// Allowed metadata values when an allowlist is configured.
    #[norito(default)]
    pub allowed_ids: Vec<String>,
}
/// Governance manifest readiness snapshot for a lane.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::block::consensus::SumeragiLaneGovernance")]
pub struct SumeragiLaneGovernance {
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Human-readable lane alias.
    pub alias: String,
    /// Governance module configured for the lane, if any.
    #[norito(default)]
    pub governance: Option<String>,
    /// Whether the lane requires a governance manifest.
    pub manifest_required: bool,
    /// Whether a manifest has been loaded and validated.
    pub manifest_ready: bool,
    /// Path of the loaded manifest (best-effort; operator visibility).
    #[norito(default)]
    pub manifest_path: Option<String>,
    /// Validator identifiers derived from the manifest.
    #[norito(default)]
    pub validator_ids: Vec<String>,
    /// Quorum threshold configured by the manifest.
    #[norito(default)]
    pub quorum: Option<u32>,
    /// Protected namespaces enforced by the manifest.
    #[norito(default)]
    pub protected_namespaces: Vec<String>,
    /// Runtime-upgrade governance hook configuration.
    #[norito(default)]
    pub runtime_upgrade: Option<SumeragiRuntimeUpgradeHook>,
}
/// Current `NPoS` epoch schedule for operator diagnostics.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, DeriveJsonSerialize, DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus::SumeragiNposDiagnostics")]
pub struct SumeragiNposDiagnostics {
    /// Length of the active epoch in blocks.
    pub epoch_length_blocks: NonZeroU64,
    /// Non-zero epoch seed used for deterministic leader and validator election.
    pub epoch_seed: [u8; 32],
}
impl SumeragiNposDiagnostics {
    /// Validate cross-field invariants that scalar wire types cannot express.
    ///
    /// # Errors
    ///
    /// Returns a stable reason when the epoch seed is zero.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.epoch_seed == [0; 32] {
            return Err("NPoS diagnostics epoch seed must be non-zero");
        }
        Ok(())
    }
}
/// Aggregate execution diagnostics for the latest block pipeline run.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    Default,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus::SumeragiPipelineExecutionStatus")]
pub struct SumeragiPipelineExecutionStatus {
    /// Total transaction vertices across all lanes.
    pub tx_vertices_total: u64,
    /// Total conflict edges across all lanes.
    pub tx_edges_total: u64,
    /// Total overlay fragments executed across all lanes.
    pub overlay_count_total: u64,
    /// Total overlay instructions executed across all lanes.
    pub overlay_instr_total: u64,
    /// Total overlay bytes executed across all lanes.
    pub overlay_bytes_total: u64,
    /// Total RBC chunks attributed across all lanes.
    pub rbc_chunks_total: u64,
    /// Total RBC payload bytes attributed across all lanes.
    pub rbc_bytes_total: u64,
    /// Transactions prepared for detached overlay execution.
    pub detached_prepared_total: u64,
    /// Detached transaction deltas merged without sequential fallback.
    pub detached_merged_total: u64,
    /// Detached transaction deltas that fell back to sequential execution.
    pub detached_fallback_total: u64,
    /// Sequential fallbacks caused by fee postprocessing.
    pub detached_fallback_fee_postprocessing_total: u64,
    /// Sequential fallbacks caused by a user-provided executor.
    pub detached_fallback_user_executor_total: u64,
    /// Sequential fallbacks caused by durable smart-contract state changes.
    pub detached_fallback_durable_state_total: u64,
    /// Sequential fallbacks caused by unsupported detached instructions.
    pub detached_fallback_unsupported_instruction_total: u64,
    /// Sequential fallbacks caused by rejected detached evaluation.
    pub detached_fallback_rejected_eval_total: u64,
    /// Sequential fallbacks caused by overlay build errors.
    pub detached_fallback_overlay_error_total: u64,
    /// Quarantine transactions executed sequentially.
    pub quarantine_executed_total: u64,
}
/// Operator and lane diagnostics returned by `/v1/sumeragi/diagnostics`.
///
/// This payload deliberately excludes reducer phase, height, view, leader, certificates, mode, and
/// timing. `/v1/sumeragi/status` is the sole source of authoritative consensus state.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DeriveJsonSerialize, DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "operator diagnostics expose independent queue-pressure flags"
)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus::SumeragiDiagnosticsStatus")]
pub struct SumeragiDiagnosticsStatus {
    /// Latest block-pipeline execution diagnostics.
    pub pipeline_execution: SumeragiPipelineExecutionStatus,
    /// Current transaction queue depth.
    pub tx_queue_depth: u64,
    /// Configured transaction queue capacity.
    pub tx_queue_capacity: u64,
    /// Estimated retained transaction queue bytes.
    pub tx_queue_retained_bytes: u64,
    /// Configured retained transaction queue byte budget.
    pub tx_queue_max_retained_bytes: u64,
    /// Whether the transaction queue is saturated.
    pub tx_queue_saturated: bool,
    /// Whether saturation is caused by transaction count.
    pub tx_queue_saturated_by_count: bool,
    /// Whether saturation is caused by retained bytes.
    pub tx_queue_saturated_by_bytes: bool,
    /// Whether the oldest queued transaction exceeded the age budget.
    pub tx_queue_saturated_by_age: bool,
    /// Oldest queued transaction age in milliseconds.
    pub tx_queue_oldest_queued_age_ms: u64,
    /// `NPoS`-only diagnostics; absent in permissioned mode.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub npos: Option<SumeragiNposDiagnostics>,
    /// Aggregated lane-level commitment snapshots.
    pub lane_commitments: Vec<SumeragiLaneCommitment>,
    /// Aggregated dataspace-level commitment snapshots.
    pub dataspace_commitments: Vec<SumeragiDataspaceCommitment>,
    /// Count of lanes that still require a governance manifest.
    pub lane_governance_sealed_total: u32,
    /// Aliases of lanes that remain sealed.
    pub lane_governance_sealed_aliases: Vec<String>,
    /// Governance manifest readiness per lane.
    pub lane_governance: Vec<SumeragiLaneGovernance>,
}
/// Minimal execution witness KV pair for SBV-AM prototypes.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::block::consensus::ExecKv")]
pub struct ExecKv {
    /// Raw key bytes.
    pub key: Vec<u8>,
    /// Raw value bytes.
    pub value: Vec<u8>,
}
/// Execution witness containing reads and writes for SMT recomputation.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Default,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::block::consensus::ExecWitness")]
pub struct ExecWitness {
    /// Witnessed reads during execution (key,value).
    pub reads: Vec<ExecKv>,
    /// Writes performed during execution (key,value). Overrides reads on conflict.
    pub writes: Vec<ExecKv>,
    /// FASTPQ transfer transcripts grouped per entry hash.
    pub fastpq_transcripts: Vec<TransferTranscriptBundle>,
    /// FASTPQ transition batches prepared for prover ingestion.
    pub fastpq_batches: Vec<FastpqTransitionBatch>,
}
/// Execution witness message bound to a specific block and round. Used on-wire.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::block::consensus::ExecWitnessMsg")]
pub struct ExecWitnessMsg {
    /// Hash of the block the witness applies to.
    pub block_hash: HashOf<BlockHeader>,
    /// Height of the block.
    pub height: Height,
    /// View/round for which the witness applies.
    pub view: View,
    /// Epoch index (0 in permissioned mode).
    pub epoch: u64,
    /// The execution witness payload.
    pub witness: ExecWitness,
}
// --- Helpers for Norito slice decoding bridges ---
fn decode_from_slice_canonical<T>(bytes: &[u8]) -> Result<(T, usize), norito::core::Error>
where
    T: DecodeAll + Encode,
{
    let _canonical_flags =
        norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    let (value, used) = norito::core::decode_field_prefix::<T>(bytes)
        .map_err(|e| norito::core::Error::Message(format!("codec decode error: {e}")))?;
    let canonical = value.encode();
    if used != canonical.len() || bytes.len() < used {
        return Err(norito::core::Error::LengthMismatch);
    }
    if bytes[..used] != canonical {
        return Err(norito::core::Error::Message("payload mismatch".into()));
    }
    Ok((value, used))
}
macro_rules! impl_decode_from_slice_via_codec {
    ($t:ty) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $t {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                decode_from_slice_canonical(bytes)
            }
        }
    };
}
impl_decode_from_slice_via_codec!(ExecKv);
impl_decode_from_slice_via_codec!(ExecWitness);
impl_decode_from_slice_via_codec!(ExecWitnessMsg);
impl_decode_from_slice_via_codec!(ConsensusGenesisParams);
impl_decode_from_slice_via_codec!(NposGenesisParams);
impl_decode_from_slice_via_codec!(SumeragiNposDiagnostics);
impl_decode_from_slice_via_codec!(SumeragiPipelineExecutionStatus);
impl_decode_from_slice_via_codec!(SumeragiDiagnosticsStatus);
impl_decode_from_slice_via_codec!(SumeragiLaneCommitment);
impl_decode_from_slice_via_codec!(SumeragiDataspaceCommitment);
impl_decode_from_slice_via_codec!(SumeragiRuntimeUpgradeHook);
impl_decode_from_slice_via_codec!(SumeragiLaneGovernance);
impl<'a> norito::core::DecodeFromSlice<'a> for LaneSettlementReceipt {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        decode_from_slice_canonical(bytes)
    }
}
#[cfg(test)]
#[path = "consensus_model_tests.rs"]
mod tests;

#[cfg(test)]
mod captured_consensus_schema_tests;
