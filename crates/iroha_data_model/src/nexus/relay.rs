//! Lane relay envelope for cross-lane commitments (NX-4).
//!
//! This carries the lane block header, optional execution QC and DA digest,
//! plus the settlement commitment and its hash so the merge ledger can verify
//! relay payloads deterministically.

use iroha_crypto::{Hash, HashOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::{
    AccountId,
    block::{BlockHeader, consensus::LaneBlockCommitment},
    consensus::Qc,
    da::commitment::DaCommitmentBundle,
    nexus::{DataSpaceId, LaneId},
    prelude::Metadata,
};

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

/// Emergency validator override for a dataspace when lane relay quorum is at risk.
///
/// Application of this override is gated by `nexus.lane_relay_emergency.enabled`.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LaneRelayEmergencyValidatorSet {
    /// Validators temporarily allowed to satisfy lane relay quorum.
    pub validators: Vec<AccountId>,
    /// Optional block height (inclusive) after which the override expires.
    #[norito(default)]
    pub expires_at_height: Option<u64>,
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
        })
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
        self.verify_settlement_hash()
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
}

impl PartialEq for LaneRelayError {
    fn eq(&self, other: &Self) -> bool {
        use LaneRelayError::*;
        match (self, other) {
            (NexusDisabled, NexusDisabled)
            | (SettlementHashMismatch, SettlementHashMismatch)
            | (SettlementBlockHeightMismatch, SettlementBlockHeightMismatch)
            | (BlockHeightMismatch, BlockHeightMismatch)
            | (SettlementLaneMismatch, SettlementLaneMismatch)
            | (SettlementDataspaceMismatch, SettlementDataspaceMismatch)
            | (QcSubjectMismatch, QcSubjectMismatch)
            | (QcHeightMismatch, QcHeightMismatch)
            | (DaCommitmentHashMismatch, DaCommitmentHashMismatch)
            | (MissingQc, MissingQc)
            | (AggregateSignatureInvalid, AggregateSignatureInvalid)
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
            LaneRelayError::Encode(_) => "encode",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_crypto::{Hash, HashOf};

    use super::*;
    use crate::{
        PeerId,
        block::{BlockHeader, consensus::LaneBlockCommitment},
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
            receipts: Vec::new(),
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
