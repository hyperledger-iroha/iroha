//! Lane privacy commitment registry bridging manifest data to runtime users.
//!
//! This module wires the NX-10 privacy descriptors (Merkle roots and zk-SNARK
//! hashes) into a deterministic registry so admission/compliance logic can look
//! up the commitments advertised by each lane.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use iroha_crypto::privacy::{
    LaneCommitmentId, LanePrivacyCommitment, PrivacyError, PrivacyWitness,
};
use iroha_data_model::nexus::{DataSpaceId, LaneId, LanePrivacyProof};
use thiserror::Error;

use crate::governance::manifest::{LaneManifestRegistry, LaneManifestStatus};

/// Registry of per-lane privacy commitments derived from governance manifests.
#[derive(Debug, Clone, Default)]
pub struct LanePrivacyRegistry {
    entries: BTreeMap<LaneId, LanePrivacyLane>,
}

/// Shared handle to a [`LanePrivacyRegistry`].
pub type LanePrivacyRegistryHandle = Arc<LanePrivacyRegistry>;

impl LanePrivacyRegistry {
    /// Construct an empty registry.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Whether the registry has no recorded entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Build the registry by snapshotting the provided manifest statuses.
    #[must_use]
    pub fn from_statuses(statuses: &[LaneManifestStatus]) -> Self {
        let mut entries = BTreeMap::new();
        for status in statuses {
            if status.privacy_commitments.is_empty() {
                continue;
            }
            entries.insert(status.lane, LanePrivacyLane::from_status(status));
        }
        Self { entries }
    }

    /// Build the registry directly from the manifest registry helper.
    #[must_use]
    pub fn from_manifest_registry(registry: &LaneManifestRegistry) -> Self {
        let statuses = registry.statuses();
        Self::from_statuses(&statuses)
    }

    /// Borrow the entry recorded for `lane_id`, if any.
    pub fn lane(&self, lane_id: LaneId) -> Option<&LanePrivacyLane> {
        self.entries.get(&lane_id)
    }

    /// Verify the provided witness against the registered commitment.
    ///
    /// # Errors
    ///
    /// Returns [`LanePrivacyRegistryError::LaneMissing`] when the lane does not
    /// advertise commitments, [`LanePrivacyRegistryError::UnknownCommitment`]
    /// when the identifier is not registered, or propagates cryptographic
    /// verification failures via
    /// [`LanePrivacyRegistryError::Verification`].
    pub fn verify(
        &self,
        lane_id: LaneId,
        commitment_id: LaneCommitmentId,
        witness: PrivacyWitness<'_>,
    ) -> Result<(), LanePrivacyRegistryError> {
        let entry = self
            .entries
            .get(&lane_id)
            .ok_or(LanePrivacyRegistryError::LaneMissing { lane_id })?;
        let commitment = entry.commitments.get(&commitment_id).copied().ok_or(
            LanePrivacyRegistryError::UnknownCommitment {
                lane_id,
                commitment_id,
            },
        )?;
        commitment
            .verify(witness)
            .map_err(|source| LanePrivacyRegistryError::Verification {
                lane_id,
                commitment_id,
                source,
            })
    }
}

/// Verify the supplied privacy proofs against the lane registry, returning the
/// set of commitment identifiers proven for `lane_id`.
///
/// # Errors
/// Propagates [`LanePrivacyRegistryError`] if the lane is missing, a
/// commitment is unknown, or a witness fails verification.
pub fn verify_lane_privacy_proofs(
    registry: &LanePrivacyRegistry,
    lane_id: LaneId,
    proofs: &[LanePrivacyProof],
) -> Result<BTreeSet<LaneCommitmentId>, LanePrivacyRegistryError> {
    let mut verified = BTreeSet::new();
    for proof in proofs {
        registry.verify(lane_id, proof.commitment_id(), proof.as_privacy_witness())?;
        verified.insert(proof.commitment_id());
    }
    Ok(verified)
}

/// Per-lane commitment snapshot derived from manifests.
#[derive(Debug, Clone)]
pub struct LanePrivacyLane {
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    commitments: BTreeMap<LaneCommitmentId, LanePrivacyCommitment>,
}

impl LanePrivacyLane {
    fn from_status(status: &LaneManifestStatus) -> Self {
        let commitments = status
            .privacy_commitments
            .iter()
            .copied()
            .map(|commitment| (commitment.id(), commitment))
            .collect();
        Self {
            lane_id: status.lane,
            dataspace_id: status.dataspace,
            commitments,
        }
    }

    /// Lane identifier associated with this entry.
    #[must_use]
    pub const fn lane_id(&self) -> LaneId {
        self.lane_id
    }

    /// Dataspace identifier advertised by the manifest.
    #[must_use]
    pub const fn dataspace_id(&self) -> DataSpaceId {
        self.dataspace_id
    }

    /// Iterator over the registered commitments.
    pub fn commitments(&self) -> impl Iterator<Item = LanePrivacyCommitment> + '_ {
        self.commitments.values().copied()
    }

    /// Retrieve a specific commitment by identifier.
    pub fn get(&self, id: LaneCommitmentId) -> Option<LanePrivacyCommitment> {
        self.commitments.get(&id).copied()
    }

    /// Whether the lane recorded any privacy commitments.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.commitments.is_empty()
    }
}

/// Errors surfaced when querying or verifying lane commitments.
#[derive(Debug, Error, PartialEq, Eq, Clone, Copy)]
pub enum LanePrivacyRegistryError {
    /// Lane has not advertised any privacy commitments.
    #[error("lane {lane_id} does not advertise privacy commitments")]
    LaneMissing {
        /// Identifier for the missing lane.
        lane_id: LaneId,
    },
    /// Commitment identifier not present in the manifest registry.
    #[error("commitment {commitment_id} not registered for lane {lane_id}")]
    UnknownCommitment {
        /// Lane identifier.
        lane_id: LaneId,
        /// Commitment identifier looked up.
        commitment_id: LaneCommitmentId,
    },
    /// Cryptographic verification failure.
    #[error("privacy verification failed for lane {lane_id} commitment {commitment_id}: {source}")]
    Verification {
        /// Lane identifier.
        lane_id: LaneId,
        /// Commitment identifier.
        commitment_id: LaneCommitmentId,
        /// Underlying proof error.
        #[source]
        source: PrivacyError,
    },
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{
        HashOf, MerkleTree,
        privacy::{
            LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment, MerkleWitness,
            PrivacyWitness,
        },
    };
    use iroha_data_model::nexus::{
        DataSpaceId, LaneId, LanePrivacyMerkleWitness, LanePrivacyProof, LanePrivacyWitness,
        LaneStorageProfile, LaneVisibility,
    };

    use super::*;

    fn status_with_commitments(
        lane: LaneId,
        dataspace: DataSpaceId,
        commitments: Vec<LanePrivacyCommitment>,
    ) -> LaneManifestStatus {
        LaneManifestStatus {
            lane,
            alias: format!("lane-{}", lane.as_u32()),
            dataspace,
            visibility: LaneVisibility::default(),
            storage: LaneStorageProfile::CommitmentOnly,
            governance: None,
            manifest_path: None,
            governance_rules: None,
            privacy_commitments: commitments,
        }
    }

    fn merkle_status_with_witness(
        lane: LaneId,
        dataspace: DataSpaceId,
        id: LaneCommitmentId,
    ) -> (LaneManifestStatus, MerkleWitness) {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&[0_u8; 32]);
        bytes.extend_from_slice(&[1_u8; 32]);
        let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&bytes, 32).expect("valid chunk");
        let root = tree.root().expect("merkle root");
        let proof = tree.get_proof(0).expect("merkle proof");
        let first_leaf_raw = *tree.leaves().next().expect("leaf iterator");
        let first_leaf_hash = HashOf::<[u8; 32]>::from_untyped_unchecked(first_leaf_raw);
        let witness = MerkleWitness::new(first_leaf_hash, proof);
        let status = status_with_commitments(
            lane,
            dataspace,
            vec![LanePrivacyCommitment::merkle(
                id,
                MerkleCommitment::new(root, 8),
            )],
        );
        (status, witness)
    }

    #[test]
    fn registry_tracks_entries_from_statuses() {
        let commitment_id = LaneCommitmentId::new(1);
        let (status, _) =
            merkle_status_with_witness(LaneId::new(7), DataSpaceId::new(99), commitment_id);
        let statuses = vec![
            status.clone(),
            status_with_commitments(LaneId::new(8), DataSpaceId::new(100), Vec::new()),
        ];
        let registry = LanePrivacyRegistry::from_statuses(&statuses);
        let entry = registry.lane(status.lane).expect("lane entry");
        assert_eq!(entry.lane_id(), status.lane);
        assert_eq!(entry.dataspace_id(), status.dataspace);
        assert_eq!(
            entry.commitments().collect::<Vec<_>>(),
            status.privacy_commitments.clone()
        );
        assert!(registry.lane(LaneId::new(8)).is_none());
    }

    #[test]
    fn registry_verifies_merkle_commitments() {
        let commitment_id = LaneCommitmentId::new(5);
        let (status, witness) =
            merkle_status_with_witness(LaneId::new(7), DataSpaceId::new(99), commitment_id);
        let statuses = vec![status];
        let registry = LanePrivacyRegistry::from_statuses(&statuses);
        registry
            .verify(
                LaneId::new(7),
                commitment_id,
                PrivacyWitness::Merkle(witness),
            )
            .expect("valid witness");
        let err = registry
            .verify(
                LaneId::new(7),
                LaneCommitmentId::new(9),
                PrivacyWitness::Merkle(MerkleWitness::from_leaf_bytes(
                    [2_u8; 32],
                    MerkleTree::<[u8; 32]>::from_byte_chunks(&[2_u8; 32], 32)
                        .expect("valid chunk")
                        .get_proof(0)
                        .unwrap(),
                )),
            )
            .expect_err("unknown commitment");
        assert!(matches!(
            err,
            LanePrivacyRegistryError::UnknownCommitment { .. }
        ));
        let missing_lane = registry
            .verify(
                LaneId::new(9),
                commitment_id,
                PrivacyWitness::Merkle(MerkleWitness::from_leaf_bytes(
                    [0_u8; 32],
                    MerkleTree::<[u8; 32]>::from_byte_chunks(&[0_u8; 32], 32)
                        .expect("valid chunk")
                        .get_proof(0)
                        .unwrap(),
                )),
            )
            .expect_err("missing lane");
        assert!(matches!(
            missing_lane,
            LanePrivacyRegistryError::LaneMissing { .. }
        ));
    }

    #[test]
    fn verify_lane_privacy_proofs_accepts_merkle_attachment() {
        let commitment_id = LaneCommitmentId::new(11);
        let lane_id = LaneId::new(3);
        let (status, witness) =
            merkle_status_with_witness(lane_id, DataSpaceId::new(99), commitment_id);
        let registry = LanePrivacyRegistry::from_statuses(&[status]);
        let mut leaf_bytes = [0u8; 32];
        leaf_bytes.copy_from_slice(witness.leaf_hash().as_ref().as_ref());
        let proof = LanePrivacyProof {
            commitment_id,
            witness: LanePrivacyWitness::Merkle(LanePrivacyMerkleWitness {
                leaf: leaf_bytes,
                proof: witness.proof().clone(),
            }),
        };
        let verified = verify_lane_privacy_proofs(&registry, lane_id, std::slice::from_ref(&proof))
            .expect("verification must pass");
        assert_eq!(verified.len(), 1);
        assert!(verified.contains(&commitment_id));
    }

    #[test]
    fn verify_lane_privacy_proofs_rejects_malformed_attachment() {
        let commitment_id = LaneCommitmentId::new(12);
        let lane_id = LaneId::new(4);
        let (status, witness) =
            merkle_status_with_witness(lane_id, DataSpaceId::new(77), commitment_id);
        let registry = LanePrivacyRegistry::from_statuses(&[status]);
        let proof = LanePrivacyProof {
            commitment_id,
            witness: LanePrivacyWitness::Merkle(LanePrivacyMerkleWitness {
                leaf: [0xFF; 32],
                proof: witness.proof().clone(),
            }),
        };
        let err = verify_lane_privacy_proofs(&registry, lane_id, std::slice::from_ref(&proof))
            .expect_err("invalid leaf must fail verification");
        assert!(matches!(
            err,
            LanePrivacyRegistryError::Verification {
                commitment_id: cid,
                ..
            } if cid == commitment_id
        ));
    }
}
