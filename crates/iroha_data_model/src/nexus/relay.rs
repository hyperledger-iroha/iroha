//! Lane relay envelope for cross-lane commitments (NX-4).
//!
//! This carries the lane block header, optional QC and DA digest,
//! plus the settlement commitment and its hash so the merge ledger can verify
//! relay payloads deterministically.

use core::cmp::Ordering;

use iroha_crypto::{Hash, HashOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::{
    asset::AssetDefinitionId,
    block::{BlockHeader, consensus::LaneBlockCommitment},
    consensus::Qc,
    da::commitment::DaCommitmentBundle,
    nexus::{AxtFastpqBinding, DataSpaceId, FeeSponsorProgramId, LaneId},
    peer::PeerId,
    prelude::Metadata,
};
use iroha_primitives::numeric::Quantity;

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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    /// QC attesting to the block header (when available).
    #[norito(default)]
    pub qc: Option<Qc>,
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
    /// `FastPQ` proof material required before this relay can be admitted into the merge path.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub fastpq_proof: Option<LaneFastpqProofMaterial>,
}

/// `FastPQ` admission state for a relay envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "status", content = "state")]
pub enum LaneRelayProofStatus {
    /// The relay is structurally valid but has not yet been upgraded with verified `FastPQ` material.
    Pending,
    /// The relay carries structurally valid `FastPQ` material and can be considered for merge.
    Verified,
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LaneRelayEnvelopeRef {
    /// Numeric dataspace identifier.
    pub dataspace_id: DataSpaceId,
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Block height associated with the settlement commitment.
    pub block_height: u64,
    /// Norito hash of the settlement payload.
    pub settlement_hash: HashOf<LaneBlockCommitment>,
}

impl LaneRelayEnvelopeRef {
    /// Return the canonical contract-state key for this verified lane relay.
    #[must_use]
    pub fn relay_state_key(&self) -> String {
        let suffix = Hash::new(self.settlement_hash.as_ref());
        format!(
            "{VERIFIED_LANE_RELAY_STATE_KEY_PREFIX}_{}_{}_{}_{}",
            self.dataspace_id.as_u64(),
            self.lane_id.as_u32(),
            self.block_height,
            hex::encode(suffix.as_ref()),
        )
    }
}

/// Verified relay record persisted for restricted-source business effects.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LaneFastpqProofMaterial {
    /// Deterministic digest of the proof payload.
    pub proof_digest: Hash,
    /// Block height where the proof was verified.
    pub verified_at_height: u64,
}

#[derive(Clone, Debug, Encode)]
struct LaneRelayFastpqClaim {
    version: u8,
    lane_id: LaneId,
    lane_incarnation: Hash,
    dataspace_id: DataSpaceId,
    block_height: u64,
    block_header: BlockHeader,
    qc: Option<Qc>,
    da_commitment_hash: Option<HashOf<DaCommitmentBundle>>,
    lane_block_descriptor_hash: Option<Hash>,
    manifest_root: [u8; 32],
    settlement_commitment: LaneBlockCommitment,
    settlement_hash: HashOf<LaneBlockCommitment>,
}

#[derive(Clone, Debug, Encode)]
struct LaneRelayMergeHint {
    version: u8,
    lane_id: LaneId,
    lane_incarnation: Hash,
    dataspace_id: DataSpaceId,
    block_height: u64,
    tip_hash: HashOf<BlockHeader>,
    parent_state_root: Hash,
    post_state_root: Hash,
    da_commitment_hash: Option<HashOf<DaCommitmentBundle>>,
    lane_block_descriptor_hash: Option<Hash>,
    settlement_hash: HashOf<LaneBlockCommitment>,
    rbc_bytes_total: u64,
}

/// Compute the canonical claim digest that a FASTPQ lane-relay proof must bind.
///
/// The encoded claim includes the relayed block coordinates, header/QC, manifest
/// root, settlement commitment, and settlement hash. Because fee receipts are
/// part of [`LaneBlockCommitment`], any receipt mutation changes this digest.
///
/// # Errors
/// Returns [`LaneRelayError::InvalidFastpqProof`] when the relay lacks a
/// manifest root, or [`LaneRelayError::Encode`] if canonical encoding fails.
pub fn lane_relay_fastpq_claim_digest(
    envelope: &LaneRelayEnvelope,
) -> Result<Hash, LaneRelayError> {
    let manifest_root = envelope
        .manifest_root
        .ok_or(LaneRelayError::InvalidFastpqProof)?;
    let claim = LaneRelayFastpqClaim {
        version: 2,
        lane_id: envelope.lane_id,
        lane_incarnation: envelope.lane_incarnation,
        dataspace_id: envelope.dataspace_id,
        block_height: envelope.block_height,
        block_header: envelope.block_header,
        qc: envelope.qc.clone(),
        da_commitment_hash: envelope.da_commitment_hash,
        lane_block_descriptor_hash: envelope.lane_block_descriptor_hash,
        manifest_root,
        settlement_commitment: envelope.settlement_commitment.clone(),
        settlement_hash: envelope.settlement_hash,
    };
    let bytes = norito::encode_canonical(&claim)?;
    Ok(domain_separated_hash(
        LANE_RELAY_FASTPQ_CLAIM_DIGEST_DOMAIN_V1,
        &bytes,
    ))
}

/// Canonical source-ledger claim authorized by a sponsor-vault spend lease.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    /// Domain-separated mode tag used for lane relay QC signatures.
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

    /// Domain-separated mode tag expected for this relay's lane QC.
    #[must_use]
    pub fn lane_qc_mode_tag(&self, base_mode_tag: &str) -> String {
        Self::lane_qc_mode_tag_for(self.lane_id, self.dataspace_id, base_mode_tag)
    }

    /// Create an envelope and derive the settlement hash from the payload.
    ///
    /// # Errors
    ///
    /// Returns [`LaneRelayError::QcSubjectMismatch`] if the optional QC
    /// does not certify the provided block header, [`LaneRelayError::QcHeightMismatch`]
    /// when the QC height diverges from the global proposal header,
    /// [`LaneRelayError::DaCommitmentHashMismatch`] when the DA commitment hash differs from the
    /// header, or [`LaneRelayError::Encode`]
    /// if hashing the settlement commitment fails.
    pub fn new(
        block_header: BlockHeader,
        qc: Option<Qc>,
        da_commitment_hash: Option<HashOf<DaCommitmentBundle>>,
        settlement_commitment: LaneBlockCommitment,
        rbc_bytes_total: u64,
    ) -> Result<Self, LaneRelayError> {
        let settlement_hash = compute_settlement_hash(&settlement_commitment)?;
        let block_height = settlement_commitment.block_height;
        let proposal_height = block_header.height().get();
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

        if let Some(qc) = qc.as_ref()
            && qc.subject_block_hash != block_header.hash()
        {
            return Err(LaneRelayError::QcSubjectMismatch);
        }
        if let Some(qc) = qc.as_ref()
            && qc.height != proposal_height
        {
            return Err(LaneRelayError::QcHeightMismatch);
        }
        if let Some(qc) = qc.as_ref()
            && (qc.phase != crate::block::consensus::CertPhase::Commit
                || qc.view != block_header.view_change_index())
        {
            return Err(LaneRelayError::AggregateSignatureInvalid);
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
            qc,
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
            block_height: self.block_height,
            settlement_hash: self.settlement_hash,
        }
    }

    /// Validate QC subject, DA commitment hash, and settlement hash.
    ///
    /// # Errors
    ///
    /// Propagates [`LaneRelayError::QcSubjectMismatch`], [`LaneRelayError::QcHeightMismatch`],
    /// [`LaneRelayError::DaCommitmentHashMismatch`], [`LaneRelayError::SettlementBlockHeightMismatch`],
    /// [`LaneRelayError::SettlementLaneMismatch`],
    /// [`LaneRelayError::SettlementDataspaceMismatch`], or [`LaneRelayError::SettlementHashMismatch`]
    /// when validation fails, and may surface [`LaneRelayError::Encode`] if settlement hashing encounters an encoding error.
    pub fn verify(&self) -> Result<(), LaneRelayError> {
        let proposal_height = self.block_header.height().get();
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
        if let Some(qc) = self.qc.as_ref()
            && qc.subject_block_hash != self.block_header.hash()
        {
            return Err(LaneRelayError::QcSubjectMismatch);
        }
        if let Some(qc) = self.qc.as_ref()
            && qc.height != proposal_height
        {
            return Err(LaneRelayError::QcHeightMismatch);
        }
        if let Some(qc) = self.qc.as_ref()
            && (qc.phase != crate::block::consensus::CertPhase::Commit
                || qc.view != self.block_header.view_change_index())
        {
            return Err(LaneRelayError::AggregateSignatureInvalid);
        }
        if self.block_header.da_commitments_hash() != self.da_commitment_hash {
            return Err(LaneRelayError::DaCommitmentHashMismatch);
        }
        self.verify_settlement_integrity()?;
        self.verify_settlement_hash()
    }

    /// Return the current `FastPQ` admission state for this envelope.
    #[must_use]
    pub fn proof_status(&self) -> LaneRelayProofStatus {
        if self.has_fastpq_proof_material() {
            LaneRelayProofStatus::Verified
        } else {
            LaneRelayProofStatus::Pending
        }
    }

    /// Whether this relay satisfies merge-admission prerequisites.
    #[must_use]
    pub fn is_merge_admissible(&self) -> bool {
        self.block_height > 0
            && self.qc.as_ref().is_some_and(|qc| {
                qc.phase == crate::block::consensus::CertPhase::Commit
                    && qc.height == self.block_header.height().get()
                    && qc.view == self.block_header.view_change_index()
                    && qc.subject_block_hash == self.block_header.hash()
            })
            && self.has_fastpq_proof_material()
    }

    /// Compute the canonical lane merge-hint root.
    ///
    /// # Errors
    ///
    /// Returns [`LaneRelayError::MissingQc`] when the relay has not yet reached lane finality,
    /// and [`LaneRelayError::Encode`] if the canonical hint payload cannot be encoded.
    pub fn merge_hint_root(&self) -> Result<Hash, LaneRelayError> {
        let qc = self.qc.as_ref().ok_or(LaneRelayError::MissingQc)?;
        let hint = LaneRelayMergeHint {
            version: 2,
            lane_id: self.lane_id,
            lane_incarnation: self.lane_incarnation,
            dataspace_id: self.dataspace_id,
            block_height: self.block_height,
            tip_hash: self.block_header.hash(),
            parent_state_root: qc.parent_state_root,
            post_state_root: qc.post_state_root,
            da_commitment_hash: self.da_commitment_hash,
            lane_block_descriptor_hash: self.lane_block_descriptor_hash,
            settlement_hash: self.settlement_hash,
            rbc_bytes_total: self.rbc_bytes_total,
        };
        let bytes = norito::encode_canonical(&hint)?;
        Ok(domain_separated_hash(
            LANE_RELAY_MERGE_HINT_DOMAIN_V1,
            &bytes,
        ))
    }

    /// Validate the relay envelope against a validator roster and quorum expectation.
    ///
    /// # Errors
    ///
    /// In addition to the checks performed by [`Self::verify`], this surfaces
    /// [`LaneRelayError::MissingQc`] when the envelope lacks a QC,
    /// [`LaneRelayError::InvalidValidatorSet`] for malformed quorum parameters,
    /// [`LaneRelayError::SignerBitmapLengthMismatch`] when the signer bitmap length does not match the roster size,
    /// [`LaneRelayError::InvalidSignerIndex`] if the signer bitmap references out-of-range validators,
    /// [`LaneRelayError::InsufficientQuorum`] when the bitmap does not satisfy the quorum, and
    /// [`LaneRelayError::AggregateSignatureInvalid`] when the aggregate signature is empty or zeroed.
    pub fn verify_with_quorum(&self, quorum: LaneRelayQuorumContext) -> Result<(), LaneRelayError> {
        quorum.ensure_valid()?;
        self.verify()?;

        let qc = self.qc.as_ref().ok_or(LaneRelayError::MissingQc)?;
        let expected_len = usize::try_from(quorum.validator_count)
            .unwrap_or(usize::MAX)
            .div_ceil(8);
        if qc.aggregate.signers_bitmap.len() != expected_len {
            return Err(LaneRelayError::SignerBitmapLengthMismatch {
                expected: expected_len,
                actual: qc.aggregate.signers_bitmap.len(),
            });
        }
        let mut observed: u32 = 0;
        for (byte_index, byte) in qc.aggregate.signers_bitmap.iter().enumerate() {
            if *byte == 0 {
                continue;
            }
            let base = u32::try_from(byte_index).expect("signer bitmap length fits in u32") * 8;
            for bit in 0..8 {
                if byte & (1 << bit) == 0 {
                    continue;
                }
                let signer_index = base + bit;
                if signer_index >= quorum.validator_count {
                    return Err(LaneRelayError::InvalidSignerIndex {
                        signer: signer_index,
                        validator_count: quorum.validator_count,
                    });
                }
                observed = observed.saturating_add(1);
            }
        }

        if observed < quorum.min_quorum {
            return Err(LaneRelayError::InsufficientQuorum {
                observed,
                expected: quorum.min_quorum,
            });
        }

        if qc.aggregate.bls_aggregate_signature.is_empty()
            || qc
                .aggregate
                .bls_aggregate_signature
                .iter()
                .all(|byte| *byte == 0)
        {
            return Err(LaneRelayError::AggregateSignatureInvalid);
        }

        Ok(())
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
        self.verify_fastpq_proof_material().is_ok()
    }

    /// Validate `FastPQ` proof metadata.
    ///
    /// # Errors
    ///
    /// Returns [`LaneRelayError::MissingFastpqProof`] when proof metadata is absent and
    /// [`LaneRelayError::InvalidFastpqProof`] when the proof binding is malformed.
    pub fn verify_fastpq_proof_material(&self) -> Result<(), LaneRelayError> {
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
        if !settlement.receipts.is_empty()
            && (total_local_amount != settlement.total_local_amount
                || total_xor_due != settlement.total_xor_due
                || total_xor_after_haircut != settlement.total_xor_after_haircut
                || total_xor_variance != settlement.total_xor_variance)
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
    pub fn new(
        relay_envelope: LaneRelayEnvelope,
        proof_payload_hash: Hash,
        fastpq_statement_digest: [u8; 32],
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
            | (MissingQc, MissingQc)
            | (AggregateSignatureInvalid, AggregateSignatureInvalid)
            | (MissingFastpqProof, MissingFastpqProof)
            | (InvalidFastpqProof, InvalidFastpqProof)
            | (Encode(_), Encode(_)) => true,
            (UnknownLane(a_lane), UnknownLane(b_lane)) => a_lane == b_lane,
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
    use std::collections::BTreeSet;
    use std::num::NonZeroU64;

    use iroha_crypto::{Hash, HashOf, KeyPair};
    use iroha_primitives::numeric::Quantity;

    use super::*;
    use crate::{
        AccountId, PeerId,
        block::{
            BlockHeader,
            consensus::{
                LaneBlockCommitment, NativeAmxReceipt, NexusFeeReceipt, NexusFeeScheduleInputs,
            },
        },
        consensus::{CertPhase, QcAggregate},
        nexus::FeeDebitSource,
    };

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

    fn build_envelope(height: u64, qc: Option<Qc>) -> LaneRelayEnvelope {
        let settlement = sample_commitment(height, 3, 2);
        let header = sample_header(height, None);
        LaneRelayEnvelope::new(header, qc, None, settlement, 0).expect("envelope")
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
    fn proof_status_distinguishes_pending_and_verified_relays() {
        let pending = build_envelope(6, None);
        assert_eq!(pending.proof_status(), LaneRelayProofStatus::Pending);
        assert!(!pending.is_merge_admissible());

        let verified = pending.with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
            proof_digest: Hash::new(b"verified-relay-proof"),
            verified_at_height: 6,
        }));
        assert_eq!(verified.proof_status(), LaneRelayProofStatus::Verified);
        assert!(
            !verified.is_merge_admissible(),
            "QC is still required for merge"
        );
    }

    #[test]
    fn relay_digest_domains_are_unique_and_bind_identical_payloads() {
        let domains = [
            LANE_RELAY_FASTPQ_CLAIM_DIGEST_DOMAIN_V1,
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
    }

    #[test]
    fn merge_hint_root_binds_qc_state_roots() {
        let mut qc = qc_with_bitmap(vec![0b0000_0001], 6, vec![0xCC; 48]);
        qc.parent_state_root = Hash::new(b"parent-a");
        qc.post_state_root = Hash::new(b"post-a");
        let envelope = build_envelope(6, Some(qc.clone()));
        let first = envelope.merge_hint_root().expect("merge hint root");

        qc.post_state_root = Hash::new(b"post-b");
        let changed = build_envelope(6, Some(qc));
        let second = changed.merge_hint_root().expect("changed merge hint root");

        assert_ne!(first, second);
    }

    #[test]
    fn merge_hint_root_binds_lane_block_descriptor_hash() {
        let qc = qc_with_bitmap(vec![0b0000_0001], 6, vec![0xCC; 48]);
        let first = build_envelope(6, Some(qc.clone()))
            .with_lane_block_descriptor_hash(Some(Hash::new(b"descriptor-a")))
            .merge_hint_root()
            .expect("merge hint root");
        let second = build_envelope(6, Some(qc))
            .with_lane_block_descriptor_hash(Some(Hash::new(b"descriptor-b")))
            .merge_hint_root()
            .expect("changed merge hint root");

        assert_ne!(first, second);
    }

    #[test]
    fn relay_consensus_identities_ignore_ambient_layout_flags() {
        let qc = qc_with_bitmap(vec![0b0000_0001], 8, vec![0xCC; 48]);
        let envelope = build_envelope(8, Some(qc))
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
        let mut envelope = build_envelope(6, None);
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
    fn settlement_tx_count_covers_union_of_receipt_sources() {
        let mut envelope = build_envelope(6, None);
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
                chain_id_hash: Hash::new(b"receipt-union-chain"),
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
        let mut envelope = build_envelope(6, None);
        envelope
            .settlement_commitment
            .native_amx_receipts
            .push(NativeAmxReceipt {
                version: 1,
                source_id: [0x5A; 32],
                chain_id_hash: Hash::new(b"native-amx-relay-test-chain"),
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
        let relay_ref = build_envelope(7, None).relay_ref();
        let first = relay_ref.relay_state_key();
        let second = relay_ref.relay_state_key();

        assert_eq!(first, second);
        assert!(first.starts_with("pkdeploy_verified_lane_relay_2_3_7_"));
        assert!(!first.contains('/'));
        let suffix = first.rsplit('_').next().expect("hash suffix");
        assert_eq!(suffix.len(), 64);
        assert!(suffix.chars().all(|ch| ch.is_ascii_hexdigit()));
    }

    fn qc_with_bitmap(bitmap: Vec<u8>, height: u64, signature: Vec<u8>) -> Qc {
        let validator_set: Vec<PeerId> = Vec::new();
        Qc {
            phase: CertPhase::Commit,
            subject_block_hash: sample_header(height, None).hash(),
            parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            post_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            height,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: crate::block::consensus::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: 1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: bitmap,
                bls_aggregate_signature: signature,
            },
        }
    }

    #[test]
    fn quorum_validation_accepts_sufficient_signers() {
        let qc = qc_with_bitmap(vec![0b0001_0111], 5, vec![0xCC; 48]);
        let envelope = build_envelope(5, Some(qc));
        let quorum = LaneRelayQuorumContext::new(6, 3).expect("quorum");

        envelope
            .verify_with_quorum(quorum)
            .expect("quorum validation should pass");
    }

    #[test]
    fn relay_rejects_zero_and_accepts_independent_lane_local_height() {
        let header = sample_header(8, None);
        assert_eq!(
            LaneRelayEnvelope::new(header, None, None, sample_commitment(0, 3, 2), 0,),
            Err(LaneRelayError::BlockHeightMismatch)
        );
        let envelope = LaneRelayEnvelope::new(header, None, None, sample_commitment(9, 3, 2), 0)
            .expect("lane-local height is independent of global proposal height");
        envelope
            .verify()
            .expect("independent lane-local and global height domains verify");

        let boundary = LaneRelayEnvelope::new(
            sample_header(1, None),
            None,
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
    fn relay_rejects_non_commit_or_wrong_view_qc() {
        let mut prepare = qc_with_bitmap(vec![0b0000_0001], 8, vec![0xCC; 48]);
        prepare.phase = CertPhase::Prepare;
        assert_eq!(
            LaneRelayEnvelope::new(
                sample_header(8, None),
                Some(prepare),
                None,
                sample_commitment(8, 3, 2),
                0,
            ),
            Err(LaneRelayError::AggregateSignatureInvalid)
        );

        let mut wrong_view = qc_with_bitmap(vec![0b0000_0001], 8, vec![0xCC; 48]);
        wrong_view.view = 1;
        assert_eq!(
            LaneRelayEnvelope::new(
                sample_header(8, None),
                Some(wrong_view),
                None,
                sample_commitment(8, 3, 2),
                0,
            ),
            Err(LaneRelayError::AggregateSignatureInvalid)
        );
    }

    #[test]
    fn quorum_validation_rejects_missing_qc() {
        let envelope = build_envelope(3, None);
        let quorum = LaneRelayQuorumContext::new(4, 2).expect("quorum");

        let err = envelope.verify_with_quorum(quorum).expect_err("qc missing");
        assert_eq!(err, LaneRelayError::MissingQc);
    }

    #[test]
    fn quorum_validation_rejects_invalid_signer_index() {
        let qc = qc_with_bitmap(vec![0b0001_0000], 4, vec![0xAA; 48]); // bit 4 set, count 4 -> out of range
        let envelope = build_envelope(4, Some(qc));
        let quorum = LaneRelayQuorumContext::new(4, 2).expect("quorum");

        let err = envelope
            .verify_with_quorum(quorum)
            .expect_err("invalid signer");
        assert_eq!(
            err,
            LaneRelayError::InvalidSignerIndex {
                signer: 4,
                validator_count: 4
            }
        );
    }

    #[test]
    fn quorum_validation_rejects_bitmap_length_mismatch() {
        let qc = qc_with_bitmap(vec![0b0000_0011], 4, vec![0xAA; 48]);
        let envelope = build_envelope(4, Some(qc));
        let quorum = LaneRelayQuorumContext::new(9, 3).expect("quorum");

        let err = envelope
            .verify_with_quorum(quorum)
            .expect_err("bitmap length mismatch");
        assert_eq!(
            err,
            LaneRelayError::SignerBitmapLengthMismatch {
                expected: 2,
                actual: 1
            }
        );
    }

    #[test]
    fn quorum_validation_rejects_insufficient_quorum() {
        let qc = qc_with_bitmap(vec![0b0000_0011], 6, vec![0xAA; 48]); // two signers
        let envelope = build_envelope(6, Some(qc));
        let quorum = LaneRelayQuorumContext::new(5, 3).expect("quorum");

        let err = envelope
            .verify_with_quorum(quorum)
            .expect_err("quorum should fail");
        assert_eq!(
            err,
            LaneRelayError::InsufficientQuorum {
                observed: 2,
                expected: 3
            }
        );
    }

    #[test]
    fn quorum_validation_rejects_zero_signature() {
        let qc = qc_with_bitmap(vec![0b0000_0111], 7, vec![0; 48]);
        let envelope = build_envelope(7, Some(qc));
        let quorum = LaneRelayQuorumContext::new(8, 2).expect("quorum");

        let err = envelope
            .verify_with_quorum(quorum)
            .expect_err("zero signature");
        assert_eq!(err, LaneRelayError::AggregateSignatureInvalid);
    }

    #[test]
    fn fastpq_proof_material_accepts_external_digest() {
        let envelope =
            build_envelope(8, None).with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
                proof_digest: Hash::new(b"external-fastpq-proof-payload"),
                verified_at_height: 8,
            }));

        envelope
            .verify_fastpq_proof_material()
            .expect("external proof digest should be accepted");
    }

    #[test]
    fn fastpq_proof_material_rejects_stale_verified_height() {
        let envelope =
            build_envelope(8, None).with_fastpq_proof_material(Some(LaneFastpqProofMaterial {
                proof_digest: Hash::new(b"external-fastpq-proof-payload"),
                verified_at_height: 7,
            }));

        let err = envelope
            .verify_fastpq_proof_material()
            .expect_err("stale verification height must be rejected");
        assert_eq!(err, LaneRelayError::InvalidFastpqProof);
    }

    #[test]
    fn lane_relay_fastpq_claim_digest_binds_fee_receipts() {
        let mut envelope = build_envelope(8, None).with_manifest_root(Some([0x42; 32]));
        let original = lane_relay_fastpq_claim_digest(&envelope).expect("lane relay claim digest");

        envelope
            .settlement_commitment
            .nexus_fee_receipts
            .push(NexusFeeReceipt {
                version: 1,
                source_id: [0x11; 32],
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
            None,
            settlement.clone(),
            0,
        )
        .expect("relay with independent height domains");
        let receipt = NativeAmxReceipt {
            version: 1,
            source_id: [0x31; 32],
            chain_id_hash: Hash::new(b"relay-native-amx-chain"),
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
        let first = build_envelope(8, None)
            .with_manifest_root(Some([0x42; 32]))
            .with_lane_block_descriptor_hash(Some(Hash::new(b"descriptor-a")));
        let second = build_envelope(8, None)
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
        let envelope = build_envelope(1, None);
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
