//! Norito-encoded consensus message types shared across Sumeragi implementations.
//!
//! These types cover shared consensus parameters and diagnostics, authenticated v2 evidence,
//! and lane-local certificates. Global consensus messages and signed RS16 data availability
//! live in [`super::consensus_v2`]; there is no global-v1 message family.
use super::Header as BlockHeader;
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    NetworkId,
    asset::AssetDefinitionId,
    fastpq::{FastpqTransitionBatch, TransferTranscriptBundle},
    nexus::{DataSpaceId, FeeDebitSource, LaneId, LaneRelayEnvelope},
    peer::PeerId,
};
use core::{fmt, num::NonZeroU64};
use iroha_crypto::{Algorithm, Hash, HashOf};
use iroha_primitives::numeric::{Numeric, Quantity};
use iroha_schema::{EnumMeta, EnumVariant, Ident, IntoSchema, MetaMap, Metadata, TypeId};
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
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
        if self.protocol_version != u32::from(super::consensus_v2::PROTOCOL_VERSION) {
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
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
    /// Maximum nominator concentration percentage.
    pub max_nominator_concentration_pct: u8,
    /// Seat allocation variance band percentage.
    pub seat_band_pct: u8,
    /// Maximum correlation percentage across validator entities.
    pub max_entity_correlation_pct: u8,
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
    /// Returns a stable diagnostic when a seed, bond, percentage, or
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
        if self.max_nominator_concentration_pct > 100
            || self.seat_band_pct > 100
            || self.max_entity_correlation_pct > 100
        {
            return Err("NPoS election percentages must be in 0..=100");
        }
        if self.finality_margin_blocks == 0
            || self.evidence_horizon_blocks == 0
            || self.activation_lag_blocks == 0
            || self.slashing_delay_blocks == 0
        {
            return Err("NPoS finality and reconfiguration bounds must be greater than zero");
        }
        Ok(())
    }
}
/// Consensus certificate phases (BLS-only).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "phase", content = "detail", rename_all = "snake_case")]
pub enum CertPhase {
    /// Prepare/lock certificate for a proposal.
    Prepare = 1,
    /// Commit QC for finalization.
    Commit = 2,
    /// New-view certificate for view change.
    NewView = 3,
}
impl TypeId for CertPhase {
    fn id() -> Ident {
        "CertPhase".to_owned()
    }
}
impl IntoSchema for CertPhase {
    fn type_name() -> Ident {
        "CertPhase".to_owned()
    }
    fn update_schema_map(metamap: &mut MetaMap) {
        let variants = vec![
            EnumVariant {
                tag: "Prepare".to_owned(),
                discriminant: CertPhase::Prepare as u32,
                ty: None,
            },
            EnumVariant {
                tag: "Commit".to_owned(),
                discriminant: CertPhase::Commit as u32,
                ty: None,
            },
            EnumVariant {
                tag: "NewView".to_owned(),
                discriminant: CertPhase::NewView as u32,
                ty: None,
            },
        ];
        metamap.insert::<Self>(Metadata::Enum(EnumMeta { variants }));
    }
}
/// Self-contained frozen context and exact signed artifacts for one Sumeragi v2 equivocation proof.
///
/// Proofs of possession are retained in roster order so an auditor can verify current-context
/// aggregate certificates referenced by the artifacts without consulting mutable validator state.
/// Production persistence additionally compares this context and `PoP` vector with the locally
/// verified immutable context record.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiV2EquivocationEvidence {
    /// Immutable context which governed both conflicting artifacts.
    pub context: super::consensus_v2::HeightContext,
    /// BLS proofs of possession in exact frozen-roster order.
    pub proofs_of_possession: Vec<Vec<u8>>,
    /// Exact pair of conflicting signed artifacts.
    pub conflict: super::consensus_v2::SumeragiV2Equivocation,
}
/// Exact, independently verifiable Sumeragi-v2 equivocation evidence.
///
/// The first release has one evidence shape. Retired global-v1 kind/payload
/// enums are intentionally absent, so old wire and storage layouts fail decode.
/// This wrapper is a binary persistence/instruction type; typed JSON embeds the
/// closed [`SumeragiV2EquivocationEvidence`] object directly where required.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
pub struct Evidence {
    /// Frozen height context, roster proofs, and exact conflicting signed artifacts.
    pub equivocation: SumeragiV2EquivocationEvidence,
}
impl Ord for Evidence {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let left = self.encode();
        let right = other.encode();
        left.cmp(&right)
    }
}
impl PartialOrd for Evidence {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
/// Persisted evidence entry annotated with commit metadata.
///
/// Every field belongs to the first-release storage layout; shortened records
/// are rejected instead of receiving implicit penalty or admission state. The
/// type is binary-only; endpoint JSON uses a purpose-built audit projection.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub struct EvidenceRecord {
    /// Slashing material captured for governance processing.
    pub evidence: Evidence,
    /// Block height at which this evidence record was appended to WSV.
    pub recorded_at_height: Height,
    /// Consensus view (round) of the block carrying the record.
    pub recorded_at_view: View,
    /// Block creation timestamp in milliseconds since UNIX epoch.
    pub recorded_at_ms: u64,
    /// Whether a penalty was already applied for this evidence record.
    pub penalty_applied: bool,
    /// Whether governance cancelled penalty application for this evidence record.
    pub penalty_cancelled: bool,
    /// Block height at which the penalty was cancelled, if any.
    pub penalty_cancelled_at_height: Option<Height>,
    /// Block height at which the penalty was applied, if any.
    pub penalty_applied_at_height: Option<Height>,
    /// Block height which first admitted this exact evidence into consensus.
    ///
    /// `None` denotes node-local pending diagnostic material. Pending material
    /// is never eligible for deterministic penalty derivation.
    pub consensus_admitted_at_height: Option<Height>,
}
/// Membership snapshot exported through `/v1/sumeragi/status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiMembershipStatus {
    /// Height associated with the snapshot.
    #[norito(default)]
    pub height: u64,
    /// View associated with the snapshot.
    #[norito(default)]
    pub view: u64,
    /// Epoch associated with the snapshot.
    #[norito(default)]
    pub epoch: u64,
    /// Deterministic roster hash for `(height, view, epoch)`.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub view_hash: Option<[u8; 32]>,
}
/// Membership mismatch snapshot exported through `/v1/sumeragi/status`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiMembershipMismatchStatus {
    /// Peers currently flagged for membership mismatches.
    #[norito(default)]
    pub active_peers: Vec<PeerId>,
    /// Last peer observed with a mismatch (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_peer: Option<PeerId>,
    /// Height associated with the last mismatch (best-effort).
    #[norito(default)]
    pub last_height: u64,
    /// View associated with the last mismatch (best-effort).
    #[norito(default)]
    pub last_view: u64,
    /// Epoch associated with the last mismatch (best-effort).
    #[norito(default)]
    pub last_epoch: u64,
    /// Local membership hash observed during the last mismatch (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_local_hash: Option<[u8; 32]>,
    /// Remote membership hash observed during the last mismatch (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_remote_hash: Option<[u8; 32]>,
    /// Milliseconds since UNIX epoch when the last mismatch was recorded.
    #[norito(default)]
    pub last_timestamp_ms: u64,
}
/// Aggregated per-lane commitment summary reported by Sumeragi status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
/// Execution status for a certified lane block whose payload is not locally recoverable yet.
pub const COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD: &str = "awaiting_executable_payload";
/// Execution status for a certified lane block whose payload can be recovered for execution.
pub const COMMITTED_LANE_STATUS_PAYLOAD_AVAILABLE_AWAITING_EXECUTOR: &str =
    "payload_available_awaiting_executor";
/// Execution status for a certified lane block whose execution input has been recovered.
pub const COMMITTED_LANE_STATUS_PAYLOAD_RECOVERED_AWAITING_STATE_APPLICATION: &str =
    "payload_recovered_awaiting_state_application";
/// Execution status for a certified lane block that preflighted cleanly at the local state tip.
pub const COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHTED_AWAITING_STATE_APPLICATION: &str =
    "payload_preflighted_awaiting_state_application";
/// Execution status for a certified lane block whose preflight produced at least one rejection.
pub const COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHT_REJECTED_AWAITING_STATE_APPLICATION: &str =
    "payload_preflight_rejected_awaiting_state_application";
/// Execution status for a certified lane block whose canonical receipt conflicts with preflight.
pub const COMMITTED_LANE_STATUS_APPLICATION_RECEIPT_CONFLICTS_WITH_PREFLIGHT: &str =
    "application_receipt_conflicts_with_preflight";
/// Execution status for a certified lane block waiting for its predecessor to be applied.
pub const COMMITTED_LANE_STATUS_AWAITING_PREDECESSOR_APPLICATION: &str =
    "awaiting_predecessor_application";
/// Execution status for a certified lane block with committed canonical application results.
pub const COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK: &str =
    "state_applied_by_canonical_block";
/// Whether a committed lane-block status may count as rollout progress evidence.
///
/// Rejected preflight evidence is an execution blocker, not progress: it proves
/// the payload was recoverable, but it must not satisfy autoscale/localnet
/// expansion evidence until a canonical receipt resolves it.
#[must_use]
pub fn committed_lane_block_status_counts_as_progress(
    execution_status: &str,
    executable_payload_available: bool,
) -> bool {
    match execution_status {
        COMMITTED_LANE_STATUS_AWAITING_EXECUTABLE_PAYLOAD => !executable_payload_available,
        COMMITTED_LANE_STATUS_PAYLOAD_AVAILABLE_AWAITING_EXECUTOR
        | COMMITTED_LANE_STATUS_PAYLOAD_RECOVERED_AWAITING_STATE_APPLICATION
        | COMMITTED_LANE_STATUS_PAYLOAD_PREFLIGHTED_AWAITING_STATE_APPLICATION
        | COMMITTED_LANE_STATUS_STATE_APPLIED_BY_CANONICAL_BLOCK => executable_payload_available,
        // Rejected preflight, predecessor waits, conflicts, and unknown future
        // status labels all fail closed.
        _ => false,
    }
}
/// Certified standalone lane-local block summary reported by Sumeragi status.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiCommittedLaneBlock {
    /// Lane whose local block is committed.
    pub lane_id: LaneId,
    /// Dataspace bound to the lane-local block.
    pub dataspace_id: DataSpaceId,
    /// Exact active incarnation of the lane when this block was certified.
    pub lane_incarnation: Hash,
    /// Lane-local block height.
    pub lane_block_height: u64,
    /// Lane-local consensus view.
    pub lane_block_view: u64,
    /// Stable hash of the standalone lane block descriptor.
    pub descriptor_hash: Hash,
    /// Stable hash of the standalone lane block proposal.
    pub proposal_hash: Hash,
    /// Operator-facing execution readiness label.
    pub execution_status: String,
    /// Whether payload material is locally available for standalone execution.
    pub executable_payload_available: bool,
    /// Subject hash certified by the lane block proposal.
    pub subject_hash: Hash,
    /// Payload ownership hash certified by the lane block proposal.
    pub payload_ownership_hash: Hash,
    /// RBC instance hash certified by the lane block proposal.
    pub rbc_instance_hash: Hash,
    /// Consensus/QC mode tag used to derive the lane hashes.
    pub qc_mode_tag: String,
    /// Validator count in the lane descriptor.
    pub validator_count: u32,
    /// Minimum quorum required by the lane descriptor.
    pub min_quorum: u32,
    /// Signers present in the prepare QC.
    pub prepare_qc_signer_count: u32,
    /// Signers present in the commit QC.
    pub commit_qc_signer_count: u32,
}
/// Planned lane-local payload ownership exported by Sumeragi status.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiLanePayloadOwnership {
    /// Global proposal height that planned this lane-local payload.
    pub proposal_height: u64,
    /// Global proposal view that planned this lane-local payload.
    pub proposal_view: u64,
    /// Lane whose payload ownership is bound by this identity.
    pub lane_id: LaneId,
    /// Dataspace bound to the lane payload.
    pub dataspace_id: DataSpaceId,
    /// Exact active lane incarnation commitment.
    ///
    /// Recreated lanes receive a new commitment, so delayed artifacts from a
    /// retired incarnation remain invalid regardless of their lane-local height.
    pub lane_incarnation: Hash,
    /// Lane-local block height for the payload.
    pub lane_block_height: u64,
    /// Lane-local view for the payload.
    pub lane_block_view: u64,
    /// Stable digest of the lane-local block subject.
    pub subject_hash: Hash,
    /// Domain-separated QC mode tag used to derive the lane-local subject.
    pub qc_mode_tag: String,
    /// Fetched-batch candidate indices owned by this lane payload.
    pub accepted_candidate_indices: Vec<u64>,
    /// Accepted transaction hashes owned by this lane payload.
    pub accepted_transaction_hashes: Vec<Hash>,
    /// Lane-local predecessor height bound by the descriptor.
    pub previous_lane_block_height: u64,
    /// Descriptor hash of the lane-local predecessor, when the predecessor is known.
    #[norito(required)]
    pub previous_lane_block_descriptor_hash: Option<Hash>,
    /// Stable descriptor hash binding standalone lane block replay context.
    #[norito(required)]
    pub lane_block_descriptor_hash: Option<Hash>,
    /// Canonical validator set bound by the descriptor.
    pub lane_block_descriptor_validator_set: Vec<PeerId>,
    /// Validator count bound by the descriptor quorum context.
    pub lane_block_descriptor_validator_count: u32,
    /// Minimum quorum bound by the descriptor quorum context.
    pub lane_block_descriptor_min_quorum: u32,
    /// Stable digest naming lane-local payload ownership.
    pub payload_ownership_hash: Hash,
    /// Stable digest naming the lane-local RBC instance for this payload.
    pub rbc_instance_hash: Hash,
}
#[derive(Clone, Debug, Encode)]
struct LaneBlockProposalPreimage {
    purpose: String,
    version: u8,
    proposal_height: u64,
    descriptor_hash: Hash,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    lane_block_height: u64,
    lane_block_view: u64,
    subject_hash: Hash,
    payload_ownership_hash: Hash,
    rbc_instance_hash: Hash,
    candidate_indices: Vec<u64>,
    candidate_hashes: Vec<Hash>,
    validator_set_hash_version: u16,
    validator_set_hash: HashOf<Vec<PeerId>>,
    validator_set: Vec<PeerId>,
    validator_count: u32,
    min_quorum: u32,
    qc_mode_tag: String,
}
/// Canonical descriptor for a standalone lane-local block proposal.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct LaneBlockDescriptorV1 {
    /// Lane whose local block is described.
    pub lane_id: LaneId,
    /// Dataspace bound to the lane-local block.
    pub dataspace_id: DataSpaceId,
    /// Exact active lane incarnation commitment.
    pub lane_incarnation: Hash,
    /// Global proposal height that planned this lane-local block.
    pub proposal_height: u64,
    /// Latest committed lane-local height used as this block's predecessor tip.
    pub previous_lane_block_height: u64,
    /// Descriptor hash of the predecessor tip, when the predecessor is known.
    #[norito(required)]
    pub previous_lane_block_descriptor_hash: Option<Hash>,
    /// Lane-local block height assigned to the descriptor.
    pub lane_block_height: u64,
    /// Lane-local view assigned to the descriptor.
    pub lane_block_view: u64,
    /// Lane-local subject hash signed by lane validators.
    pub subject_hash: Hash,
    /// DA/RBC payload ownership hash.
    pub payload_ownership_hash: Hash,
    /// DA/RBC instance hash.
    pub rbc_instance_hash: Hash,
    /// Accepted fetched-batch candidate indices in scheduler order.
    pub accepted_candidate_indices: Vec<u64>,
    /// Accepted transaction hashes in scheduler order.
    pub accepted_transaction_hashes: Vec<Hash>,
    /// Version of the validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Stable hash of the validator set eligible to sign this lane block.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Canonical validator order eligible to sign this lane block.
    pub validator_set: Vec<PeerId>,
    /// Number of validators bound by the descriptor quorum context.
    pub validator_count: u32,
    /// Minimum distinct signer count required for quorum.
    pub min_quorum: u32,
    /// Domain-separated QC mode tag used for lane-local votes.
    pub qc_mode_tag: String,
    /// Stable descriptor digest binding predecessor, work, ownership, committee, and quorum.
    pub descriptor_hash: Hash,
}
impl LaneBlockDescriptorV1 {
    /// Compute the canonical descriptor hash from all descriptor fields except `descriptor_hash`.
    #[must_use]
    pub fn computed_descriptor_hash(&self) -> Hash {
        Hash::new(
            norito::encode_canonical(&LaneBlockDescriptorPreimage {
                purpose: "nexus:lane-block-descriptor:v1".to_string(),
                version: 1,
                lane_id: self.lane_id,
                dataspace_id: self.dataspace_id,
                lane_incarnation: self.lane_incarnation,
                proposal_height: self.proposal_height,
                previous_lane_block_height: self.previous_lane_block_height,
                previous_lane_block_descriptor_hash: self.previous_lane_block_descriptor_hash,
                lane_block_height: self.lane_block_height,
                lane_block_view: self.lane_block_view,
                subject_hash: self.subject_hash,
                payload_ownership_hash: self.payload_ownership_hash,
                rbc_instance_hash: self.rbc_instance_hash,
                candidate_indices: self.accepted_candidate_indices.clone(),
                candidate_hashes: self.accepted_transaction_hashes.clone(),
                validator_set_hash_version: self.validator_set_hash_version,
                validator_set_hash: self.validator_set_hash,
                validator_set: self.validator_set.clone(),
                validator_count: self.validator_count,
                min_quorum: self.min_quorum,
                qc_mode_tag: self.qc_mode_tag.clone(),
            })
            .expect("lane block descriptor must encode"),
        )
    }
    /// Compute the canonical validator-set hash for the embedded validator order.
    #[must_use]
    pub fn computed_validator_set_hash(&self) -> HashOf<Vec<PeerId>> {
        HashOf::new(&self.validator_set)
    }
}
/// Advisory pointer to the canonical global block that carried a lane payload.
///
/// This is deliberately not part of [`LaneBlockProposalV1::computed_proposal_hash`]. Peers use it
/// only as a recovery hint for fetching a certified block body; the fetched block still has to
/// validate against its commit certificate and the lane descriptor before any payload is replayed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct LaneBlockProposalPayloadHintV1 {
    /// Global proposal height that anchored the lane payload ownership.
    pub proposal_height: u64,
    /// Global proposal view that anchored the lane payload ownership.
    pub proposal_view: u64,
    /// Hash of the global block body that carried the lane payload ownership.
    pub proposal_block_hash: HashOf<BlockHeader>,
}
/// Canonical standalone lane-local block proposal artifact.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct LaneBlockProposalV1 {
    /// Replayable descriptor proposed to the lane committee.
    pub descriptor: LaneBlockDescriptorV1,
    /// Stable proposal digest binding descriptor, work, committee, and quorum.
    pub proposal_hash: Hash,
    /// Optional recovery hint for fetching the global block body with the payload.
    #[norito(required)]
    pub payload_block_hint: Option<LaneBlockProposalPayloadHintV1>,
}
impl LaneBlockProposalV1 {
    /// Compute the canonical proposal hash from the embedded descriptor.
    #[must_use]
    pub fn computed_proposal_hash(&self) -> Hash {
        let descriptor = &self.descriptor;
        Hash::new(
            norito::encode_canonical(&LaneBlockProposalPreimage {
                purpose: "nexus:lane-block-proposal:v1".to_string(),
                version: 1,
                proposal_height: descriptor.proposal_height,
                descriptor_hash: descriptor.descriptor_hash,
                lane_id: descriptor.lane_id,
                dataspace_id: descriptor.dataspace_id,
                lane_incarnation: descriptor.lane_incarnation,
                lane_block_height: descriptor.lane_block_height,
                lane_block_view: descriptor.lane_block_view,
                subject_hash: descriptor.subject_hash,
                payload_ownership_hash: descriptor.payload_ownership_hash,
                rbc_instance_hash: descriptor.rbc_instance_hash,
                candidate_indices: descriptor.accepted_candidate_indices.clone(),
                candidate_hashes: descriptor.accepted_transaction_hashes.clone(),
                validator_set_hash_version: descriptor.validator_set_hash_version,
                validator_set_hash: descriptor.validator_set_hash,
                validator_set: descriptor.validator_set.clone(),
                validator_count: descriptor.validator_count,
                min_quorum: descriptor.min_quorum,
                qc_mode_tag: descriptor.qc_mode_tag.clone(),
            })
            .expect("lane block proposal must encode"),
        )
    }
    /// Return `true` when two proposals identify the same certified lane block.
    #[must_use]
    pub fn same_consensus_identity(&self, other: &Self) -> bool {
        self.descriptor == other.descriptor && self.proposal_hash == other.proposal_hash
    }
    /// Attach a payload recovery hint without changing the proposal identity.
    #[must_use]
    pub fn with_payload_block_hint(mut self, hint: LaneBlockProposalPayloadHintV1) -> Self {
        self.payload_block_hint = Some(hint);
        self
    }
    /// Build a canonical lane-block vote body for this proposal and phase.
    #[must_use]
    pub fn vote_body(&self, phase: CertPhase) -> LaneBlockVoteBodyV1 {
        let descriptor = &self.descriptor;
        LaneBlockVoteBodyV1 {
            phase,
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            proposal_height: descriptor.proposal_height,
            lane_block_height: descriptor.lane_block_height,
            lane_block_view: descriptor.lane_block_view,
            proposal_hash: self.proposal_hash,
            descriptor_hash: descriptor.descriptor_hash,
            subject_hash: descriptor.subject_hash,
            payload_ownership_hash: descriptor.payload_ownership_hash,
            rbc_instance_hash: descriptor.rbc_instance_hash,
            accepted_candidate_indices: descriptor.accepted_candidate_indices.clone(),
            accepted_transaction_hashes: descriptor.accepted_transaction_hashes.clone(),
            validator_set_hash_version: descriptor.validator_set_hash_version,
            validator_set_hash: descriptor.validator_set_hash,
            validator_count: descriptor.validator_count,
            min_quorum: descriptor.min_quorum,
            qc_mode_tag: descriptor.qc_mode_tag.clone(),
        }
    }
}
/// Canonical lane-local block vote payload signed by lane committees.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct LaneBlockVoteBodyV1 {
    /// Lane-local QC phase certified by this vote body.
    pub phase: CertPhase,
    /// Lane whose local block is being certified.
    pub lane_id: LaneId,
    /// Dataspace bound to the lane-local block.
    pub dataspace_id: DataSpaceId,
    /// Exact active lane incarnation commitment.
    pub lane_incarnation: Hash,
    /// Global proposal height that planned this lane-local block.
    pub proposal_height: u64,
    /// Lane-local block height being certified.
    pub lane_block_height: u64,
    /// Lane-local view being certified.
    pub lane_block_view: u64,
    /// Standalone lane-block proposal hash.
    pub proposal_hash: Hash,
    /// Standalone lane-block descriptor hash.
    pub descriptor_hash: Hash,
    /// Lane-local subject hash.
    pub subject_hash: Hash,
    /// DA/RBC payload ownership hash.
    pub payload_ownership_hash: Hash,
    /// DA/RBC instance hash.
    pub rbc_instance_hash: Hash,
    /// Accepted fetched-batch candidate indices in scheduler order.
    pub accepted_candidate_indices: Vec<u64>,
    /// Accepted transaction hashes in scheduler order.
    pub accepted_transaction_hashes: Vec<Hash>,
    /// Version of the validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Stable hash of the validator set that may sign this lane block.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Number of validators bound by the descriptor quorum context.
    pub validator_count: u32,
    /// Minimum distinct signer count required for quorum.
    pub min_quorum: u32,
    /// Domain-separated QC mode tag for this lane block.
    pub qc_mode_tag: String,
}
impl LaneBlockVoteBodyV1 {
    /// Build the domain-separated signature preimage for this lane-block vote body.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + 512);
        out.extend_from_slice(b"iroha:lane-block-vote:v1");
        out.extend_from_slice(
            &norito::encode_canonical(self).expect("lane block vote body must encode"),
        );
        out
    }
}
/// Exact autonomous lane payload retained by one READY signer.
///
/// The body names both the immutable payload's origin proposal and the view-specific proposal being
/// prepared. This prevents a valid payload certificate from being rebound across networks, epochs,
/// lane incarnations, proposals, `NewView` transitions, or DA/RBC instances.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct LanePayloadAvailabilityBodyV1 {
    /// Artifact schema version. Only version one is accepted.
    pub version: u8,
    /// Exact genesis-derived network identity that owns the payload.
    pub network_id: NetworkId,
    /// Consensus epoch at the global proposal height.
    pub epoch: u64,
    /// Lane whose executable payload is retained.
    pub lane_id: LaneId,
    /// Dataspace bound to the lane.
    pub dataspace_id: DataSpaceId,
    /// Exact lane lifecycle incarnation.
    pub lane_incarnation: Hash,
    /// Global proposal height that selected the lane work.
    pub proposal_height: u64,
    /// Lane-local block height whose bytes are retained.
    pub lane_block_height: u64,
    /// View of the immutable producer-authenticated origin proposal.
    pub origin_lane_block_view: u64,
    /// Hash of the immutable producer-authenticated origin proposal.
    pub origin_proposal_hash: Hash,
    /// Descriptor hash of the immutable origin proposal.
    pub origin_descriptor_hash: Hash,
    /// View of the exact proposal currently being prepared.
    pub current_lane_block_view: u64,
    /// Hash of the exact proposal currently being prepared.
    pub current_proposal_hash: Hash,
    /// Descriptor hash of the exact proposal currently being prepared.
    pub current_descriptor_hash: Hash,
    /// View-specific lane subject hash.
    pub current_subject_hash: Hash,
    /// View-specific DA/RBC payload ownership hash.
    pub current_payload_ownership_hash: Hash,
    /// View-specific reliable-broadcast instance hash.
    pub current_rbc_instance_hash: Hash,
    /// View-neutral digest of the exact executable payload bytes.
    pub executable_payload_hash: Hash,
    /// Version of the validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Hash of the canonical lane committee.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Number of validators in the canonical lane committee.
    pub validator_count: u32,
    /// Minimum distinct READY signers required for availability.
    pub min_quorum: u32,
    /// Lane consensus domain tag.
    pub qc_mode_tag: String,
}
impl LanePayloadAvailabilityBodyV1 {
    /// Build the domain-separated READY signature preimage.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + 512);
        out.extend_from_slice(b"iroha:lane-payload-availability-ready:v1");
        out.extend_from_slice(
            &norito::encode_canonical(self).expect("lane payload availability body must encode"),
        );
        out
    }
}
/// Quorum proof that the exact autonomous executable payload is durably held.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct LanePayloadAvailabilityQcV1 {
    /// READY body certified by the aggregate signature.
    pub body: LanePayloadAvailabilityBodyV1,
    /// Version of the validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Stable hash of the validator set that produced the certificate.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Ordered historical validator set indexed by `signers_bitmap`.
    pub validator_set: Vec<PeerId>,
    /// Valid historical `PoPs` aligned exactly with `validator_set`.
    pub validator_set_pops: Vec<Vec<u8>>,
    /// Compact READY signer bitmap (LSB-first).
    pub signers_bitmap: Vec<u8>,
    /// BLS12-381 aggregate READY signature bytes (compressed).
    pub bls_aggregate_signature: Vec<u8>,
}
/// Validator-set proof for a standalone lane-local block proposal.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct LaneBlockQcV1 {
    /// Vote body certified by the aggregate signature.
    pub body: LaneBlockVoteBodyV1,
    /// Version of the validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Stable hash of the validator set that produced the certificate.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Ordered validator set used when assembling the certificate.
    pub validator_set: Vec<PeerId>,
    /// Compact signer bitmap (LSB-first).
    pub signers_bitmap: Vec<u8>,
    /// BLS12-381 aggregate signature bytes (compressed).
    pub bls_aggregate_signature: Vec<u8>,
    /// Exact payload-availability proof for autonomous prepare QCs.
    ///
    /// This is `None` for commit QCs and for globally anchored lane proposals
    /// whose payload availability is inherited from a canonical global block.
    #[norito(required)]
    pub payload_availability_qc: Option<LanePayloadAvailabilityQcV1>,
}
/// Complete certified lane-block artifact used for authenticated recovery.
///
/// A lagging validator retransmits the exact canonical proposal as an idempotent request. A peer
/// which durably retains the matching Kura artifact returns this single envelope, so Prepare and
/// Commit evidence cannot be split across a volatile transport-capacity boundary.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct LaneBlockCertificateV1 {
    /// Exact canonical proposal certified by both quorum certificates.
    pub proposal: LaneBlockProposalV1,
    /// Prepare quorum certificate for [`Self::proposal`].
    pub prepare_qc: LaneBlockQcV1,
    /// Commit quorum certificate for [`Self::proposal`].
    pub commit_qc: LaneBlockQcV1,
}
#[derive(Clone, Debug, Encode)]
struct LanePayloadOwnershipSubjectPreimage {
    version: u8,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    lane_block_height: u64,
    lane_block_view: u64,
    candidate_indices: Vec<u64>,
    candidate_hashes: Vec<Hash>,
    qc_mode_tag: String,
}
#[derive(Clone, Debug, Encode)]
struct LanePayloadOwnershipPreimage {
    purpose: String,
    version: u8,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    lane_block_height: u64,
    lane_block_view: u64,
    subject_hash: Hash,
    candidate_indices: Vec<u64>,
    candidate_hashes: Vec<Hash>,
    qc_mode_tag: String,
}
#[derive(Clone, Debug, Encode)]
struct LanePayloadOwnershipRbcPreimage {
    purpose: String,
    version: u8,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    lane_block_height: u64,
    lane_block_view: u64,
    subject_hash: Hash,
    payload_ownership_hash: Hash,
}
#[derive(Clone, Debug, Encode)]
struct LaneBlockDescriptorPreimage {
    purpose: String,
    version: u8,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    proposal_height: u64,
    previous_lane_block_height: u64,
    previous_lane_block_descriptor_hash: Option<Hash>,
    lane_block_height: u64,
    lane_block_view: u64,
    subject_hash: Hash,
    payload_ownership_hash: Hash,
    rbc_instance_hash: Hash,
    candidate_indices: Vec<u64>,
    candidate_hashes: Vec<Hash>,
    validator_set_hash_version: u16,
    validator_set_hash: HashOf<Vec<PeerId>>,
    validator_set: Vec<PeerId>,
    validator_count: u32,
    min_quorum: u32,
    qc_mode_tag: String,
}
/// Canonical lane payload ownership replay hashes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SumeragiLanePayloadOwnershipReplayHashes {
    /// Expected lane-local block subject hash.
    pub subject_hash: Hash,
    /// Expected lane-local payload ownership hash.
    pub payload_ownership_hash: Hash,
    /// Expected lane-local RBC instance hash.
    pub rbc_instance_hash: Hash,
    /// Expected standalone lane block descriptor hash.
    pub lane_block_descriptor_hash: Hash,
}
/// Validation error for lane payload ownership replay material.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SumeragiLanePayloadOwnershipReplayError {
    /// Lane incarnation commitment is the reserved all-zero value.
    ZeroLaneIncarnation,
    /// QC mode tag is empty.
    BlankQcModeTag,
    /// No accepted candidate indices are present.
    EmptyCandidateIndices,
    /// Candidate index and accepted transaction hash counts differ.
    CandidateHashCountMismatch,
    /// Lane block height is zero.
    ZeroLaneBlockHeight,
    /// Previous lane block height does not equal `lane_block_height - 1`.
    PreviousLaneBlockHeightMismatch,
    /// Genesis predecessor unexpectedly carries a descriptor hash.
    UnexpectedGenesisPredecessorDescriptorHash,
    /// Descriptor hash is absent.
    MissingDescriptorHash,
    /// Descriptor validator set is empty.
    EmptyValidatorSet,
    /// Descriptor validator set is not in canonical sorted order.
    ValidatorSetNotCanonical,
    /// Descriptor validator set contains duplicate peers.
    DuplicateValidator,
    /// Descriptor validator count does not match the validator set length.
    ValidatorCountMismatch,
    /// Descriptor quorum is zero or exceeds validator count.
    InvalidQuorum,
    /// Norito encoding failed while deriving replay hashes.
    Encode,
    /// Subject hash does not match the replay material.
    SubjectHashMismatch,
    /// Payload ownership hash does not match the replay material.
    PayloadOwnershipHashMismatch,
    /// RBC instance hash does not match the replay material.
    RbcInstanceHashMismatch,
    /// Descriptor hash does not match the replay material.
    DescriptorHashMismatch,
}
impl fmt::Display for SumeragiLanePayloadOwnershipReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::ZeroLaneIncarnation => "zero lane incarnation commitment",
            Self::BlankQcModeTag => "blank QC mode tag",
            Self::EmptyCandidateIndices => "empty candidate indices",
            Self::CandidateHashCountMismatch => "candidate hash count mismatch",
            Self::ZeroLaneBlockHeight => "zero lane block height",
            Self::PreviousLaneBlockHeightMismatch => "previous lane block height mismatch",
            Self::UnexpectedGenesisPredecessorDescriptorHash => {
                "unexpected genesis predecessor descriptor hash"
            }
            Self::MissingDescriptorHash => "missing descriptor hash",
            Self::EmptyValidatorSet => "empty descriptor validator set",
            Self::ValidatorSetNotCanonical => "non-canonical descriptor validator set",
            Self::DuplicateValidator => "duplicate descriptor validator",
            Self::ValidatorCountMismatch => "descriptor validator count mismatch",
            Self::InvalidQuorum => "invalid descriptor quorum",
            Self::Encode => "failed to encode replay preimage",
            Self::SubjectHashMismatch => "subject hash mismatch",
            Self::PayloadOwnershipHashMismatch => "payload ownership hash mismatch",
            Self::RbcInstanceHashMismatch => "RBC instance hash mismatch",
            Self::DescriptorHashMismatch => "descriptor hash mismatch",
        };
        f.write_str(message)
    }
}
impl SumeragiLanePayloadOwnership {
    /// Compute the canonical lane-local subject hash from replay material.
    ///
    /// # Errors
    ///
    /// Returns [`SumeragiLanePayloadOwnershipReplayError::Encode`] if the
    /// canonical preimage cannot be encoded.
    #[expect(
        clippy::too_many_arguments,
        reason = "the public replay helper mirrors the canonical V1 subject preimage fields; grouping them would change the established API contract"
    )]
    pub fn compute_replay_subject_hash(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        lane_block_height: u64,
        lane_block_view: u64,
        accepted_candidate_indices: &[u64],
        accepted_transaction_hashes: &[Hash],
        qc_mode_tag: &str,
    ) -> Result<Hash, SumeragiLanePayloadOwnershipReplayError> {
        Ok(Hash::new(
            norito::encode_canonical(&LanePayloadOwnershipSubjectPreimage {
                version: 1,
                lane_id,
                dataspace_id,
                lane_incarnation,
                lane_block_height,
                lane_block_view,
                candidate_indices: accepted_candidate_indices.to_vec(),
                candidate_hashes: accepted_transaction_hashes.to_vec(),
                qc_mode_tag: qc_mode_tag.to_string(),
            })
            .map_err(|_| SumeragiLanePayloadOwnershipReplayError::Encode)?,
        ))
    }
    /// Compute the canonical lane-local payload ownership hash.
    ///
    /// # Errors
    ///
    /// Returns [`SumeragiLanePayloadOwnershipReplayError::Encode`] if the
    /// canonical preimage cannot be encoded.
    #[expect(
        clippy::too_many_arguments,
        reason = "the public replay helper mirrors the canonical V1 ownership preimage fields; grouping them would change the established API contract"
    )]
    pub fn compute_replay_payload_ownership_hash(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        lane_block_height: u64,
        lane_block_view: u64,
        subject_hash: Hash,
        accepted_candidate_indices: &[u64],
        accepted_transaction_hashes: &[Hash],
        qc_mode_tag: &str,
    ) -> Result<Hash, SumeragiLanePayloadOwnershipReplayError> {
        Ok(Hash::new(
            norito::encode_canonical(&LanePayloadOwnershipPreimage {
                purpose: "nexus:lane-payload-ownership:v1".to_string(),
                version: 1,
                lane_id,
                dataspace_id,
                lane_incarnation,
                lane_block_height,
                lane_block_view,
                subject_hash,
                candidate_indices: accepted_candidate_indices.to_vec(),
                candidate_hashes: accepted_transaction_hashes.to_vec(),
                qc_mode_tag: qc_mode_tag.to_string(),
            })
            .map_err(|_| SumeragiLanePayloadOwnershipReplayError::Encode)?,
        ))
    }
    /// Compute the canonical lane-local RBC instance hash.
    ///
    /// # Errors
    ///
    /// Returns [`SumeragiLanePayloadOwnershipReplayError::Encode`] if the
    /// canonical preimage cannot be encoded.
    pub fn compute_replay_rbc_instance_hash(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        lane_incarnation: Hash,
        lane_block_height: u64,
        lane_block_view: u64,
        subject_hash: Hash,
        payload_ownership_hash: Hash,
    ) -> Result<Hash, SumeragiLanePayloadOwnershipReplayError> {
        Ok(Hash::new(
            norito::encode_canonical(&LanePayloadOwnershipRbcPreimage {
                purpose: "nexus:lane-rbc-instance:v1".to_string(),
                version: 1,
                lane_id,
                dataspace_id,
                lane_incarnation,
                lane_block_height,
                lane_block_view,
                subject_hash,
                payload_ownership_hash,
            })
            .map_err(|_| SumeragiLanePayloadOwnershipReplayError::Encode)?,
        ))
    }
    /// Compute canonical replay hashes from the embedded descriptor material.
    ///
    /// # Errors
    ///
    /// Returns [`SumeragiLanePayloadOwnershipReplayError`] when required replay
    /// material is missing, malformed, or cannot be encoded.
    pub fn compute_replay_hashes(
        &self,
    ) -> Result<SumeragiLanePayloadOwnershipReplayHashes, SumeragiLanePayloadOwnershipReplayError>
    {
        self.validate_replay_shape()?;
        let subject_hash = Self::compute_replay_subject_hash(
            self.lane_id,
            self.dataspace_id,
            self.lane_incarnation,
            self.lane_block_height,
            self.lane_block_view,
            &self.accepted_candidate_indices,
            &self.accepted_transaction_hashes,
            &self.qc_mode_tag,
        )?;
        let payload_ownership_hash = Self::compute_replay_payload_ownership_hash(
            self.lane_id,
            self.dataspace_id,
            self.lane_incarnation,
            self.lane_block_height,
            self.lane_block_view,
            subject_hash,
            &self.accepted_candidate_indices,
            &self.accepted_transaction_hashes,
            &self.qc_mode_tag,
        )?;
        let rbc_instance_hash = Self::compute_replay_rbc_instance_hash(
            self.lane_id,
            self.dataspace_id,
            self.lane_incarnation,
            self.lane_block_height,
            self.lane_block_view,
            subject_hash,
            payload_ownership_hash,
        )?;
        let lane_block_descriptor_hash = Hash::new(
            norito::encode_canonical(&LaneBlockDescriptorPreimage {
                purpose: "nexus:lane-block-descriptor:v1".to_string(),
                version: 1,
                lane_id: self.lane_id,
                dataspace_id: self.dataspace_id,
                lane_incarnation: self.lane_incarnation,
                proposal_height: self.proposal_height,
                previous_lane_block_height: self.previous_lane_block_height,
                previous_lane_block_descriptor_hash: self.previous_lane_block_descriptor_hash,
                lane_block_height: self.lane_block_height,
                lane_block_view: self.lane_block_view,
                subject_hash,
                payload_ownership_hash,
                rbc_instance_hash,
                candidate_indices: self.accepted_candidate_indices.clone(),
                candidate_hashes: self.accepted_transaction_hashes.clone(),
                validator_set_hash_version: crate::consensus::VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&self.lane_block_descriptor_validator_set),
                validator_set: self.lane_block_descriptor_validator_set.clone(),
                validator_count: self.lane_block_descriptor_validator_count,
                min_quorum: self.lane_block_descriptor_min_quorum,
                qc_mode_tag: self.qc_mode_tag.clone(),
            })
            .map_err(|_| SumeragiLanePayloadOwnershipReplayError::Encode)?,
        );
        Ok(SumeragiLanePayloadOwnershipReplayHashes {
            subject_hash,
            payload_ownership_hash,
            rbc_instance_hash,
            lane_block_descriptor_hash,
        })
    }
    /// Validate embedded replay material and all canonical ownership hashes.
    ///
    /// # Errors
    ///
    /// Returns [`SumeragiLanePayloadOwnershipReplayError`] when any replay field
    /// or canonical hash does not match the lane-local payload ownership.
    pub fn validate_replay_material(&self) -> Result<(), SumeragiLanePayloadOwnershipReplayError> {
        let expected = self.compute_replay_hashes()?;
        if self.subject_hash != expected.subject_hash {
            return Err(SumeragiLanePayloadOwnershipReplayError::SubjectHashMismatch);
        }
        if self.payload_ownership_hash != expected.payload_ownership_hash {
            return Err(SumeragiLanePayloadOwnershipReplayError::PayloadOwnershipHashMismatch);
        }
        if self.rbc_instance_hash != expected.rbc_instance_hash {
            return Err(SumeragiLanePayloadOwnershipReplayError::RbcInstanceHashMismatch);
        }
        if self.lane_block_descriptor_hash != Some(expected.lane_block_descriptor_hash) {
            return Err(SumeragiLanePayloadOwnershipReplayError::DescriptorHashMismatch);
        }
        Ok(())
    }
    fn validate_replay_shape(&self) -> Result<(), SumeragiLanePayloadOwnershipReplayError> {
        if self.lane_incarnation.as_ref().iter().all(|byte| *byte == 0) {
            return Err(SumeragiLanePayloadOwnershipReplayError::ZeroLaneIncarnation);
        }
        if self.qc_mode_tag.trim().is_empty() {
            return Err(SumeragiLanePayloadOwnershipReplayError::BlankQcModeTag);
        }
        if self.accepted_candidate_indices.is_empty() {
            return Err(SumeragiLanePayloadOwnershipReplayError::EmptyCandidateIndices);
        }
        if self.accepted_candidate_indices.len() != self.accepted_transaction_hashes.len() {
            return Err(SumeragiLanePayloadOwnershipReplayError::CandidateHashCountMismatch);
        }
        let Some(expected_previous) = self.lane_block_height.checked_sub(1) else {
            return Err(SumeragiLanePayloadOwnershipReplayError::ZeroLaneBlockHeight);
        };
        if self.previous_lane_block_height != expected_previous {
            return Err(SumeragiLanePayloadOwnershipReplayError::PreviousLaneBlockHeightMismatch);
        }
        if self.previous_lane_block_height == 0
            && self.previous_lane_block_descriptor_hash.is_some()
        {
            return Err(
                SumeragiLanePayloadOwnershipReplayError::UnexpectedGenesisPredecessorDescriptorHash,
            );
        }
        if self.previous_lane_block_height > 0 && self.previous_lane_block_descriptor_hash.is_none()
        {
            // Keep the public error surface stable: this variant also covers a
            // required predecessor descriptor hash that is absent.
            return Err(SumeragiLanePayloadOwnershipReplayError::MissingDescriptorHash);
        }
        if self.lane_block_descriptor_hash.is_none() {
            return Err(SumeragiLanePayloadOwnershipReplayError::MissingDescriptorHash);
        }
        if self.lane_block_descriptor_validator_set.is_empty() {
            return Err(SumeragiLanePayloadOwnershipReplayError::EmptyValidatorSet);
        }
        let mut canonical_validator_set = self.lane_block_descriptor_validator_set.clone();
        canonical_validator_set.sort();
        if canonical_validator_set != self.lane_block_descriptor_validator_set {
            return Err(SumeragiLanePayloadOwnershipReplayError::ValidatorSetNotCanonical);
        }
        for pair in canonical_validator_set.windows(2) {
            if pair[0] == pair[1] {
                return Err(SumeragiLanePayloadOwnershipReplayError::DuplicateValidator);
            }
        }
        let Ok(validator_count) = u32::try_from(self.lane_block_descriptor_validator_set.len())
        else {
            return Err(SumeragiLanePayloadOwnershipReplayError::ValidatorCountMismatch);
        };
        if self.lane_block_descriptor_validator_count != validator_count {
            return Err(SumeragiLanePayloadOwnershipReplayError::ValidatorCountMismatch);
        }
        if self.lane_block_descriptor_min_quorum == 0
            || self.lane_block_descriptor_min_quorum > self.lane_block_descriptor_validator_count
        {
            return Err(SumeragiLanePayloadOwnershipReplayError::InvalidQuorum);
        }
        Ok(())
    }
}
/// Deterministic settlement receipt emitted for audit and reconciliation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
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
/// Native AMX v2 receipt version accepted by live diagnostics clients.
pub const NATIVE_AMX_RECEIPT_VERSION_V2: u16 = 2;
/// Maximum number of ordered transaction sources in one grouped Native AMX control.
pub const NATIVE_AMX_GROUP_SOURCES_MAX: usize = 4_096;
/// Maximum participant legs after reserving one route-plan slot for the coordinator.
pub const NATIVE_AMX_PARTICIPANT_LEGS_MAX: usize = 255;
/// Maximum validators in one Native AMX participant committee.
pub const NATIVE_AMX_VALIDATORS_MAX: usize = 128;
/// Canonical compressed BLS-normal proof-of-possession and signature size.
pub const NATIVE_AMX_BLS_PROOF_BYTES: usize = 96;
/// Phase certified by a native AMX participant committee.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "phase",
    content = "detail",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum NativeAmxPhase {
    /// Participant prepared its dataspace-local leg.
    Prepare,
    /// Participant committed its dataspace-local leg.
    Commit,
}
/// Canonical Sumeragi v2 native AMX attestation payload.
///
/// The exact frozen round and election epoch are part of the signed payload,
/// preventing a valid lane-local vote from being replayed across networks,
/// parent decisions, epochs, heights, or views.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct NativeAmxAttestationBodyV2 {
    /// Exact frozen global round in which the receipt may be included.
    pub round: super::consensus_v2::ConsensusRound,
    /// Finalized election epoch repeated from the frozen height context.
    pub epoch: u64,
    /// Exact genesis-derived network identity that owns this attestation.
    pub network_id: NetworkId,
    /// Source transaction hash/id.
    pub source_id: [u8; 32],
    /// Hash of the canonical transaction entrypoint.
    pub tx_entrypoint_hash: HashOf<crate::transaction::TransactionEntrypoint>,
    /// Deterministic digest of the full coordinator/participant routing plan.
    pub plan_digest: Hash,
    /// Native AMX phase certified by this body.
    pub phase: NativeAmxPhase,
    /// Coordinator lane selected by the routing plan.
    pub coordinator_lane_id: LaneId,
    /// Coordinator dataspace selected by the routing plan.
    pub coordinator_dataspace_id: DataSpaceId,
    /// Exact active coordinator-lane incarnation at the frozen authority context.
    pub coordinator_lane_incarnation: Hash,
    /// Participant lane certified by the committee.
    pub participant_lane_id: LaneId,
    /// Participant dataspace certified by the committee.
    pub participant_dataspace_id: DataSpaceId,
    /// Exact active participant-lane incarnation at the frozen authority context.
    pub participant_lane_incarnation: Hash,
    /// Last globally anchored Native AMX participant block for this lane incarnation.
    pub participant_previous_block_height: u64,
    /// Descriptor hash of the last globally anchored participant block, when one exists.
    #[norito(required)]
    pub participant_previous_block_descriptor_hash: Option<Hash>,
    /// Contiguous Native AMX participant-lane block height certified by this vote.
    pub participant_lane_block_height: u64,
    /// Participant-lane consensus view certified independently from the coordinator view.
    pub participant_lane_block_view: u64,
    /// Exact participant-lane proposal certified by this vote.
    pub participant_proposal_hash: Hash,
    /// Commitment to this transaction's participant-local settlement leaf.
    pub participant_settlement_commitment: Hash,
    /// Hash of the exact canonical participant committee that may attest this leg.
    pub participant_validator_set_hash: HashOf<Vec<PeerId>>,
    /// Number of validators in the exact participant committee.
    pub participant_validator_count: u32,
    /// Minimum number of participant signatures required by the lane quorum policy.
    pub participant_min_quorum: u32,
    /// Global/catalog height used to resolve routes, lane incarnations, keys, and `PoPs`.
    pub authority_context_height: u64,
    /// Coordinator block height planned for final inclusion.
    pub planned_coordinator_block_height: u64,
    /// Coordinator lane-local consensus view for this exact attestation.
    pub coordinator_lane_block_view: u64,
    /// Exact coordinator lane-block proposal authenticated by the full-plan request.
    pub coordinator_proposal_hash: Hash,
}
impl NativeAmxAttestationBodyV2 {
    /// Build the domain-separated signature preimage for this v2 attestation.
    #[must_use]
    pub fn signature_preimage(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(32 + 320);
        out.extend_from_slice(b"iroha:native-amx:v2");
        out.extend_from_slice(
            &norito::encode_canonical(self).expect("native AMX v2 attestation body must encode"),
        );
        out
    }
    /// Build the exact grouped zero-effect participant settlement certified by this body.
    ///
    /// The source group is shared by every receipt in one Native AMX control. Its caller-provided
    /// order is the canonical candidate/block order, so this constructor preserves it exactly. The
    /// group must contain this body's source exactly once and must not contain duplicates.
    ///
    /// # Errors
    ///
    /// Returns a stable reason when the group is empty, oversized, duplicated, or does not contain
    /// this body's source exactly once.
    pub fn computed_grouped_participant_settlement(
        &self,
        sources: &[[u8; 32]],
    ) -> Result<LaneBlockCommitment, &'static str> {
        if sources.is_empty() || sources.len() > NATIVE_AMX_GROUP_SOURCES_MAX {
            return Err("Native AMX participant source group is out of bounds");
        }
        if sources
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != sources.len()
        {
            return Err("Native AMX participant source group must be unique");
        }
        if sources
            .iter()
            .filter(|source| **source == self.source_id)
            .count()
            != 1
        {
            return Err("Native AMX participant source group must contain the current source once");
        }
        let tx_count = u64::try_from(sources.len())
            .map_err(|_| "Native AMX participant source group is out of bounds")?;
        Ok(LaneBlockCommitment {
            block_height: self.participant_lane_block_height,
            lane_id: self.participant_lane_id,
            lane_incarnation: self.participant_lane_incarnation,
            dataspace_id: self.participant_dataspace_id,
            tx_count,
            total_local_amount: Quantity::zero(),
            total_xor_due: Quantity::zero(),
            total_xor_after_haircut: Quantity::zero(),
            total_xor_variance: Quantity::zero(),
            swap_metadata: None,
            receipts: sources
                .iter()
                .copied()
                .map(|source_id| LaneSettlementReceipt {
                    source_id,
                    local_amount: Quantity::zero(),
                    xor_due: Quantity::zero(),
                    xor_after_haircut: Quantity::zero(),
                    xor_variance: Quantity::zero(),
                    timestamp_ms: self.authority_context_height,
                })
                .collect(),
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        })
    }
    /// Compute the commitment to an exact grouped participant settlement.
    ///
    /// # Errors
    ///
    /// Returns the same source-group validation errors as
    /// [`Self::computed_grouped_participant_settlement`].
    pub fn computed_grouped_participant_settlement_commitment(
        &self,
        sources: &[[u8; 32]],
    ) -> Result<Hash, &'static str> {
        let settlement = self.computed_grouped_participant_settlement(sources)?;
        Ok(Hash::from(
            crate::nexus::compute_settlement_hash(&settlement)
                .expect("native AMX participant settlement must hash"),
        ))
    }
}
/// Error returned when a native AMX validator set and its proofs of possession
/// are not aligned one-for-one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NativeAmxAttestationQcV2AlignmentError {
    validator_count: usize,
    proof_count: usize,
}
impl NativeAmxAttestationQcV2AlignmentError {
    const fn new(validator_count: usize, proof_count: usize) -> Self {
        Self {
            validator_count,
            proof_count,
        }
    }
    /// Number of validators supplied to the rejected constructor or decoder.
    #[must_use]
    pub const fn validator_count(self) -> usize {
        self.validator_count
    }
    /// Number of proofs of possession supplied to the rejected constructor or decoder.
    #[must_use]
    pub const fn proof_count(self) -> usize {
        self.proof_count
    }
}
impl fmt::Display for NativeAmxAttestationQcV2AlignmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "native AMX validator set has {} validators but {} proofs of possession",
            self.validator_count, self.proof_count
        )
    }
}
impl std::error::Error for NativeAmxAttestationQcV2AlignmentError {}
/// Validator-set proof for a context-bound native AMX v2 attestation.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(crate::DeriveJsonSerialize))]
pub struct NativeAmxAttestationQcV2 {
    /// Context-bound body certified by the aggregate signature.
    pub body: NativeAmxAttestationBodyV2,
    /// Version of the validator-set hashing scheme.
    pub validator_set_hash_version: u16,
    /// Stable hash of the validator set that produced the certificate.
    pub validator_set_hash: HashOf<Vec<PeerId>>,
    /// Ordered validator set used when assembling the certificate.
    validator_set: Vec<PeerId>,
    /// Historical BLS proofs-of-possession aligned exactly with `validator_set`.
    ///
    /// Embedding the full aligned vector keeps a certificate independently
    /// verifiable after consensus-key rotation or lane retirement.
    validator_set_pops: Vec<Vec<u8>>,
    /// Compact signer bitmap (LSB-first).
    pub signers_bitmap: Vec<u8>,
    /// BLS12-381 aggregate signature bytes (compressed).
    pub bls_aggregate_signature: Vec<u8>,
}
impl NativeAmxAttestationQcV2 {
    /// Construct a certificate with an aligned validator set and proof vector.
    ///
    /// # Errors
    ///
    /// Returns an error when `validator_set` and `validator_set_pops` do not
    /// contain exactly the same number of entries.
    pub fn try_new(
        body: NativeAmxAttestationBodyV2,
        validator_set_hash_version: u16,
        validator_set_hash: HashOf<Vec<PeerId>>,
        validator_set: Vec<PeerId>,
        validator_set_pops: Vec<Vec<u8>>,
        signers_bitmap: Vec<u8>,
        bls_aggregate_signature: Vec<u8>,
    ) -> Result<Self, NativeAmxAttestationQcV2AlignmentError> {
        if validator_set.len() != validator_set_pops.len() {
            return Err(NativeAmxAttestationQcV2AlignmentError::new(
                validator_set.len(),
                validator_set_pops.len(),
            ));
        }
        Ok(Self {
            body,
            validator_set_hash_version,
            validator_set_hash,
            validator_set,
            validator_set_pops,
            signers_bitmap,
            bls_aggregate_signature,
        })
    }
    /// Return the ordered validator set certified by this QC.
    #[must_use]
    pub fn validator_set(&self) -> &[PeerId] {
        &self.validator_set
    }
    /// Return the proofs of possession aligned with [`Self::validator_set`].
    #[must_use]
    pub fn validator_set_pops(&self) -> &[Vec<u8>] {
        &self.validator_set_pops
    }
    /// Recompute the canonical hash of the embedded validator set.
    #[must_use]
    pub fn computed_validator_set_hash(&self) -> HashOf<Vec<PeerId>> {
        HashOf::new(&self.validator_set)
    }
    /// Iterate over validator/proof pairs without independent indexing.
    pub fn validators_with_pops(&self) -> impl ExactSizeIterator<Item = (&PeerId, &[u8])> {
        self.validator_set
            .iter()
            .zip(&self.validator_set_pops)
            .map(|(validator, pop)| (validator, pop.as_slice()))
    }
}
#[derive(Clone, Debug, Encode, Decode)]
#[cfg_attr(feature = "json", derive(crate::DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
struct NativeAmxAttestationQcV2Wire {
    body: NativeAmxAttestationBodyV2,
    validator_set_hash_version: u16,
    validator_set_hash: HashOf<Vec<PeerId>>,
    validator_set: Vec<PeerId>,
    validator_set_pops: Vec<Vec<u8>>,
    signers_bitmap: Vec<u8>,
    bls_aggregate_signature: Vec<u8>,
}
impl TryFrom<NativeAmxAttestationQcV2Wire> for NativeAmxAttestationQcV2 {
    type Error = NativeAmxAttestationQcV2AlignmentError;
    fn try_from(wire: NativeAmxAttestationQcV2Wire) -> Result<Self, Self::Error> {
        Self::try_new(
            wire.body,
            wire.validator_set_hash_version,
            wire.validator_set_hash,
            wire.validator_set,
            wire.validator_set_pops,
            wire.signers_bitmap,
            wire.bls_aggregate_signature,
        )
    }
}
impl<'de> norito::core::NoritoDeserialize<'de> for NativeAmxAttestationQcV2 {
    fn schema_hash() -> [u8; 16] {
        <Self as norito::core::NoritoSerialize>::schema_hash()
    }
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("native AMX attestation QC wire invariant must hold")
    }
    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let wire =
            <NativeAmxAttestationQcV2Wire as norito::core::NoritoDeserialize>::try_deserialize(
                archived.cast(),
            )?;
        Self::try_from(wire).map_err(|error| norito::core::Error::Message(error.to_string()))
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for NativeAmxAttestationQcV2 {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let wire =
            <NativeAmxAttestationQcV2Wire as norito::json::JsonDeserialize>::json_deserialize(
                parser,
            )?;
        Self::try_from(wire).map_err(|error| norito::json::Error::Message(error.to_string()))
    }
}
/// Per-dataspace native AMX v2 leg committed by the routing-plan coordinator.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct NativeAmxLegRecordV2 {
    /// Participant lane certified by both phase QCs.
    pub lane_id: LaneId,
    /// Dataspace participating in the native AMX group.
    pub dataspace_id: DataSpaceId,
    /// Exact control-only participant proposal certified by both phase QCs.
    pub participant_proposal: LaneBlockProposalV1,
    /// Deterministic participant-local settlement committed by the proposal.
    pub participant_settlement: LaneBlockCommitment,
    /// Canonical hash of `participant_settlement` signed by both phase QCs.
    pub participant_settlement_hash: HashOf<LaneBlockCommitment>,
    /// Context-bound participant prepare QC.
    pub prepare_qc: NativeAmxAttestationQcV2,
    /// Context-bound participant commit QC.
    pub commit_qc: NativeAmxAttestationQcV2,
}
impl Ord for NativeAmxLegRecordV2 {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.encode().cmp(&other.encode())
    }
}
impl PartialOrd for NativeAmxLegRecordV2 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
/// Versioned native AMX receipt committed by a finalized coordinator block.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct NativeAmxReceipt {
    /// Receipt format version.
    pub version: u16,
    /// Source transaction hash/id.
    pub source_id: [u8; 32],
    /// Exact genesis-derived network identity that owns this receipt.
    pub network_id: NetworkId,
    /// Deterministic digest of the coordinator/participant routing plan.
    pub plan_digest: Hash,
    /// Coordinator lane that finalized the transaction.
    pub lane_id: LaneId,
    /// Coordinator dataspace that finalized the transaction.
    pub dataspace_id: DataSpaceId,
    /// Exact coordinator-lane incarnation at the authority context.
    pub lane_incarnation: Hash,
    /// Global/catalog height used to resolve all lane and key authority.
    pub authority_context_height: u64,
    /// Coordinator lane-local height that owns the transaction.
    pub lane_block_height: u64,
    /// Coordinator lane-local view that owns the transaction.
    pub lane_block_view: u64,
    /// Exact coordinator lane-block proposal authenticated by participant QCs.
    pub coordinator_proposal_hash: Hash,
    /// Prepared and committed dataspace legs.
    pub legs: Vec<NativeAmxLegRecordV2>,
}
impl NativeAmxLegRecordV2 {
    /// Return whether this leg needs block-wide mixed-role anchor validation.
    ///
    /// A participant proposal that does not contain the current transaction's entrypoint can still
    /// be valid when it is the exact executable proposal for another role in the same block.
    /// Stateless receipt validation records that condition through this predicate; only
    /// authority-aware admission can prove the corresponding block-wide anchor.
    #[must_use]
    pub fn requires_mixed_role_anchor_validation(&self) -> bool {
        let entrypoint_hash = Hash::from(self.prepare_qc.body.tx_entrypoint_hash);
        !self
            .participant_proposal
            .descriptor
            .accepted_transaction_hashes
            .contains(&entrypoint_hash)
    }
}
/// Liquidity profile applied when computing XOR conversions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "profile", content = "state")]
pub enum LaneLiquidityProfile {
    /// Deep pools with negligible slippage.
    Tier1,
    /// Medium depth pools with moderate slippage.
    Tier2,
    /// Thin pools or credit-constrained venues.
    Tier3,
}
/// Volatility bucket applied when computing the safety margin.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "bucket", content = "state")]
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
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
/// Aggregated per-lane settlement commitment captured within a block.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct LaneBlockCommitment {
    /// Lane-local block height associated with the commitment.
    pub block_height: u64,
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Active incarnation commitment for the lane-local height namespace.
    pub lane_incarnation: Hash,
    /// Numeric dataspace identifier.
    pub dataspace_id: DataSpaceId,
    /// Number of transactions contributing settlement receipts.
    pub tx_count: u64,
    /// Exact total local gas-token amount recorded in the block.
    pub total_local_amount: Quantity,
    /// Exact total XOR due immediately after inclusion.
    pub total_xor_due: Quantity,
    /// Exact total XOR expected after applying liquidity haircuts.
    pub total_xor_after_haircut: Quantity,
    /// Exact aggregate difference between XOR due and the post-haircut expectation.
    pub total_xor_variance: Quantity,
    /// Deterministic metadata describing the conversion parameters.
    #[norito(required)]
    pub swap_metadata: Option<LaneSwapMetadata>,
    /// Deterministic receipts contributing to the commitment.
    pub receipts: Vec<LaneSettlementReceipt>,
    /// Versioned Nexus fee receipts committed for asynchronous public XOR settlement.
    pub nexus_fee_receipts: Vec<NexusFeeReceipt>,
    /// Versioned native AMX receipts committed by coordinator execution.
    pub native_amx_receipts: Vec<NativeAmxReceipt>,
}
fn native_amx_nonzero(bytes: &[u8]) -> bool {
    bytes.iter().any(|byte| *byte != 0)
}
fn native_amx_expected_quorum(validator_count: usize) -> usize {
    validator_count.saturating_sub(validator_count.saturating_sub(1) / 3)
}
fn validate_native_amx_participant_proposal_shape(
    proposal: &LaneBlockProposalV1,
) -> Result<(), &'static str> {
    let descriptor = &proposal.descriptor;
    let validator_count = usize::try_from(descriptor.validator_count)
        .map_err(|_| "Native AMX participant descriptor validator count is invalid")?;
    let min_quorum = usize::try_from(descriptor.min_quorum)
        .map_err(|_| "Native AMX participant descriptor quorum is invalid")?;
    if proposal.payload_block_hint.is_some()
        || descriptor.proposal_height == 0
        || descriptor.lane_block_height == 0
        || descriptor.previous_lane_block_height.checked_add(1)
            != Some(descriptor.lane_block_height)
        || (descriptor.previous_lane_block_height == 0)
            != descriptor.previous_lane_block_descriptor_hash.is_none()
        || descriptor
            .previous_lane_block_descriptor_hash
            .is_some_and(|hash| !native_amx_nonzero(hash.as_ref()))
        || !native_amx_nonzero(descriptor.lane_incarnation.as_ref())
        || !native_amx_nonzero(descriptor.subject_hash.as_ref())
        || !native_amx_nonzero(descriptor.payload_ownership_hash.as_ref())
        || !native_amx_nonzero(descriptor.rbc_instance_hash.as_ref())
        || !native_amx_nonzero(descriptor.descriptor_hash.as_ref())
        || !native_amx_nonzero(proposal.proposal_hash.as_ref())
        || descriptor.qc_mode_tag.trim().is_empty()
        || descriptor.accepted_candidate_indices.is_empty()
        || descriptor.accepted_candidate_indices.len() > NATIVE_AMX_GROUP_SOURCES_MAX
        || descriptor.accepted_candidate_indices.len()
            != descriptor.accepted_transaction_hashes.len()
        || descriptor
            .accepted_transaction_hashes
            .iter()
            .any(|hash| !native_amx_nonzero(hash.as_ref()))
        || descriptor
            .accepted_candidate_indices
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != descriptor.accepted_candidate_indices.len()
        || descriptor
            .accepted_transaction_hashes
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != descriptor.accepted_transaction_hashes.len()
        || descriptor.validator_set_hash_version != crate::consensus::VALIDATOR_SET_HASH_VERSION_V1
        || descriptor.validator_set.is_empty()
        || descriptor.validator_set.len() > NATIVE_AMX_VALIDATORS_MAX
        || descriptor
            .validator_set
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || descriptor
            .validator_set
            .iter()
            .any(|peer| peer.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal))
        || validator_count != descriptor.validator_set.len()
        || min_quorum != native_amx_expected_quorum(validator_count)
        || descriptor.validator_set_hash != HashOf::new(&descriptor.validator_set)
        || descriptor.descriptor_hash != descriptor.computed_descriptor_hash()
        || proposal.proposal_hash != proposal.computed_proposal_hash()
    {
        return Err("Native AMX participant proposal is structurally invalid");
    }
    Ok(())
}
fn validate_native_amx_qc_shape(
    qc: &NativeAmxAttestationQcV2,
    expected_phase: NativeAmxPhase,
) -> Result<(), &'static str> {
    let body = &qc.body;
    let validator_count = qc.validator_set().len();
    let advertised_validator_count = usize::try_from(body.participant_validator_count)
        .map_err(|_| "Native AMX participant validator count is invalid")?;
    let advertised_min_quorum = usize::try_from(body.participant_min_quorum)
        .map_err(|_| "Native AMX participant quorum is invalid")?;
    let expected_quorum = native_amx_expected_quorum(validator_count);
    let expected_bitmap_len = validator_count.div_ceil(8);
    let trailing_bits_clear = qc.signers_bitmap.last().is_none_or(|last| {
        let used = validator_count % 8;
        used == 0 || *last & !((1_u8 << used) - 1) == 0
    });
    let signer_count = qc
        .signers_bitmap
        .iter()
        .map(|byte| byte.count_ones() as usize)
        .sum::<usize>();
    if body.phase != expected_phase
        || body.round.height == 0
        || !native_amx_nonzero(body.round.context_id.0.as_ref())
        || body.authority_context_height != body.round.height
        || body.planned_coordinator_block_height == 0
        || !native_amx_nonzero(body.network_id.as_bytes())
        || !native_amx_nonzero(&body.source_id)
        || !native_amx_nonzero(body.tx_entrypoint_hash.as_ref())
        || !native_amx_nonzero(body.plan_digest.as_ref())
        || !native_amx_nonzero(body.coordinator_lane_incarnation.as_ref())
        || !native_amx_nonzero(body.participant_lane_incarnation.as_ref())
        || body.participant_lane_block_height == 0
        || body.participant_previous_block_height.checked_add(1)
            != Some(body.participant_lane_block_height)
        || (body.participant_previous_block_height == 0)
            != body.participant_previous_block_descriptor_hash.is_none()
        || body
            .participant_previous_block_descriptor_hash
            .is_some_and(|hash| !native_amx_nonzero(hash.as_ref()))
        || !native_amx_nonzero(body.participant_proposal_hash.as_ref())
        || !native_amx_nonzero(body.participant_settlement_commitment.as_ref())
        || !native_amx_nonzero(body.participant_validator_set_hash.as_ref())
        || !native_amx_nonzero(body.coordinator_proposal_hash.as_ref())
        || qc.validator_set_hash_version != crate::consensus::VALIDATOR_SET_HASH_VERSION_V1
        || validator_count == 0
        || validator_count > NATIVE_AMX_VALIDATORS_MAX
        || advertised_validator_count != validator_count
        || advertised_min_quorum != expected_quorum
        || qc.validator_set().windows(2).any(|pair| pair[0] >= pair[1])
        || qc
            .validator_set()
            .iter()
            .any(|peer| peer.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal))
        || qc.validator_set_hash != qc.computed_validator_set_hash()
        || body.participant_validator_set_hash != qc.validator_set_hash
        || qc
            .validator_set_pops()
            .iter()
            .any(|pop| pop.len() != NATIVE_AMX_BLS_PROOF_BYTES || !native_amx_nonzero(pop))
        || qc.signers_bitmap.len() != expected_bitmap_len
        || !trailing_bits_clear
        || signer_count != expected_quorum
        || qc.bls_aggregate_signature.len() != NATIVE_AMX_BLS_PROOF_BYTES
        || !native_amx_nonzero(&qc.bls_aggregate_signature)
    {
        return Err("Native AMX participant QC is structurally invalid");
    }
    Ok(())
}
#[expect(
    clippy::too_many_lines,
    reason = "the ordered Native AMX V2 audit preserves stable first-error precedence across one cross-field protocol record"
)]
fn validate_native_amx_leg_shape(
    receipt: &NativeAmxReceipt,
    leg: &NativeAmxLegRecordV2,
    coordinator_sources: &[[u8; Hash::LENGTH]],
    expected_round: super::consensus_v2::ConsensusRound,
    expected_epoch: u64,
    expected_entrypoint_hash: Hash,
) -> Result<(), &'static str> {
    validate_native_amx_participant_proposal_shape(&leg.participant_proposal)?;
    validate_native_amx_qc_shape(&leg.prepare_qc, NativeAmxPhase::Prepare)?;
    validate_native_amx_qc_shape(&leg.commit_qc, NativeAmxPhase::Commit)?;
    let prepare = &leg.prepare_qc;
    let commit = &leg.commit_qc;
    let body = &prepare.body;
    let mut expected_commit_body = *body;
    expected_commit_body.phase = NativeAmxPhase::Commit;
    if commit.body != expected_commit_body
        || prepare.validator_set_hash_version != commit.validator_set_hash_version
        || prepare.validator_set_hash != commit.validator_set_hash
        || prepare.validator_set() != commit.validator_set()
        || prepare.validator_set_pops() != commit.validator_set_pops()
    {
        return Err("Native AMX prepare and commit certificates disagree");
    }
    let descriptor = &leg.participant_proposal.descriptor;
    if body.round != expected_round
        || body.epoch != expected_epoch
        || body.network_id != receipt.network_id
        || body.source_id != receipt.source_id
        || Hash::from(body.tx_entrypoint_hash) != expected_entrypoint_hash
        || body.plan_digest != receipt.plan_digest
        || body.coordinator_lane_id != receipt.lane_id
        || body.coordinator_dataspace_id != receipt.dataspace_id
        || body.coordinator_lane_incarnation != receipt.lane_incarnation
        || body.authority_context_height != receipt.authority_context_height
        || body.planned_coordinator_block_height != receipt.lane_block_height
        || body.coordinator_lane_block_view != receipt.lane_block_view
        || body.coordinator_proposal_hash != receipt.coordinator_proposal_hash
        || body.participant_lane_id != leg.lane_id
        || body.participant_dataspace_id != leg.dataspace_id
        || descriptor.lane_id != leg.lane_id
        || descriptor.dataspace_id != leg.dataspace_id
        || descriptor.lane_incarnation != body.participant_lane_incarnation
        || descriptor.proposal_height != body.authority_context_height
        || descriptor.previous_lane_block_height != body.participant_previous_block_height
        || descriptor.previous_lane_block_descriptor_hash
            != body.participant_previous_block_descriptor_hash
        || descriptor.lane_block_height != body.participant_lane_block_height
        || descriptor.lane_block_view != body.participant_lane_block_view
        || leg.participant_proposal.proposal_hash != body.participant_proposal_hash
        || descriptor.validator_set_hash_version != prepare.validator_set_hash_version
        || descriptor.validator_set_hash != prepare.validator_set_hash
        || descriptor.validator_set.as_slice() != prepare.validator_set()
        || descriptor.validator_count != body.participant_validator_count
        || descriptor.min_quorum != body.participant_min_quorum
    {
        return Err("Native AMX participant leg identity is internally inconsistent");
    }
    let settlement = &leg.participant_settlement;
    let settlement_hash = crate::nexus::compute_settlement_hash(settlement)
        .map_err(|_| "Native AMX participant settlement cannot be hashed")?;
    let same_route = leg.lane_id == receipt.lane_id && leg.dataspace_id == receipt.dataspace_id;
    let settlement_sources = settlement
        .receipts
        .iter()
        .map(|entry| entry.source_id)
        .collect::<Vec<_>>();
    let settlement_sources_are_unique = settlement_sources
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        == settlement_sources.len();
    let settlement_sources_match =
        !same_route || settlement_sources.as_slice() == coordinator_sources;
    if settlement.receipts.is_empty()
        || settlement.receipts.len() > NATIVE_AMX_GROUP_SOURCES_MAX
        || settlement.tx_count != u64::try_from(settlement.receipts.len()).unwrap_or(u64::MAX)
        || settlement.block_height != body.participant_lane_block_height
        || settlement.lane_id != leg.lane_id
        || settlement.dataspace_id != leg.dataspace_id
        || settlement.lane_incarnation != body.participant_lane_incarnation
        || !settlement.total_local_amount.is_zero()
        || !settlement.total_xor_due.is_zero()
        || !settlement.total_xor_after_haircut.is_zero()
        || !settlement.total_xor_variance.is_zero()
        || settlement.swap_metadata.is_some()
        || !settlement.nexus_fee_receipts.is_empty()
        || !settlement.native_amx_receipts.is_empty()
        || !settlement_sources_are_unique
        || settlement
            .receipts
            .iter()
            .filter(|entry| entry.source_id == receipt.source_id)
            .count()
            != 1
        || settlement.receipts.iter().any(|entry| {
            !entry.local_amount.is_zero()
                || !entry.xor_due.is_zero()
                || !entry.xor_after_haircut.is_zero()
                || !entry.xor_variance.is_zero()
                || entry.timestamp_ms != body.authority_context_height
        })
        || !settlement_sources_match
        || settlement_hash != leg.participant_settlement_hash
        || Hash::from(settlement_hash) != body.participant_settlement_commitment
    {
        return Err("Native AMX participant settlement is structurally invalid");
    }
    let entrypoint_hash = Hash::from(body.tx_entrypoint_hash);
    let entrypoint_position = descriptor
        .accepted_transaction_hashes
        .iter()
        .position(|hash| *hash == entrypoint_hash);
    if entrypoint_position.is_some_and(|position| {
        descriptor.accepted_candidate_indices.len() != settlement.receipts.len()
            || descriptor.accepted_transaction_hashes.len() != settlement.receipts.len()
            || settlement
                .receipts
                .get(position)
                .is_none_or(|entry| entry.source_id != body.source_id)
    }) {
        return Err("Native AMX participant proposal and grouped settlement are not aligned");
    }
    let proposal_uses_coordinator_authority_context =
        descriptor.proposal_height == receipt.authority_context_height;
    if same_route
        && (entrypoint_position.is_none()
            || descriptor.lane_incarnation != receipt.lane_incarnation
            || !proposal_uses_coordinator_authority_context
            || descriptor.lane_block_height != receipt.lane_block_height
            || descriptor.lane_block_view != receipt.lane_block_view
            || leg.participant_proposal.proposal_hash != receipt.coordinator_proposal_hash)
    {
        return Err("Native AMX same-route leg differs from the coordinator identity");
    }
    Ok(())
}
impl LaneBlockCommitment {
    /// Validate grouped Native AMX receipt structure without live authority state.
    ///
    /// This validates clean-break versions, bounds, unique source membership, participant proposal
    /// hashes, prepare/commit identity, committee geometry, bitmaps/quorum, 96-byte proof/signature
    /// fields, zero-effect settlements, and exact same-route coordinator identity and source order.
    /// Separate participant groups can span more than one coordinator commitment, so their exact
    /// block-wide membership is validated by the core execution-context boundary instead. This
    /// deliberately does not claim that an embedded committee, incarnation, predecessor, proof of
    /// possession, or aggregate signature is authoritative at its historical height.
    ///
    /// # Errors
    ///
    /// Returns a stable reason when any embedded Native AMX receipt is malformed.
    pub fn validate_native_amx_receipts(&self) -> Result<(), &'static str> {
        if self.native_amx_receipts.len() > NATIVE_AMX_GROUP_SOURCES_MAX {
            return Err("Native AMX receipt group exceeds its source limit");
        }
        if self.native_amx_receipts.is_empty() {
            return Ok(());
        }
        let expected_sources = self
            .native_amx_receipts
            .iter()
            .map(|receipt| receipt.source_id)
            .collect::<Vec<_>>();
        if expected_sources
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != expected_sources.len()
        {
            return Err("Native AMX receipt sources must be unique");
        }
        for receipt in &self.native_amx_receipts {
            let receipt_belongs_to_commitment_height =
                receipt.lane_block_height == self.block_height;
            if receipt.version != NATIVE_AMX_RECEIPT_VERSION_V2
                || !native_amx_nonzero(&receipt.source_id)
                || !native_amx_nonzero(receipt.network_id.as_bytes())
                || !native_amx_nonzero(receipt.plan_digest.as_ref())
                || !native_amx_nonzero(receipt.lane_incarnation.as_ref())
                || receipt.authority_context_height == 0
                || receipt.lane_block_height == 0
                || !native_amx_nonzero(receipt.coordinator_proposal_hash.as_ref())
                || receipt.lane_id != self.lane_id
                || receipt.dataspace_id != self.dataspace_id
                || receipt.lane_incarnation != self.lane_incarnation
                || !receipt_belongs_to_commitment_height
            {
                return Err("Native AMX receipt coordinator identity is invalid");
            }
            if receipt.legs.is_empty() || receipt.legs.len() > NATIVE_AMX_PARTICIPANT_LEGS_MAX {
                return Err("Native AMX receipt participant leg count is out of bounds");
            }
            let mut routes = std::collections::BTreeSet::new();
            if receipt
                .legs
                .iter()
                .any(|leg| !routes.insert((leg.lane_id, leg.dataspace_id)))
            {
                return Err("Native AMX receipt contains duplicate participant routes");
            }
            let first_body = &receipt.legs[0].prepare_qc.body;
            let expected_round = first_body.round;
            let expected_epoch = first_body.epoch;
            let expected_entrypoint_hash = Hash::from(first_body.tx_entrypoint_hash);
            for leg in &receipt.legs {
                validate_native_amx_leg_shape(
                    receipt,
                    leg,
                    &expected_sources,
                    expected_round,
                    expected_epoch,
                    expected_entrypoint_hash,
                )?;
            }
        }
        Ok(())
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for LaneSwapMetadata {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        decode_from_slice_canonical(bytes)
    }
}
/// Runtime-upgrade governance hook snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
/// Snapshot of missing-block fetch attempts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiMissingBlockFetchStatus {
    /// Total fetch evaluations after QC-first arrival (including backoff/no-target cases).
    pub total: u64,
    /// Target count on the most recent fetch attempt.
    pub last_targets: u64,
    /// Dwell time in milliseconds observed before the most recent fetch attempt.
    pub last_dwell_ms: u64,
}
/// Snapshot of kura persistence failures and retries.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiKuraStoreStatus {
    /// Total times a block failed to enqueue for persistence.
    pub failures_total: u64,
    /// Total times kura persistence retries were exhausted for a block.
    pub abort_total: u64,
    /// Total times a block reached the staging phase before persistence.
    #[norito(default)]
    pub stage_total: u64,
    /// Total times a staged commit was rolled back before WSV application.
    #[norito(default)]
    pub rollback_total: u64,
    /// Height of the last staged block (best-effort).
    #[norito(default)]
    pub stage_last_height: u64,
    /// View of the last staged block (best-effort).
    #[norito(default)]
    pub stage_last_view: u64,
    /// Hash of the last staged block (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub stage_last_hash: Option<HashOf<BlockHeader>>,
    /// Height of the last staged commit rolled back (best-effort).
    #[norito(default)]
    pub rollback_last_height: u64,
    /// View of the last staged commit rolled back (best-effort).
    #[norito(default)]
    pub rollback_last_view: u64,
    /// Hash of the last staged commit rolled back (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub rollback_last_hash: Option<HashOf<BlockHeader>>,
    /// Reason label for the last rollback (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub rollback_last_reason: Option<String>,
    /// Total times Highest/Locked QC were reset after a kura abort.
    #[norito(default)]
    pub lock_reset_total: u64,
    /// Height associated with the last lock reset (best-effort).
    #[norito(default)]
    pub lock_reset_last_height: u64,
    /// View associated with the last lock reset (best-effort).
    #[norito(default)]
    pub lock_reset_last_view: u64,
    /// Hash associated with the last lock reset (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub lock_reset_last_hash: Option<HashOf<BlockHeader>>,
    /// Reason label for the last lock reset (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub lock_reset_last_reason: Option<String>,
    /// Last observed retry attempt count.
    pub last_retry_attempt: u64,
    /// Last observed retry backoff in milliseconds.
    pub last_retry_backoff_ms: u64,
    /// Height of the last block that failed to persist (best-effort).
    pub last_height: u64,
    /// View of the last block that failed to persist (best-effort).
    pub last_view: u64,
    /// Hash of the last block that failed to persist (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    pub last_hash: Option<HashOf<BlockHeader>>,
}
/// View-change cause counters surfaced via `/v1/sumeragi/status`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiViewChangeCauseStatus {
    /// Total view changes triggered after commit failures (with QC quorum).
    #[norito(default)]
    pub commit_failure_total: u64,
    /// Total view changes triggered after quorum timeouts/missing commits.
    #[norito(default)]
    pub quorum_timeout_total: u64,
    /// Total view changes triggered after stake-quorum timeouts (`NPoS` only).
    #[norito(default)]
    pub stake_quorum_timeout_total: u64,
    /// Total view changes triggered after roster-unavailability recovery.
    #[norito(default)]
    pub roster_unavailable_total: u64,
    /// Total view changes triggered after censorship evidence reaches quorum.
    #[norito(default)]
    pub censorship_evidence_total: u64,
    /// Total view changes triggered after missing payloads exceeded dwell.
    #[norito(default)]
    pub missing_payload_total: u64,
    /// Total view changes triggered after missing or stale QCs.
    #[norito(default)]
    pub missing_qc_total: u64,
    /// Total view changes triggered after validation rejects before voting.
    #[norito(default)]
    pub validation_reject_total: u64,
    /// Last recorded view-change cause label (if any).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_cause: Option<String>,
    /// Milliseconds since UNIX epoch when the last cause was recorded.
    #[norito(default)]
    pub last_cause_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a commit-failure cause was last recorded.
    #[norito(default)]
    pub last_commit_failure_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a quorum-timeout cause was last recorded.
    #[norito(default)]
    pub last_quorum_timeout_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a stake-quorum-timeout cause was last recorded.
    #[norito(default)]
    pub last_stake_quorum_timeout_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a roster-unavailable cause was last recorded.
    #[norito(default)]
    pub last_roster_unavailable_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a censorship-evidence cause was last recorded.
    #[norito(default)]
    pub last_censorship_evidence_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a missing-payload cause was last recorded.
    #[norito(default)]
    pub last_missing_payload_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a missing-QC cause was last recorded.
    #[norito(default)]
    pub last_missing_qc_timestamp_ms: u64,
    /// Milliseconds since UNIX epoch when a validation-reject cause was last recorded.
    #[norito(default)]
    pub last_validation_reject_timestamp_ms: u64,
}
/// Validation-gate reject counters and last-occurrence snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiValidationRejectStatus {
    /// Total rejects recorded before voting.
    #[norito(default)]
    pub total: u64,
    /// Stateless validation rejects (header, timestamps, genesis checks).
    #[norito(default)]
    pub stateless_total: u64,
    /// Execution/stateful validation rejects (transaction execution, DA availability checks).
    #[norito(default)]
    pub execution_total: u64,
    /// Prev-block hash mismatch rejects.
    #[norito(default)]
    pub prev_hash_total: u64,
    /// Prev-block height mismatch rejects.
    #[norito(default)]
    pub prev_height_total: u64,
    /// Topology/roster mismatch rejects.
    #[norito(default)]
    pub topology_total: u64,
    /// Last recorded reason label (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_reason: Option<String>,
    /// Last rejected block height (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_height: Option<u64>,
    /// Last rejected block view (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_view: Option<u64>,
    /// Last rejected block hash (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_block: Option<HashOf<BlockHeader>>,
    /// Milliseconds since UNIX epoch when the last reject was recorded.
    #[norito(default)]
    pub last_timestamp_ms: u64,
}
/// Peer consensus-key policy reject counters and last-occurrence snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiPeerKeyPolicyStatus {
    /// Total peer-key policy rejects recorded.
    #[norito(default)]
    pub total: u64,
    /// Rejects due to missing HSM binding when required.
    #[norito(default)]
    pub missing_hsm_total: u64,
    /// Rejects due to disallowed public-key algorithm.
    #[norito(default)]
    pub disallowed_algorithm_total: u64,
    /// Rejects due to disallowed HSM provider.
    #[norito(default)]
    pub disallowed_provider_total: u64,
    /// Rejects due to activation height violating lead-time policy.
    #[norito(default)]
    pub lead_time_violation_total: u64,
    /// Rejects due to activation height being in the past.
    #[norito(default)]
    pub activation_in_past_total: u64,
    /// Rejects due to expiry occurring before activation.
    #[norito(default)]
    pub expiry_before_activation_total: u64,
    /// Rejects due to identifier collisions for the same public key.
    #[norito(default)]
    pub identifier_collision_total: u64,
    /// Last recorded reject reason (best-effort).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_reason: Option<String>,
    /// Milliseconds since UNIX epoch when the last reject was recorded.
    #[norito(default)]
    pub last_timestamp_ms: u64,
}
/// Consensus message drop/deferral counter entry.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiConsensusMessageHandlingEntry {
    /// Message kind label (e.g., `block_created`).
    pub kind: String,
    /// Handling outcome label (e.g., `dropped` or `deferred`).
    pub outcome: String,
    /// Drop/deferral reason label.
    pub reason: String,
    /// Total observed for the `(kind,outcome,reason)` tuple.
    pub total: u64,
}
/// Consensus message drop/deferral counters surfaced via Sumeragi status.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiConsensusMessageHandlingStatus {
    /// Per-kind drop/deferral counters (best-effort).
    #[norito(default)]
    pub entries: Vec<SumeragiConsensusMessageHandlingEntry>,
}
/// Vote validation drop entry with roster context.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiVoteValidationDropEntry {
    /// Drop reason label.
    pub reason: String,
    /// Vote height.
    pub height: u64,
    /// Vote view.
    pub view: u64,
    /// Vote epoch.
    pub epoch: u64,
    /// Signer index from the vote payload.
    pub signer_index: u32,
    /// Peer ID resolved from the validation roster (if any).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub peer_id: Option<PeerId>,
    /// Validator roster hash used for validation (if any).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub roster_hash: Option<HashOf<Vec<PeerId>>>,
    /// Validator roster length used for validation (if known).
    pub roster_len: u32,
    /// Block hash referenced by the vote.
    pub block_hash: HashOf<BlockHeader>,
    /// Milliseconds since UNIX epoch when the drop was recorded.
    pub timestamp_ms: u64,
}
/// Aggregated count for a vote-validation drop reason.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiVoteValidationDropReasonCount {
    /// Drop reason label.
    pub reason: String,
    /// Total drops recorded for the reason.
    pub total: u64,
}
/// Aggregated vote validation drops for a peer/roster hash pairing.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiVoteValidationDropPeerEntry {
    /// Peer associated with the drop counts.
    pub peer_id: PeerId,
    /// Validator roster hash used for validation (if any).
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub roster_hash: Option<HashOf<Vec<PeerId>>>,
    /// Validator roster length used for validation (if known).
    pub roster_len: u32,
    /// Total drops recorded for this peer/roster pairing.
    pub total: u64,
    /// Per-reason drop counters.
    #[norito(default)]
    pub reasons: Vec<SumeragiVoteValidationDropReasonCount>,
    /// Height associated with the last drop.
    pub last_height: u64,
    /// View associated with the last drop.
    pub last_view: u64,
    /// Epoch associated with the last drop.
    pub last_epoch: u64,
    /// Milliseconds since UNIX epoch when the last drop was recorded.
    pub last_timestamp_ms: u64,
}
/// Vote validation drop snapshot surfaced via Sumeragi status.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiVoteValidationDropStatus {
    /// Total vote validation drops recorded.
    #[norito(default)]
    pub total: u64,
    /// Recent drop entries (newest-first, bounded).
    #[norito(default)]
    pub entries: Vec<SumeragiVoteValidationDropEntry>,
    /// Aggregated drop counters per peer/roster pairing.
    #[norito(default)]
    pub peer_entries: Vec<SumeragiVoteValidationDropPeerEntry>,
}
/// Deterministic consensus configuration caps captured alongside status snapshots.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiConsensusCapsStatus {
    /// Canonical digest of deterministic, locally configured Nexus policy.
    #[norito(default)]
    pub nexus_policy_digest: [u8; 32],
    /// Canonical digest of the complete shared Sumeragi v2 runtime projection.
    #[norito(default)]
    pub v2_config_fingerprint: [u8; 32],
}
/// Queue depth snapshot for Sumeragi worker-loop channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiWorkerQueueDepths {
    /// Vote channel depth.
    #[norito(default)]
    pub vote_rx: u64,
    /// Block payload channel depth.
    #[norito(default)]
    pub block_payload_rx: u64,
    /// RBC chunk channel depth.
    #[norito(default)]
    pub rbc_chunk_rx: u64,
    /// Block channel depth.
    #[norito(default)]
    pub block_rx: u64,
    /// Consensus control channel depth.
    #[norito(default)]
    pub consensus_rx: u64,
    /// Lane relay channel depth.
    #[norito(default)]
    pub lane_relay_rx: u64,
    /// Background post channel depth.
    #[norito(default)]
    pub background_rx: u64,
}
/// Per-queue totals for worker-loop diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiWorkerQueueTotals {
    /// Vote channel total.
    #[norito(default)]
    pub vote_rx: u64,
    /// Block payload channel total.
    #[norito(default)]
    pub block_payload_rx: u64,
    /// RBC chunk channel total.
    #[norito(default)]
    pub rbc_chunk_rx: u64,
    /// Block channel total.
    #[norito(default)]
    pub block_rx: u64,
    /// Consensus control channel total.
    #[norito(default)]
    pub consensus_rx: u64,
    /// Lane relay channel total.
    #[norito(default)]
    pub lane_relay_rx: u64,
    /// Background post channel total.
    #[norito(default)]
    pub background_rx: u64,
}
/// Worker-loop queue diagnostics (drops/blocking).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiWorkerQueueDiagnostics {
    /// Total count of blocking enqueues per queue.
    #[norito(default)]
    pub blocked_total: SumeragiWorkerQueueTotals,
    /// Total time spent blocked (ms) per queue.
    #[norito(default)]
    pub blocked_ms_total: SumeragiWorkerQueueTotals,
    /// Maximum block duration (ms) per queue.
    #[norito(default)]
    pub blocked_max_ms: SumeragiWorkerQueueTotals,
    /// Total count of dropped enqueues per queue.
    #[norito(default)]
    pub dropped_total: SumeragiWorkerQueueTotals,
}
/// Worker-loop diagnostics exposed by `/v1/sumeragi/status`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiWorkerLoopStatus {
    /// Last observed worker-loop stage label.
    #[norito(default)]
    pub stage: String,
    /// Timestamp (ms since UNIX epoch) when the stage was last updated.
    #[norito(default)]
    pub stage_started_ms: u64,
    /// Duration of the most recent worker iteration in milliseconds.
    #[norito(default)]
    pub last_iteration_ms: u64,
    /// Queue depth snapshot for worker-loop channels.
    #[norito(default)]
    pub queue_depths: SumeragiWorkerQueueDepths,
    /// Queue enqueue diagnostics (drops/blocking).
    #[norito(default)]
    pub queue_diagnostics: SumeragiWorkerQueueDiagnostics,
}
/// Commit inflight diagnostics exposed by `/v1/sumeragi/status`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiCommitInflightStatus {
    /// Whether a commit job is currently in flight.
    #[norito(default)]
    pub active: bool,
    /// Inflight commit id (best-effort).
    #[norito(default)]
    pub id: u64,
    /// Block height associated with the inflight commit.
    #[norito(default)]
    pub height: u64,
    /// View associated with the inflight commit.
    #[norito(default)]
    pub view: u64,
    /// Block hash associated with the inflight commit.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub block_hash: Option<HashOf<BlockHeader>>,
    /// Timestamp (ms since UNIX epoch) when the inflight commit was enqueued.
    #[norito(default)]
    pub started_ms: u64,
    /// Milliseconds elapsed since the inflight commit started (best-effort).
    #[norito(default)]
    pub elapsed_ms: u64,
    /// Configured inflight timeout in milliseconds.
    #[norito(default)]
    pub timeout_ms: u64,
    /// Total inflight timeouts observed.
    #[norito(default)]
    pub timeout_total: u64,
    /// Timestamp (ms since UNIX epoch) of the last inflight timeout.
    #[norito(default)]
    pub last_timeout_timestamp_ms: u64,
    /// Duration (ms) of the last inflight timeout.
    #[norito(default)]
    pub last_timeout_elapsed_ms: u64,
    /// Height associated with the last inflight timeout.
    #[norito(default)]
    pub last_timeout_height: u64,
    /// View associated with the last inflight timeout.
    #[norito(default)]
    pub last_timeout_view: u64,
    /// Block hash associated with the last inflight timeout.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub last_timeout_block_hash: Option<HashOf<BlockHeader>>,
    /// Total number of pacemaker pauses caused by inflight commits.
    #[norito(default)]
    pub pause_total: u64,
    /// Total number of pacemaker resumes following inflight completion.
    #[norito(default)]
    pub resume_total: u64,
    /// Timestamp (ms since UNIX epoch) when the current pause began.
    #[norito(default)]
    pub paused_since_ms: u64,
    /// Queue depth snapshot recorded when the inflight pause started.
    #[norito(default)]
    pub pause_queue_depths: SumeragiWorkerQueueDepths,
    /// Queue depth snapshot recorded when the inflight pause ended.
    #[norito(default)]
    pub resume_queue_depths: SumeragiWorkerQueueDepths,
}
/// Commit-pipeline timing snapshot exposed by `/v1/sumeragi/status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiCommitPipelineStatus {
    /// End-to-end time spent in the most recent commit-pipeline run.
    #[norito(default)]
    pub last_total_ms: u64,
    /// Time spent validating/finalizing candidate blocks before gating.
    #[norito(default)]
    pub last_validation_ms: u64,
    /// Time spent rebuilding cached QCs from votes.
    #[norito(default)]
    pub last_qc_rebuild_ms: u64,
    /// Time spent in validation/availability gate checks.
    #[norito(default)]
    pub last_gate_ms: u64,
    /// Time spent finalizing pending blocks into the commit worker.
    #[norito(default)]
    pub last_finalize_ms: u64,
    /// Time spent draining finished commit results.
    #[norito(default)]
    pub last_drain_results_ms: u64,
    /// Sum of QC verification subtotals across drained commit results.
    #[norito(default)]
    pub last_drain_qc_verify_ms: u64,
    /// Sum of persistence subtotals across drained commit results.
    #[norito(default)]
    pub last_drain_persist_ms: u64,
    /// Sum of Kura store subtotals across drained commit results.
    #[norito(default)]
    pub last_drain_kura_store_ms: u64,
    /// Sum of state-apply subtotals across drained commit results.
    #[norito(default)]
    pub last_drain_state_apply_ms: u64,
    /// Sum of state-commit subtotals across drained commit results.
    #[norito(default)]
    pub last_drain_state_commit_ms: u64,
    /// EMA of end-to-end commit-pipeline time.
    #[norito(default)]
    pub ema_total_ms: u64,
    /// EMA of validation time.
    #[norito(default)]
    pub ema_validation_ms: u64,
    /// EMA of gate time.
    #[norito(default)]
    pub ema_gate_ms: u64,
    /// EMA of finalize time.
    #[norito(default)]
    pub ema_finalize_ms: u64,
}
/// DELIVER-to-next-proposal gap snapshot exposed by `/v1/sumeragi/status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiRoundGapStatus {
    /// Most recent elapsed time from first accepted DELIVER to local state commit.
    #[norito(default)]
    pub last_deliver_to_state_commit_ms: u64,
    /// Most recent elapsed time from local state commit to pacemaker unblock.
    #[norito(default)]
    pub last_state_commit_to_next_propose_ms: u64,
    /// Most recent elapsed time from first accepted DELIVER to pacemaker unblock.
    #[norito(default)]
    pub last_deliver_to_next_propose_ms: u64,
    /// EMA of DELIVER-to-state-commit.
    #[norito(default)]
    pub ema_deliver_to_state_commit_ms: u64,
    /// EMA of state-commit-to-next-propose.
    #[norito(default)]
    pub ema_state_commit_to_next_propose_ms: u64,
    /// EMA of DELIVER-to-next-propose.
    #[norito(default)]
    pub ema_deliver_to_next_propose_ms: u64,
}
/// Latest commit-quorum signature tally exposed by `/v1/sumeragi/status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiCommitQuorumStatus {
    /// Block height associated with the tally.
    #[norito(default)]
    pub height: u64,
    /// View associated with the tally.
    #[norito(default)]
    pub view: u64,
    /// Block hash associated with the tally.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub block_hash: Option<HashOf<BlockHeader>>,
    /// Total signatures present on the block.
    #[norito(default)]
    pub signatures_present: u64,
    /// Signatures counted toward the commit quorum.
    #[norito(default)]
    pub signatures_counted: u64,
    /// Signatures contributed by set-B validators.
    #[norito(default)]
    pub signatures_set_b: u64,
    /// Required commit quorum size.
    #[norito(default)]
    pub signatures_required: u64,
    /// Timestamp (ms since UNIX epoch) when the tally was recorded.
    #[norito(default)]
    pub last_updated_ms: u64,
}
/// Latest commit QC summary exposed by `/v1/sumeragi/status`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiQcStatus {
    /// Block height certified by the commit QC.
    #[norito(default)]
    pub height: u64,
    /// View associated with the commit QC.
    #[norito(default)]
    pub view: u64,
    /// Epoch associated with the commit QC.
    #[norito(default)]
    pub epoch: u64,
    /// Block hash certified by the commit QC.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub block_hash: Option<HashOf<BlockHeader>>,
    /// Stable hash of the validator set that produced the QC.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub validator_set_hash: Option<HashOf<Vec<PeerId>>>,
    /// Number of validators in the recorded set.
    #[norito(default)]
    pub validator_set_len: u64,
    /// Total signatures attached to the QC.
    #[norito(default)]
    pub signatures_total: u64,
}
/// Observational `NPoS` repair fanout stake-coverage snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiNposRepairCoverageStatus {
    /// Last height for which a repair fanout selection was recorded.
    #[norito(default)]
    pub last_repair_height: u64,
    /// Last view for which a repair fanout selection was recorded.
    #[norito(default)]
    pub last_repair_view: u64,
    /// Operator-facing reason label for the latest repair selection.
    #[norito(default)]
    pub reason: String,
    /// Number of peers selected for the latest repair fanout.
    #[norito(default)]
    pub selected_repair_peer_count: u64,
    /// Required stake quorum threshold in basis points.
    #[norito(default)]
    pub required_stake_quorum_bps: u16,
    /// Selected repair fanout stake coverage in basis points.
    #[norito(default)]
    pub selected_stake_coverage_bps: u16,
    /// Whether the latest selected fanout reached the stake quorum threshold.
    #[norito(default)]
    pub reached_stake_quorum_coverage: bool,
}
/// Fail-closed consensus safety halt exposed via `/v1/sumeragi/status`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SumeragiSafetyHaltStatus {
    /// Whether this process has halted consensus participation.
    #[norito(default)]
    pub active: bool,
    /// Stable machine-readable halt reason.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub reason: Option<String>,
    /// Height at which the unsafe condition was detected.
    #[norito(default)]
    pub height: u64,
    /// Epoch at which the unsafe condition was detected.
    #[norito(default)]
    pub epoch: u64,
    /// First authenticated block subject involved in the halt.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub first_block_hash: Option<HashOf<BlockHeader>>,
    /// Conflicting authenticated block subject, when applicable.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub conflicting_block_hash: Option<HashOf<BlockHeader>>,
    /// Parent state root authenticated by the first certificate.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub first_parent_state_root: Option<Hash>,
    /// Post-state root authenticated by the first certificate.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub first_post_state_root: Option<Hash>,
    /// Parent state root authenticated by the conflicting certificate.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub conflicting_parent_state_root: Option<Hash>,
    /// Post-state root authenticated by the conflicting certificate.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub conflicting_post_state_root: Option<Hash>,
}
/// Cached standalone lane-block consensus session status.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[expect(
    clippy::struct_excessive_bools,
    reason = "each boolean is an independent V1 operator-visible session fact; collapsing them would change the canonical diagnostics wire shape"
)]
pub struct SumeragiLaneBlockSessionStatus {
    /// Lane whose lane-local block is being certified.
    #[norito(default)]
    pub lane_id: LaneId,
    /// Dataspace bound to the lane-local block.
    #[norito(default)]
    pub dataspace_id: DataSpaceId,
    /// Exact lane incarnation bound to every proposal, vote, and certificate.
    pub lane_incarnation: Hash,
    /// Lane-local block height.
    #[norito(default)]
    pub lane_block_height: u64,
    /// Lane-local block view.
    #[norito(default)]
    pub lane_block_view: u64,
    /// Proposal hash identifying the cached session.
    pub proposal_hash: Hash,
    /// Whether the proposal artifact is cached locally.
    #[norito(default)]
    pub has_proposal: bool,
    /// Number of cached prepare votes.
    #[norito(default)]
    pub prepare_vote_count: u32,
    /// Number of cached commit votes.
    #[norito(default)]
    pub commit_vote_count: u32,
    /// Whether a prepare QC is cached.
    #[norito(default)]
    pub has_prepare_qc: bool,
    /// Whether a commit QC is cached.
    #[norito(default)]
    pub has_commit_qc: bool,
    /// Whether this peer has a pending local commit-vote opportunity.
    #[norito(default)]
    pub pending_commit_vote_request: bool,
    /// Whether this session is ready to drain as a committed lane block.
    #[norito(default)]
    pub pending_committed_session_drain: bool,
    /// Whether this session already drained to the committed-lane queue.
    #[norito(default)]
    pub committed_session_drained: bool,
    /// Validator count advertised by the session body.
    #[norito(default)]
    pub validator_count: u32,
    /// Minimum quorum advertised by the session body.
    #[norito(default)]
    pub min_quorum: u32,
}
/// Proposal-gate inputs from the most recent pacemaker evaluation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[expect(
    clippy::struct_excessive_bools,
    reason = "operator diagnostics expose independent proposal-gate booleans"
)]
pub struct SumeragiProposalGateStatus {
    /// Height currently considered by the proposal path.
    #[norito(default)]
    pub height: u64,
    /// View currently considered by the proposal path.
    #[norito(default)]
    pub view: u64,
    /// Number of locally queued transactions when the gate was evaluated.
    #[norito(default)]
    pub queue_len: u64,
    /// Total locally tracked pending blocks.
    #[norito(default)]
    pub pending_blocks_total: u64,
    /// Pending blocks considered blocking by proposal backpressure.
    #[norito(default)]
    pub pending_blocks_blocking: u64,
    /// Active pending blocks that still extend the local tip.
    #[norito(default)]
    pub active_pending_for_tip: u64,
    /// Whether transaction-queue capacity pressure is gating proposals.
    #[norito(default)]
    pub queue_saturated: bool,
    /// Whether active pending block state is gating proposals.
    #[norito(default)]
    pub active_pending: bool,
    /// Whether RBC backlog is gating proposals.
    #[norito(default)]
    pub rbc_backlog: bool,
    /// Whether lane relay backpressure is gating proposals.
    #[norito(default)]
    pub relay_backpressure: bool,
    /// Whether consensus worker queues are gating proposals.
    #[norito(default)]
    pub consensus_queue_backpressure: bool,
    /// Whether aggregate proposal backpressure defers proposal assembly.
    #[norito(default)]
    pub should_defer: bool,
    /// Whether deferral is only queue/consensus pacing.
    #[norito(default)]
    pub only_pacing_backpressure: bool,
    /// Whether a commit job is currently in flight.
    #[norito(default)]
    pub commit_inflight_active: bool,
    /// Whether the current height/view has a cached proposal.
    #[norito(default)]
    pub cached_proposal_present: bool,
    /// Whether the current height/view has a cached proposal hint.
    #[norito(default)]
    pub cached_proposal_hint_present: bool,
    /// Whether local round-liveness evidence exists for the current height/view.
    #[norito(default)]
    pub round_liveness_present: bool,
    /// Whether a local frontier owner still exists for this height/view.
    #[norito(default)]
    pub frontier_owner_present: bool,
    /// Whether missing-QC liveness recovery is active for this height/view.
    #[norito(default)]
    pub missing_qc_liveness_active: bool,
    /// Milliseconds since the last pacemaker proposal attempt.
    #[norito(default)]
    pub last_pacemaker_attempt_age_ms: u64,
    /// Milliseconds since the last successful proposal assembly.
    #[norito(default)]
    pub last_successful_proposal_age_ms: u64,
}
/// Current `NPoS` schedule and PRF context for operator diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiNposDiagnostics {
    /// Length of the active epoch in blocks.
    pub epoch_length_blocks: NonZeroU64,
    /// Non-zero epoch seed used for deterministic leader and validator election.
    pub epoch_seed: [u8; 32],
    /// Height associated with the recorded PRF context.
    pub prf_height: u64,
    /// View associated with the recorded PRF context.
    pub prf_view: u64,
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
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
/// Maximum number of Native AMX participant-application rows exposed by one
/// `/v1/sumeragi/diagnostics` response.
///
/// The bound matches the compiled maximum number of active execution lanes, so diagnostics never
/// need to truncate an active route/incarnation while still refusing an unbounded operator payload.
pub const SUMERAGI_NATIVE_AMX_PARTICIPANT_APPLICATIONS_MAX: usize =
    crate::nexus::MAX_ACTIVE_EXECUTION_LANES;
/// Maximum number of grouped Native AMX sources represented by one participant-application row.
pub const SUMERAGI_NATIVE_AMX_PARTICIPANT_APPLICATION_SOURCES_MAX: u64 = 4_096;
/// Maximum number of autonomous lane-execution rows exposed by one
/// `/v1/sumeragi/diagnostics` response.
///
/// This reuses the core lane-diagnostics suffix bound. The projection retains
/// identifiers and counters only; executable payload bytes remain in Kura.
pub const SUMERAGI_AUTONOMOUS_LANE_EXECUTIONS_MAX: usize = 128;
/// Highest independently durable autonomous lane-execution stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
#[norito(rename_all = "snake_case")]
pub enum SumeragiAutonomousLaneExecutionStage {
    /// Exact Queue-owned reservation keys are fsynced before executable-payload durability.
    ReservationsDurable,
    /// The producer-authenticated executable payload is durable.
    ExecutablePayloadDurable,
    /// A lane availability QC durably proves quorum payload retention.
    PayloadAvailabilityCertified,
    /// Prepare and commit lane QCs are durable.
    LaneCertified,
    /// The complete authenticated source bundle is durably reconstructible.
    CertifiedBundleDurable,
    /// A merge-QC-certified pending sidecar contains the exact source.
    MergeCandidateDurable,
    /// The exact source appears in the globally committed merge log.
    GlobalCarrierCommitted,
    /// An exact merge application receipt proves Kura and WSV application.
    KuraWsvApplicationReceiptDurable,
    /// Durable Queue replay has no exact ownership or unfinished crash barrier.
    QueueFinalized,
    /// Durable evidence disagrees for the same lane-local slot.
    Conflict,
}
impl SumeragiAutonomousLaneExecutionStage {
    /// Stable JSON/OpenAPI label used by diagnostics clients.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReservationsDurable => "reservations_durable",
            Self::ExecutablePayloadDurable => "executable_payload_durable",
            Self::PayloadAvailabilityCertified => "payload_availability_certified",
            Self::LaneCertified => "lane_certified",
            Self::CertifiedBundleDurable => "certified_bundle_durable",
            Self::MergeCandidateDurable => "merge_candidate_durable",
            Self::GlobalCarrierCommitted => "global_carrier_committed",
            Self::KuraWsvApplicationReceiptDurable => "kura_wsv_application_receipt_durable",
            Self::QueueFinalized => "queue_finalized",
            Self::Conflict => "conflict",
        }
    }
}
#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for SumeragiAutonomousLaneExecutionStage {
    fn write_json(&self, out: &mut String) {
        norito::json::write_json_string(self.as_str(), out);
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_json_string_to(self.as_str(), out)
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for SumeragiAutonomousLaneExecutionStage {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        match parser.parse_string()?.as_str() {
            "reservations_durable" => Ok(Self::ReservationsDurable),
            "executable_payload_durable" => Ok(Self::ExecutablePayloadDurable),
            "payload_availability_certified" => Ok(Self::PayloadAvailabilityCertified),
            "lane_certified" => Ok(Self::LaneCertified),
            "certified_bundle_durable" => Ok(Self::CertifiedBundleDurable),
            "merge_candidate_durable" => Ok(Self::MergeCandidateDurable),
            "global_carrier_committed" => Ok(Self::GlobalCarrierCommitted),
            "kura_wsv_application_receipt_durable" => Ok(Self::KuraWsvApplicationReceiptDurable),
            "queue_finalized" => Ok(Self::QueueFinalized),
            "conflict" => Ok(Self::Conflict),
            other => Err(norito::json::Error::Message(format!(
                "unknown autonomous lane execution stage `{other}`"
            ))),
        }
    }
}
/// Evidence-derived reason that an autonomous lane execution is not advancing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
#[norito(rename_all = "snake_case")]
pub enum SumeragiAutonomousLaneExecutionStuckReason {
    /// Queue ownership is durable, but the producer-authenticated executable payload is not.
    AwaitingExecutablePayload,
    /// The durable executable payload has no matching availability QC.
    AwaitingPayloadAvailability,
    /// Available payload bytes have no matching prepare/commit certification.
    AwaitingLaneCertification,
    /// Certification exists but the exact complete source bundle cannot be rebuilt.
    CertifiedBundleUnavailable,
    /// A complete certified bundle has not entered a merge candidate.
    AwaitingMergeSelection,
    /// A certified merge sidecar has not entered the committed merge log.
    AwaitingGlobalCarrier,
    /// A committed carrier has no exact durable application receipt yet.
    AwaitingApplicationReceipt,
    /// The application receipt is durable, but local Queue replay cannot yet prove finalization.
    QueueFinalizationUnverifiable,
    /// Same-height durable identities or cross-stage hashes disagree.
    EvidenceConflict,
}
impl SumeragiAutonomousLaneExecutionStuckReason {
    /// Stable JSON/OpenAPI label used by diagnostics clients.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AwaitingExecutablePayload => "awaiting_executable_payload",
            Self::AwaitingPayloadAvailability => "awaiting_payload_availability",
            Self::AwaitingLaneCertification => "awaiting_lane_certification",
            Self::CertifiedBundleUnavailable => "certified_bundle_unavailable",
            Self::AwaitingMergeSelection => "awaiting_merge_selection",
            Self::AwaitingGlobalCarrier => "awaiting_global_carrier",
            Self::AwaitingApplicationReceipt => "awaiting_application_receipt",
            Self::QueueFinalizationUnverifiable => "queue_finalization_unverifiable",
            Self::EvidenceConflict => "evidence_conflict",
        }
    }
}
#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for SumeragiAutonomousLaneExecutionStuckReason {
    fn write_json(&self, out: &mut String) {
        norito::json::write_json_string(self.as_str(), out);
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_json_string_to(self.as_str(), out)
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for SumeragiAutonomousLaneExecutionStuckReason {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        match parser.parse_string()?.as_str() {
            "awaiting_executable_payload" => Ok(Self::AwaitingExecutablePayload),
            "awaiting_payload_availability" => Ok(Self::AwaitingPayloadAvailability),
            "awaiting_lane_certification" => Ok(Self::AwaitingLaneCertification),
            "certified_bundle_unavailable" => Ok(Self::CertifiedBundleUnavailable),
            "awaiting_merge_selection" => Ok(Self::AwaitingMergeSelection),
            "awaiting_global_carrier" => Ok(Self::AwaitingGlobalCarrier),
            "awaiting_application_receipt" => Ok(Self::AwaitingApplicationReceipt),
            "queue_finalization_unverifiable" => Ok(Self::QueueFinalizationUnverifiable),
            "evidence_conflict" => Ok(Self::EvidenceConflict),
            other => Err(norito::json::Error::Message(format!(
                "unknown autonomous lane execution stuck reason `{other}`"
            ))),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AutonomousLaneEvidenceGeometry {
    ReservationsOnly,
    PayloadOnly,
    CertifiedBundle,
    MergeSelected,
    Applied,
}
impl AutonomousLaneEvidenceGeometry {
    const fn from_presence(
        presence: (bool, bool, bool, bool),
    ) -> Option<AutonomousLaneEvidenceGeometry> {
        match presence {
            (false, false, false, false) => Some(Self::ReservationsOnly),
            (true, false, false, false) => Some(Self::PayloadOnly),
            (true, true, false, false) => Some(Self::CertifiedBundle),
            (true, true, true, false) => Some(Self::MergeSelected),
            (true, true, true, true) => Some(Self::Applied),
            _ => None,
        }
    }
}
impl SumeragiAutonomousLaneExecutionStage {
    const fn expected_stuck_reason(self) -> Option<SumeragiAutonomousLaneExecutionStuckReason> {
        match self {
            Self::ReservationsDurable => {
                Some(SumeragiAutonomousLaneExecutionStuckReason::AwaitingExecutablePayload)
            }
            Self::ExecutablePayloadDurable => {
                Some(SumeragiAutonomousLaneExecutionStuckReason::AwaitingPayloadAvailability)
            }
            Self::PayloadAvailabilityCertified => {
                Some(SumeragiAutonomousLaneExecutionStuckReason::AwaitingLaneCertification)
            }
            Self::LaneCertified => {
                Some(SumeragiAutonomousLaneExecutionStuckReason::CertifiedBundleUnavailable)
            }
            Self::CertifiedBundleDurable => {
                Some(SumeragiAutonomousLaneExecutionStuckReason::AwaitingMergeSelection)
            }
            Self::MergeCandidateDurable => {
                Some(SumeragiAutonomousLaneExecutionStuckReason::AwaitingGlobalCarrier)
            }
            Self::GlobalCarrierCommitted => {
                Some(SumeragiAutonomousLaneExecutionStuckReason::AwaitingApplicationReceipt)
            }
            Self::KuraWsvApplicationReceiptDurable => {
                Some(SumeragiAutonomousLaneExecutionStuckReason::QueueFinalizationUnverifiable)
            }
            Self::QueueFinalized => None,
            Self::Conflict => Some(SumeragiAutonomousLaneExecutionStuckReason::EvidenceConflict),
        }
    }
    const fn expected_evidence_geometry(self) -> Option<AutonomousLaneEvidenceGeometry> {
        match self {
            Self::ReservationsDurable => Some(AutonomousLaneEvidenceGeometry::ReservationsOnly),
            Self::ExecutablePayloadDurable
            | Self::PayloadAvailabilityCertified
            | Self::LaneCertified => Some(AutonomousLaneEvidenceGeometry::PayloadOnly),
            Self::CertifiedBundleDurable => Some(AutonomousLaneEvidenceGeometry::CertifiedBundle),
            Self::MergeCandidateDurable | Self::GlobalCarrierCommitted => {
                Some(AutonomousLaneEvidenceGeometry::MergeSelected)
            }
            Self::KuraWsvApplicationReceiptDurable | Self::QueueFinalized => {
                Some(AutonomousLaneEvidenceGeometry::Applied)
            }
            Self::Conflict => None,
        }
    }
}
/// One bounded, payload-free autonomous lane-execution diagnostics row.
///
/// Rows are ordered by their complete lane slot and proposal identity. Optional
/// hashes appear only after the corresponding durable evidence revalidates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiAutonomousLaneExecution {
    /// Execution lane.
    pub lane_id: LaneId,
    /// Dataspace bound to the lane.
    pub dataspace_id: DataSpaceId,
    /// Exact active lane incarnation.
    pub lane_incarnation: Hash,
    /// Lane-local block height.
    pub lane_block_height: u64,
    /// Lane-local block view.
    pub lane_block_view: u64,
    /// Global proposal height that allocated the lane-local slot.
    pub proposal_height: u64,
    /// Authenticated global proposal view, once a canonical payload anchor supplies it.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub proposal_view: Option<u64>,
    /// Stable identity of the leader/session that durably owns the reservation group.
    pub reservation_owner_hash: Hash,
    /// Stable provisional proposal-slot identity persisted by every reservation key.
    pub proposal_identity_hash: Hash,
    /// Domain-separated digest of the exact FIFO-ordered reservation keys.
    pub reservation_group_hash: Hash,
    /// Exact finalized lane proposal identity, absent before executable-payload durability.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub proposal_hash: Option<Hash>,
    /// Exact finalized descriptor identity, paired with `proposal_hash`.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub descriptor_hash: Option<Hash>,
    /// Producer-authenticated executable payload digest, when durable.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub executable_payload_hash: Option<Hash>,
    /// Complete authenticated source-bundle digest, when reconstructible.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub source_bundle_hash: Option<Hash>,
    /// Merge-ledger entry containing this source, when selected.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub merge_entry_hash: Option<HashOf<crate::merge::MergeLedgerEntry>>,
    /// Actual canonical carrier height, known only from an application receipt.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub application_block_height: Option<u64>,
    /// Actual canonical carrier hash, paired with `application_block_height`.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub application_block_hash: Option<HashOf<BlockHeader>>,
    /// Exact number of durable reservation identities.
    pub reservation_count: u64,
    /// Exact number of ordered transaction entrypoints.
    pub transaction_count: u64,
    /// Highest independently durable stage.
    pub highest_durable_stage: SumeragiAutonomousLaneExecutionStage,
    /// Evidence-derived wait/conflict reason, absent only for a proven terminal stage.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub stuck_reason: Option<SumeragiAutonomousLaneExecutionStuckReason>,
}
impl SumeragiAutonomousLaneExecution {
    /// Return the canonical ordering key for this row.
    #[must_use]
    pub const fn ordering_key(&self) -> (LaneId, DataSpaceId, Hash, u64, u64, u64, Hash) {
        (
            self.lane_id,
            self.dataspace_id,
            self.lane_incarnation,
            self.lane_block_height,
            self.lane_block_view,
            self.proposal_height,
            self.proposal_identity_hash,
        )
    }
    fn validate_identity_and_counts(&self) -> Result<(), &'static str> {
        let nonzero = |hash: &[u8]| hash.iter().any(|byte| *byte != 0);
        if self.lane_block_height == 0
            || self.proposal_height == 0
            || !nonzero(self.lane_incarnation.as_ref())
            || !nonzero(self.reservation_owner_hash.as_ref())
            || !nonzero(self.proposal_identity_hash.as_ref())
            || !nonzero(self.reservation_group_hash.as_ref())
        {
            return Err("autonomous lane execution diagnostics identity is malformed");
        }
        if self.proposal_hash.is_some() != self.descriptor_hash.is_some() {
            return Err(
                "autonomous lane execution proposal and descriptor hashes must appear together",
            );
        }
        if self
            .proposal_hash
            .into_iter()
            .chain(self.descriptor_hash)
            .any(|hash| !nonzero(hash.as_ref()))
        {
            return Err("autonomous lane execution finalized identity is malformed");
        }
        if self.application_block_height.is_some() != self.application_block_hash.is_some() {
            return Err("autonomous lane execution carrier height and hash must appear together");
        }
        if self.application_block_height == Some(0) {
            return Err("autonomous lane execution carrier height must be positive");
        }
        if self
            .application_block_hash
            .is_some_and(|hash| !nonzero(hash.as_ref()))
        {
            return Err("autonomous lane execution carrier hash must be non-zero");
        }
        let transaction_limit =
            u64::try_from(crate::merge::MAX_MERGE_EXECUTION_ENTRYPOINTS).unwrap_or(u64::MAX);
        if self.transaction_count == 0
            || self.transaction_count > transaction_limit
            || self.reservation_count > transaction_limit
            || (self.highest_durable_stage != SumeragiAutonomousLaneExecutionStage::Conflict
                && self.reservation_count != self.transaction_count)
        {
            return Err("autonomous lane execution counters are malformed");
        }
        Ok(())
    }
    /// Validate bounded counters, paired carrier identity, and stage geometry.
    ///
    /// # Errors
    ///
    /// Returns a stable reason when identity fields are zero, carrier fields are incomplete,
    /// counters exceed protocol bounds, or durable stage and wait-state evidence disagree.
    pub fn validate(&self) -> Result<(), &'static str> {
        self.validate_identity_and_counts()?;
        let nonzero = |hash: &[u8]| hash.iter().any(|byte| *byte != 0);
        for hash in [self.executable_payload_hash, self.source_bundle_hash] {
            if hash.is_some_and(|hash| !nonzero(hash.as_ref())) {
                return Err("autonomous lane execution evidence hash must be non-zero");
            }
        }
        if self
            .merge_entry_hash
            .is_some_and(|hash| !nonzero(hash.as_ref()))
        {
            return Err("autonomous lane execution merge entry hash must be non-zero");
        }
        let expected_reason = self.highest_durable_stage.expected_stuck_reason();
        if self.highest_durable_stage == SumeragiAutonomousLaneExecutionStage::Conflict
            && self.stuck_reason
                != Some(SumeragiAutonomousLaneExecutionStuckReason::EvidenceConflict)
        {
            return Err("autonomous lane execution conflict requires an evidence-conflict reason");
        }
        if self.stuck_reason != expected_reason {
            return Err("autonomous lane execution stage and stuck reason disagree");
        }
        if self.highest_durable_stage == SumeragiAutonomousLaneExecutionStage::Conflict {
            return Ok(());
        }
        if self.highest_durable_stage == SumeragiAutonomousLaneExecutionStage::ReservationsDurable
            && self.proposal_view.is_some()
        {
            return Err(
                "autonomous lane reservation diagnostics cannot claim a global proposal view",
            );
        }
        if (self.highest_durable_stage == SumeragiAutonomousLaneExecutionStage::ReservationsDurable)
            != self.proposal_hash.is_none()
        {
            return Err(
                "autonomous lane execution finalized identity disagrees with its durable stage",
            );
        }
        let has_payload = self.executable_payload_hash.is_some();
        let has_bundle = self.source_bundle_hash.is_some();
        let has_merge = self.merge_entry_hash.is_some();
        let has_carrier = self.application_block_height.is_some();
        if matches!(
            self.highest_durable_stage,
            SumeragiAutonomousLaneExecutionStage::KuraWsvApplicationReceiptDurable
                | SumeragiAutonomousLaneExecutionStage::QueueFinalized
        ) && !has_carrier
        {
            return Err("durable autonomous application stage requires a carrier identity");
        }
        let observed_geometry = AutonomousLaneEvidenceGeometry::from_presence((
            has_payload,
            has_bundle,
            has_merge,
            has_carrier,
        ));
        if observed_geometry != self.highest_durable_stage.expected_evidence_geometry() {
            return Err("autonomous lane execution evidence does not match its durable stage");
        }
        Ok(())
    }
}
/// Durable-application state of one Native AMX participant control.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode)]
#[norito(rename_all = "snake_case")]
pub enum SumeragiNativeAmxParticipantApplicationState {
    /// Participant QCs are certified, but no canonical global carrier is committed yet.
    CertifiedPendingCarrier,
    /// A canonical carrier is committed, but its exact durable evidence is incomplete.
    CommittedEvidencePending,
    /// The exact application sidecar and replicated WSV frontier revalidate.
    DurablyApplied,
    /// Same-height durable evidence contains conflicting authenticated identities.
    Conflict,
}
impl SumeragiNativeAmxParticipantApplicationState {
    /// Stable JSON/OpenAPI label used by diagnostics clients.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CertifiedPendingCarrier => "certified_pending_carrier",
            Self::CommittedEvidencePending => "committed_evidence_pending",
            Self::DurablyApplied => "durably_applied",
            Self::Conflict => "conflict",
        }
    }
}
#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for SumeragiNativeAmxParticipantApplicationState {
    fn write_json(&self, out: &mut String) {
        norito::json::write_json_string(self.as_str(), out);
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_json_string_to(self.as_str(), out)
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for SumeragiNativeAmxParticipantApplicationState {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        match parser.parse_string()?.as_str() {
            "certified_pending_carrier" => Ok(Self::CertifiedPendingCarrier),
            "committed_evidence_pending" => Ok(Self::CommittedEvidencePending),
            "durably_applied" => Ok(Self::DurablyApplied),
            "conflict" => Ok(Self::Conflict),
            other => Err(norito::json::Error::Message(format!(
                "unknown Native AMX participant application state `{other}`"
            ))),
        }
    }
}
/// One bounded Native AMX participant-application diagnostics row.
///
/// Rows are ordered by `(lane_id, dataspace_id, lane_incarnation)` in the containing diagnostics
/// response. The record carries only hashes and counters; transaction bodies and other unbounded
/// application material stay in Kura.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct SumeragiNativeAmxParticipantApplication {
    /// Participant lane.
    pub lane_id: LaneId,
    /// Participant dataspace.
    pub dataspace_id: DataSpaceId,
    /// Exact active lane incarnation.
    pub lane_incarnation: Hash,
    /// Participant lane-local height.
    pub participant_height: u64,
    /// Participant lane-local view.
    pub participant_view: u64,
    /// Immediate predecessor lane-local height.
    pub predecessor_height: u64,
    /// Descriptor hash of the predecessor, absent only at the lane genesis frontier.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub predecessor_descriptor_hash: Option<Hash>,
    /// Descriptor hash certified for this participant height.
    pub descriptor_hash: Hash,
    /// Proposal hash certified for this participant height.
    pub proposal_hash: Hash,
    /// Zero-effect participant settlement hash certified by both phase QCs.
    pub settlement_hash: HashOf<LaneBlockCommitment>,
    /// Number of ordered grouped transaction sources represented by the control.
    pub source_count: u64,
    /// Canonical global carrier height, present for committed or durable evidence only.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub application_block_height: Option<u64>,
    /// Canonical global carrier hash, paired with `application_block_height`.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub application_block_hash: Option<HashOf<BlockHeader>>,
    /// Current evidence-derived application state.
    pub state: SumeragiNativeAmxParticipantApplicationState,
}
impl SumeragiNativeAmxParticipantApplication {
    /// Validate geometry, bounded grouping, and optional carrier identity.
    ///
    /// # Errors
    ///
    /// Returns a stable reason when the row is malformed or internally inconsistent.
    pub fn validate(&self) -> Result<(), &'static str> {
        let nonzero_hash = |hash: &[u8]| hash.iter().any(|byte| *byte != 0);
        if !nonzero_hash(self.lane_incarnation.as_ref())
            || !nonzero_hash(self.descriptor_hash.as_ref())
            || !nonzero_hash(self.proposal_hash.as_ref())
            || !nonzero_hash(self.settlement_hash.as_ref())
        {
            return Err("Native AMX participant diagnostics hashes must be non-zero");
        }
        if self.participant_height == 0
            || self.predecessor_height.checked_add(1) != Some(self.participant_height)
        {
            return Err("Native AMX participant diagnostics predecessor must be contiguous");
        }
        if (self.predecessor_height == 0) != self.predecessor_descriptor_hash.is_none() {
            return Err(
                "Native AMX participant diagnostics predecessor hash must match its height",
            );
        }
        if self
            .predecessor_descriptor_hash
            .is_some_and(|hash| !nonzero_hash(hash.as_ref()))
        {
            return Err("Native AMX participant diagnostics predecessor hash must be non-zero");
        }
        if !(1..=SUMERAGI_NATIVE_AMX_PARTICIPANT_APPLICATION_SOURCES_MAX)
            .contains(&self.source_count)
        {
            return Err("Native AMX participant diagnostics source count is out of bounds");
        }
        if self.application_block_height.is_some() != self.application_block_hash.is_some() {
            return Err(
                "Native AMX participant diagnostics application height and hash must appear together",
            );
        }
        if self.application_block_height == Some(0) {
            return Err("Native AMX participant diagnostics application height must be positive");
        }
        if self
            .application_block_hash
            .is_some_and(|hash| !nonzero_hash(hash.as_ref()))
        {
            return Err("Native AMX participant diagnostics application hash must be non-zero");
        }
        let state_requires_application = matches!(
            self.state,
            SumeragiNativeAmxParticipantApplicationState::CommittedEvidencePending
                | SumeragiNativeAmxParticipantApplicationState::DurablyApplied
        );
        if state_requires_application != self.application_block_height.is_some() {
            return Err(
                "Native AMX participant diagnostics state disagrees with its application block",
            );
        }
        Ok(())
    }
}
/// Operator and lane diagnostics returned by `/v1/sumeragi/diagnostics`.
///
/// This payload deliberately excludes reducer phase, height, view, leader, certificates, mode, and
/// timing. `/v1/sumeragi/status` is the sole source of authoritative consensus state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "operator diagnostics expose independent queue-pressure flags"
)]
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
    /// Aggregated lane-level settlement commitments.
    pub lane_settlement_commitments: Vec<LaneBlockCommitment>,
    /// Certified lane relay envelopes.
    pub lane_relay_envelopes: Vec<LaneRelayEnvelope>,
    /// Planned lane-local payload ownership and RBC identities.
    pub lane_payload_ownerships: Vec<SumeragiLanePayloadOwnership>,
    /// Certified standalone lane-local block summaries.
    pub committed_lane_blocks: Vec<SumeragiCommittedLaneBlock>,
    /// Cached standalone lane-local block consensus sessions.
    pub lane_block_sessions: Vec<SumeragiLaneBlockSessionStatus>,
    /// Count of lanes that still require a governance manifest.
    pub lane_governance_sealed_total: u32,
    /// Aliases of lanes that remain sealed.
    pub lane_governance_sealed_aliases: Vec<String>,
    /// Governance manifest readiness per lane.
    pub lane_governance: Vec<SumeragiLaneGovernance>,
    /// Bounded Native AMX participant-control application evidence.
    pub native_amx_participant_applications: Vec<SumeragiNativeAmxParticipantApplication>,
    /// Bounded restart-stable autonomous lane execution stages.
    pub autonomous_lane_executions: Vec<SumeragiAutonomousLaneExecution>,
}
impl SumeragiDiagnosticsStatus {
    /// Validate Native AMX receipts embedded directly in diagnostics settlements.
    ///
    /// Both top-level lane settlements and relay-contained settlements are
    /// checked because either vector may be consumed independently by an SDK.
    ///
    /// # Errors
    ///
    /// Returns a stable reason when any embedded receipt group is malformed.
    pub fn validate_native_amx_receipts(&self) -> Result<(), &'static str> {
        for settlement in &self.lane_settlement_commitments {
            settlement.validate_native_amx_receipts()?;
        }
        for envelope in &self.lane_relay_envelopes {
            envelope
                .settlement_commitment
                .validate_native_amx_receipts()?;
        }
        Ok(())
    }
    /// Validate bounded, canonical Native AMX participant diagnostics.
    ///
    /// # Errors
    ///
    /// Returns a stable reason when the vector exceeds its hard cap, a row is
    /// malformed, or route/incarnation keys are not strictly ordered.
    pub fn validate_native_amx_participant_applications(&self) -> Result<(), &'static str> {
        if self.native_amx_participant_applications.len()
            > SUMERAGI_NATIVE_AMX_PARTICIPANT_APPLICATIONS_MAX
        {
            return Err("Native AMX participant diagnostics vector exceeds its hard limit");
        }
        let mut previous = None;
        for row in &self.native_amx_participant_applications {
            row.validate()?;
            let key = (row.lane_id, row.dataspace_id, row.lane_incarnation);
            if previous.is_some_and(|previous| previous >= key) {
                return Err(
                    "Native AMX participant diagnostics must be strictly ordered by route and incarnation",
                );
            }
            previous = Some(key);
        }
        Ok(())
    }
    /// Validate bounded, canonical autonomous lane-execution diagnostics.
    ///
    /// # Errors
    ///
    /// Returns a stable reason when the vector exceeds its hard cap, a row is
    /// malformed, or complete lane-slot identities are not strictly ordered.
    pub fn validate_autonomous_lane_executions(&self) -> Result<(), &'static str> {
        if self.autonomous_lane_executions.len() > SUMERAGI_AUTONOMOUS_LANE_EXECUTIONS_MAX {
            return Err("autonomous lane execution diagnostics vector exceeds its hard limit");
        }
        let mut previous = None;
        for row in &self.autonomous_lane_executions {
            row.validate()?;
            let key = row.ordering_key();
            if previous.is_some_and(|previous| previous >= key) {
                return Err(
                    "autonomous lane execution diagnostics must be strictly ordered by exact identity",
                );
            }
            previous = Some(key);
        }
        Ok(())
    }
}
/// Minimal execution witness KV pair for SBV-AM prototypes.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ExecKv {
    /// Raw key bytes.
    pub key: Vec<u8>,
    /// Raw value bytes.
    pub value: Vec<u8>,
}
/// Execution witness containing reads and writes for SMT recomputation.
#[derive(Clone, Debug, PartialEq, Eq, Default, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
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
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode, IntoSchema)]
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
impl_decode_from_slice_via_codec!(Evidence);
impl_decode_from_slice_via_codec!(ExecWitnessMsg);
impl_decode_from_slice_via_codec!(ConsensusGenesisParams);
impl_decode_from_slice_via_codec!(NposGenesisParams);
impl_decode_from_slice_via_codec!(SumeragiMembershipStatus);
impl_decode_from_slice_via_codec!(SumeragiNposDiagnostics);
impl_decode_from_slice_via_codec!(SumeragiPipelineExecutionStatus);
impl_decode_from_slice_via_codec!(SumeragiNativeAmxParticipantApplicationState);
impl_decode_from_slice_via_codec!(SumeragiNativeAmxParticipantApplication);
impl_decode_from_slice_via_codec!(SumeragiDiagnosticsStatus);
impl_decode_from_slice_via_codec!(SumeragiLaneCommitment);
impl_decode_from_slice_via_codec!(SumeragiDataspaceCommitment);
impl_decode_from_slice_via_codec!(SumeragiCommittedLaneBlock);
impl_decode_from_slice_via_codec!(SumeragiLanePayloadOwnership);
impl_decode_from_slice_via_codec!(LaneBlockDescriptorV1);
impl_decode_from_slice_via_codec!(LaneBlockProposalV1);
impl_decode_from_slice_via_codec!(LaneBlockVoteBodyV1);
impl_decode_from_slice_via_codec!(LaneBlockQcV1);
impl_decode_from_slice_via_codec!(SumeragiRuntimeUpgradeHook);
impl_decode_from_slice_via_codec!(SumeragiLaneGovernance);
impl_decode_from_slice_via_codec!(NativeAmxPhase);
impl_decode_from_slice_via_codec!(NativeAmxAttestationBodyV2);
impl_decode_from_slice_via_codec!(NativeAmxAttestationQcV2);
impl_decode_from_slice_via_codec!(NativeAmxLegRecordV2);
impl_decode_from_slice_via_codec!(NativeAmxReceipt);
// Provide nicer `Debug` rendering for validator indices in test snapshots.
impl fmt::Display for CertPhase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            CertPhase::Prepare => "Prepare",
            CertPhase::Commit => "Commit",
            CertPhase::NewView => "NewView",
        };
        f.write_str(s)
    }
}
impl<'a> norito::core::DecodeFromSlice<'a> for LaneSettlementReceipt {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        decode_from_slice_canonical(bytes)
    }
}
#[cfg(test)]
#[path = "consensus_model_tests.rs"]
mod tests;
