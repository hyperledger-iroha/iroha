//! Merkle proof construction and verification for DA commitment bundles.
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    block::BlockHeader,
    da::commitment::{
        DaCommitmentBundle, DaCommitmentLocation, DaCommitmentProof, DaProofPolicyBundle,
        MerkleDirection, MerklePathItem, commitment_internal_hash, commitment_leaf_hash,
        commitment_merkle_commitment,
    },
    da::pin_intent::{
        DaPinIntentBundle, DaPinIntentProof, pin_intent_internal_hash, pin_intent_leaf_hash,
        pin_intent_merkle_commitment,
    },
};
use thiserror::Error;
use super::{DaProofPolicyError, enforce_committed_proof_policy};
/// Errors surfaced while validating a DA commitment proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DaProofVerificationError {
    /// Tree-descriptor commitment in the proof does not match the block header.
    #[error("DA commitment tree descriptor mismatch: header={expected:?}, proof={observed:?}")]
    BundleHashMismatch {
        /// Versioned tree-descriptor commitment recorded in the block header.
        expected: iroha_crypto::HashOf<DaCommitmentBundle>,
        /// Tree-descriptor commitment supplied by or reconstructed from the proof.
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
    /// The referenced bundle is too large to encode in proof metadata.
    #[error("commitment bundle length {len} exceeds supported proof metadata range")]
    BundleLengthUnsupported {
        /// Number of commitments in the referenced bundle.
        len: usize,
    },
    /// The commitment index falls outside the bundle bounds.
    #[error("commitment index {index} out of bounds for bundle length {len}")]
    IndexOutOfBounds {
        /// Index advertised by the proof.
        index: usize,
        /// Number of commitments in the bundle.
        len: usize,
    },
    /// Proof violates the policy authenticated by the referenced block.
    #[error("DA proof violates lane policy: {0}")]
    Policy(#[from] DaProofPolicyError),
    /// Merkle path failed to fold into the supplied root.
    #[error("Merkle path does not fold into the supplied root")]
    PathMismatch,
    /// The Merkle path shape does not correspond to the claimed bundle index.
    #[error("Merkle path is not valid for index {index} in bundle length {len}")]
    PathLocationMismatch {
        /// Index advertised by the proof.
        index: usize,
        /// Number of commitments in the bundle.
        len: usize,
    },
    /// Block header lacks a commitment-tree descriptor.
    #[error("block header does not advertise a DA commitment tree descriptor")]
    MissingCommitmentsHash,
    /// Block header lacks a committed proof-policy-sidecar hash.
    #[error("block header does not advertise a committed DA proof-policy sidecar hash")]
    MissingProofPoliciesHash,
    /// The supplied proof-policy bundle is not the bundle authenticated by the header.
    #[error("DA proof-policy bundle hash mismatch: header={expected:?}, supplied={observed:?}")]
    ProofPoliciesHashMismatch {
        /// Policy-bundle hash authenticated by the header.
        expected: HashOf<DaProofPolicyBundle>,
        /// Hash of the supplied policy bundle.
        observed: HashOf<DaProofPolicyBundle>,
    },
}
/// Errors surfaced while validating a DA pin-intent membership proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DaPinIntentProofVerificationError {
    /// The referenced pin-intent bundle is empty.
    #[error("DA pin-intent bundle is empty")]
    EmptyBundle,
    /// The referenced block does not commit a pin-intent tree descriptor.
    #[error("block header is missing its DA pin-intent tree descriptor")]
    MissingBundleHash,
    /// The proof and block header disagree on the committed tree descriptor.
    #[error("DA pin-intent tree descriptor mismatch: expected={expected:?}, observed={observed:?}")]
    BundleHashMismatch {
        /// Versioned tree-descriptor commitment expected by the verifier.
        expected: iroha_crypto::HashOf<DaPinIntentBundle>,
        /// Tree-descriptor commitment supplied by or reconstructed from the proof.
        observed: iroha_crypto::HashOf<DaPinIntentBundle>,
    },
    /// The proof targets a different block height than the supplied header.
    #[error("DA pin-intent proof targets block {expected} but header reports block {observed}")]
    BlockHeightMismatch {
        /// Height encoded by the proof.
        expected: u64,
        /// Height encoded by the block header.
        observed: u64,
    },
    /// The proof's index is outside the committed bundle.
    #[error("DA pin-intent index {index} out of bounds for bundle length {len}")]
    IndexOutOfBounds {
        /// Index advertised by the proof.
        index: usize,
        /// Number of intents in the bundle.
        len: usize,
    },
    /// The supplied Merkle path does not reconstruct the advertised root.
    #[error("DA pin-intent Merkle path does not reconstruct the advertised root")]
    PathMismatch,
    /// The Merkle path shape does not correspond to the claimed bundle index.
    #[error("DA pin-intent Merkle path is not valid for index {index} in bundle length {len}")]
    PathLocationMismatch {
        /// Index advertised by the proof.
        index: usize,
        /// Number of intents in the bundle.
        len: usize,
    },
}
/// Build a Merkle membership proof for a commitment.
///
/// Returns `None` when the bundle is empty, the requested index lies outside
/// the bundle bounds, or the bundle length cannot be represented in proof
/// metadata.
#[must_use]
pub fn build_da_commitment_proof(
    bundle: &DaCommitmentBundle,
    block_height: u64,
    index: usize,
) -> Option<DaCommitmentProof> {
    if bundle.version != DaCommitmentBundle::VERSION_V1
        || bundle.commitments.is_empty()
        || index >= bundle.commitments.len()
    {
        return None;
    }
    let bundle_len = bundle_len_u32(bundle.commitments.len()).ok()?;
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
                next.push(commitment_internal_hash(left, right));
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
    let root = layer.pop()?;
    let index_in_bundle = u32::try_from(index).ok()?;
    Some(DaCommitmentProof {
        commitment: bundle.commitments[index].clone(),
        location: DaCommitmentLocation {
            block_height,
            index_in_bundle,
        },
        bundle_hash: bundle.merkle_commitment()?,
        bundle_len,
        root,
        path,
    })
}
/// Verify a DA commitment proof against a caller-authenticated block header and
/// its policy sidecar.
///
/// The header commitment authenticates the V1 tree version, leaf count, and
/// root, so verification is logarithmic and does not require the full bundle.
///
/// # Errors
///
/// Returns an error when the header binding, path, location, or committed lane
/// policy is invalid.
pub fn verify_da_commitment_proof(
    proof: &DaCommitmentProof,
    header: &BlockHeader,
    policies: &DaProofPolicyBundle,
) -> Result<(), DaProofVerificationError> {
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
    let bundle_len = usize::try_from(proof.bundle_len).unwrap_or(usize::MAX);
    let idx = usize::try_from(proof.location.index_in_bundle).map_err(|_| {
        DaProofVerificationError::IndexOutOfBounds {
            index: usize::MAX,
            len: bundle_len,
        }
    })?;
    if idx >= bundle_len {
        return Err(DaProofVerificationError::IndexOutOfBounds {
            index: idx,
            len: bundle_len,
        });
    }
    let Some(policy_hash) = header.da_proof_policies_hash() else {
        return Err(DaProofVerificationError::MissingProofPoliciesHash);
    };
    let supplied_policy_hash = HashOf::new(policies);
    if supplied_policy_hash != policy_hash {
        return Err(DaProofVerificationError::ProofPoliciesHashMismatch {
            expected: policy_hash,
            observed: supplied_policy_hash,
        });
    }
    enforce_committed_proof_policy(&proof.commitment, policies)?;
    if !merkle_path_matches_location(&proof.path, idx, bundle_len) {
        return Err(DaProofVerificationError::PathLocationMismatch {
            index: idx,
            len: bundle_len,
        });
    }
    let mut acc = commitment_leaf_hash(&proof.commitment);
    for hop in &proof.path {
        acc = match hop.direction {
            MerkleDirection::Left => commitment_internal_hash(&hop.sibling, &acc),
            MerkleDirection::Right => commitment_internal_hash(&acc, &hop.sibling),
        };
    }
    if acc != proof.root {
        return Err(DaProofVerificationError::PathMismatch);
    }
    let reconstructed = commitment_merkle_commitment(
        DaCommitmentBundle::VERSION_V1,
        proof.bundle_len,
        &proof.root,
    );
    if reconstructed != commitments_hash {
        return Err(DaProofVerificationError::BundleHashMismatch {
            expected: commitments_hash,
            observed: reconstructed,
        });
    }
    Ok(())
}
/// Build a Merkle membership proof for a pin intent.
///
/// Returns `None` when the bundle is empty, the index is outside the bundle,
/// or the V1 proof metadata cannot represent the bundle length.
#[must_use]
pub fn build_da_pin_intent_proof(
    bundle: &DaPinIntentBundle,
    block_height: u64,
    index: usize,
) -> Option<DaPinIntentProof> {
    if bundle.version != DaPinIntentBundle::VERSION_V1
        || bundle.intents.is_empty()
        || index >= bundle.intents.len()
    {
        return None;
    }
    let bundle_len = u32::try_from(bundle.intents.len()).ok()?;
    let mut layer: Vec<Hash> = bundle.intents.iter().map(pin_intent_leaf_hash).collect();
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
                next.push(pin_intent_internal_hash(left, right));
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
    let root = layer.pop()?;
    Some(DaPinIntentProof {
        intent: bundle.intents[index].clone(),
        location: DaCommitmentLocation {
            block_height,
            index_in_bundle: u32::try_from(index).ok()?,
        },
        bundle_hash: pin_intent_merkle_commitment(bundle.version, bundle_len, &root),
        bundle_len,
        root,
        path,
    })
}
/// Verify a pin-intent membership proof against a caller-authenticated block
/// header.
///
/// The header commitment authenticates the V1 tree version, leaf count, and
/// root, so the full pin-intent bundle is not required.
///
/// # Errors
///
/// Returns an error when header binding, location, or Merkle path verification
/// fails.
pub fn verify_da_pin_intent_proof(
    proof: &DaPinIntentProof,
    header: &BlockHeader,
) -> Result<(), DaPinIntentProofVerificationError> {
    let Some(header_bundle_hash) = header.da_pin_intents_hash() else {
        return Err(DaPinIntentProofVerificationError::MissingBundleHash);
    };
    if proof.bundle_hash != header_bundle_hash {
        return Err(DaPinIntentProofVerificationError::BundleHashMismatch {
            expected: header_bundle_hash,
            observed: proof.bundle_hash,
        });
    }
    if proof.location.block_height != header.height().get() {
        return Err(DaPinIntentProofVerificationError::BlockHeightMismatch {
            expected: proof.location.block_height,
            observed: header.height().get(),
        });
    }
    let bundle_len = usize::try_from(proof.bundle_len).unwrap_or(usize::MAX);
    if bundle_len == 0 {
        return Err(DaPinIntentProofVerificationError::EmptyBundle);
    }
    let index = usize::try_from(proof.location.index_in_bundle).map_err(|_| {
        DaPinIntentProofVerificationError::IndexOutOfBounds {
            index: usize::MAX,
            len: bundle_len,
        }
    })?;
    if index >= bundle_len {
        return Err(DaPinIntentProofVerificationError::IndexOutOfBounds {
            index,
            len: bundle_len,
        });
    }
    if !merkle_path_matches_location(&proof.path, index, bundle_len) {
        return Err(DaPinIntentProofVerificationError::PathLocationMismatch {
            index,
            len: bundle_len,
        });
    }
    let mut acc = pin_intent_leaf_hash(&proof.intent);
    for hop in &proof.path {
        acc = match hop.direction {
            MerkleDirection::Left => pin_intent_internal_hash(&hop.sibling, &acc),
            MerkleDirection::Right => pin_intent_internal_hash(&acc, &hop.sibling),
        };
    }
    if acc != proof.root {
        return Err(DaPinIntentProofVerificationError::PathMismatch);
    }
    let reconstructed =
        pin_intent_merkle_commitment(DaPinIntentBundle::VERSION_V1, proof.bundle_len, &proof.root);
    if reconstructed != header_bundle_hash {
        return Err(DaPinIntentProofVerificationError::BundleHashMismatch {
            expected: header_bundle_hash,
            observed: reconstructed,
        });
    }
    Ok(())
}
fn merkle_path_matches_location(
    path: &[MerklePathItem],
    mut index: usize,
    mut width: usize,
) -> bool {
    if width == 0 || index >= width {
        return false;
    }
    let mut path_index = 0;
    while width > 1 {
        let expected_direction = if index % 2 == 1 {
            Some(MerkleDirection::Left)
        } else if index + 1 < width {
            Some(MerkleDirection::Right)
        } else {
            None
        };
        if let Some(expected) = expected_direction {
            let Some(hop) = path.get(path_index) else {
                return false;
            };
            if hop.direction != expected {
                return false;
            }
            path_index += 1;
        }
        index /= 2;
        width = width.div_ceil(2);
    }
    path_index == path.len()
}
fn bundle_len_u32(len: usize) -> Result<u32, DaProofVerificationError> {
    u32::try_from(len).map_err(|_| DaProofVerificationError::BundleLengthUnsupported { len })
}
#[cfg(test)]
mod tests {
    use std::num::NonZeroU64;
    use super::*;
    use iroha_crypto::HashOf;
    use iroha_data_model::{
        block::BlockHeader,
        da::{
            commitment::{
                DaCommitmentRecord, DaProofPolicy, DaProofPolicyBundle, DaProofScheme,
                RetentionClass,
            },
            pin_intent::{DaPinIntent, DaPinIntentBundle},
            types::{BlobDigest, StorageTicketId},
        },
        nexus::{DataSpaceId, LaneId},
    };
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
            RetentionClass::default(),
            StorageTicketId::new([manifest_tag; 32]),
            iroha_crypto::Signature::try_from_bytes(&[0xAA; 64])
                .expect("checked core DA proof acknowledgement signature fixture"),
        )
    }
    fn policy_bundle() -> DaProofPolicyBundle {
        DaProofPolicyBundle::new(vec![DaProofPolicy {
            lane_id: LaneId::new(1),
            dataspace_id: DataSpaceId::UNIVERSAL,
            alias: "lane-1".to_owned(),
            proof_scheme: DaProofScheme::MerkleSha256,
        }])
    }
    fn header_with_hash(height: u64, da_hash: HashOf<DaCommitmentBundle>) -> BlockHeader {
        let height = NonZeroU64::new(height).expect("non-zero height");
        let mut header = BlockHeader::new(height, None, None, None, 0, 0);
        header.set_da_commitments_hash(Some(da_hash));
        header.set_da_proof_policies_hash(Some(HashOf::new(&policy_bundle())));
        header
    }
    fn header_without_da_hash(height: u64) -> BlockHeader {
        let height = NonZeroU64::new(height).expect("non-zero height");
        BlockHeader::new(height, None, None, None, 0, 0)
    }
    fn sample_pin_intent(lane: u32, sequence: u64) -> DaPinIntent {
        let tag = u8::try_from(sequence).expect("test sequence fits u8");
        let lane_id = LaneId::new(lane);
        let key_pair =
            iroha_crypto::KeyPair::try_from_seed(vec![0xD5; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("valid deterministic DA proof key");
        let network_id = iroha_data_model::NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
            BlockHeader,
        >::from_untyped_unchecked(
            Hash::prehashed([0xD6; 32]),
        ));
        DaPinIntent::new(
            lane_id,
            1,
            sequence,
            StorageTicketId::new([tag; 32]),
            iroha_data_model::sorafs::pin_registry::ManifestDigest::new([tag; 32]),
            crate::da::signed_test_ingest_authorization(
                network_id, &key_pair, lane_id, 1, sequence, 1,
            ),
        )
    }
    fn header_with_pin_intent_hash(height: u64, hash: HashOf<DaPinIntentBundle>) -> BlockHeader {
        let mut header = BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        header.set_da_pin_intents_hash(Some(hash));
        header
    }
    #[test]
    fn pin_intent_proof_binds_payload_index_path_and_block_bundle() {
        let bundle = DaPinIntentBundle::new(vec![
            sample_pin_intent(1, 1),
            sample_pin_intent(1, 2),
            sample_pin_intent(1, 3),
        ]);
        let proof = build_da_pin_intent_proof(&bundle, 4, 1).expect("pin-intent proof");
        let header =
            header_with_pin_intent_hash(4, bundle.merkle_commitment().expect("non-empty bundle"));
        verify_da_pin_intent_proof(&proof, &header)
            .expect("canonical pin-intent proof must verify");
        let mut tampered = proof;
        tampered.location.index_in_bundle = 0;
        assert!(matches!(
            verify_da_pin_intent_proof(&tampered, &header),
            Err(DaPinIntentProofVerificationError::PathLocationMismatch { .. })
        ));
    }
    #[test]
    fn pin_intent_proof_rejects_path_and_header_binding_drift() {
        let bundle = DaPinIntentBundle::new(vec![sample_pin_intent(1, 1), sample_pin_intent(1, 2)]);
        let proof = build_da_pin_intent_proof(&bundle, 4, 0).expect("pin-intent proof");
        let wrong_header = header_with_pin_intent_hash(
            4,
            DaPinIntentBundle::new(vec![sample_pin_intent(1, 9)])
                .merkle_commitment()
                .expect("non-empty bundle"),
        );
        assert!(matches!(
            verify_da_pin_intent_proof(&proof, &wrong_header),
            Err(DaPinIntentProofVerificationError::BundleHashMismatch { .. })
        ));
        let header =
            header_with_pin_intent_hash(4, bundle.merkle_commitment().expect("non-empty bundle"));
        let mut tampered = proof;
        tampered.path[0].sibling = Hash::prehashed([0xA5; 32]);
        assert!(matches!(
            verify_da_pin_intent_proof(&tampered, &header),
            Err(DaPinIntentProofVerificationError::PathMismatch)
        ));
    }
    #[test]
    fn pin_intent_proof_binds_index_when_payloads_are_identical() {
        let intent = sample_pin_intent(1, 1);
        let bundle = DaPinIntentBundle::new(vec![intent.clone(), intent]);
        let mut proof = build_da_pin_intent_proof(&bundle, 4, 0).expect("pin-intent proof");
        proof.location.index_in_bundle = 1;
        let header =
            header_with_pin_intent_hash(4, bundle.merkle_commitment().expect("non-empty bundle"));
        assert!(matches!(
            verify_da_pin_intent_proof(&proof, &header),
            Err(DaPinIntentProofVerificationError::PathLocationMismatch { index: 1, len: 2 })
        ));
    }
    #[test]
    fn pin_intent_proofs_verify_at_every_position_in_odd_width_trees() {
        for width in [3_u64, 5] {
            let bundle = DaPinIntentBundle::new(
                (1..=width)
                    .map(|sequence| sample_pin_intent(1, sequence))
                    .collect(),
            );
            let header =
                header_with_pin_intent_hash(4, bundle.merkle_commitment().expect("commitment"));
            for index in 0..bundle.intents.len() {
                let proof = build_da_pin_intent_proof(&bundle, 4, index).expect("membership proof");
                verify_da_pin_intent_proof(&proof, &header)
                    .unwrap_or_else(|error| panic!("width {width}, index {index}: {error}"));
            }
        }
    }
    #[test]
    fn pin_intent_proof_rejects_shape_compatible_leaf_count_tampering() {
        let bundle = DaPinIntentBundle::new(vec![
            sample_pin_intent(1, 1),
            sample_pin_intent(1, 2),
            sample_pin_intent(1, 3),
        ]);
        let mut proof = build_da_pin_intent_proof(&bundle, 4, 0).expect("membership proof");
        let header =
            header_with_pin_intent_hash(4, bundle.merkle_commitment().expect("commitment"));
        // Widths three and four have the same path shape for index zero. The
        // signed leaf count must still make this substitution fail.
        proof.bundle_len = 4;
        assert!(matches!(
            verify_da_pin_intent_proof(&proof, &header),
            Err(DaPinIntentProofVerificationError::BundleHashMismatch { .. })
        ));
    }
    #[test]
    fn build_and_verify_proof_succeeds() {
        let bundle = DaCommitmentBundle::new(vec![sample_record(1, 1), sample_record(1, 2)]);
        let proof = build_da_commitment_proof(&bundle, 3, 1).expect("proof");
        let header = header_with_hash(3, bundle.merkle_commitment().expect("non-empty bundle"));
        assert!(verify_da_commitment_proof(&proof, &header, &policy_bundle()).is_ok());
    }
    #[test]
    fn commitment_proofs_verify_at_every_position_in_odd_width_trees() {
        for width in [3_u8, 5] {
            let bundle =
                DaCommitmentBundle::new((1..=width).map(|tag| sample_record(1, tag)).collect());
            let header = header_with_hash(3, bundle.merkle_commitment().expect("commitment"));
            for index in 0..bundle.commitments.len() {
                let proof = build_da_commitment_proof(&bundle, 3, index).expect("membership proof");
                verify_da_commitment_proof(&proof, &header, &policy_bundle())
                    .unwrap_or_else(|error| panic!("width {width}, index {index}: {error}"));
            }
        }
    }
    #[test]
    fn commitment_proof_rejects_shape_compatible_leaf_count_tampering() {
        let bundle = DaCommitmentBundle::new(vec![
            sample_record(1, 1),
            sample_record(1, 2),
            sample_record(1, 3),
        ]);
        let mut proof = build_da_commitment_proof(&bundle, 3, 0).expect("membership proof");
        let header = header_with_hash(3, bundle.merkle_commitment().expect("commitment"));
        // Widths three and four have the same path shape for index zero. The
        // signed leaf count must still make this substitution fail.
        proof.bundle_len = 4;
        assert!(matches!(
            verify_da_commitment_proof(&proof, &header, &policy_bundle()),
            Err(DaProofVerificationError::BundleHashMismatch { .. })
        ));
    }
    #[test]
    fn verify_rejects_root_mismatch() {
        let bundle = DaCommitmentBundle::new(vec![sample_record(1, 1), sample_record(1, 2)]);
        let mut proof = build_da_commitment_proof(&bundle, 3, 0).expect("proof");
        proof.root = Hash::prehashed([0xFF; 32]);
        let header = header_with_hash(3, bundle.merkle_commitment().expect("non-empty bundle"));
        let err =
            verify_da_commitment_proof(&proof, &header, &policy_bundle()).expect_err("should fail");
        assert!(matches!(err, DaProofVerificationError::PathMismatch));
    }
    #[test]
    fn verify_rejects_bundle_len_mismatch() {
        let bundle = DaCommitmentBundle::new(vec![sample_record(1, 1), sample_record(1, 2)]);
        let mut proof = build_da_commitment_proof(&bundle, 3, 0).expect("proof");
        proof.bundle_len = proof.bundle_len.saturating_add(1);
        let header = header_with_hash(3, bundle.merkle_commitment().expect("non-empty bundle"));
        let err = verify_da_commitment_proof(&proof, &header, &policy_bundle())
            .expect_err("tampered bundle length must fail");
        assert!(matches!(
            err,
            DaProofVerificationError::PathLocationMismatch { index: 0, len: 3 }
        ));
    }
    #[test]
    fn bundle_len_metadata_rejects_overflow_without_saturating() {
        assert_eq!(
            bundle_len_u32(u32::MAX as usize).expect("u32::MAX is representable"),
            u32::MAX
        );
        if let Some(unsupported_len) = (u32::MAX as usize).checked_add(1) {
            let err = bundle_len_u32(unsupported_len)
                .expect_err("oversized bundle length must be rejected");
            assert!(matches!(
                err,
                DaProofVerificationError::BundleLengthUnsupported { len } if len == unsupported_len
            ));
        }
    }
    #[test]
    fn verify_rejects_missing_header_commitment_hash() {
        let bundle = DaCommitmentBundle::new(vec![sample_record(1, 1), sample_record(1, 2)]);
        let proof = build_da_commitment_proof(&bundle, 3, 0).expect("proof");
        let header = header_without_da_hash(3);
        let err = verify_da_commitment_proof(&proof, &header, &policy_bundle())
            .expect_err("proof must fail without header commitment hash");
        assert!(matches!(
            err,
            DaProofVerificationError::MissingCommitmentsHash
        ));
    }
    #[test]
    fn verify_rejects_block_height_mismatch() {
        let bundle = DaCommitmentBundle::new(vec![sample_record(1, 1), sample_record(1, 2)]);
        let proof = build_da_commitment_proof(&bundle, 4, 0).expect("proof");
        let header = header_with_hash(3, bundle.merkle_commitment().expect("non-empty bundle"));
        let err = verify_da_commitment_proof(&proof, &header, &policy_bundle())
            .expect_err("proof must fail when location height drifts from header height");
        assert!(matches!(
            err,
            DaProofVerificationError::BlockHeightMismatch {
                expected: 4,
                observed: 3
            }
        ));
    }
    #[test]
    fn verify_rejects_commitment_index_tampering() {
        let bundle = DaCommitmentBundle::new(vec![sample_record(1, 1), sample_record(1, 2)]);
        let mut proof = build_da_commitment_proof(&bundle, 3, 0).expect("proof");
        proof.location.index_in_bundle = u32::MAX;
        let header = header_with_hash(3, bundle.merkle_commitment().expect("non-empty bundle"));
        let err = verify_da_commitment_proof(&proof, &header, &policy_bundle())
            .expect_err("proof must fail when index leaves bundle bounds");
        assert!(matches!(
            err,
            DaProofVerificationError::IndexOutOfBounds {
                index,
                len: 2
            } if index == usize::try_from(u32::MAX).expect("u32 fits usize")
        ));
    }
    #[test]
    fn verify_binds_commitment_index_when_payloads_are_identical() {
        let commitment = sample_record(1, 1);
        let bundle = DaCommitmentBundle::new(vec![commitment.clone(), commitment]);
        let mut proof = build_da_commitment_proof(&bundle, 3, 0).expect("proof");
        proof.location.index_in_bundle = 1;
        let header = header_with_hash(3, bundle.merkle_commitment().expect("non-empty bundle"));
        let err = verify_da_commitment_proof(&proof, &header, &policy_bundle())
            .expect_err("proof path must bind an identical payload to its claimed index");
        assert!(matches!(
            err,
            DaProofVerificationError::PathLocationMismatch { index: 1, len: 2 }
        ));
    }
    #[test]
    fn verify_rejects_commitment_payload_tampering() {
        let bundle = DaCommitmentBundle::new(vec![sample_record(1, 1), sample_record(1, 2)]);
        let mut proof = build_da_commitment_proof(&bundle, 3, 0).expect("proof");
        proof.commitment = sample_record(1, 9);
        let header = header_with_hash(3, bundle.merkle_commitment().expect("non-empty bundle"));
        let err = verify_da_commitment_proof(&proof, &header, &policy_bundle())
            .expect_err("proof must fail when commitment payload is replaced");
        assert!(matches!(err, DaProofVerificationError::PathMismatch));
    }
    #[test]
    fn verify_rejects_merkle_path_tampering() {
        let bundle = DaCommitmentBundle::new(vec![sample_record(1, 1), sample_record(1, 2)]);
        let mut proof = build_da_commitment_proof(&bundle, 3, 0).expect("proof");
        proof
            .path
            .first_mut()
            .expect("two-leaf proof should have a sibling")
            .sibling = Hash::prehashed([0xAB; 32]);
        let header = header_with_hash(3, bundle.merkle_commitment().expect("non-empty bundle"));
        let err = verify_da_commitment_proof(&proof, &header, &policy_bundle())
            .expect_err("proof must fail when a Merkle sibling is replaced");
        assert!(matches!(err, DaProofVerificationError::PathMismatch));
    }
    #[test]
    fn verify_rejects_lane_policy_violation() {
        let bundle = DaCommitmentBundle::new(vec![sample_record(9, 1)]);
        let proof = build_da_commitment_proof(&bundle, 3, 0).expect("proof");
        let header = header_with_hash(3, bundle.merkle_commitment().expect("non-empty bundle"));
        let err = verify_da_commitment_proof(&proof, &header, &policy_bundle())
            .expect_err("proof must fail when its lane is not configured");
        assert!(matches!(
            err,
            DaProofVerificationError::Policy(DaProofPolicyError::UnknownLane { lane })
                if lane == LaneId::new(9)
        ));
    }
    #[test]
    fn verify_rejects_uncommitted_policy_sidecar() {
        let bundle = DaCommitmentBundle::new(vec![sample_record(1, 1)]);
        let proof = build_da_commitment_proof(&bundle, 3, 0).expect("proof");
        let header = header_with_hash(3, bundle.merkle_commitment().expect("commitment"));
        let supplied = DaProofPolicyBundle::new(vec![DaProofPolicy {
            lane_id: LaneId::new(1),
            dataspace_id: DataSpaceId::UNIVERSAL,
            alias: "substituted".to_owned(),
            proof_scheme: DaProofScheme::MerkleSha256,
        }]);
        assert!(matches!(
            verify_da_commitment_proof(&proof, &header, &supplied),
            Err(DaProofVerificationError::ProofPoliciesHashMismatch { .. })
        ));
    }
    #[test]
    fn verify_rejects_malformed_committed_policy_sidecar() {
        let bundle = DaCommitmentBundle::new(vec![sample_record(1, 1)]);
        let proof = build_da_commitment_proof(&bundle, 3, 0).expect("proof");
        let mut malformed = policy_bundle();
        malformed.policy_hash = Hash::new(b"invalid-internal-policy-hash");
        let mut header = header_with_hash(3, bundle.merkle_commitment().expect("commitment"));
        header.set_da_proof_policies_hash(Some(HashOf::new(&malformed)));
        assert!(matches!(
            verify_da_commitment_proof(&proof, &header, &malformed),
            Err(DaProofVerificationError::Policy(
                DaProofPolicyError::PolicyBundleHashMismatch
            ))
        ));
    }
    #[test]
    fn verify_rejects_duplicate_committed_lane_policy() {
        let bundle = DaCommitmentBundle::new(vec![sample_record(1, 1)]);
        let proof = build_da_commitment_proof(&bundle, 3, 0).expect("proof");
        let policy = policy_bundle().policies[0].clone();
        let duplicate = DaProofPolicyBundle::new(vec![policy.clone(), policy]);
        let mut header = header_with_hash(3, bundle.merkle_commitment().expect("commitment"));
        header.set_da_proof_policies_hash(Some(HashOf::new(&duplicate)));
        assert!(matches!(
            verify_da_commitment_proof(&proof, &header, &duplicate),
            Err(DaProofVerificationError::Policy(
                DaProofPolicyError::DuplicateLanePolicy { lane }
            )) if lane == LaneId::new(1)
        ));
    }
    #[test]
    fn proof_builders_reject_unsupported_bundle_versions() {
        let mut commitments = DaCommitmentBundle::new(vec![sample_record(1, 1)]);
        commitments.version = DaCommitmentBundle::VERSION_V1 + 1;
        assert!(build_da_commitment_proof(&commitments, 3, 0).is_none());
        let mut intents = DaPinIntentBundle::new(vec![sample_pin_intent(1, 1)]);
        intents.version = DaPinIntentBundle::VERSION_V1 + 1;
        assert!(build_da_pin_intent_proof(&intents, 3, 0).is_none());
    }
    #[test]
    fn build_returns_none_for_out_of_bounds() {
        let bundle = DaCommitmentBundle::new(vec![sample_record(1, 1)]);
        assert!(build_da_commitment_proof(&bundle, 1, 2).is_none());
    }
}
