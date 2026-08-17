//! Lane relay envelope for cross-lane commitments (NX-4).
//!
//! This carries the lane block header, compact global-finality reference and DA digest,
//! plus the settlement commitment and its hash so the merge ledger can verify
//! relay payloads deterministically. Pending in-memory envelopes omit authority;
//! authoritative use resolves the compact reference against Kura's verified
//! Sumeragi-v2 finality artifact and checks the statement inclusion proof.
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    asset::AssetDefinitionId,
    block::{
        BlockHeader, consensus::LaneBlockCommitment, consensus_v2::finality::V2FinalityArtifact,
    },
    da::commitment::DaCommitmentBundle,
    nexus::{AxtFastpqBinding, DataSpaceId, FeeSponsorProgramId, LaneId},
    peer::PeerId,
    prelude::Metadata,
};
use core::cmp::Ordering;
use iroha_crypto::{Hash, HashOf, MerkleProof};
use iroha_primitives::numeric::Quantity;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;
/// Prefix for contract-visible verified relay state keys.
pub const VERIFIED_LANE_RELAY_STATE_KEY_PREFIX: &str = "pkdeploy_verified_lane_relay";
/// Prefix for contract-visible verified fee sponsor vault-allocation keys.
pub const VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_STATE_KEY_PREFIX: &str =
    "pkdeploy_verified_fee_sponsor_vault_allocation";
/// Prefix for cumulative spend recorded against a verified sponsor-vault allocation.
pub const VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_USAGE_STATE_KEY_PREFIX: &str =
    "pkdeploy_fee_sponsor_vault_allocation_usage";
/// Prefix for cumulative merge-settled spend against a verified allocation.
pub const VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_SETTLED_USAGE_STATE_KEY_PREFIX: &str =
    "pkdeploy_fee_sponsor_vault_allocation_settled_usage";
/// FASTPQ business effect expected for verified lane-relay block commitments.
pub const LANE_RELAY_FASTPQ_EFFECT_TYPE: &str = "lane_relay_block";
const LANE_RELAY_FASTPQ_CLAIM_DIGEST_DOMAIN_V1: &[u8] = b"iroha.nexus.lane-relay.fastpq-claim.v1";
const LANE_FINALITY_STATEMENT_DOMAIN_V1: &[u8] = b"iroha.nexus.lane-finality-statement.v1";
const LANE_RELAY_MERGE_HINT_DOMAIN_V1: &[u8] = b"iroha.nexus.lane-relay.merge-hint.v1";
const LANE_RELAY_SETTLEMENT_HASH_DOMAIN_V1: &[u8] = b"iroha.nexus.lane-relay.settlement.v1";
const FEE_SPONSOR_VAULT_SOURCE_STATE_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.nexus.fee-sponsor-vault.source-state.v1";
const FEE_SPONSOR_VAULT_ALLOCATION_CLAIM_DOMAIN_V1: &[u8] =
    b"iroha.nexus.fee-sponsor-vault.allocation-claim.v1";
fn domain_separated_hash(domain: &[u8], payload: &[u8]) -> Hash {
    let domain_len = u64::try_from(domain.len())
        .expect("protocol-defined digest domains fit in u64")
        .to_le_bytes();
    Hash::new_from_chunks(&[&domain_len, domain, payload])
}
/// Relay envelope broadcast by Nexus lanes for merge validation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct LaneRelayEnvelope {
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Active incarnation commitment for the lane-local height namespace.
    pub lane_incarnation: Hash,
    /// Numeric dataspace identifier.
    pub dataspace_id: DataSpaceId,
    /// Lane-local block height associated with the settlement commitment.
    pub block_height: u64,
    /// Full lane block header being relayed.
    pub block_header: BlockHeader,
    /// Compact reference to genuine global finality for this exact effect.
    ///
    /// `None` is permitted only for pending transport/status. State resolves a
    /// present reference through Kura and verifies the proof before persistence.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub finality_authority: Option<LaneFinalityAuthorityV1>,
    /// Optional hash of the DA commitment bundle for the block payload.
    #[norito(default)]
    pub da_commitment_hash: Option<HashOf<DaCommitmentBundle>>,
    /// Optional standalone lane block descriptor hash for this relayed lane block.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub lane_block_descriptor_hash: Option<Hash>,
    /// Settlement commitment captured at the end of the lane block.
    pub settlement_commitment: LaneBlockCommitment,
    /// Norito hash of the settlement payload for quick verification.
    pub settlement_hash: HashOf<LaneBlockCommitment>,
    /// Total RBC bytes attributed to the lane in this block.
    #[norito(default)]
    pub rbc_bytes_total: u64,
    /// Optional manifest Merkle root for the dataspace associated with the lane.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub manifest_root: Option<[u8; 32]>,
    /// Untrusted `FastPQ` proof metadata carried with the relay.
    ///
    /// This metadata is progress evidence only. Merge authority additionally requires the exact
    /// envelope to have a committed cryptographically verified relay record.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub fastpq_proof: Option<LaneFastpqProofMaterial>,
}
/// Canonical post-execution effect authenticated by the global `CommitQC`.
///
/// This statement deliberately excludes the QC and `FastPQ` proof material.
/// Validators can therefore derive and sign it before either proof is attached,
/// while every merge-relevant effect remains committed by the resulting QC.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct LaneFinalityStatement {
    /// Statement format version.
    pub version: u8,
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Active lane incarnation for this lane-local height namespace.
    pub lane_incarnation: Hash,
    /// Numeric dataspace identifier.
    pub dataspace_id: DataSpaceId,
    /// Lane-local block height.
    pub block_height: u64,
    /// Hash of the global block header that carried the lane execution.
    pub block_header_hash: HashOf<BlockHeader>,
    /// Exact DA commitment advertised by that header.
    pub da_commitment_hash: Option<HashOf<DaCommitmentBundle>>,
    /// Canonical standalone lane-block descriptor hash.
    pub lane_block_descriptor_hash: Hash,
    /// Exact dataspace manifest root used for proof policy.
    pub manifest_root: [u8; 32],
    /// Complete post-execution settlement effect.
    pub settlement_commitment: LaneBlockCommitment,
    /// Domain-separated hash of the settlement effect.
    pub settlement_hash: HashOf<LaneBlockCommitment>,
    /// Total RBC bytes attributed to this lane block.
    pub rbc_bytes_total: u64,
}
/// Bounded reference from one relay effect to immutable Sumeragi-v2 finality.
///
/// The full finality artifact remains in Kura. Persisted relay state carries
/// only its hash, global height, and an `O(log lanes)` inclusion proof.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct LaneFinalityAuthorityV1 {
    /// Authority format version; exactly one in the first release.
    pub version: u8,
    /// Global block height whose `CommitQC` authenticated the statement tree.
    pub global_block_height: u64,
    /// Hash of the exact immutable finality artifact retained by Kura.
    pub finality_artifact_hash: HashOf<V2FinalityArtifact>,
    /// Inclusion proof for the envelope-derived statement.
    pub statement_proof: MerkleProof<LaneFinalityStatement>,
}
impl LaneFinalityStatement {
    /// Compute the domain-separated hash signed by the lane-finality committee.
    ///
    /// # Errors
    ///
    /// Returns [`LaneRelayError::Encode`] if canonical Norito encoding fails.
    pub fn hash(&self) -> Result<Hash, LaneRelayError> {
        let bytes = norito::encode_canonical(self)?;
        Ok(domain_separated_hash(
            LANE_FINALITY_STATEMENT_DOMAIN_V1,
            &bytes,
        ))
    }
}
impl PartialOrd for LaneFinalityStatement {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for LaneFinalityStatement {
    fn cmp(&self, other: &Self) -> Ordering {
        let lhs = norito::encode_canonical(self).expect("lane finality statement should encode");
        let rhs = norito::encode_canonical(other).expect("lane finality statement should encode");
        lhs.cmp(&rhs)
    }
}
/// Presence state for structurally valid relay `FastPQ` metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(tag = "status", content = "state")]
pub enum LaneRelayFastpqMaterialStatus {
    /// The relay carries no structurally valid `FastPQ` metadata.
    Missing,
    /// The relay carries structurally valid but unauthenticated `FastPQ` metadata.
    Present,
}
impl PartialOrd for LaneRelayEnvelope {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for LaneRelayEnvelope {
    fn cmp(&self, other: &Self) -> Ordering {
        let lhs = norito::encode_canonical(self).expect("lane relay envelope should encode");
        let rhs = norito::encode_canonical(other).expect("lane relay envelope should encode");
        lhs.cmp(&rhs)
    }
}
/// Stable business-facing reference for a previously verified lane relay envelope.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct LaneRelayEnvelopeRef {
    /// Numeric dataspace identifier.
    pub dataspace_id: DataSpaceId,
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Active incarnation of the lane-local height namespace.
    pub lane_incarnation: Hash,
    /// Lane-local block height associated with the finalized effect.
    pub block_height: u64,
}
impl LaneRelayEnvelopeRef {
    /// Return the canonical contract-state key for this verified lane relay.
    #[must_use]
    pub fn relay_state_key(&self) -> String {
        format!(
            "{VERIFIED_LANE_RELAY_STATE_KEY_PREFIX}_{}_{}_{}_{}",
            self.dataspace_id.as_u64(),
            self.lane_id.as_u32(),
            hex::encode(self.lane_incarnation.as_ref()),
            self.block_height,
        )
    }
}
/// Verified relay record persisted for restricted-source business effects.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct VerifiedLaneRelayRecord {
    /// Canonical relay reference used by business flows.
    pub relay_ref: LaneRelayEnvelopeRef,
    /// Original relay envelope that passed verification.
    pub relay_envelope: LaneRelayEnvelope,
    /// Deterministic hash of the proof payload used during registration.
    pub proof_payload_hash: Hash,
    /// `FastPQ` statement digest verified during registration.
    #[norito(default)]
    pub fastpq_statement_digest: [u8; 32],
    /// Canonical lane-finality statement hash authenticated by the QC.
    pub lane_finality_statement_hash: Hash,
    /// Pre-execution state root proven by `FastPQ`.
    pub fastpq_old_root: [u8; 32],
    /// Post-execution state root proven by `FastPQ`.
    pub fastpq_new_root: [u8; 32],
    /// Lane-finality statement hash proven as the `FastPQ` transaction-set root.
    pub fastpq_tx_set_hash: [u8; 32],
    /// Deterministic digest of the embedded `FastPQ` proof payload.
    pub fastpq_proof_digest: Hash,
    /// Block height where the relay proof was verified and persisted.
    pub verified_at_height: u64,
    /// Manifest root enforced during registration.
    pub manifest_root: [u8; 32],
    /// FASTPQ binding that contracts consume on-ledger.
    pub fastpq_binding: AxtFastpqBinding,
}
/// Proof-backed cross-lane spend allocation for one sponsor-program vault asset.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct VerifiedFeeSponsorVaultAllocation {
    /// Exact sponsor program authorized to consume the allocation.
    pub program_id: FeeSponsorProgramId,
    /// Immutable program revision bound by the source proof.
    pub program_revision: u64,
    /// Canonical fee asset allocated by the source vault.
    pub asset_definition_id: AssetDefinitionId,
    /// Maximum amount authorized by this spend lease.
    pub verified_allocation: Quantity,
    /// Source dataspace that owns the authoritative program vault.
    pub source_dataspace_id: DataSpaceId,
    /// Monotonic source consensus height bound by the proof.
    pub source_height: u64,
    /// Source state root that commits the vault allocation and counters.
    pub source_state_root: Hash,
    /// Consensus height after which the allocation cannot admit new charges.
    pub expires_at_height: u64,
    /// Globally unique proof-bound spend lease identifier.
    pub lease_id: Hash,
    /// Deterministic hash of the proof payload used during registration.
    pub proof_payload_hash: Hash,
    /// `FastPQ` statement digest verified during registration.
    #[norito(default)]
    pub fastpq_statement_digest: [u8; 32],
    /// Deterministic digest of the embedded `FastPQ` proof payload.
    pub fastpq_proof_digest: Hash,
    /// Block height where the balance proof was verified and persisted.
    pub verified_at_height: u64,
    /// Manifest root enforced during registration.
    pub manifest_root: [u8; 32],
    /// FASTPQ binding that admission consumes on-ledger.
    pub fastpq_binding: AxtFastpqBinding,
}
/// `FastPQ` proof metadata attached to a lane relay envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct LaneFastpqProofMaterial {
    /// Deterministic digest of the proof payload.
    pub proof_digest: Hash,
    /// Block height where the proof was verified.
    pub verified_at_height: u64,
}
#[derive(Clone, Debug, Encode)]
struct LaneRelayFastpqClaim {
    version: u8,
    lane_finality_statement_hash: Hash,
}
#[derive(Clone, Debug, Encode)]
struct LaneRelayMergeHint {
    version: u8,
    lane_id: LaneId,
    lane_incarnation: Hash,
    dataspace_id: DataSpaceId,
    block_height: u64,
    tip_hash: HashOf<BlockHeader>,
    global_block_height: u64,
    finality_artifact_hash: HashOf<V2FinalityArtifact>,
    da_commitment_hash: Option<HashOf<DaCommitmentBundle>>,
    lane_block_descriptor_hash: Option<Hash>,
    settlement_hash: HashOf<LaneBlockCommitment>,
    rbc_bytes_total: u64,
    lane_finality_statement_hash: Hash,
}
/// Compute the canonical claim digest that a FASTPQ lane-relay proof must bind.
///
/// The encoded claim binds the canonical lane-finality statement. Because the
/// statement contains the exact descriptor, manifest, DA commitment, settlement
/// effect, and RBC accounting, proof validity cannot authorize an alternate
/// effect and remains subordinate to the lane committee's finality signature.
///
/// # Errors
/// Returns a [`LaneRelayError`] when the finality statement is incomplete or
/// inconsistent, or [`LaneRelayError::Encode`] if canonical encoding fails.
pub fn lane_relay_fastpq_claim_digest(
    envelope: &LaneRelayEnvelope,
) -> Result<Hash, LaneRelayError> {
    let claim = LaneRelayFastpqClaim {
        version: 1,
        lane_finality_statement_hash: envelope.lane_finality_statement_hash()?,
    };
    let bytes = norito::encode_canonical(&claim)?;
    Ok(domain_separated_hash(
        LANE_RELAY_FASTPQ_CLAIM_DIGEST_DOMAIN_V1,
        &bytes,
    ))
}
/// Canonical source-ledger claim authorized by a sponsor-vault spend lease.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct FeeSponsorVaultAllocationClaim {
    /// Exact sponsor program authorized to spend the allocation.
    pub program_id: FeeSponsorProgramId,
    /// Immutable program revision bound by the source proof.
    pub program_revision: u64,
    /// Canonical fee asset allocated by the source vault.
    pub asset_definition_id: AssetDefinitionId,
    /// Maximum amount authorized by this spend lease.
    pub verified_allocation: Quantity,
    /// Dataspace containing the authoritative source vault.
    pub source_dataspace_id: DataSpaceId,
    /// Monotonic source consensus height bound by the proof.
    pub source_height: u64,
    /// Source state root committing the vault and budget state.
    pub source_state_root: Hash,
    /// Consensus height after which the spend lease expires.
    pub expires_at_height: u64,
    /// Globally unique proof-bound spend lease identifier.
    pub lease_id: Hash,
}
#[derive(Clone, Debug, Encode)]
struct FeeSponsorVaultSourceStateCommitment {
    version: u8,
    program_id: FeeSponsorProgramId,
    program_revision: u64,
    asset_definition_id: AssetDefinitionId,
    vault_balance: Quantity,
    source_dataspace_id: DataSpaceId,
    source_height: u64,
}
/// Commit the exact source-vault snapshot from which a relay allocation was derived.
///
/// Registration recomputes this commitment from authoritative world state. This
/// prevents a valid proof over a self-declared amount from allocating more than
/// the isolated program vault actually contains.
#[must_use]
pub fn fee_sponsor_vault_source_state_root(
    program_id: &FeeSponsorProgramId,
    program_revision: u64,
    asset_definition_id: &AssetDefinitionId,
    vault_balance: &Quantity,
    source_dataspace_id: DataSpaceId,
    source_height: u64,
) -> Hash {
    let commitment = FeeSponsorVaultSourceStateCommitment {
        version: 1,
        program_id: program_id.clone(),
        program_revision,
        asset_definition_id: asset_definition_id.clone(),
        vault_balance: vault_balance.clone(),
        source_dataspace_id,
        source_height,
    };
    domain_separated_hash(
        FEE_SPONSOR_VAULT_SOURCE_STATE_ROOT_DOMAIN_V1,
        &commitment.encode(),
    )
}
/// Compute the canonical claim digest for a verified sponsor-vault allocation proof.
#[must_use]
pub fn fee_sponsor_vault_allocation_claim_digest(
    allocation: &FeeSponsorVaultAllocationClaim,
) -> Hash {
    domain_separated_hash(
        FEE_SPONSOR_VAULT_ALLOCATION_CLAIM_DOMAIN_V1,
        &allocation.encode(),
    )
}
/// Operator evidence bundle captured when ingesting a lane relay envelope fails.
///
/// This payload is intended for local persistence and troubleshooting workflows. It is not
/// required for consensus, but it provides a stable Norito-encoded bundle that operators can
/// export when investigating invalid or conflicting relay proofs.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct LaneRelayEvidenceBundle {
    /// Lane relay envelope that triggered the failure.
    pub envelope: LaneRelayEnvelope,
    /// Stable error label describing why ingestion failed.
    pub error_label: String,
    /// Human-readable error detail (best-effort).
    #[norito(default)]
    pub error_message: String,
}
/// Emergency validator-peer override for a lane when lane relay quorum is at risk.
///
/// Application of this override is gated by `nexus.lane_relay_emergency.enabled`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct LaneRelayEmergencyValidatorSet {
    /// Live consensus peers temporarily allowed to fill missing lane-relay committee slots.
    pub peers: Vec<PeerId>,
    /// Block height (inclusive) after which the override expires.
    pub expires_at_height: u64,
    /// Optional metadata describing why the override was applied.
    #[norito(default)]
    pub metadata: Metadata,
}
/// Quorum parameters used to validate [`LaneRelayEnvelope`] proofs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaneRelayQuorumContext {
    /// Total validators expected in the roster.
    pub validator_count: u32,
    /// Minimum signatures required for quorum.
    pub min_quorum: u32,
}
impl LaneRelayQuorumContext {
    /// Construct a new quorum context.
    ///
    /// # Errors
    ///
    /// Returns [`LaneRelayError::InvalidValidatorSet`] when the validator count is zero or the quorum exceeds the roster length.
    pub fn new(validator_count: u32, min_quorum: u32) -> Result<Self, LaneRelayError> {
        let ctx = Self {
            validator_count,
            min_quorum,
        };
        ctx.ensure_valid()?;
        Ok(ctx)
    }
    fn ensure_valid(self) -> Result<(), LaneRelayError> {
        if self.validator_count == 0 || self.min_quorum == 0 {
            return Err(LaneRelayError::InvalidValidatorSet {
                validator_count: self.validator_count,
                min_quorum: self.min_quorum,
            });
        }
        if self.min_quorum > self.validator_count {
            return Err(LaneRelayError::InvalidValidatorSet {
                validator_count: self.validator_count,
                min_quorum: self.min_quorum,
            });
        }
        Ok(())
    }
}
impl LaneRelayEnvelope {
    /// Domain-separated context tag used by standalone lane-block votes.
    ///
    /// This tag identifies the lane-local consensus domain only. It is not
    /// merge authority; authoritative relay effects are bound by the global
    /// execution commitment and [`LaneFinalityAuthorityV1`].
    #[must_use]
    pub fn lane_qc_mode_tag_for(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        base_mode_tag: &str,
    ) -> String {
        format!(
            "{base_mode_tag}::lane-relay:v1:{}:{}",
            dataspace_id.as_u64(),
            lane_id.as_u32()
        )
    }
    /// Derive the canonical post-execution lane-finality statement.
    ///
    /// The statement excludes both proof carriers, so callers may derive it on
    /// an unsigned envelope and attach the QC and `FastPQ` proof afterward.
    ///
    /// # Errors
    ///
    /// Returns a [`LaneRelayError`] if the envelope is structurally invalid,
    /// lacks a non-zero descriptor or manifest root, or cannot be encoded.
    pub fn lane_finality_statement(&self) -> Result<LaneFinalityStatement, LaneRelayError> {
        self.verify()?;
        let lane_block_descriptor_hash = self
            .lane_block_descriptor_hash
            .ok_or(LaneRelayError::MissingLaneBlockDescriptorHash)?;
        if lane_block_descriptor_hash
            .as_ref()
            .iter()
            .all(|byte| *byte == 0)
        {
            return Err(LaneRelayError::ZeroLaneBlockDescriptorHash);
        }
        let manifest_root = self
            .manifest_root
            .ok_or(LaneRelayError::MissingManifestRoot)?;
        if manifest_root.iter().all(|byte| *byte == 0) {
            return Err(LaneRelayError::ZeroManifestRoot);
        }
        Ok(LaneFinalityStatement {
            version: 1,
            lane_id: self.lane_id,
            lane_incarnation: self.lane_incarnation,
            dataspace_id: self.dataspace_id,
            block_height: self.block_height,
            block_header_hash: self.block_header.hash(),
            da_commitment_hash: self.da_commitment_hash,
            lane_block_descriptor_hash,
            manifest_root,
            settlement_commitment: self.settlement_commitment.clone(),
            settlement_hash: self.settlement_hash,
            rbc_bytes_total: self.rbc_bytes_total,
        })
    }
    /// Compute the canonical post-execution lane-finality statement hash.
    ///
    /// # Errors
    ///
    /// Returns a [`LaneRelayError`] when statement derivation or encoding fails.
    pub fn lane_finality_statement_hash(&self) -> Result<Hash, LaneRelayError> {
        self.lane_finality_statement()?.hash()
    }
    /// Return whether two envelopes describe the exact same finality effect.
    ///
    /// QC aggregation and proof carriers are deliberately ignored: they can be
    /// enriched independently, but no merge-relevant field may change at a
    /// fixed `(lane, dataspace, incarnation, height)` coordinate.
    #[must_use]
    pub fn same_finality_effect(&self, other: &Self) -> bool {
        self.lane_id == other.lane_id
            && self.lane_incarnation == other.lane_incarnation
            && self.dataspace_id == other.dataspace_id
            && self.block_height == other.block_height
            && self.block_header == other.block_header
            && self.da_commitment_hash == other.da_commitment_hash
            && self.lane_block_descriptor_hash == other.lane_block_descriptor_hash
            && self.settlement_commitment == other.settlement_commitment
            && self.settlement_hash == other.settlement_hash
            && self.rbc_bytes_total == other.rbc_bytes_total
            && self.manifest_root == other.manifest_root
    }
    /// Create an envelope and derive the settlement hash from the payload.
    ///
    /// # Errors
    ///
    /// Returns [`LaneRelayError::DaCommitmentHashMismatch`] when the DA commitment hash differs
    /// from the header, or [`LaneRelayError::Encode`] if hashing the settlement commitment fails.
    pub fn new(
        block_header: BlockHeader,
        da_commitment_hash: Option<HashOf<DaCommitmentBundle>>,
        settlement_commitment: LaneBlockCommitment,
        rbc_bytes_total: u64,
    ) -> Result<Self, LaneRelayError> {
        let settlement_hash = compute_settlement_hash(&settlement_commitment)?;
        let block_height = settlement_commitment.block_height;
        if block_height == 0 {
            return Err(LaneRelayError::BlockHeightMismatch);
        }
        if settlement_commitment
            .lane_incarnation
            .as_ref()
            .iter()
            .all(|byte| *byte == 0)
        {
            return Err(LaneRelayError::ZeroLaneIncarnation);
        }
        if block_header.da_commitments_hash() != da_commitment_hash {
            return Err(LaneRelayError::DaCommitmentHashMismatch);
        }
        Ok(Self {
            lane_id: settlement_commitment.lane_id,
            lane_incarnation: settlement_commitment.lane_incarnation,
            dataspace_id: settlement_commitment.dataspace_id,
            block_height,
            block_header,
            finality_authority: None,
            da_commitment_hash,
            lane_block_descriptor_hash: None,
            settlement_commitment,
            settlement_hash,
            rbc_bytes_total,
            manifest_root: None,
            fastpq_proof: None,
        })
    }
    /// Stable business-facing reference for this relay envelope.
    #[must_use]
    pub fn relay_ref(&self) -> LaneRelayEnvelopeRef {
        LaneRelayEnvelopeRef {
            dataspace_id: self.dataspace_id,
            lane_id: self.lane_id,
            lane_incarnation: self.lane_incarnation,
            block_height: self.block_height,
        }
    }
    /// Validate the DA commitment and settlement shape.
    ///
    /// # Errors
    ///
    /// Propagates [`LaneRelayError::DaCommitmentHashMismatch`], [`LaneRelayError::SettlementBlockHeightMismatch`],
    /// [`LaneRelayError::SettlementLaneMismatch`],
    /// [`LaneRelayError::SettlementDataspaceMismatch`], or [`LaneRelayError::SettlementHashMismatch`]
    /// when validation fails, and may surface [`LaneRelayError::Encode`] if settlement hashing encounters an encoding error.
    pub fn verify(&self) -> Result<(), LaneRelayError> {
        if self.block_height == 0 {
            return Err(LaneRelayError::BlockHeightMismatch);
        }
        if self.settlement_commitment.block_height != self.block_height {
            return Err(LaneRelayError::SettlementBlockHeightMismatch);
        }
        if self.settlement_commitment.lane_id != self.lane_id {
            return Err(LaneRelayError::SettlementLaneMismatch);
        }
        if self.lane_incarnation.as_ref().iter().all(|byte| *byte == 0) {
            return Err(LaneRelayError::ZeroLaneIncarnation);
        }
        if self.settlement_commitment.lane_incarnation != self.lane_incarnation {
            return Err(LaneRelayError::SettlementLaneIncarnationMismatch);
        }
        if self.settlement_commitment.dataspace_id != self.dataspace_id {
            return Err(LaneRelayError::SettlementDataspaceMismatch);
        }
        if self.block_header.da_commitments_hash() != self.da_commitment_hash {
            return Err(LaneRelayError::DaCommitmentHashMismatch);
        }
        self.verify_settlement_integrity()?;
        self.verify_settlement_hash()
    }
    /// Return whether this envelope carries structurally valid `FastPQ` metadata.
    #[must_use]
    pub fn fastpq_metadata_status(&self) -> LaneRelayFastpqMaterialStatus {
        if self.has_fastpq_proof_material() {
            LaneRelayFastpqMaterialStatus::Present
        } else {
            LaneRelayFastpqMaterialStatus::Missing
        }
    }
    /// Whether this relay carries the structural material required for merge admission.
    ///
    /// This check does not resolve the finality reference or authenticate the
    /// `FastPQ` proof. Consensus code must validate both against authoritative state
    /// before treating the settlement as merge input.
    #[must_use]
    pub fn has_merge_admission_material(&self) -> bool {
        self.block_height > 0
            && self.finality_authority.as_ref().is_some_and(|authority| {
                authority.version == 1
                    && authority.global_block_height == self.block_header.height().get()
            })
            && self.lane_finality_statement().is_ok()
            && self.has_fastpq_proof_material()
    }
    /// Compute the canonical lane merge-hint root.
    ///
    /// # Errors
    ///
    /// Returns [`LaneRelayError::MissingFinalityAuthority`] when the relay has not yet reached finality,
    /// and [`LaneRelayError::Encode`] if the canonical hint payload cannot be encoded.
    pub fn merge_hint_root(&self) -> Result<Hash, LaneRelayError> {
        let authority = self
            .finality_authority
            .as_ref()
            .ok_or(LaneRelayError::MissingFinalityAuthority)?;
        let lane_finality_statement_hash = self.lane_finality_statement_hash()?;
        let hint = LaneRelayMergeHint {
            version: 4,
            lane_id: self.lane_id,
            lane_incarnation: self.lane_incarnation,
            dataspace_id: self.dataspace_id,
            block_height: self.block_height,
            tip_hash: self.block_header.hash(),
            global_block_height: authority.global_block_height,
            finality_artifact_hash: authority.finality_artifact_hash,
            da_commitment_hash: self.da_commitment_hash,
            lane_block_descriptor_hash: self.lane_block_descriptor_hash,
            settlement_hash: self.settlement_hash,
            rbc_bytes_total: self.rbc_bytes_total,
            lane_finality_statement_hash,
        };
        let bytes = norito::encode_canonical(&hint)?;
        Ok(domain_separated_hash(
            LANE_RELAY_MERGE_HINT_DOMAIN_V1,
            &bytes,
        ))
    }
    /// Attach the dataspace manifest root to the envelope for gossip/telemetry.
    #[must_use]
    pub fn with_manifest_root(mut self, manifest_root: Option<[u8; 32]>) -> Self {
        self.manifest_root = manifest_root;
        self
    }
    /// Attach the standalone lane block descriptor hash to the envelope.
    #[must_use]
    pub fn with_lane_block_descriptor_hash(
        mut self,
        lane_block_descriptor_hash: Option<Hash>,
    ) -> Self {
        self.lane_block_descriptor_hash = lane_block_descriptor_hash;
        self
    }
    /// Attach compact Kura-backed global finality authority.
    #[must_use]
    pub fn with_finality_authority(
        mut self,
        finality_authority: Option<LaneFinalityAuthorityV1>,
    ) -> Self {
        self.finality_authority = finality_authority;
        self
    }
    /// Validate the bounded finality-reference shape without consulting Kura.
    ///
    /// Cryptographic verification and statement inclusion are performed by
    /// consensus state, which owns the immutable artifact store.
    ///
    /// # Errors
    ///
    /// Returns an authority error when the reference is absent, versioned
    /// incorrectly, or names a different global height.
    pub fn validate_finality_authority_ref(&self) -> Result<(), LaneRelayError> {
        let authority = self
            .finality_authority
            .as_ref()
            .ok_or(LaneRelayError::MissingFinalityAuthority)?;
        if authority.version != 1 {
            return Err(LaneRelayError::UnsupportedFinalityAuthorityVersion(
                authority.version,
            ));
        }
        if authority.global_block_height != self.block_header.height().get() {
            return Err(LaneRelayError::FinalityAuthorityHeightMismatch);
        }
        Ok(())
    }
    /// Attach `FastPQ` proof material to the envelope.
    #[must_use]
    pub fn with_fastpq_proof_material(
        mut self,
        fastpq_proof: Option<LaneFastpqProofMaterial>,
    ) -> Self {
        self.fastpq_proof = fastpq_proof;
        self
    }
    /// Whether the envelope includes structurally valid `FastPQ` proof material.
    #[must_use]
    pub fn has_fastpq_proof_material(&self) -> bool {
        self.validate_fastpq_proof_metadata().is_ok()
    }
    /// Validate the shape of untrusted `FastPQ` proof metadata.
    ///
    /// This does not verify a proof. Consensus callers must additionally
    /// require the exact committed [`VerifiedLaneRelayRecord`].
    ///
    /// # Errors
    ///
    /// Returns [`LaneRelayError::MissingFastpqProof`] when proof metadata is absent and
    /// [`LaneRelayError::InvalidFastpqProof`] when the proof binding is malformed.
    pub fn validate_fastpq_proof_metadata(&self) -> Result<(), LaneRelayError> {
        let Some(material) = self.fastpq_proof.as_ref() else {
            return Err(LaneRelayError::MissingFastpqProof);
        };
        let bytes = material.proof_digest.as_ref();
        let is_zero_like =
            bytes[..Hash::LENGTH - 1].iter().all(|byte| *byte == 0) && bytes[Hash::LENGTH - 1] <= 1;
        if is_zero_like {
            return Err(LaneRelayError::InvalidFastpqProof);
        }
        if material.verified_at_height < self.block_header.height().get() {
            return Err(LaneRelayError::InvalidFastpqProof);
        }
        Ok(())
    }
    /// Re-compute the settlement hash and ensure it matches the envelope.
    ///
    /// # Errors
    ///
    /// Returns [`LaneRelayError::SettlementHashMismatch`] if the payload hash diverges or
    /// [`LaneRelayError::Encode`] if hashing the settlement commitment fails.
    pub fn verify_settlement_hash(&self) -> Result<(), LaneRelayError> {
        let expected = compute_settlement_hash(&self.settlement_commitment)?;
        if expected == self.settlement_hash {
            Ok(())
        } else {
            Err(LaneRelayError::SettlementHashMismatch)
        }
    }
    fn verify_settlement_integrity(&self) -> Result<(), LaneRelayError> {
        let settlement = &self.settlement_commitment;
        let mut total_local_amount = Quantity::zero();
        let mut total_xor_due = Quantity::zero();
        let mut total_xor_after_haircut = Quantity::zero();
        let mut total_xor_variance = Quantity::zero();
        let mut settlement_sources = std::collections::BTreeSet::new();
        let mut all_sources = std::collections::BTreeSet::new();
        for receipt in &settlement.receipts {
            if !settlement_sources.insert(receipt.source_id) {
                return Err(LaneRelayError::DuplicateSettlementSource);
            }
            all_sources.insert(receipt.source_id);
            total_local_amount = total_local_amount
                .try_add(&receipt.local_amount)
                .map_err(|_| LaneRelayError::SettlementTotalsMismatch)?;
            total_xor_due = total_xor_due
                .try_add(&receipt.xor_due)
                .map_err(|_| LaneRelayError::SettlementTotalsMismatch)?;
            total_xor_after_haircut = total_xor_after_haircut
                .try_add(&receipt.xor_after_haircut)
                .map_err(|_| LaneRelayError::SettlementTotalsMismatch)?;
            total_xor_variance = total_xor_variance
                .try_add(&receipt.xor_variance)
                .map_err(|_| LaneRelayError::SettlementTotalsMismatch)?;
        }
        if total_local_amount != settlement.total_local_amount
            || total_xor_due != settlement.total_xor_due
            || total_xor_after_haircut != settlement.total_xor_after_haircut
            || total_xor_variance != settlement.total_xor_variance
        {
            return Err(LaneRelayError::SettlementTotalsMismatch);
        }
        let mut nexus_fee_sources = std::collections::BTreeSet::new();
        for receipt in &settlement.nexus_fee_receipts {
            if receipt.lane_id != settlement.lane_id
                || receipt.dataspace_id != settlement.dataspace_id
                || receipt.block_height != settlement.block_height
            {
                return Err(LaneRelayError::SettlementReceiptCoordinateMismatch);
            }
            if !nexus_fee_sources.insert(receipt.source_id) {
                return Err(LaneRelayError::DuplicateSettlementSource);
            }
            all_sources.insert(receipt.source_id);
        }
        let mut native_amx_sources = std::collections::BTreeSet::new();
        for receipt in &settlement.native_amx_receipts {
            if receipt.lane_id != settlement.lane_id
                || receipt.dataspace_id != settlement.dataspace_id
                || receipt.lane_incarnation != settlement.lane_incarnation
                || receipt.authority_context_height != self.block_header.height().get()
                || receipt.lane_block_height != settlement.block_height
            {
                return Err(LaneRelayError::SettlementReceiptCoordinateMismatch);
            }
            if !native_amx_sources.insert(receipt.source_id) {
                return Err(LaneRelayError::DuplicateSettlementSource);
            }
            all_sources.insert(receipt.source_id);
        }
        if settlement.tx_count < u64::try_from(all_sources.len()).unwrap_or(u64::MAX) {
            return Err(LaneRelayError::SettlementTxCountMismatch);
        }
        Ok(())
    }
}
impl VerifiedLaneRelayRecord {
    /// Construct a verified relay record from the canonical verified inputs.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the canonical verified proof record"
    )]
    pub fn new(
        relay_envelope: LaneRelayEnvelope,
        proof_payload_hash: Hash,
        fastpq_statement_digest: [u8; 32],
        lane_finality_statement_hash: Hash,
        fastpq_old_root: [u8; 32],
        fastpq_new_root: [u8; 32],
        fastpq_tx_set_hash: [u8; 32],
        fastpq_proof_digest: Hash,
        verified_at_height: u64,
        manifest_root: [u8; 32],
        fastpq_binding: AxtFastpqBinding,
    ) -> Self {
        Self {
            relay_ref: relay_envelope.relay_ref(),
            relay_envelope,
            proof_payload_hash,
            fastpq_statement_digest,
            lane_finality_statement_hash,
            fastpq_old_root,
            fastpq_new_root,
            fastpq_tx_set_hash,
            fastpq_proof_digest,
            verified_at_height,
            manifest_root,
            fastpq_binding,
        }
    }
}
impl VerifiedFeeSponsorVaultAllocation {
    /// Construct a verified vault allocation from canonical verified inputs.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the canonical verified record fields"
    )]
    pub fn new(
        program_id: FeeSponsorProgramId,
        program_revision: u64,
        asset_definition_id: AssetDefinitionId,
        verified_allocation: Quantity,
        source_dataspace_id: DataSpaceId,
        source_height: u64,
        source_state_root: Hash,
        expires_at_height: u64,
        lease_id: Hash,
        proof_payload_hash: Hash,
        fastpq_statement_digest: [u8; 32],
        fastpq_proof_digest: Hash,
        verified_at_height: u64,
        manifest_root: [u8; 32],
        fastpq_binding: AxtFastpqBinding,
    ) -> Self {
        Self {
            program_id,
            program_revision,
            asset_definition_id,
            verified_allocation,
            source_dataspace_id,
            source_height,
            source_state_root,
            expires_at_height,
            lease_id,
            proof_payload_hash,
            fastpq_statement_digest,
            fastpq_proof_digest,
            verified_at_height,
            manifest_root,
            fastpq_binding,
        }
    }
    /// Return the canonical contract-state key for this exact spend lease.
    #[must_use]
    pub fn state_key_for(
        program_id: &FeeSponsorProgramId,
        asset_definition_id: &AssetDefinitionId,
        lease_id: &Hash,
    ) -> String {
        let material = format!("{program_id}|{asset_definition_id}|{lease_id}");
        let suffix = Hash::new(material.as_bytes());
        format!(
            "{VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_STATE_KEY_PREFIX}_{}",
            hex::encode(suffix.as_ref())
        )
    }
    /// Return the canonical state key for cumulative spend against one proof-bound lease.
    #[must_use]
    pub fn usage_state_key_for(lease_id: &Hash) -> String {
        format!(
            "{VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_USAGE_STATE_KEY_PREFIX}_{}",
            hex::encode(lease_id.as_ref())
        )
    }
    /// Return the canonical state key for cumulative merge-settled spend on one lease.
    #[must_use]
    pub fn settled_usage_state_key_for(lease_id: &Hash) -> String {
        format!(
            "{VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_SETTLED_USAGE_STATE_KEY_PREFIX}_{}",
            hex::encode(lease_id.as_ref())
        )
    }
}
/// Compute the Norito hash of a settlement commitment for relay envelopes.
///
/// # Errors
///
/// Returns [`LaneRelayError::Encode`] when Norito encoding of the settlement commitment fails.
pub fn compute_settlement_hash(
    settlement: &LaneBlockCommitment,
) -> Result<HashOf<LaneBlockCommitment>, LaneRelayError> {
    let bytes = norito::encode_canonical(settlement)?;
    Ok(HashOf::from_untyped_unchecked(domain_separated_hash(
        LANE_RELAY_SETTLEMENT_HASH_DOMAIN_V1,
        &bytes,
    )))
}
/// Errors encountered while validating or deriving relay envelopes.
#[derive(Debug, Error)]
pub enum LaneRelayError {
    /// Nexus lane lifecycle is disabled so relays are not accepted.
    #[error("lane relay processing requires nexus.enabled=true")]
    NexusDisabled,
    /// Lane identifier not present in the configured catalog.
    #[error("lane relay references unknown lane {0}")]
    UnknownLane(LaneId),
    /// Dataspace identifier not present in the configured catalog.
    #[error("lane relay references unknown dataspace {0}")]
    UnknownDataspace(DataSpaceId),
    /// Dataspace identifier does not match the configured lane.
    #[error("lane relay dataspace mismatch (expected {expected}, got {actual})")]
    DataspaceMismatch {
        /// Dataspace declared in the lane catalog.
        expected: DataSpaceId,
        /// Dataspace carried by the relay envelope.
        actual: DataSpaceId,
    },
    /// Relay height regresses compared to the latest known height for the lane.
    #[error("stale lane relay for {lane}: latest height {latest_height}, received {new_height}")]
    StaleRelay {
        /// Lane identifier associated with the relay.
        lane: LaneId,
        /// Highest height seen so far for the lane.
        latest_height: u64,
        /// Height carried by the stale relay.
        new_height: u64,
    },
    /// Relay belongs to an older incarnation of a lane that has since been reset.
    #[error(
        "stale lane relay for {lane}: lane reset at height {reset_height}, received height {relay_height}"
    )]
    StaleLaneIncarnation {
        /// Lane identifier associated with the relay.
        lane: LaneId,
        /// Committed height where the lane incarnation was reset.
        reset_height: u64,
        /// Height carried by the stale relay.
        relay_height: u64,
    },
    /// Relay carries a non-current lane incarnation commitment.
    #[error("stale lane relay for {lane}: expected incarnation {expected:?}, received {actual:?}")]
    LaneIncarnationMismatch {
        /// Lane identifier associated with the relay.
        lane: LaneId,
        /// Current active incarnation commitment.
        expected: Hash,
        /// Incarnation carried by the relay.
        actual: Hash,
    },
    /// Conflicting relay detected for the same lane/height with a different payload.
    #[error("conflicting lane relay for {lane} at height {height}")]
    ConflictingRelay {
        /// Lane identifier associated with the relay.
        lane: LaneId,
        /// Height shared by the conflicting relay.
        height: u64,
    },
    /// Settlement payload hash does not match the envelope.
    #[error("relay settlement hash does not match payload")]
    SettlementHashMismatch,
    /// Settlement commitment aggregate totals do not match its receipts.
    #[error("settlement commitment totals do not match receipts")]
    SettlementTotalsMismatch,
    /// Settlement commitment has fewer transactions than committed receipt sources.
    #[error("settlement commitment transaction count is lower than receipt count")]
    SettlementTxCountMismatch,
    /// Settlement commitment contains duplicate receipt source identifiers.
    #[error("settlement commitment contains duplicate receipt source identifiers")]
    DuplicateSettlementSource,
    /// Settlement receipt coordinates do not match the enclosing commitment.
    #[error("settlement receipt coordinates do not match commitment coordinates")]
    SettlementReceiptCoordinateMismatch,
    /// Settlement commitment height does not match the envelope's lane-local height.
    #[error("settlement commitment block height does not match envelope lane-local height")]
    SettlementBlockHeightMismatch,
    /// Envelope lane-local or global proposal block height is invalid.
    #[error("lane relay block height is invalid")]
    BlockHeightMismatch,
    /// Settlement commitment lane identifier differs from the envelope lane id.
    #[error("settlement commitment lane id does not match envelope lane id")]
    SettlementLaneMismatch,
    /// Settlement commitment incarnation differs from the envelope incarnation.
    #[error("settlement commitment lane incarnation does not match envelope")]
    SettlementLaneIncarnationMismatch,
    /// Lane incarnation is the reserved all-zero value.
    #[error("lane relay incarnation commitment must be non-zero")]
    ZeroLaneIncarnation,
    /// Settlement commitment dataspace identifier differs from the envelope dataspace id.
    #[error("settlement commitment dataspace id does not match envelope dataspace id")]
    SettlementDataspaceMismatch,
    /// QC does not certify the relayed block header.
    #[error("QC subject hash does not match block header hash")]
    QcSubjectMismatch,
    /// QC height does not match the relayed block height.
    #[error("QC height does not match block header height")]
    QcHeightMismatch,
    /// DA commitment hash in the envelope does not match the block header.
    #[error("DA commitment hash in envelope does not match block header")]
    DaCommitmentHashMismatch,
    /// Finality statement is missing the standalone lane-block descriptor hash.
    #[error("lane finality statement requires a lane block descriptor hash")]
    MissingLaneBlockDescriptorHash,
    /// Finality statement carries the reserved all-zero lane-block descriptor hash.
    #[error("lane finality statement lane block descriptor hash must be non-zero")]
    ZeroLaneBlockDescriptorHash,
    /// Finality statement is missing the dataspace manifest root.
    #[error("lane finality statement requires a manifest root")]
    MissingManifestRoot,
    /// Finality statement carries the reserved all-zero manifest root.
    #[error("lane finality statement manifest root must be non-zero")]
    ZeroManifestRoot,
    /// QC mode tag does not authenticate the canonical lane-finality statement.
    #[error("QC does not authenticate the canonical lane finality statement")]
    QcFinalityStatementMismatch,
    /// Compact global-finality authority is absent.
    #[error("global finality authority missing for relay envelope")]
    MissingFinalityAuthority,
    /// Compact global-finality authority uses an unsupported version.
    #[error("unsupported lane finality authority version {0}")]
    UnsupportedFinalityAuthorityVersion(u8),
    /// Compact authority references a different global block height.
    #[error("lane finality authority height does not match the relayed global block")]
    FinalityAuthorityHeightMismatch,
    /// Referenced immutable finality artifact is unavailable or differs from Kura.
    #[error("referenced lane finality artifact is unavailable or mismatched")]
    FinalityArtifactMismatch,
    /// The envelope-derived statement is not included in the `CommitQC` manifest.
    #[error("lane finality statement proof is invalid")]
    FinalityStatementProofInvalid,
    /// Norito encoding failed while hashing the settlement.
    #[error(transparent)]
    Encode(#[from] norito::core::Error),
    /// Validator roster length or quorum is invalid.
    #[error("invalid validator roster ({validator_count}) or quorum requirement ({min_quorum})")]
    InvalidValidatorSet {
        /// Total validators expected.
        validator_count: u32,
        /// Required quorum size.
        min_quorum: u32,
    },
    /// QC is missing while quorum validation is requested.
    #[error("QC missing for relay envelope")]
    MissingQc,
    /// Signer bitmap length does not match expected roster size.
    #[error("signer bitmap length {actual} does not match expected {expected}")]
    SignerBitmapLengthMismatch {
        /// Expected bitmap length in bytes.
        expected: usize,
        /// Actual bitmap length in bytes.
        actual: usize,
    },
    /// Signer bitmap references a validator outside the roster.
    #[error("signer bitmap references validator {signer} but roster size is {validator_count}")]
    InvalidSignerIndex {
        /// Signer index found in the bitmap.
        signer: u32,
        /// Total validators expected.
        validator_count: u32,
    },
    /// Signer bitmap does not satisfy the quorum.
    #[error("insufficient quorum: observed {observed}, expected {expected}")]
    InsufficientQuorum {
        /// Observed signatures in the bitmap.
        observed: u32,
        /// Expected minimum quorum.
        expected: u32,
    },
    /// Aggregate signature bytes are missing, zeroed, or invalid.
    #[error("aggregate signature missing or invalid for QC")]
    AggregateSignatureInvalid,
    /// `FastPQ` proof metadata is required for merge admission.
    #[error("FastPQ proof metadata missing for lane relay envelope")]
    MissingFastpqProof,
    /// `FastPQ` proof metadata failed structural validation.
    #[error("FastPQ proof metadata is invalid for lane relay envelope")]
    InvalidFastpqProof,
}
impl PartialEq for LaneRelayError {
    #[expect(
        clippy::too_many_lines,
        reason = "the exhaustive error comparison keeps all payload-bearing variants explicit"
    )]
    fn eq(&self, other: &Self) -> bool {
        use LaneRelayError::*;
        match (self, other) {
            (NexusDisabled, NexusDisabled)
            | (SettlementHashMismatch, SettlementHashMismatch)
            | (SettlementTotalsMismatch, SettlementTotalsMismatch)
            | (SettlementTxCountMismatch, SettlementTxCountMismatch)
            | (DuplicateSettlementSource, DuplicateSettlementSource)
            | (SettlementReceiptCoordinateMismatch, SettlementReceiptCoordinateMismatch)
            | (SettlementBlockHeightMismatch, SettlementBlockHeightMismatch)
            | (BlockHeightMismatch, BlockHeightMismatch)
            | (SettlementLaneMismatch, SettlementLaneMismatch)
            | (SettlementLaneIncarnationMismatch, SettlementLaneIncarnationMismatch)
            | (ZeroLaneIncarnation, ZeroLaneIncarnation)
            | (SettlementDataspaceMismatch, SettlementDataspaceMismatch)
            | (QcSubjectMismatch, QcSubjectMismatch)
            | (QcHeightMismatch, QcHeightMismatch)
            | (DaCommitmentHashMismatch, DaCommitmentHashMismatch)
            | (MissingLaneBlockDescriptorHash, MissingLaneBlockDescriptorHash)
            | (ZeroLaneBlockDescriptorHash, ZeroLaneBlockDescriptorHash)
            | (MissingManifestRoot, MissingManifestRoot)
            | (ZeroManifestRoot, ZeroManifestRoot)
            | (QcFinalityStatementMismatch, QcFinalityStatementMismatch)
            | (MissingFinalityAuthority, MissingFinalityAuthority)
            | (FinalityAuthorityHeightMismatch, FinalityAuthorityHeightMismatch)
            | (FinalityArtifactMismatch, FinalityArtifactMismatch)
            | (FinalityStatementProofInvalid, FinalityStatementProofInvalid)
            | (MissingQc, MissingQc)
            | (AggregateSignatureInvalid, AggregateSignatureInvalid)
            | (MissingFastpqProof, MissingFastpqProof)
            | (InvalidFastpqProof, InvalidFastpqProof)
            | (Encode(_), Encode(_)) => true,
            (UnknownLane(a_lane), UnknownLane(b_lane)) => a_lane == b_lane,
            (UnsupportedFinalityAuthorityVersion(a), UnsupportedFinalityAuthorityVersion(b)) => {
                a == b
            }
            (UnknownDataspace(a_dataspace), UnknownDataspace(b_dataspace)) => {
                a_dataspace == b_dataspace
            }
            (
                ConflictingRelay {
                    lane: a_lane,
                    height: a_height,
                },
                ConflictingRelay {
                    lane: b_lane,
                    height: b_height,
                },
            ) => a_lane == b_lane && a_height == b_height,
            (
                InvalidValidatorSet {
                    validator_count: a_count,
                    min_quorum: a_quorum,
                },
                InvalidValidatorSet {
                    validator_count: b_count,
                    min_quorum: b_quorum,
                },
            ) => a_count == b_count && a_quorum == b_quorum,
            (
                InvalidSignerIndex {
                    signer: a_signer,
                    validator_count: a_count,
                },
                InvalidSignerIndex {
                    signer: b_signer,
                    validator_count: b_count,
                },
            ) => a_signer == b_signer && a_count == b_count,
            (
                InsufficientQuorum {
                    observed: a_observed,
                    expected: a_expected,
                },
                InsufficientQuorum {
                    observed: b_observed,
                    expected: b_expected,
                },
            ) => a_observed == b_observed && a_expected == b_expected,
            (
                SignerBitmapLengthMismatch {
                    expected: a_expected,
                    actual: a_actual,
                },
                SignerBitmapLengthMismatch {
                    expected: b_expected,
                    actual: b_actual,
                },
            ) => a_expected == b_expected && a_actual == b_actual,
            (
                DataspaceMismatch {
                    expected: a_expected,
                    actual: a_actual,
                },
                DataspaceMismatch {
                    expected: b_expected,
                    actual: b_actual,
                },
            ) => a_expected == b_expected && a_actual == b_actual,
            (
                StaleRelay {
                    lane: a_lane,
                    latest_height: a_latest,
                    new_height: a_new,
                },
                StaleRelay {
                    lane: b_lane,
                    latest_height: b_latest,
                    new_height: b_new,
                },
            ) => a_lane == b_lane && a_latest == b_latest && a_new == b_new,
            (
                StaleLaneIncarnation {
                    lane: a_lane,
                    reset_height: a_reset,
                    relay_height: a_relay,
                },
                StaleLaneIncarnation {
                    lane: b_lane,
                    reset_height: b_reset,
                    relay_height: b_relay,
                },
            ) => a_lane == b_lane && a_reset == b_reset && a_relay == b_relay,
            (
                LaneIncarnationMismatch {
                    lane: a_lane,
                    expected: a_expected,
                    actual: a_actual,
                },
                LaneIncarnationMismatch {
                    lane: b_lane,
                    expected: b_expected,
                    actual: b_actual,
                },
            ) => a_lane == b_lane && a_expected == b_expected && a_actual == b_actual,
            _ => false,
        }
    }
}
impl Eq for LaneRelayError {}
impl LaneRelayError {
    /// Stable label for telemetry/logging.
    #[must_use]
    pub fn as_label(&self) -> &'static str {
        match self {
            LaneRelayError::NexusDisabled => "nexus_disabled",
            LaneRelayError::UnknownLane(_) => "unknown_lane",
            LaneRelayError::UnknownDataspace(_) => "unknown_dataspace",
            LaneRelayError::DataspaceMismatch { .. } => "dataspace_mismatch",
            LaneRelayError::StaleRelay { .. } => "stale_height",
            LaneRelayError::StaleLaneIncarnation { .. } => "stale_lane_incarnation",
            LaneRelayError::LaneIncarnationMismatch { .. } => "lane_incarnation_mismatch",
            LaneRelayError::ConflictingRelay { .. } => "conflicting_relay",
            LaneRelayError::SettlementHashMismatch => "settlement_hash_mismatch",
            LaneRelayError::SettlementTotalsMismatch => "settlement_totals_mismatch",
            LaneRelayError::SettlementTxCountMismatch => "settlement_tx_count_mismatch",
            LaneRelayError::DuplicateSettlementSource => "duplicate_settlement_source",
            LaneRelayError::SettlementReceiptCoordinateMismatch => {
                "settlement_receipt_coordinate_mismatch"
            }
            LaneRelayError::SettlementBlockHeightMismatch => "settlement_block_height_mismatch",
            LaneRelayError::BlockHeightMismatch => "block_height_mismatch",
            LaneRelayError::SettlementLaneMismatch => "settlement_lane_mismatch",
            LaneRelayError::SettlementLaneIncarnationMismatch => {
                "settlement_lane_incarnation_mismatch"
            }
            LaneRelayError::ZeroLaneIncarnation => "zero_lane_incarnation",
            LaneRelayError::SettlementDataspaceMismatch => "settlement_dataspace_mismatch",
            LaneRelayError::QcSubjectMismatch => "qc_subject_mismatch",
            LaneRelayError::QcHeightMismatch => "qc_height_mismatch",
            LaneRelayError::DaCommitmentHashMismatch => "da_commitment_hash_mismatch",
            LaneRelayError::MissingLaneBlockDescriptorHash => "missing_lane_block_descriptor_hash",
            LaneRelayError::ZeroLaneBlockDescriptorHash => "zero_lane_block_descriptor_hash",
            LaneRelayError::MissingManifestRoot => "missing_manifest_root",
            LaneRelayError::ZeroManifestRoot => "zero_manifest_root",
            LaneRelayError::QcFinalityStatementMismatch => "qc_finality_statement_mismatch",
            LaneRelayError::MissingFinalityAuthority => "missing_finality_authority",
            LaneRelayError::UnsupportedFinalityAuthorityVersion(_) => {
                "unsupported_finality_authority_version"
            }
            LaneRelayError::FinalityAuthorityHeightMismatch => "finality_authority_height_mismatch",
            LaneRelayError::FinalityArtifactMismatch => "finality_artifact_mismatch",
            LaneRelayError::FinalityStatementProofInvalid => "finality_statement_proof_invalid",
            LaneRelayError::InvalidValidatorSet { .. } => "invalid_validator_set",
            LaneRelayError::MissingQc => "missing_qc",
            LaneRelayError::SignerBitmapLengthMismatch { .. } => "signer_bitmap_length_mismatch",
            LaneRelayError::InvalidSignerIndex { .. } => "invalid_signer_index",
            LaneRelayError::InsufficientQuorum { .. } => "insufficient_quorum",
            LaneRelayError::AggregateSignatureInvalid => "aggregate_signature_invalid",
            LaneRelayError::MissingFastpqProof => "missing_fastpq_proof",
            LaneRelayError::InvalidFastpqProof => "invalid_fastpq_proof",
            LaneRelayError::Encode(_) => "encode",
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AccountId, NetworkId,
        block::{
            BlockHeader,
            consensus::{
                LaneBlockCommitment, NativeAmxReceipt, NexusFeeReceipt, NexusFeeScheduleInputs,
            },
        },
        nexus::FeeDebitSource,
    };
    use iroha_crypto::{Hash, HashOf, KeyPair, MerkleProof};
    use iroha_primitives::numeric::Quantity;
    use std::collections::BTreeSet;
    use std::num::NonZeroU64;
    fn test_network_id(label: &[u8]) -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(label)))
    }
    fn sample_commitment(height: u64, lane_id: u32, dataspace_id: u64) -> LaneBlockCommitment {
        LaneBlockCommitment {
            block_height: height,
            lane_id: LaneId::new(lane_id),
            lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
            dataspace_id: DataSpaceId::new(dataspace_id),
            tx_count: 1,
            total_local_amount: "0.00001".parse().expect("valid settlement quantity"),
            total_xor_due: "0.000005".parse().expect("valid settlement quantity"),
            total_xor_after_haircut: "0.000004".parse().expect("valid settlement quantity"),
            total_xor_variance: "0.000001".parse().expect("valid settlement quantity"),
            swap_metadata: None,
            receipts: vec![crate::block::consensus::LaneSettlementReceipt {
                source_id: [0xA5; 32],
                local_amount: "0.00001".parse().expect("valid settlement quantity"),
                xor_due: "0.000005".parse().expect("valid settlement quantity"),
                xor_after_haircut: "0.000004".parse().expect("valid settlement quantity"),
                xor_variance: "0.000001".parse().expect("valid settlement quantity"),
                timestamp_ms: 1_700_000_000_000,
            }],
            nexus_fee_receipts: Vec::new(),
            native_amx_receipts: Vec::new(),
        }
    }
    fn sample_header(height: u64, da_hash: Option<HashOf<DaCommitmentBundle>>) -> BlockHeader {
        let mut header = BlockHeader::new(
            NonZeroU64::new(height).expect("nonzero height"),
            None,
            None,
            None,
            1_700_000_000_000,
            0,
        );
        header.set_da_commitments_hash(da_hash);
        header
    }
    fn sample_lane_finality_statement() -> LaneFinalityStatement {
        let settlement_commitment = sample_commitment(6, 3, 2);
        let settlement_hash =
            compute_settlement_hash(&settlement_commitment).expect("sample settlement hashes");
        LaneFinalityStatement {
            version: 1,
            lane_id: settlement_commitment.lane_id,
            lane_incarnation: settlement_commitment.lane_incarnation,
            dataspace_id: settlement_commitment.dataspace_id,
            block_height: settlement_commitment.block_height,
            block_header_hash: sample_header(6, None).hash(),
            da_commitment_hash: None,
            lane_block_descriptor_hash: Hash::new(b"sample-lane-finality-descriptor"),
            manifest_root: [0x42; 32],
            settlement_commitment,
            settlement_hash,
            rbc_bytes_total: 128,
        }
    }
    fn build_envelope(height: u64) -> LaneRelayEnvelope {
        let settlement = sample_commitment(height, 3, 2);
        let header = sample_header(height, None);
        LaneRelayEnvelope::new(header, None, settlement, 0)
            .expect("envelope")
            .with_lane_block_descriptor_hash(Some(Hash::new(b"test-lane-block-descriptor")))
            .with_manifest_root(Some([0x42; 32]))
    }
    fn with_test_authority(
        envelope: LaneRelayEnvelope,
        artifact_tag: &'static [u8],
    ) -> LaneRelayEnvelope {
        let global_block_height = envelope.block_header.height().get();
        envelope.with_finality_authority(Some(LaneFinalityAuthorityV1 {
            version: 1,
            global_block_height,
            finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(artifact_tag)),
            statement_proof: MerkleProof::from_audit_path(0, Vec::new()),
        }))
    }
    fn checked_account_id() -> AccountId {
        AccountId::new(
            KeyPair::try_random()
                .expect("generate checked Nexus relay fixture keypair")
                .public_key()
                .clone(),
        )
    }
    #[test]
    fn fastpq_metadata_status_distinguishes_missing_and_present_metadata() {
        let pending = build_envelope(6);
        assert_eq!(
            pending.fastpq_metadata_status(),
            LaneRelayFastpqMaterialStatus::Missing
        );
        assert!(!pending.has_merge_admission_material());
        let verified = with_test_authority(pending, b"test-finality-artifact")
            .with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
                proof_digest: Hash::new(b"verified-relay-proof"),
                verified_at_height: 6,
            }));
        assert_eq!(
            verified.fastpq_metadata_status(),
            LaneRelayFastpqMaterialStatus::Present
        );
        assert!(verified.has_merge_admission_material());
    }
    #[test]
    fn relay_digest_domains_are_unique_and_bind_identical_payloads() {
        let domains = [
            LANE_RELAY_FASTPQ_CLAIM_DIGEST_DOMAIN_V1,
            LANE_FINALITY_STATEMENT_DOMAIN_V1,
            LANE_RELAY_MERGE_HINT_DOMAIN_V1,
            LANE_RELAY_SETTLEMENT_HASH_DOMAIN_V1,
            FEE_SPONSOR_VAULT_SOURCE_STATE_ROOT_DOMAIN_V1,
            FEE_SPONSOR_VAULT_ALLOCATION_CLAIM_DOMAIN_V1,
        ];
        assert_eq!(
            domains.into_iter().collect::<BTreeSet<_>>().len(),
            domains.len()
        );
        let payload = b"identical canonical payload";
        assert_ne!(
            domain_separated_hash(LANE_RELAY_FASTPQ_CLAIM_DIGEST_DOMAIN_V1, payload),
            domain_separated_hash(LANE_RELAY_MERGE_HINT_DOMAIN_V1, payload)
        );
        assert_ne!(
            domain_separated_hash(b"a", b"bc"),
            domain_separated_hash(b"ab", b"c"),
            "the framed domain cannot be shifted into the payload"
        );
    }
    #[test]
    fn lane_finality_statement_order_uses_the_complete_canonical_effect() {
        let original = sample_lane_finality_statement();
        let mut conflicting = original.clone();
        conflicting.settlement_commitment.receipts[0].timestamp_ms =
            conflicting.settlement_commitment.receipts[0]
                .timestamp_ms
                .saturating_add(1);
        conflicting.settlement_hash = compute_settlement_hash(&conflicting.settlement_commitment)
            .expect("conflicting settlement hashes");
        assert_eq!(
            (
                original.lane_id,
                original.dataspace_id,
                original.lane_incarnation,
                original.block_height,
            ),
            (
                conflicting.lane_id,
                conflicting.dataspace_id,
                conflicting.lane_incarnation,
                conflicting.block_height,
            ),
            "the adversarial pair must occupy one semantic finality coordinate"
        );
        assert_ne!(original, conflicting);
        assert_ne!(original.cmp(&conflicting), Ordering::Equal);
        let original_wire =
            norito::encode_canonical(&original).expect("canonical original statement");
        let conflicting_wire =
            norito::encode_canonical(&conflicting).expect("canonical conflicting statement");
        assert_eq!(
            original.cmp(&conflicting),
            original_wire.cmp(&conflicting_wire),
            "statement ordering must be exactly canonical-wire ordering"
        );
        assert_eq!(
            original.partial_cmp(&conflicting),
            Some(original.cmp(&conflicting))
        );
        assert_eq!(original.cmp(&original.clone()), Ordering::Equal);
    }
    #[test]
    fn merge_hint_root_binds_finality_artifact() {
        let envelope = with_test_authority(build_envelope(6), b"finality-artifact-a");
        let first = envelope.merge_hint_root().expect("merge hint root");
        let changed = with_test_authority(build_envelope(6), b"finality-artifact-b");
        let second = changed.merge_hint_root().expect("changed merge hint root");
        assert_ne!(first, second);
    }
    #[test]
    fn merge_hint_root_binds_lane_block_descriptor_hash() {
        let first = with_test_authority(build_envelope(6), b"finality-artifact")
            .with_lane_block_descriptor_hash(Some(Hash::new(b"descriptor-a")))
            .merge_hint_root()
            .expect("merge hint root");
        let second = with_test_authority(build_envelope(6), b"finality-artifact")
            .with_lane_block_descriptor_hash(Some(Hash::new(b"descriptor-b")))
            .merge_hint_root()
            .expect("changed merge hint root");
        assert_ne!(first, second);
    }
    #[test]
    fn relay_consensus_identities_ignore_ambient_layout_flags() {
        let envelope = with_test_authority(build_envelope(8), b"canonical-finality-artifact")
            .with_manifest_root(Some([0x42; 32]))
            .with_lane_block_descriptor_hash(Some(Hash::new(b"canonical-relay-descriptor")));
        let mut ordered_peer = envelope.clone();
        ordered_peer.rbc_bytes_total = ordered_peer.rbc_bytes_total.saturating_add(1);
        let baseline = (
            envelope.cmp(&ordered_peer),
            lane_relay_fastpq_claim_digest(&envelope).expect("canonical FASTPQ claim digest"),
            envelope
                .merge_hint_root()
                .expect("canonical merge hint root"),
            compute_settlement_hash(&envelope.settlement_commitment)
                .expect("canonical settlement hash"),
        );
        let canonical_wire =
            norito::encode_canonical(&envelope).expect("encode canonical relay envelope");
        let alternate_flags =
            norito::core::default_encode_flags() | norito::core::header_flags::PACKED_STRUCT;
        let (alternate_wire, alternate) = {
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            (
                norito::to_bytes(&envelope).expect("encode alternate-layout relay envelope"),
                (
                    envelope.cmp(&ordered_peer),
                    lane_relay_fastpq_claim_digest(&envelope)
                        .expect("ambient-independent FASTPQ claim digest"),
                    envelope
                        .merge_hint_root()
                        .expect("ambient-independent merge hint root"),
                    compute_settlement_hash(&envelope.settlement_commitment)
                        .expect("ambient-independent settlement hash"),
                ),
            )
        };
        assert_ne!(alternate_wire, canonical_wire);
        assert_eq!(
            alternate, baseline,
            "relay ordering and consensus identity hashes must use canonical framing"
        );
    }
    #[test]
    fn verify_rejects_settlement_total_mismatch_when_receipts_are_present() {
        let mut envelope = build_envelope(6);
        let one_micro = "0.000001"
            .parse::<Quantity>()
            .expect("valid one-micro settlement quantity");
        let mismatched_total = envelope
            .settlement_commitment
            .total_local_amount
            .checked_add(&one_micro)
            .expect("bounded settlement mismatch fixture");
        envelope.settlement_commitment.total_local_amount = mismatched_total;
        let err = envelope
            .verify()
            .expect_err("mismatched receipt totals must fail verification");
        assert_eq!(err, LaneRelayError::SettlementTotalsMismatch);
    }
    #[test]
    fn verify_requires_zero_totals_when_settlement_receipts_are_empty() {
        let mut envelope = build_envelope(6);
        envelope.settlement_commitment.receipts.clear();
        envelope.settlement_commitment.tx_count = 0;
        envelope.settlement_hash = compute_settlement_hash(&envelope.settlement_commitment)
            .expect("empty-receipt settlement hashes");
        assert_eq!(
            envelope
                .verify()
                .expect_err("empty receipts cannot authenticate nonzero aggregate totals"),
            LaneRelayError::SettlementTotalsMismatch
        );

        envelope.settlement_commitment.total_local_amount = Quantity::zero();
        envelope.settlement_commitment.total_xor_due = Quantity::zero();
        envelope.settlement_commitment.total_xor_after_haircut = Quantity::zero();
        envelope.settlement_commitment.total_xor_variance = Quantity::zero();
        envelope.settlement_hash = compute_settlement_hash(&envelope.settlement_commitment)
            .expect("zero-valued empty settlement hashes");
        envelope
            .verify()
            .expect("empty receipts with zero aggregate totals remain valid");
    }
    #[test]
    fn settlement_tx_count_covers_union_of_receipt_sources() {
        let mut envelope = build_envelope(6);
        envelope
            .settlement_commitment
            .nexus_fee_receipts
            .push(NexusFeeReceipt {
                version: NexusFeeReceipt::VERSION,
                source_id: [0xB6; 32],
                debit_source: FeeDebitSource::Account(checked_account_id()),
                fee_asset_id: "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
                    .parse()
                    .expect("canonical asset definition id"),
                program_revision: None,
                lease_id: None,
                fee_amount: Quantity::from(1_u32),
                schedule: NexusFeeScheduleInputs {
                    tx_bytes_len: 1,
                    instruction_count: 1,
                    gas_used: 0,
                    base_fee: Quantity::zero(),
                    per_byte_fee: Quantity::zero(),
                    per_instruction_fee: Quantity::from(1_u32),
                    per_gas_unit_fee: Quantity::zero(),
                },
                lane_id: envelope.lane_id,
                dataspace_id: envelope.dataspace_id,
                block_height: envelope.block_height,
            });
        envelope
            .settlement_commitment
            .native_amx_receipts
            .push(NativeAmxReceipt {
                version: NexusFeeReceipt::VERSION,
                source_id: [0xC7; 32],
                network_id: test_network_id(b"receipt-union-genesis"),
                plan_digest: Hash::new(b"receipt-union-plan"),
                lane_id: envelope.lane_id,
                dataspace_id: envelope.dataspace_id,
                lane_incarnation: envelope.lane_incarnation,
                authority_context_height: envelope.block_header.height().get(),
                lane_block_height: envelope.block_height,
                lane_block_view: 0,
                coordinator_proposal_hash: Hash::new(b"receipt-union-proposal"),
                legs: Vec::new(),
            });
        envelope.settlement_hash =
            compute_settlement_hash(&envelope.settlement_commitment).expect("settlement hash");
        assert_eq!(
            envelope.verify(),
            Err(LaneRelayError::SettlementTxCountMismatch)
        );
        envelope.settlement_commitment.nexus_fee_receipts[0].source_id = [0xA5; 32];
        envelope.settlement_commitment.native_amx_receipts[0].source_id = [0xA5; 32];
        envelope.settlement_hash =
            compute_settlement_hash(&envelope.settlement_commitment).expect("settlement hash");
        envelope
            .verify()
            .expect("one transaction may produce evidence in every receipt category");
    }
    #[test]
    fn verify_accepts_native_amx_receipt_with_lane_local_height() {
        let mut envelope = build_envelope(6);
        envelope
            .settlement_commitment
            .native_amx_receipts
            .push(NativeAmxReceipt {
                version: 1,
                source_id: [0x5A; 32],
                network_id: test_network_id(b"native-amx-relay-test-genesis"),
                plan_digest: Hash::new(b"native-amx-relay-test-plan"),
                lane_id: envelope.lane_id,
                dataspace_id: envelope.dataspace_id,
                lane_incarnation: envelope.lane_incarnation,
                authority_context_height: envelope.block_header.height().get(),
                lane_block_height: envelope.block_height,
                lane_block_view: 2,
                coordinator_proposal_hash: Hash::new(b"native-amx-relay-test-proposal"),
                legs: Vec::new(),
            });
        envelope.settlement_commitment.tx_count = 2;
        envelope.settlement_hash =
            compute_settlement_hash(&envelope.settlement_commitment).expect("settlement hash");
        envelope
            .verify()
            .expect("lane-local height must not be compared with global block height");
    }
    #[test]
    fn relay_envelope_ref_state_key_is_canonical_and_deterministic() {
        let relay_ref = build_envelope(7).relay_ref();
        let first = relay_ref.relay_state_key();
        let second = relay_ref.relay_state_key();
        assert_eq!(first, second);
        assert_eq!(
            first,
            format!(
                "pkdeploy_verified_lane_relay_2_3_{}_7",
                hex::encode(relay_ref.lane_incarnation.as_ref())
            )
        );
        assert!(!first.contains('/'));
        let incarnation = first.split('_').nth_back(1).expect("incarnation segment");
        assert_eq!(incarnation.len(), 64);
        assert!(incarnation.chars().all(|ch| ch.is_ascii_hexdigit()));
    }
    #[test]
    fn relay_rejects_zero_and_accepts_independent_lane_local_height() {
        let header = sample_header(8, None);
        assert_eq!(
            LaneRelayEnvelope::new(header, None, sample_commitment(0, 3, 2), 0,),
            Err(LaneRelayError::BlockHeightMismatch)
        );
        let envelope = LaneRelayEnvelope::new(header, None, sample_commitment(9, 3, 2), 0)
            .expect("lane-local height is independent of global proposal height");
        envelope
            .verify()
            .expect("independent lane-local and global height domains verify");
        let boundary = LaneRelayEnvelope::new(
            sample_header(1, None),
            None,
            sample_commitment(u64::MAX, 3, 2),
            0,
        )
        .expect("maximal lane-local height is valid at a non-zero global proposal height");
        boundary
            .verify()
            .expect("independent height boundary verifies");
    }
    #[test]
    fn fastpq_proof_material_accepts_external_digest() {
        let envelope =
            build_envelope(8).with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
                proof_digest: Hash::new(b"external-fastpq-proof-payload"),
                verified_at_height: 8,
            }));
        envelope
            .validate_fastpq_proof_metadata()
            .expect("external proof digest should be accepted");
    }
    #[test]
    fn fastpq_proof_material_rejects_stale_verified_height() {
        let envelope =
            build_envelope(8).with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
                proof_digest: Hash::new(b"external-fastpq-proof-payload"),
                verified_at_height: 7,
            }));
        let err = envelope
            .validate_fastpq_proof_metadata()
            .expect_err("stale verification height must be rejected");
        assert_eq!(err, LaneRelayError::InvalidFastpqProof);
    }
    #[test]
    fn lane_relay_fastpq_claim_digest_binds_fee_receipts() {
        let mut envelope = build_envelope(8).with_manifest_root(Some([0x42; 32]));
        let original = lane_relay_fastpq_claim_digest(&envelope).expect("lane relay claim digest");
        envelope
            .settlement_commitment
            .nexus_fee_receipts
            .push(NexusFeeReceipt {
                version: 1,
                source_id: [0xA5; 32],
                debit_source: FeeDebitSource::Account(checked_account_id()),
                fee_asset_id: "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
                    .parse()
                    .expect("canonical asset definition id"),
                program_revision: None,
                lease_id: None,
                fee_amount: Quantity::from(1_u32),
                schedule: NexusFeeScheduleInputs {
                    tx_bytes_len: 1,
                    instruction_count: 1,
                    gas_used: 0,
                    base_fee: Quantity::zero(),
                    per_byte_fee: Quantity::zero(),
                    per_instruction_fee: Quantity::from(1_u32),
                    per_gas_unit_fee: Quantity::zero(),
                },
                lane_id: envelope.lane_id,
                dataspace_id: envelope.dataspace_id,
                block_height: envelope.block_height,
            });
        envelope.settlement_hash =
            compute_settlement_hash(&envelope.settlement_commitment).expect("settlement hash");
        let changed = lane_relay_fastpq_claim_digest(&envelope).expect("changed claim digest");
        assert_ne!(original, changed);
    }
    #[test]
    fn native_amx_relay_coordinate_uses_global_authority_height() {
        let proposal_height = 8;
        let lane_block_height = 3;
        let mut settlement = sample_commitment(lane_block_height, 3, 2);
        let mut envelope = LaneRelayEnvelope::new(
            sample_header(proposal_height, None),
            None,
            settlement.clone(),
            0,
        )
        .expect("relay with independent height domains");
        let receipt = NativeAmxReceipt {
            version: 1,
            source_id: [0x31; 32],
            network_id: test_network_id(b"relay-native-amx-genesis"),
            plan_digest: Hash::new(b"relay-native-amx-plan"),
            lane_id: envelope.lane_id,
            dataspace_id: envelope.dataspace_id,
            lane_incarnation: envelope.lane_incarnation,
            authority_context_height: proposal_height,
            lane_block_height,
            lane_block_view: 1,
            coordinator_proposal_hash: Hash::new(b"relay-native-amx-proposal"),
            legs: Vec::new(),
        };
        assert_ne!(
            receipt.lane_block_height,
            envelope.block_header.height().get(),
            "fixture must keep the lane-local and global authority heights distinct"
        );
        settlement.native_amx_receipts.push(receipt);
        settlement.tx_count = 2;
        envelope.settlement_commitment = settlement;
        envelope.settlement_hash =
            compute_settlement_hash(&envelope.settlement_commitment).expect("settlement hash");
        envelope
            .verify()
            .expect("lane-local height need not equal the global relay height");
        envelope.settlement_commitment.native_amx_receipts[0].authority_context_height += 1;
        envelope.settlement_hash =
            compute_settlement_hash(&envelope.settlement_commitment).expect("settlement hash");
        assert_eq!(
            envelope.verify(),
            Err(LaneRelayError::SettlementReceiptCoordinateMismatch)
        );
    }
    #[test]
    fn lane_relay_fastpq_claim_digest_binds_lane_block_descriptor_hash() {
        let first = build_envelope(8)
            .with_manifest_root(Some([0x42; 32]))
            .with_lane_block_descriptor_hash(Some(Hash::new(b"descriptor-a")));
        let second = build_envelope(8)
            .with_manifest_root(Some([0x42; 32]))
            .with_lane_block_descriptor_hash(Some(Hash::new(b"descriptor-b")));
        assert_ne!(
            lane_relay_fastpq_claim_digest(&first).expect("first lane relay claim digest"),
            lane_relay_fastpq_claim_digest(&second).expect("second lane relay claim digest")
        );
    }
    #[test]
    fn fee_sponsor_vault_claim_digest_binds_program_asset_amount_and_lease() {
        let claim = FeeSponsorVaultAllocationClaim {
            program_id: FeeSponsorProgramId::new(
                checked_account_id(),
                "retail".parse().expect("program name"),
            ),
            program_revision: 3,
            asset_definition_id: "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
                .parse()
                .expect("canonical asset definition id"),
            verified_allocation: Quantity::from(10_u32),
            source_dataspace_id: DataSpaceId::new(2),
            source_height: 40,
            source_state_root: Hash::new(b"source-state"),
            expires_at_height: 100,
            lease_id: Hash::new(b"lease-a"),
        };
        let original = fee_sponsor_vault_allocation_claim_digest(&claim);
        let ambient = {
            let alternate_flags =
                norito::core::default_encode_flags() | norito::core::header_flags::PACKED_STRUCT;
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            fee_sponsor_vault_allocation_claim_digest(&claim)
        };
        assert_eq!(
            ambient, original,
            "the sponsor allocation proof preimage must ignore ambient Norito layout"
        );
        let mut changed = claim.clone();
        changed.verified_allocation = Quantity::from(11_u32);
        assert_ne!(
            original,
            fee_sponsor_vault_allocation_claim_digest(&changed)
        );
        changed = claim.clone();
        changed.lease_id = Hash::new(b"lease-b");
        assert_ne!(
            original,
            fee_sponsor_vault_allocation_claim_digest(&changed)
        );
        changed = claim;
        changed.program_revision = 4;
        assert_ne!(
            original,
            fee_sponsor_vault_allocation_claim_digest(&changed)
        );
    }
    #[test]
    fn fee_sponsor_vault_source_root_binds_authoritative_snapshot() {
        let program_id = FeeSponsorProgramId::new(
            checked_account_id(),
            "retail".parse().expect("program name"),
        );
        let asset_definition_id = "66owaQmAQMuHxPzxUN3bqZ6FJfDa"
            .parse()
            .expect("canonical asset definition id");
        let original = fee_sponsor_vault_source_state_root(
            &program_id,
            3,
            &asset_definition_id,
            &Quantity::from(10_u32),
            DataSpaceId::new(2),
            40,
        );
        let ambient = {
            let alternate_flags =
                norito::core::default_encode_flags() | norito::core::header_flags::PACKED_STRUCT;
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            fee_sponsor_vault_source_state_root(
                &program_id,
                3,
                &asset_definition_id,
                &Quantity::from(10_u32),
                DataSpaceId::new(2),
                40,
            )
        };
        assert_eq!(
            ambient, original,
            "the sponsor source-state commitment must ignore ambient Norito layout"
        );
        assert_ne!(
            original,
            fee_sponsor_vault_source_state_root(
                &program_id,
                3,
                &asset_definition_id,
                &Quantity::from(11_u32),
                DataSpaceId::new(2),
                40,
            )
        );
        assert_ne!(
            original,
            fee_sponsor_vault_source_state_root(
                &program_id,
                3,
                &asset_definition_id,
                &Quantity::from(10_u32),
                DataSpaceId::new(3),
                40,
            )
        );
    }
    #[test]
    fn fee_sponsor_vault_allocation_usage_keys_are_lease_bound_and_disjoint() {
        let first = Hash::new(b"fee-sponsor-lease-a");
        let second = Hash::new(b"fee-sponsor-lease-b");
        let executed = VerifiedFeeSponsorVaultAllocation::usage_state_key_for(&first);
        let settled = VerifiedFeeSponsorVaultAllocation::settled_usage_state_key_for(&first);
        assert!(executed.starts_with(VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_USAGE_STATE_KEY_PREFIX));
        assert!(
            settled
                .starts_with(VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_SETTLED_USAGE_STATE_KEY_PREFIX)
        );
        assert_ne!(executed, settled);
        assert_ne!(
            executed,
            VerifiedFeeSponsorVaultAllocation::usage_state_key_for(&second)
        );
        assert!(!executed.starts_with(VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_STATE_KEY_PREFIX));
        assert!(!settled.starts_with(VERIFIED_FEE_SPONSOR_VAULT_ALLOCATION_STATE_KEY_PREFIX));
    }
    #[test]
    fn quorum_context_rejects_invalid_roster() {
        let err = LaneRelayQuorumContext::new(0, 1).expect_err("invalid roster");
        assert_eq!(
            err,
            LaneRelayError::InvalidValidatorSet {
                validator_count: 0,
                min_quorum: 1
            }
        );
        let err = LaneRelayQuorumContext::new(2, 3).expect_err("quorum > roster");
        assert_eq!(
            err,
            LaneRelayError::InvalidValidatorSet {
                validator_count: 2,
                min_quorum: 3
            }
        );
    }
    #[test]
    fn evidence_bundle_roundtrip() {
        let envelope = build_envelope(1);
        let bundle = LaneRelayEvidenceBundle {
            envelope,
            error_label: "example_error".to_string(),
            error_message: "example".to_string(),
        };
        let encoded = Encode::encode(&bundle);
        let decoded = LaneRelayEvidenceBundle::decode(&mut &encoded[..])
            .expect("evidence bundle round-trips");
        assert_eq!(decoded, bundle);
    }
}
