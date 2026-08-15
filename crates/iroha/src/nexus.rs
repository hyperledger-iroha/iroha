//! Cross-lane helpers for Nexus (NX-11).
//!
//! The helpers here let SDK callers build and validate the `LaneRelayEnvelope`
//! payloads surfaced by `/v1/sumeragi/status`, ensuring settlement proofs stay
//! self-consistent before they are forwarded to other lanes.
use iroha_crypto::HashOf;
use iroha_data_model::{
    block::{BlockHeader, consensus::LaneBlockCommitment},
    da::commitment::DaCommitmentBundle,
    nexus::{DataSpaceId, LaneFinalityAuthorityV1, LaneId, LaneRelayEnvelope, LaneRelayError},
};
use iroha_logger::prelude::*;
use std::collections::HashSet;
use thiserror::Error;
/// Error surfaced when validating or building cross-lane relay proofs.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CrossLaneProofError {
    /// The underlying relay envelope failed verification.
    #[error(transparent)]
    Relay(#[from] LaneRelayError),
    /// Duplicate relay envelope for the same `(lane_id, dataspace_id, block_height)` tuple.
    #[error(
        "duplicate relay envelope for lane {lane_id}, dataspace {dataspace_id}, height {block_height}"
    )]
    DuplicateProof {
        /// Lane identifier for the duplicate envelope.
        lane_id: LaneId,
        /// Dataspace identifier for the duplicate envelope.
        dataspace_id: DataSpaceId,
        /// Block height for the duplicate envelope.
        block_height: u64,
    },
}
/// Strongly typed wrapper around a [`LaneRelayEnvelope`].
#[derive(Debug, Clone)]
pub struct CrossLaneTransferProof {
    envelope: LaneRelayEnvelope,
}
impl CrossLaneTransferProof {
    /// Construct a proof wrapper from a relay envelope.
    #[must_use]
    pub const fn new(envelope: LaneRelayEnvelope) -> Self {
        Self { envelope }
    }
    /// Validate the structural DA and settlement bindings.
    ///
    /// # Errors
    /// Returns an error if the inner relay envelope fails validation.
    pub fn verify(&self) -> Result<(), CrossLaneProofError> {
        self.envelope.verify()?;
        Ok(())
    }
    /// Access the inner relay envelope.
    #[must_use]
    pub const fn envelope(&self) -> &LaneRelayEnvelope {
        &self.envelope
    }
}
/// Helper to build relay envelopes from lane settlement data.
#[derive(Debug)]
pub struct CrossLaneTransferBuilder {
    block_header: BlockHeader,
    da_commitment_hash: Option<HashOf<DaCommitmentBundle>>,
    settlement_commitment: LaneBlockCommitment,
    rbc_bytes_total: u64,
    finality_authority: Option<LaneFinalityAuthorityV1>,
}
impl CrossLaneTransferBuilder {
    /// Create a builder for a cross-lane relay envelope.
    #[must_use]
    #[allow(clippy::large_types_passed_by_value)]
    pub fn new(
        block_header: BlockHeader,
        da_commitment_hash: Option<HashOf<DaCommitmentBundle>>,
        settlement_commitment: LaneBlockCommitment,
    ) -> Self {
        Self {
            block_header,
            da_commitment_hash,
            settlement_commitment,
            rbc_bytes_total: 0,
            finality_authority: None,
        }
    }
    /// Override the RBC byte count attached to the relay envelope.
    #[must_use]
    pub fn with_rbc_bytes_total(mut self, rbc_bytes_total: u64) -> Self {
        self.rbc_bytes_total = rbc_bytes_total;
        self
    }
    /// Attach a compact authority reference obtained from finalized node output.
    #[must_use]
    pub fn with_finality_authority(mut self, authority: LaneFinalityAuthorityV1) -> Self {
        self.finality_authority = Some(authority);
        self
    }
    /// Build the relay envelope and wrap it in a proof helper.
    ///
    /// # Errors
    /// Returns an error if relay envelope construction or validation fails.
    pub fn build(self) -> Result<CrossLaneTransferProof, CrossLaneProofError> {
        let envelope = LaneRelayEnvelope::new(
            self.block_header,
            self.da_commitment_hash,
            self.settlement_commitment,
            self.rbc_bytes_total,
        )?
        .with_finality_authority(self.finality_authority);
        Ok(CrossLaneTransferProof::new(envelope))
    }
}
/// Verify a batch of relay envelopes and reject duplicates.
///
/// # Errors
/// Returns an error if any envelope fails validation or if a duplicate
/// `(lane_id, dataspace_id, lane_incarnation, block_height)` coordinate is found.
pub fn verify_lane_relay_envelopes(
    envelopes: &[LaneRelayEnvelope],
) -> Result<(), CrossLaneProofError> {
    let mut seen = HashSet::new();
    for envelope in envelopes {
        if let Err(err) = envelope.verify() {
            warn!(
                lane = ?envelope.lane_id,
                dataspace = ?envelope.dataspace_id,
                height = envelope.block_height,
                reason = %err.as_label(),
                "lane relay envelope rejected"
            );
            return Err(err.into());
        }
        let key = (
            envelope.lane_id,
            envelope.dataspace_id,
            envelope.lane_incarnation,
            envelope.block_height,
        );
        if !seen.insert(key) {
            warn!(
                lane = ?envelope.lane_id,
                dataspace = ?envelope.dataspace_id,
                height = envelope.block_height,
                reason = "duplicate_envelope",
                "lane relay envelope rejected"
            );
            return Err(CrossLaneProofError::DuplicateProof {
                lane_id: envelope.lane_id,
                dataspace_id: envelope.dataspace_id,
                block_height: envelope.block_height,
            });
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Hash, MerkleProof};
    use iroha_data_model::{
        block::{
            consensus::{
                LaneLiquidityProfile, LaneSettlementReceipt, LaneSwapMetadata, LaneVolatilityClass,
            },
            consensus_v2::finality::V2FinalityArtifact,
        },
        nexus::LaneId,
    };
    use std::num::NonZeroU64;
    fn sample_settlement(
        lane_id: LaneId,
        dataspace_id: DataSpaceId,
        block_height: u64,
    ) -> LaneBlockCommitment {
        LaneBlockCommitment {
            block_height,
            lane_id,
            lane_incarnation: iroha_crypto::Hash::new(b"lane-block-commitment-incarnation"),
            dataspace_id,
            tx_count: 1,
            total_local_amount: "0.00001".parse().expect("valid settlement quantity"),
            total_xor_due: "0.000005".parse().expect("valid settlement quantity"),
            total_xor_after_haircut: "0.000004".parse().expect("valid settlement quantity"),
            total_xor_variance: "0.000001".parse().expect("valid settlement quantity"),
            swap_metadata: Some(LaneSwapMetadata {
                epsilon_bps: 25,
                twap_window_seconds: 30,
                liquidity_profile: LaneLiquidityProfile::Tier1,
                twap_local_per_xor: "1.23".parse().expect("canonical TWAP"),
                volatility_class: LaneVolatilityClass::Stable,
            }),
            receipts: vec![LaneSettlementReceipt {
                source_id: [1u8; 32],
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
    fn header_with_da_hash(
        height: NonZeroU64,
        da_hash: Option<HashOf<DaCommitmentBundle>>,
    ) -> BlockHeader {
        let mut header = BlockHeader::new(height, None, None, None, 1_700_000_000_000, 0);
        header.set_da_commitments_hash(da_hash);
        header
    }
    fn sample_finality_authority(header: &BlockHeader) -> LaneFinalityAuthorityV1 {
        LaneFinalityAuthorityV1 {
            version: 1,
            global_block_height: header.height().get(),
            finality_artifact_hash: HashOf::<V2FinalityArtifact>::from_untyped_unchecked(
                Hash::new(b"sdk-finality-artifact"),
            ),
            statement_proof: MerkleProof::from_audit_path(0, Vec::new()),
        }
    }
    #[test]
    fn builder_constructs_verifiable_envelope() {
        let lane_id = LaneId::new(7);
        let dataspace_id = DataSpaceId::new(3);
        let da_hash = Some(HashOf::from_untyped_unchecked(Hash::new([0xAA; 4]))); // short input OK
        let header = header_with_da_hash(NonZeroU64::new(5).expect("nonzero height"), da_hash);
        let settlement = sample_settlement(lane_id, dataspace_id, header.height().get());
        let authority = sample_finality_authority(&header);
        let proof = CrossLaneTransferBuilder::new(header, da_hash, settlement)
            .with_finality_authority(authority.clone())
            .with_rbc_bytes_total(64)
            .build()
            .expect("builder should produce a valid envelope");
        proof.verify().expect("verification should succeed");
        assert_eq!(proof.envelope().block_height, header.height().get());
        assert_eq!(proof.envelope().rbc_bytes_total, 64);
        assert_eq!(proof.envelope().finality_authority, Some(authority));
    }
    #[test]
    fn builder_preserves_pending_envelopes_without_authority() {
        let lane_id = LaneId::new(1);
        let dataspace_id = DataSpaceId::new(2);
        let header = header_with_da_hash(NonZeroU64::new(9).expect("nonzero height"), None);
        let settlement = sample_settlement(lane_id, dataspace_id, header.height().get());
        let proof = CrossLaneTransferBuilder::new(header, None, settlement)
            .build()
            .expect("pending proof without authority");
        assert!(proof.envelope().finality_authority.is_none());
    }
    #[test]
    fn batch_verification_accepts_empty_envelopes() {
        verify_lane_relay_envelopes(&[]).expect("empty relay envelope batch should be valid");
    }
    #[test]
    fn batch_verification_rejects_duplicates() {
        let lane_id = LaneId::new(10);
        let dataspace_id = DataSpaceId::new(4);
        let header = header_with_da_hash(NonZeroU64::new(11).expect("nonzero height"), None);
        let settlement = sample_settlement(lane_id, dataspace_id, header.height().get());
        let proof = CrossLaneTransferBuilder::new(header, None, settlement)
            .build()
            .expect("envelope should be valid");
        let envelope = proof.envelope().clone();
        verify_lane_relay_envelopes(std::slice::from_ref(&envelope))
            .expect("single envelope should pass");
        let err =
            verify_lane_relay_envelopes(&[envelope.clone(), envelope]).expect_err("duplicate");
        match err {
            CrossLaneProofError::DuplicateProof {
                lane_id,
                dataspace_id: dup_dataspace_id,
                block_height,
            } => {
                assert_eq!(lane_id, LaneId::new(10));
                assert_eq!(dup_dataspace_id, dataspace_id);
                assert_eq!(block_height, 11);
            }
            other => panic!("unexpected duplicate proof error: {other:?}"),
        }
    }
    #[test]
    fn batch_verification_rejects_non_adjacent_duplicate_tuple() {
        let lane_id = LaneId::new(12);
        let dataspace_id = DataSpaceId::new(6);
        let duplicate_height = 19_u64;
        let duplicate_header = header_with_da_hash(
            NonZeroU64::new(duplicate_height).expect("nonzero height"),
            None,
        );
        let duplicate_settlement = sample_settlement(lane_id, dataspace_id, duplicate_height);
        let first =
            CrossLaneTransferBuilder::new(duplicate_header, None, duplicate_settlement.clone())
                .build()
                .expect("first envelope should be valid")
                .envelope()
                .clone();
        let third = CrossLaneTransferBuilder::new(duplicate_header, None, duplicate_settlement)
            .build()
            .expect("third envelope should be valid")
            .envelope()
            .clone();
        let middle_header = header_with_da_hash(
            NonZeroU64::new(duplicate_height + 1).expect("nonzero height"),
            None,
        );
        let middle_settlement = sample_settlement(lane_id, dataspace_id, duplicate_height + 1);
        let middle = CrossLaneTransferBuilder::new(middle_header, None, middle_settlement)
            .build()
            .expect("middle envelope should be valid")
            .envelope()
            .clone();
        let err = verify_lane_relay_envelopes(&[first, middle, third])
            .expect_err("non-adjacent duplicate tuple should be rejected");
        match err {
            CrossLaneProofError::DuplicateProof {
                lane_id: duplicate_lane_id,
                dataspace_id: duplicate_dataspace,
                block_height,
            } => {
                assert_eq!(duplicate_lane_id, lane_id);
                assert_eq!(duplicate_dataspace, dataspace_id);
                assert_eq!(block_height, duplicate_height);
            }
            other => panic!("unexpected duplicate proof error: {other:?}"),
        }
    }
    #[test]
    fn batch_verification_allows_distinct_dataspaces_on_same_lane_and_height() {
        let lane_id = LaneId::new(10);
        let first_dataspace = DataSpaceId::new(4);
        let second_dataspace = DataSpaceId::new(5);
        let header = header_with_da_hash(NonZeroU64::new(11).expect("nonzero height"), None);
        let first_settlement = sample_settlement(lane_id, first_dataspace, header.height().get());
        let second_settlement = sample_settlement(lane_id, second_dataspace, header.height().get());
        let first = CrossLaneTransferBuilder::new(header, None, first_settlement)
            .build()
            .expect("first envelope should be valid")
            .envelope()
            .clone();
        let second = CrossLaneTransferBuilder::new(header, None, second_settlement)
            .build()
            .expect("second envelope should be valid")
            .envelope()
            .clone();
        verify_lane_relay_envelopes(&[first, second])
            .expect("distinct dataspaces on same lane/height should not be duplicates");
    }
    #[test]
    fn batch_verification_allows_distinct_lanes_on_same_dataspace_and_height() {
        let first_lane = LaneId::new(10);
        let second_lane = LaneId::new(11);
        let dataspace_id = DataSpaceId::new(4);
        let header = header_with_da_hash(NonZeroU64::new(11).expect("nonzero height"), None);
        let first_settlement = sample_settlement(first_lane, dataspace_id, header.height().get());
        let second_settlement = sample_settlement(second_lane, dataspace_id, header.height().get());
        let first = CrossLaneTransferBuilder::new(header, None, first_settlement)
            .build()
            .expect("first envelope should be valid")
            .envelope()
            .clone();
        let second = CrossLaneTransferBuilder::new(header, None, second_settlement)
            .build()
            .expect("second envelope should be valid")
            .envelope()
            .clone();
        verify_lane_relay_envelopes(&[first, second])
            .expect("distinct lanes on same dataspace/height should not be duplicates");
    }
    #[test]
    fn batch_verification_duplicate_detection_depends_on_full_tuple_permutations() {
        let base_lane = LaneId::new(30);
        let base_dataspace = DataSpaceId::new(40);
        let base_height = 21_u64;
        let build_envelope = |lane_id: LaneId, dataspace_id: DataSpaceId, block_height: u64| {
            let header =
                header_with_da_hash(NonZeroU64::new(block_height).expect("nonzero height"), None);
            let settlement = sample_settlement(lane_id, dataspace_id, block_height);
            CrossLaneTransferBuilder::new(header, None, settlement)
                .build()
                .expect("envelope should be valid")
                .envelope()
                .clone()
        };
        let base = build_envelope(base_lane, base_dataspace, base_height);
        for lane_matches in [true, false] {
            for dataspace_matches in [true, false] {
                for height_matches in [true, false] {
                    let lane_id = if lane_matches {
                        base_lane
                    } else {
                        LaneId::new(base_lane.as_u32() + 1)
                    };
                    let dataspace_id = if dataspace_matches {
                        base_dataspace
                    } else {
                        DataSpaceId::new(base_dataspace.as_u64() + 1)
                    };
                    let block_height = if height_matches {
                        base_height
                    } else {
                        base_height + 1
                    };
                    let second = build_envelope(lane_id, dataspace_id, block_height);
                    let result = verify_lane_relay_envelopes(&[base.clone(), second]);
                    let should_duplicate = lane_matches && dataspace_matches && height_matches;
                    if should_duplicate {
                        assert!(
                            matches!(result, Err(CrossLaneProofError::DuplicateProof { .. })),
                            "expected duplicate for exact tuple match (lane={lane_matches}, dataspace={dataspace_matches}, height={height_matches})"
                        );
                    } else {
                        assert!(
                            result.is_ok(),
                            "expected non-duplicate for tuple permutation (lane={lane_matches}, dataspace={dataspace_matches}, height={height_matches})"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn batch_verification_reports_relay_error_before_duplicate_key_check() {
        let lane_id = LaneId::new(20);
        let dataspace_id = DataSpaceId::new(8);
        let header = header_with_da_hash(NonZeroU64::new(15).expect("nonzero height"), None);
        let settlement = sample_settlement(lane_id, dataspace_id, header.height().get());
        let valid = CrossLaneTransferBuilder::new(header, None, settlement)
            .build()
            .expect("envelope should be valid")
            .envelope()
            .clone();
        let mut tampered_duplicate = valid.clone();
        tampered_duplicate.settlement_hash = HashOf::from_untyped_unchecked(Hash::new([0xEE; 4]));
        let err = verify_lane_relay_envelopes(&[valid, tampered_duplicate])
            .expect_err("invalid envelope should be rejected before duplicate check");
        assert!(matches!(
            err,
            CrossLaneProofError::Relay(LaneRelayError::SettlementHashMismatch)
        ));
    }
    #[test]
    fn builder_accepts_independent_lane_local_settlement_height() {
        let lane_id = LaneId::new(2);
        let dataspace_id = DataSpaceId::new(3);
        let header = header_with_da_hash(NonZeroU64::new(12).expect("nonzero height"), None);
        let settlement = sample_settlement(lane_id, dataspace_id, header.height().get() - 1);
        let proof = CrossLaneTransferBuilder::new(header, None, settlement)
            .build()
            .expect("lane-local settlement height may differ from global proposal height");
        assert_eq!(proof.envelope().block_height, 11);
        assert_eq!(proof.envelope().block_header.height().get(), 12);
        proof
            .verify()
            .expect("independent lane-local and global heights should verify");
    }
    #[test]
    fn builder_rejects_da_commitment_hash_mismatch() {
        let lane_id = LaneId::new(2);
        let dataspace_id = DataSpaceId::new(3);
        let header_da_hash = Some(HashOf::from_untyped_unchecked(Hash::new([0x11; 4])));
        let provided_da_hash = Some(HashOf::from_untyped_unchecked(Hash::new([0x22; 4])));
        let header =
            header_with_da_hash(NonZeroU64::new(12).expect("nonzero height"), header_da_hash);
        let settlement = sample_settlement(lane_id, dataspace_id, header.height().get());
        let err = CrossLaneTransferBuilder::new(header, provided_da_hash, settlement)
            .build()
            .expect_err("da mismatch should fail");
        assert!(matches!(
            err,
            CrossLaneProofError::Relay(LaneRelayError::DaCommitmentHashMismatch)
        ));
    }
}
