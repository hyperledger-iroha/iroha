//! Lane privacy proof attachments for Nexus private lanes.
//!
//! These helpers let transactions attach Merkle or zk-SNARK witnesses that can
//! be verified against the lane privacy registry at admission time.

use iroha_crypto::{
    Hash, HashOf, MerkleProof,
    privacy::{LaneCommitmentId, MerkleWitness, PrivacyWitness, SnarkWitness},
};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

/// Proof payload bound to a specific lane commitment identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[norito(reuse_archived)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LanePrivacyProof {
    /// Commitment identifier advertised by the lane manifest.
    pub commitment_id: LaneCommitmentId,
    /// Witness payload proving membership or circuit validity.
    pub witness: LanePrivacyWitness,
}

impl LanePrivacyProof {
    /// Access the commitment identifier referenced by this proof.
    #[must_use]
    pub const fn commitment_id(&self) -> LaneCommitmentId {
        self.commitment_id
    }

    /// Convert to the runtime witness representation for verification.
    #[must_use]
    pub fn as_privacy_witness(&self) -> PrivacyWitness<'_> {
        self.witness.as_privacy_witness()
    }

    /// Size of the encoded proof in bytes (used for attachment budget checks).
    #[must_use]
    pub fn encoded_len(&self) -> usize {
        norito::to_bytes(self)
            .map(|bytes| bytes.len())
            .unwrap_or(usize::MAX)
    }

    /// Construct a Merkle-based lane privacy proof from raw sibling hashes.
    ///
    /// The `leaf` and each `audit_path` entry are treated as already-hashed
    /// digests; they are wrapped with [`Hash::prehashed`] to preserve the bit
    /// layout expected by the runtime.
    ///
    /// # Errors
    ///
    /// Returns [`LanePrivacyProofError::EmptyMerklePath`] when no siblings are
    /// provided.
    pub fn merkle_from_raw_path(
        commitment_id: LaneCommitmentId,
        leaf: [u8; 32],
        leaf_index: u32,
        audit_path: Vec<Option<[u8; 32]>>,
    ) -> Result<Self, LanePrivacyProofError> {
        if audit_path.is_empty() {
            return Err(LanePrivacyProofError::EmptyMerklePath);
        }

        let audit_path = audit_path
            .into_iter()
            .map(|entry| {
                entry
                    .map(|bytes| HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(bytes)))
            })
            .collect();

        Ok(Self {
            commitment_id,
            witness: LanePrivacyWitness::Merkle(LanePrivacyMerkleWitness {
                leaf: Hash::prehashed(leaf).into(),
                proof: MerkleProof::from_audit_path(leaf_index, audit_path),
            }),
        })
    }
}

/// Witness payload for a lane privacy proof.
/// Merkle witness payload for lane privacy proofs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[norito(reuse_archived)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LanePrivacyMerkleWitness {
    /// Leaf bytes used to derive the committed hash.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub leaf: [u8; 32],
    /// Inclusion path from the leaf to the committed root.
    pub proof: MerkleProof<[u8; 32]>,
}

/// zk-SNARK witness payload for lane privacy proofs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[norito(reuse_archived)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct LanePrivacySnarkWitness {
    /// Canonical encoding of the public inputs.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub public_inputs: Vec<u8>,
    /// Proof bytes emitted by the prover.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
    pub proof: Vec<u8>,
}

/// Witness payload for a lane privacy proof.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[norito(reuse_archived)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(tag = "kind", content = "payload"))]
pub enum LanePrivacyWitness {
    /// Merkle inclusion proof bound to a committed root.
    #[norito(rename = "merkle")]
    Merkle(LanePrivacyMerkleWitness),
    /// zk-SNARK witness bound to the circuit commitment.
    #[norito(rename = "snark")]
    Snark(LanePrivacySnarkWitness),
}

impl LanePrivacyWitness {
    /// Convert the attachment to a runtime witness suitable for verification.
    #[must_use]
    pub fn as_privacy_witness(&self) -> PrivacyWitness<'_> {
        match self {
            Self::Merkle(witness) => {
                let witness = MerkleWitness::from_leaf_bytes(witness.leaf, witness.proof.clone());
                PrivacyWitness::Merkle(witness)
            }
            Self::Snark(witness) => PrivacyWitness::Snark(SnarkWitness {
                public_inputs: &witness.public_inputs,
                proof: &witness.proof,
            }),
        }
    }
}

/// Errors constructing [`LanePrivacyProof`] instances.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum LanePrivacyProofError {
    /// Merkle proofs must carry at least one sibling entry.
    #[error("merkle path must not be empty")]
    EmptyMerklePath,
}

#[cfg(test)]
mod tests {
    use iroha_crypto::MerkleTree;

    use super::*;

    #[test]
    fn merkle_from_raw_path_sets_prehashed_bits() {
        let mut leaves = Vec::new();
        leaves.extend_from_slice(&[0xAA_u8; 32]);
        leaves.extend_from_slice(&[0xBB_u8; 32]);
        let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&leaves, 32).expect("valid chunk");
        let leaf: [u8; 32] = *tree.leaves().next().expect("merkle leaf present").as_ref();
        let proof = tree.get_proof(0).expect("merkle proof");

        let audit_path: Vec<Option<[u8; 32]>> = proof
            .audit_path()
            .iter()
            .map(|entry| entry.map(|hash| *hash.as_ref()))
            .collect();

        let built = LanePrivacyProof::merkle_from_raw_path(
            LaneCommitmentId::new(7),
            leaf,
            proof.leaf_index(),
            audit_path.clone(),
        )
        .expect("builder should succeed");

        let LanePrivacyWitness::Merkle(witness) = built.witness else {
            panic!("expected merkle witness")
        };
        assert_eq!(witness.proof.leaf_index(), proof.leaf_index());
        assert_eq!(witness.proof.audit_path().len(), proof.audit_path().len());
        assert_eq!(witness.leaf.len(), 32);
        assert_eq!(
            witness.leaf[31] & 1,
            1,
            "lsb should be set by Hash::prehashed"
        );

        for (expected, actual) in audit_path.iter().zip(witness.proof.audit_path().iter()) {
            match (expected, actual) {
                (None, None) => {}
                (Some(raw), Some(wrapped)) => {
                    let wrapped_bytes: &[u8; 32] = wrapped.as_ref();
                    assert_eq!(wrapped_bytes.len(), 32);
                    assert_eq!(
                        wrapped_bytes[31] & 1,
                        1,
                        "wrapped siblings must have lsb set"
                    );
                    let mut raw_with_lsb = *raw;
                    raw_with_lsb[31] |= 1;
                    assert_eq!(
                        wrapped_bytes, &raw_with_lsb,
                        "sibling bytes should be preserved with lsb set"
                    );
                }
                _ => panic!("mismatched optional entries"),
            }
        }
    }

    #[test]
    fn merkle_from_raw_path_rejects_empty_path() {
        let err = LanePrivacyProof::merkle_from_raw_path(
            LaneCommitmentId::new(1),
            [0_u8; 32],
            0,
            Vec::new(),
        )
        .expect_err("empty path must be rejected");
        assert_eq!(err, LanePrivacyProofError::EmptyMerklePath);
    }
}
