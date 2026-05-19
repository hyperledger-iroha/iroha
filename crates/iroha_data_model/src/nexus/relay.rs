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
    account::AccountId,
    block::{BlockHeader, consensus::LaneBlockCommitment},
    consensus::Qc,
    da::commitment::DaCommitmentBundle,
    nexus::{AxtFastpqBinding, DataSpaceId, LaneId},
    peer::PeerId,
    prelude::Metadata,
};
use iroha_primitives::numeric::Numeric;

/// Prefix for contract-visible verified relay state keys.
pub const VERIFIED_LANE_RELAY_STATE_KEY_PREFIX: &str = "pkdeploy_verified_lane_relay";
/// Prefix for contract-visible verified Nexus fee-budget cache keys.
pub const VERIFIED_NEXUS_FEE_BUDGET_STATE_KEY_PREFIX: &str = "pkdeploy_verified_nexus_fee_budget";
/// FASTPQ business effect expected for verified lane-relay block commitments.
pub const LANE_RELAY_FASTPQ_EFFECT_TYPE: &str = "lane_relay_block";

/// Relay envelope broadcast by Nexus lanes for merge validation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LaneRelayEnvelope {
    /// Numeric lane identifier.
    pub lane_id: LaneId,
    /// Numeric dataspace identifier.
    pub dataspace_id: DataSpaceId,
    /// Block height associated with the settlement commitment.
    pub block_height: u64,
    /// Full lane block header being relayed.
    pub block_header: BlockHeader,
    /// QC attesting to the block header (when available).
    #[norito(default)]
    pub qc: Option<Qc>,
    /// Optional hash of the DA commitment bundle for the block payload.
    #[norito(default)]
    pub da_commitment_hash: Option<HashOf<DaCommitmentBundle>>,
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
        let lhs = norito::to_bytes(self).expect("lane relay envelope should encode");
        let rhs = norito::to_bytes(other).expect("lane relay envelope should encode");
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

/// Verified Nexus XOR fee-budget cache record persisted by the relay protocol.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct VerifiedNexusFeeBudgetRecord {
    /// Sponsor or payer account whose public Nexus XOR balance was verified.
    pub sponsor_account_id: AccountId,
    /// Fee asset selector used for the verified balance, fixed operationally to public XOR.
    pub fee_asset_id: String,
    /// Latest verified public Nexus balance for `fee_asset_id`.
    pub verified_balance: Numeric,
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
    dataspace_id: DataSpaceId,
    block_height: u64,
    block_header: BlockHeader,
    qc: Option<Qc>,
    da_commitment_hash: Option<HashOf<DaCommitmentBundle>>,
    manifest_root: [u8; 32],
    settlement_commitment: LaneBlockCommitment,
    settlement_hash: HashOf<LaneBlockCommitment>,
}

#[derive(Clone, Debug, Encode)]
struct LaneRelayMergeHint {
    version: u8,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    block_height: u64,
    tip_hash: HashOf<BlockHeader>,
    parent_state_root: Hash,
    post_state_root: Hash,
    da_commitment_hash: Option<HashOf<DaCommitmentBundle>>,
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
        version: 1,
        lane_id: envelope.lane_id,
        dataspace_id: envelope.dataspace_id,
        block_height: envelope.block_height,
        block_header: envelope.block_header,
        qc: envelope.qc.clone(),
        da_commitment_hash: envelope.da_commitment_hash,
        manifest_root,
        settlement_commitment: envelope.settlement_commitment.clone(),
        settlement_hash: envelope.settlement_hash,
    };
    let bytes = norito::to_bytes(&claim)?;
    Ok(Hash::new(bytes))
}

/// Compute the canonical claim digest for a verified Nexus sponsor fee-budget proof.
#[must_use]
pub fn nexus_fee_budget_claim_digest(
    sponsor: &AccountId,
    fee_asset_id: &str,
    verified_balance: &Numeric,
) -> Hash {
    Hash::new(
        format!(
            "nexus_fee_budget:v1:{sponsor}:{}:{verified_balance}",
            fee_asset_id.trim()
        )
        .as_bytes(),
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
    /// when the QC height diverges from the block, [`LaneRelayError::DaCommitmentHashMismatch`]
    /// when the DA commitment hash differs from the header, [`LaneRelayError::SettlementBlockHeightMismatch`]
    /// when the settlement commitment height does not match the header, or [`LaneRelayError::Encode`]
    /// if hashing the settlement commitment fails.
    pub fn new(
        block_header: BlockHeader,
        qc: Option<Qc>,
        da_commitment_hash: Option<HashOf<DaCommitmentBundle>>,
        settlement_commitment: LaneBlockCommitment,
        rbc_bytes_total: u64,
    ) -> Result<Self, LaneRelayError> {
        let settlement_hash = compute_settlement_hash(&settlement_commitment)?;
        let block_height = block_header.height().get();

        if settlement_commitment.block_height != block_height {
            return Err(LaneRelayError::SettlementBlockHeightMismatch);
        }

        if let Some(qc) = qc.as_ref()
            && qc.subject_block_hash != block_header.hash()
        {
            return Err(LaneRelayError::QcSubjectMismatch);
        }
        if let Some(qc) = qc.as_ref()
            && qc.height != block_height
        {
            return Err(LaneRelayError::QcHeightMismatch);
        }

        if block_header.da_commitments_hash() != da_commitment_hash {
            return Err(LaneRelayError::DaCommitmentHashMismatch);
        }

        Ok(Self {
            lane_id: settlement_commitment.lane_id,
            dataspace_id: settlement_commitment.dataspace_id,
            block_height,
            block_header,
            qc,
            da_commitment_hash,
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
    /// [`LaneRelayError::BlockHeightMismatch`], [`LaneRelayError::SettlementLaneMismatch`],
    /// [`LaneRelayError::SettlementDataspaceMismatch`], or [`LaneRelayError::SettlementHashMismatch`]
    /// when validation fails, and may surface [`LaneRelayError::Encode`] if settlement hashing encounters an encoding error.
    pub fn verify(&self) -> Result<(), LaneRelayError> {
        if self.block_height != self.block_header.height().get() {
            return Err(LaneRelayError::BlockHeightMismatch);
        }
        if self.settlement_commitment.block_height != self.block_height {
            return Err(LaneRelayError::SettlementBlockHeightMismatch);
        }
        if self.settlement_commitment.lane_id != self.lane_id {
            return Err(LaneRelayError::SettlementLaneMismatch);
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
            && qc.height != self.block_height
        {
            return Err(LaneRelayError::QcHeightMismatch);
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
        self.qc.is_some() && self.has_fastpq_proof_material()
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
            version: 1,
            lane_id: self.lane_id,
            dataspace_id: self.dataspace_id,
            block_height: self.block_height,
            tip_hash: self.block_header.hash(),
            parent_state_root: qc.parent_state_root,
            post_state_root: qc.post_state_root,
            da_commitment_hash: self.da_commitment_hash,
            settlement_hash: self.settlement_hash,
            rbc_bytes_total: self.rbc_bytes_total,
        };
        let bytes = norito::to_bytes(&hint)?;
        Ok(Hash::new(bytes))
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
        if material.verified_at_height < self.block_height {
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
        let mut total_local_micro = 0u128;
        let mut total_xor_due_micro = 0u128;
        let mut total_xor_after_haircut_micro = 0u128;
        let mut total_xor_variance_micro = 0u128;
        let mut settlement_sources = std::collections::BTreeSet::new();
        for receipt in &settlement.receipts {
            if !settlement_sources.insert(receipt.source_id) {
                return Err(LaneRelayError::DuplicateSettlementSource);
            }
            total_local_micro = total_local_micro
                .checked_add(receipt.local_amount_micro)
                .ok_or(LaneRelayError::SettlementTotalsMismatch)?;
            total_xor_due_micro = total_xor_due_micro
                .checked_add(receipt.xor_due_micro)
                .ok_or(LaneRelayError::SettlementTotalsMismatch)?;
            total_xor_after_haircut_micro = total_xor_after_haircut_micro
                .checked_add(receipt.xor_after_haircut_micro)
                .ok_or(LaneRelayError::SettlementTotalsMismatch)?;
            total_xor_variance_micro = total_xor_variance_micro
                .checked_add(receipt.xor_variance_micro)
                .ok_or(LaneRelayError::SettlementTotalsMismatch)?;
        }
        if !settlement.receipts.is_empty()
            && (total_local_micro != settlement.total_local_micro
                || total_xor_due_micro != settlement.total_xor_due_micro
                || total_xor_after_haircut_micro != settlement.total_xor_after_haircut_micro
                || total_xor_variance_micro != settlement.total_xor_variance_micro)
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
        }

        let mut native_amx_sources = std::collections::BTreeSet::new();
        for receipt in &settlement.native_amx_receipts {
            if receipt.lane_id != settlement.lane_id
                || receipt.dataspace_id != settlement.dataspace_id
                || receipt.block_height != settlement.block_height
            {
                return Err(LaneRelayError::SettlementReceiptCoordinateMismatch);
            }
            if !native_amx_sources.insert(receipt.source_id) {
                return Err(LaneRelayError::DuplicateSettlementSource);
            }
        }

        let receipt_count = settlement
            .receipts
            .len()
            .max(settlement.nexus_fee_receipts.len())
            .max(settlement.native_amx_receipts.len());
        if settlement.tx_count < u64::try_from(receipt_count).unwrap_or(u64::MAX) {
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

impl VerifiedNexusFeeBudgetRecord {
    /// Construct a verified fee-budget cache record from canonical verified inputs.
    #[must_use]
    #[expect(
        clippy::too_many_arguments,
        reason = "the constructor mirrors the canonical verified record fields"
    )]
    pub fn new(
        sponsor_account_id: AccountId,
        fee_asset_id: String,
        verified_balance: Numeric,
        proof_payload_hash: Hash,
        fastpq_statement_digest: [u8; 32],
        fastpq_proof_digest: Hash,
        verified_at_height: u64,
        manifest_root: [u8; 32],
        fastpq_binding: AxtFastpqBinding,
    ) -> Self {
        Self {
            sponsor_account_id,
            fee_asset_id,
            verified_balance,
            proof_payload_hash,
            fastpq_statement_digest,
            fastpq_proof_digest,
            verified_at_height,
            manifest_root,
            fastpq_binding,
        }
    }

    /// Return the canonical contract-state key for this sponsor/asset budget cache.
    #[must_use]
    pub fn state_key_for(sponsor_account_id: &AccountId, fee_asset_id: &str) -> String {
        let material = format!("{sponsor_account_id}|{}", fee_asset_id.trim());
        let suffix = Hash::new(material.as_bytes());
        format!(
            "{VERIFIED_NEXUS_FEE_BUDGET_STATE_KEY_PREFIX}_{}",
            hex::encode(suffix.as_ref())
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
    let bytes = norito::to_bytes(settlement)?;
    Ok(HashOf::from_untyped_unchecked(Hash::new(bytes)))
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
    /// Settlement commitment height does not match the block header height.
    #[error("settlement commitment block height does not match block header")]
    SettlementBlockHeightMismatch,
    /// Envelope block height does not match the embedded block header.
    #[error("block height in envelope does not match block header")]
    BlockHeightMismatch,
    /// Settlement commitment lane identifier differs from the envelope lane id.
    #[error("settlement commitment lane id does not match envelope lane id")]
    SettlementLaneMismatch,
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
            LaneRelayError::DataspaceMismatch { .. } => "dataspace_mismatch",
            LaneRelayError::StaleRelay { .. } => "stale_height",
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
    use std::num::NonZeroU64;

    use iroha_crypto::{Hash, HashOf, KeyPair};

    use super::*;
    use crate::{
        AccountId, PeerId,
        block::{
            BlockHeader,
            consensus::{LaneBlockCommitment, NexusFeeReceipt, NexusFeeScheduleInputs},
        },
        consensus::{CertPhase, QcAggregate},
    };

    fn sample_commitment(height: u64, lane_id: u32, dataspace_id: u64) -> LaneBlockCommitment {
        LaneBlockCommitment {
            block_height: height,
            lane_id: LaneId::new(lane_id),
            dataspace_id: DataSpaceId::new(dataspace_id),
            tx_count: 1,
            total_local_micro: 10,
            total_xor_due_micro: 5,
            total_xor_after_haircut_micro: 4,
            total_xor_variance_micro: 1,
            swap_metadata: None,
            receipts: vec![crate::block::consensus::LaneSettlementReceipt {
                source_id: [0xA5; 32],
                local_amount_micro: 10,
                xor_due_micro: 5,
                xor_after_haircut_micro: 4,
                xor_variance_micro: 1,
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
    fn verify_rejects_settlement_total_mismatch_when_receipts_are_present() {
        let mut envelope = build_envelope(6, None);
        envelope.settlement_commitment.total_local_micro = envelope
            .settlement_commitment
            .total_local_micro
            .saturating_add(1);

        let err = envelope
            .verify()
            .expect_err("mismatched receipt totals must fail verification");
        assert_eq!(err, LaneRelayError::SettlementTotalsMismatch);
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
            view: 1,
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
                payer_account_id: AccountId::new(KeyPair::random().public_key().clone()),
                fee_asset_id: "xor#universal".to_owned(),
                fee_amount: Numeric::from(1_u32),
                schedule: NexusFeeScheduleInputs {
                    tx_bytes_len: 1,
                    instruction_count: 1,
                    gas_used: 0,
                    base_fee: Numeric::zero(),
                    per_byte_fee: Numeric::zero(),
                    per_instruction_fee: Numeric::from(1_u32),
                    per_gas_unit_fee: Numeric::zero(),
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
    fn nexus_fee_budget_claim_digest_binds_sponsor_asset_and_balance() {
        let sponsor = AccountId::new(KeyPair::random().public_key().clone());
        let original =
            nexus_fee_budget_claim_digest(&sponsor, "xor#universal", &Numeric::from(10_u32));

        assert_ne!(
            original,
            nexus_fee_budget_claim_digest(&sponsor, "xor#universal", &Numeric::from(11_u32))
        );
        assert_ne!(
            original,
            nexus_fee_budget_claim_digest(
                &sponsor,
                "xor#universal.universal",
                &Numeric::from(10_u32)
            )
        );
        assert_ne!(
            original,
            nexus_fee_budget_claim_digest(
                &AccountId::new(KeyPair::random().public_key().clone()),
                "xor#universal",
                &Numeric::from(10_u32),
            )
        );
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
