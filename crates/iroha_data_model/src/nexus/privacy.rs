//! Lane privacy proof attachments for Nexus private lanes.
//!
//! These helpers let transactions attach domain-separated Merkle witnesses
//! that can be verified against the lane privacy registry at admission time.

use iroha_crypto::{
    Hash, HashOf, MerkleProof,
    privacy::{LaneCommitmentId, MerkleWitness, PrivacyWitness},
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
    /// Witness payload proving membership.
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
    pub fn as_privacy_witness(&self) -> PrivacyWitness {
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
    /// `leaf` is the raw 32-byte lane leaf. Each `audit_path` entry is an
    /// already-hashed sibling digest and is wrapped with [`Hash::prehashed`]
    /// to preserve the canonical hash representation expected by the runtime.
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
        if let Some(index) = audit_path.iter().position(Option::is_none) {
            return Err(LanePrivacyProofError::MissingMerkleSibling { index });
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
                leaf,
                proof: MerkleProof::from_audit_path(leaf_index, audit_path),
            }),
        })
    }
}

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
}

impl LanePrivacyWitness {
    /// Convert the attachment to a runtime witness suitable for verification.
    #[must_use]
    pub fn as_privacy_witness(&self) -> PrivacyWitness {
        match self {
            Self::Merkle(witness) => {
                let witness = MerkleWitness::from_leaf_bytes(witness.leaf, witness.proof.clone());
                PrivacyWitness::Merkle(witness)
            }
        }
    }
}

/// Errors constructing [`LanePrivacyProof`] instances.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum LanePrivacyProofError {
    /// Merkle proofs must carry at least one sibling entry.
    #[error("merkle path must not be empty")]
    EmptyMerklePath,
    /// Lane privacy Merkle paths must contain a sibling at every level.
    #[error("merkle path is missing sibling at index {index}")]
    MissingMerkleSibling {
        /// Zero-based path entry containing `None`.
        index: usize,
    },
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{
        MerkleTree,
        privacy::{LanePrivacyCommitment, MerkleCommitment, PrivacyError},
    };

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

        let LanePrivacyWitness::Merkle(witness) = built.witness;
        assert_eq!(witness.proof.leaf_index(), proof.leaf_index());
        assert_eq!(witness.proof.audit_path().len(), proof.audit_path().len());
        assert_eq!(witness.leaf.len(), 32);
        assert_eq!(witness.leaf, leaf, "raw leaf bytes must be preserved");

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

    #[test]
    fn merkle_from_raw_path_rejects_missing_sibling() {
        let err = LanePrivacyProof::merkle_from_raw_path(
            LaneCommitmentId::new(1),
            [0_u8; 32],
            0,
            vec![None],
        )
        .expect_err("sparse lane privacy paths must be rejected");
        assert_eq!(
            err,
            LanePrivacyProofError::MissingMerkleSibling { index: 0 }
        );
    }

    #[test]
    fn decoded_empty_merkle_path_is_rejected_by_runtime_verifier() {
        let wire = LanePrivacyProof {
            commitment_id: LaneCommitmentId::new(3),
            witness: LanePrivacyWitness::Merkle(LanePrivacyMerkleWitness {
                leaf: [0xAA; 32],
                proof: MerkleProof::from_audit_path_bytes(0, Vec::new()),
            }),
        };
        let bytes = norito::to_bytes(&wire).expect("encode lane privacy proof");
        let archived =
            norito::from_bytes::<LanePrivacyProof>(&bytes).expect("decode lane privacy proof");
        let decoded: LanePrivacyProof = norito::core::NoritoDeserialize::deserialize(archived);
        let commitment = LanePrivacyCommitment::merkle(
            LaneCommitmentId::new(3),
            MerkleCommitment::from_root_bytes([0xAA; 32], 8),
        );
        assert_eq!(
            commitment
                .verify(decoded.as_privacy_witness())
                .expect_err("decoded empty path must fail at the verifier"),
            PrivacyError::EmptyMerkleProof
        );
    }

    #[cfg(feature = "json")]
    #[test]
    fn removed_snark_witness_is_rejected_by_json_decoder() {
        let stale = r#"{"kind":"snark","payload":{}}"#;
        assert!(
            norito::json::from_str::<LanePrivacyWitness>(stale).is_err(),
            "the data model must not decode the removed hash-only SNARK witness"
        );
    }
}
