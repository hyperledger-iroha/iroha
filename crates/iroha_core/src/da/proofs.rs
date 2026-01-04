//! Merkle proof construction and verification for DA commitment bundles.

use iroha_config::parameters::actual::LaneConfig;
use iroha_crypto::Hash;
use iroha_data_model::{
    block::BlockHeader,
    da::commitment::{
        DaCommitmentBundle, DaCommitmentLocation, DaCommitmentProof, DaCommitmentRecord,
        MerkleDirection, MerklePathItem, commitment_leaf_hash,
    },
};
use thiserror::Error;

use super::{DaProofPolicyError, enforce_lane_proof_policy};

/// Errors surfaced while validating a DA commitment proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DaProofVerificationError {
    /// Bundle hash recorded in the proof does not match the block header.
    #[error("DA commitment bundle hash mismatch: header={expected:?}, proof={observed:?}")]
    BundleHashMismatch {
        /// Hash of the bundle as recorded in the block header.
        expected: iroha_crypto::HashOf<DaCommitmentBundle>,
        /// Hash supplied by the proof.
        observed: iroha_crypto::HashOf<DaCommitmentBundle>,
    },
    /// Block height attached to the proof disagrees with the referenced header.
    #[error("DA commitment proof targets block {expected} but header reports block {observed}")]
    BlockHeightMismatch {
        /// Height encoded in the proof.
        expected: u64,
        /// Height extracted from the block header.
        observed: u64,
    },
    /// The commitment index falls outside the bundle bounds.
    #[error("commitment index {index} out of bounds for bundle length {len}")]
    IndexOutOfBounds {
        /// Index advertised by the proof.
        index: usize,
        /// Number of commitments in the bundle.
        len: usize,
    },
    /// Commitment embedded in the proof does not match the bundle entry.
    #[error("commitment payload differs from bundle entry at advertised index")]
    CommitmentMismatch,
    /// Proof violates the configured lane proof policy.
    #[error("DA proof violates lane policy: {0}")]
    Policy(#[from] DaProofPolicyError),
    /// Merkle root reconstructed from the bundle does not match the proof root.
    #[error("expected Merkle root {expected:?}, reconstructed {observed:?}")]
    RootMismatch {
        /// Root derived from the stored bundle.
        expected: Hash,
        /// Root reconstructed from the supplied proof path.
        observed: Hash,
    },
    /// Merkle path failed to fold into the supplied root.
    #[error("Merkle path does not fold into the supplied root")]
    PathMismatch,
    /// Commitment bundle referenced by the proof is missing.
    #[error("commitment bundle for block {0} is not available")]
    MissingBundle(u64),
    /// Block header lacks a commitment hash.
    #[error("block header does not advertise a DA commitment bundle hash")]
    MissingCommitmentsHash,
    /// The referenced bundle contains no commitments.
    #[error("commitment bundle is empty")]
    EmptyBundle,
}

/// Build a Merkle membership proof for a commitment.
///
/// Returns `None` when the bundle is empty or the requested index lies outside
/// the bundle bounds.
#[must_use]
pub fn build_da_commitment_proof(
    bundle: &DaCommitmentBundle,
    block_height: u64,
    index: usize,
) -> Option<DaCommitmentProof> {
    if bundle.commitments.is_empty() || index >= bundle.commitments.len() {
        return None;
    }

    let bundle_len = bundle.commitments.len();
    let mut layer: Vec<Hash> = bundle
        .commitments
        .iter()
        .map(commitment_leaf_hash)
        .collect();
    let mut path = Vec::new();
    let mut idx = index;

    while layer.len() > 1 {
        let mut next = Vec::with_capacity(layer.len().div_ceil(2));
        let mut i = 0;
        while i < layer.len() {
            if i + 1 < layer.len() {
                let left = &layer[i];
                let right = &layer[i + 1];

                if idx == i {
                    path.push(MerklePathItem {
                        sibling: *right,
                        direction: MerkleDirection::Right,
                    });
                    idx = next.len();
                } else if idx == i + 1 {
                    path.push(MerklePathItem {
                        sibling: *left,
                        direction: MerkleDirection::Left,
                    });
                    idx = next.len();
                }

                next.push(hash_internal(left, right));
                i += 2;
            } else {
                if idx == i {
                    idx = next.len();
                }
                next.push(layer[i]);
                i += 1;
            }
        }
        layer = next;
    }

    let root = layer.pop().expect("non-empty layer must have a root");
    let index_in_bundle = u32::try_from(index).ok()?;

    Some(DaCommitmentProof {
        commitment: bundle.commitments[index].clone(),
        location: DaCommitmentLocation {
            block_height,
            index_in_bundle,
        },
        bundle_hash: bundle.canonical_hash(),
        bundle_len: u32::try_from(bundle_len).unwrap_or(u32::MAX),
        root,
        path,
    })
}

/// Verify a DA commitment proof against the supplied bundle and block header.
///
/// This routine enforces lane proof policy, validates the Merkle path, and
/// ensures the bundle hash matches the block header metadata.
///
/// # Errors
///
/// Returns an error when the bundle hash/header height mismatch, the Merkle
/// path cannot be verified, or the lane proof policy validation fails.
pub fn verify_da_commitment_proof(
    proof: &DaCommitmentProof,
    bundle: &DaCommitmentBundle,
    header: &BlockHeader,
    lane_config: &LaneConfig,
) -> Result<(), DaProofVerificationError> {
    if bundle.commitments.is_empty() {
        return Err(DaProofVerificationError::EmptyBundle);
    }

    let Some(commitments_hash) = header.da_commitments_hash() else {
        return Err(DaProofVerificationError::MissingCommitmentsHash);
    };

    if proof.bundle_hash != commitments_hash {
        return Err(DaProofVerificationError::BundleHashMismatch {
            expected: commitments_hash,
            observed: proof.bundle_hash,
        });
    }

    let header_height = header.height().get();
    if proof.location.block_height != header_height {
        return Err(DaProofVerificationError::BlockHeightMismatch {
            expected: proof.location.block_height,
            observed: header_height,
        });
    }

    let idx = usize::try_from(proof.location.index_in_bundle).map_err(|_| {
        DaProofVerificationError::IndexOutOfBounds {
            index: usize::MAX,
            len: bundle.commitments.len(),
        }
    })?;
    if idx >= bundle.commitments.len() {
        return Err(DaProofVerificationError::IndexOutOfBounds {
            index: idx,
            len: bundle.commitments.len(),
        });
    }

    let expected_bundle_hash = bundle.canonical_hash();
    if expected_bundle_hash != proof.bundle_hash {
        return Err(DaProofVerificationError::BundleHashMismatch {
            expected: expected_bundle_hash,
            observed: proof.bundle_hash,
        });
    }

    let expected_commitment: &DaCommitmentRecord = &bundle.commitments[idx];
    if expected_commitment != &proof.commitment {
        return Err(DaProofVerificationError::CommitmentMismatch);
    }

    enforce_lane_proof_policy(&proof.commitment, lane_config)?;

    let mut acc = commitment_leaf_hash(&proof.commitment);
    for hop in &proof.path {
        acc = match hop.direction {
            MerkleDirection::Left => hash_internal(&hop.sibling, &acc),
            MerkleDirection::Right => hash_internal(&acc, &hop.sibling),
        };
    }

    if acc != proof.root {
        return Err(DaProofVerificationError::PathMismatch);
    }

    let Some(expected_root) = bundle.merkle_root() else {
        return Err(DaProofVerificationError::EmptyBundle);
    };
    if acc != expected_root {
        return Err(DaProofVerificationError::RootMismatch {
            expected: expected_root,
            observed: acc,
        });
    }

    Ok(())
}

fn hash_internal(left: &Hash, right: &Hash) -> Hash {
    let mut buf = Vec::with_capacity(Hash::LENGTH * 2);
    buf.extend_from_slice(left.as_ref());
    buf.extend_from_slice(right.as_ref());
    Hash::new(buf)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;

    use iroha_config::parameters::actual::LaneConfig as ConfigLaneConfig;
    use iroha_crypto::HashOf;
    use iroha_data_model::{
        block::BlockHeader,
        da::{
            commitment::{DaCommitmentRecord, DaProofScheme, RetentionClass},
            types::{BlobDigest, StorageTicketId},
        },
        nexus::{LaneCatalog, LaneConfig as ModelLaneConfig, LaneId},
    };

    use super::*;

    fn sample_record(lane: u32, manifest_tag: u8) -> DaCommitmentRecord {
        let lane_byte = u8::try_from(lane).expect("lane fits in u8 for test record");
        DaCommitmentRecord::new(
            LaneId::new(lane),
            1,
            1,
            BlobDigest::new([lane_byte; 32]),
            iroha_data_model::sorafs::pin_registry::ManifestDigest::new([manifest_tag; 32]),
            DaProofScheme::MerkleSha256,
            Hash::prehashed([manifest_tag; 32]),
            None,
            None,
            RetentionClass::default(),
            StorageTicketId::new([manifest_tag; 32]),
            iroha_crypto::Signature::from_bytes(&[0xAA; 64]),
        )
    }

    fn lane_config() -> ConfigLaneConfig {
        let meta = ModelLaneConfig {
            id: LaneId::new(1),
            alias: "lane-1".to_string(),
            ..ModelLaneConfig::default()
        };
        let catalog =
            LaneCatalog::new(std::num::NonZeroU32::new(2).unwrap(), vec![meta]).expect("catalog");
        ConfigLaneConfig::from_catalog(&catalog)
    }

    fn header_with_hash(height: u64, da_hash: HashOf<DaCommitmentBundle>) -> BlockHeader {
        let height = NonZeroU64::new(height).expect("non-zero height");
        let mut header = BlockHeader::new(height, None, None, None, 0, 0);
        header.set_da_commitments_hash(Some(da_hash));
        header
    }

    #[test]
    fn build_and_verify_proof_succeeds() {
        let bundle = DaCommitmentBundle::new(vec![sample_record(1, 1), sample_record(1, 2)]);
        let proof = build_da_commitment_proof(&bundle, 3, 1).expect("proof");
        let header = header_with_hash(3, bundle.canonical_hash());

        assert!(verify_da_commitment_proof(&proof, &bundle, &header, &lane_config()).is_ok());
    }

    #[test]
    fn verify_rejects_root_mismatch() {
        let bundle = DaCommitmentBundle::new(vec![sample_record(1, 1), sample_record(1, 2)]);
        let mut proof = build_da_commitment_proof(&bundle, 3, 0).expect("proof");
        proof.root = Hash::prehashed([0xFF; 32]);
        let header = header_with_hash(3, bundle.canonical_hash());
        let err = verify_da_commitment_proof(&proof, &bundle, &header, &lane_config())
            .expect_err("should fail");
        assert!(matches!(err, DaProofVerificationError::PathMismatch));
    }

    #[test]
    fn build_returns_none_for_out_of_bounds() {
        let bundle = DaCommitmentBundle::new(vec![sample_record(1, 1)]);
        assert!(build_da_commitment_proof(&bundle, 1, 2).is_none());
    }
}
