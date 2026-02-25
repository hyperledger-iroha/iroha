//! Offline multi-allowance merge proof scaffolding.
//!
//! This module provides deterministic inputs/outputs for the offline merge lane.
//! The current implementation keeps a compact canonical transcript hash that is
//! ready to be wrapped by a recursive transparent STARK envelope.

use sha2::{Digest, Sha256};

/// Canonical witness leaf representing one merged certificate lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, norito::NoritoSerialize, norito::NoritoDeserialize)]
pub struct OfflineMergeLeaf {
    /// Sender certificate identifier (32-byte hash).
    pub certificate_id: [u8; 32],
    /// Poseidon root for receipts tied to this certificate.
    pub receipts_root: [u8; 32],
    /// Claimed deposited amount for this certificate.
    pub claimed_delta: u128,
}

/// Witness payload for the offline merge circuit.
#[derive(Debug, Clone, PartialEq, Eq, norito::NoritoSerialize, norito::NoritoDeserialize)]
pub struct OfflineMergeWitness {
    /// Bundle identifier for domain separation.
    pub bundle_id: [u8; 32],
    /// Leaves included in this merge.
    pub leaves: Vec<OfflineMergeLeaf>,
}

/// Deterministic merge artifact that can be embedded into a recursive STARK.
#[derive(Debug, Clone, Copy, PartialEq, Eq, norito::NoritoSerialize, norito::NoritoDeserialize)]
pub struct OfflineMergeArtifact {
    /// Canonical transcript digest over the witness.
    pub transcript_digest: [u8; 32],
    /// Number of recursive folds required by the planner.
    pub recursion_depth: u32,
}

/// Build the deterministic merge artifact for an offline bundle.
///
/// The transcript is deterministic and transparent; no trusted setup artifacts
/// are required for this lane.
#[must_use]
pub fn build_offline_merge_artifact(witness: &OfflineMergeWitness) -> OfflineMergeArtifact {
    let mut hasher = Sha256::new();
    hasher.update(b"fastpq:offline-merge:v1");
    hasher.update(witness.bundle_id);
    hasher.update(
        u64::try_from(witness.leaves.len())
            .unwrap_or(u64::MAX)
            .to_le_bytes(),
    );

    let mut leaves = witness.leaves.clone();
    leaves.sort_by_key(|leaf| leaf.certificate_id);
    for leaf in &leaves {
        hasher.update(leaf.certificate_id);
        hasher.update(leaf.receipts_root);
        hasher.update(leaf.claimed_delta.to_le_bytes());
    }

    let digest = hasher.finalize();
    let mut transcript_digest = [0u8; 32];
    transcript_digest.copy_from_slice(&digest);

    let recursion_depth = if leaves.is_empty() {
        0
    } else {
        leaves
            .len()
            .next_power_of_two()
            .trailing_zeros()
            .max(1)
    };
    OfflineMergeArtifact {
        transcript_digest,
        recursion_depth,
    }
}

/// Verify that an artifact matches the provided witness.
#[must_use]
pub fn verify_offline_merge_artifact(
    witness: &OfflineMergeWitness,
    artifact: &OfflineMergeArtifact,
) -> bool {
    let expected = build_offline_merge_artifact(witness);
    expected == *artifact
}

#[cfg(test)]
mod tests {
    use super::{
        OfflineMergeArtifact, OfflineMergeLeaf, OfflineMergeWitness, build_offline_merge_artifact,
        verify_offline_merge_artifact,
    };

    #[test]
    fn offline_merge_artifact_is_deterministic() {
        let witness = OfflineMergeWitness {
            bundle_id: [0x11; 32],
            leaves: vec![
                OfflineMergeLeaf {
                    certificate_id: [0xB2; 32],
                    receipts_root: [0xD3; 32],
                    claimed_delta: 2_000,
                },
                OfflineMergeLeaf {
                    certificate_id: [0xA1; 32],
                    receipts_root: [0xC2; 32],
                    claimed_delta: 1_000,
                },
            ],
        };

        let first = build_offline_merge_artifact(&witness);
        let second = build_offline_merge_artifact(&witness);
        assert_eq!(first, second);
        assert!(verify_offline_merge_artifact(&witness, &first));
    }

    #[test]
    fn offline_merge_artifact_detects_mutation() {
        let witness = OfflineMergeWitness {
            bundle_id: [0x44; 32],
            leaves: vec![OfflineMergeLeaf {
                certificate_id: [0x10; 32],
                receipts_root: [0x20; 32],
                claimed_delta: 3_000,
            }],
        };
        let mut artifact = build_offline_merge_artifact(&witness);
        artifact.transcript_digest[0] ^= 0x01;
        assert!(!verify_offline_merge_artifact(&witness, &artifact));
    }

    #[test]
    fn recursion_depth_tracks_leaf_count() {
        let witness = OfflineMergeWitness {
            bundle_id: [0x22; 32],
            leaves: vec![
                OfflineMergeLeaf {
                    certificate_id: [0x01; 32],
                    receipts_root: [0xA1; 32],
                    claimed_delta: 1,
                },
                OfflineMergeLeaf {
                    certificate_id: [0x02; 32],
                    receipts_root: [0xA2; 32],
                    claimed_delta: 1,
                },
                OfflineMergeLeaf {
                    certificate_id: [0x03; 32],
                    receipts_root: [0xA3; 32],
                    claimed_delta: 1,
                },
            ],
        };
        let artifact = build_offline_merge_artifact(&witness);
        assert_eq!(
            artifact,
            OfflineMergeArtifact {
                transcript_digest: artifact.transcript_digest,
                recursion_depth: 2,
            }
        );
    }
}
