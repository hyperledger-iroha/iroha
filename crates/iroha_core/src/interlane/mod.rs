//! Lane privacy commitment registry bridging manifest data to runtime users.
//!
//! This module wires the NX-10 domain-separated Merkle descriptors into a deterministic registry so
//! admission/compliance logic can look up the commitments advertised by each lane.
use iroha_crypto::privacy::{LaneCommitmentId, PrivacyError, PrivacyWitness};
use iroha_data_model::nexus::LanePrivacyProof;
use iroha_model_base::topology::LaneId;
use std::collections::BTreeSet;
use thiserror::Error;
mod registry;
pub use registry::{LanePrivacyLane, LanePrivacyRegistry};

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
    use super::*;
    use crate::governance::manifest::{LaneManifestRegistry, LaneManifestStatus};
    use iroha_allocation::AllocationBudget;
    use iroha_crypto::{
        MerkleProof,
        privacy::{
            LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment, MerkleWitness,
            PrivacyWitness,
        },
    };
    use iroha_data_model::nexus::{
        LanePrivacyMerkleWitness, LanePrivacyProof, LanePrivacyWitness, LaneStorageProfile,
        LaneVisibility,
    };
    use iroha_model_base::{topology::DataSpaceId, topology::LaneId};
    pub(super) fn status_with_commitments(
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
    pub(super) fn merkle_status_with_witness(
        lane: LaneId,
        dataspace: DataSpaceId,
        id: LaneCommitmentId,
    ) -> (LaneManifestStatus, MerkleWitness) {
        let witness = MerkleWitness::new(
            [0_u8; 32],
            MerkleProof::from_audit_path_bytes(0, vec![[1_u8; 32]]),
        );
        let root = witness
            .implied_root(8)
            .expect("valid lane proof must produce a root");
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
        let pool = AllocationBudget::new(1024 * 1024);
        let commitment_id = LaneCommitmentId::new(1);
        let (status, _) =
            merkle_status_with_witness(LaneId::new(7), DataSpaceId::new(99), commitment_id);
        let statuses = vec![
            status.clone(),
            status_with_commitments(LaneId::new(8), DataSpaceId::new(100), Vec::new()),
        ];
        let registry = LanePrivacyRegistry::try_from_statuses(&statuses, &pool).unwrap();
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
        let pool = AllocationBudget::new(1024 * 1024);
        let commitment_id = LaneCommitmentId::new(5);
        let (status, witness) =
            merkle_status_with_witness(LaneId::new(7), DataSpaceId::new(99), commitment_id);
        let statuses = vec![status];
        let registry = LanePrivacyRegistry::try_from_statuses(&statuses, &pool).unwrap();
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
                    MerkleProof::from_audit_path_bytes(0, vec![[3_u8; 32]]),
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
                    MerkleProof::from_audit_path_bytes(0, vec![[1_u8; 32]]),
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
        let pool = AllocationBudget::new(1024 * 1024);
        let commitment_id = LaneCommitmentId::new(11);
        let lane_id = LaneId::new(3);
        let (status, witness) =
            merkle_status_with_witness(lane_id, DataSpaceId::new(99), commitment_id);
        let registry = LanePrivacyRegistry::try_from_statuses(&[status], &pool).unwrap();
        let proof = LanePrivacyProof {
            commitment_id,
            witness: LanePrivacyWitness::Merkle(LanePrivacyMerkleWitness {
                leaf: *witness.leaf(),
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
        let pool = AllocationBudget::new(1024 * 1024);
        let commitment_id = LaneCommitmentId::new(12);
        let lane_id = LaneId::new(4);
        let (status, witness) =
            merkle_status_with_witness(lane_id, DataSpaceId::new(77), commitment_id);
        let registry = LanePrivacyRegistry::try_from_statuses(&[status], &pool).unwrap();
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

    #[test]
    fn manifest_privacy_projection_borrows_original_statuses_without_empty_graph_allocations() {
        let pool = AllocationBudget::new(1024 * 1024);
        let source = LaneManifestRegistry::from_statuses(
            [
                status_with_commitments(LaneId::new(9), DataSpaceId::new(90), Vec::new()),
                status_with_commitments(LaneId::new(3), DataSpaceId::new(30), Vec::new()),
            ]
            .into_iter()
            .map(|status| (status.lane, status))
            .collect(),
        );
        let mut borrowed = [std::ptr::null(); 2];
        let mut count = 0;
        let allocations = crate::test_allocations::allocations_during(|| {
            for status in source.iter_statuses() {
                borrowed[count] = std::ptr::from_ref(status);
                count += 1;
            }
        });
        assert_eq!(allocations, 0);
        assert_eq!(count, 2);
        assert_eq!(
            borrowed[0],
            std::ptr::from_ref(source.status(LaneId::new(3)).unwrap())
        );
        assert_eq!(
            borrowed[1],
            std::ptr::from_ref(source.status(LaneId::new(9)).unwrap())
        );

        let mut projected = None;
        let allocations = crate::test_allocations::allocations_during(|| {
            projected =
                Some(LanePrivacyRegistry::try_from_manifest_registry(&source, &pool).unwrap());
        });
        assert_eq!(
            allocations, 0,
            "empty privacy projection must not copy manifest metadata"
        );
        assert!(projected.unwrap().is_empty());
        assert!(source.status(LaneId::new(3)).is_some());
        assert!(source.status(LaneId::new(9)).is_some());
    }

    #[test]
    fn manifest_privacy_projection_matches_snapshot_with_fewer_allocations() {
        let pool = AllocationBudget::new(1024 * 1024);
        let lane = LaneId::new(7);
        let commitment = LaneCommitmentId::new(17);
        let (mut populated, witness) =
            merkle_status_with_witness(lane, DataSpaceId::new(99), commitment);
        populated.alias = "original-manifest-metadata-".repeat(64);
        let empty = status_with_commitments(LaneId::new(8), DataSpaceId::new(100), Vec::new());
        let source = LaneManifestRegistry::from_statuses(
            [(populated.lane, populated), (empty.lane, empty)]
                .into_iter()
                .collect(),
        );
        let original = std::ptr::from_ref(source.status(lane).unwrap());
        let mut snapshot_projection = None;
        let snapshot_allocations = crate::test_allocations::allocations_during(|| {
            let snapshot = source.statuses();
            snapshot_projection =
                Some(LanePrivacyRegistry::try_from_statuses(&snapshot, &pool).unwrap());
        });
        let mut borrowed_projection = None;
        let borrowed_allocations = crate::test_allocations::allocations_during(|| {
            borrowed_projection =
                Some(LanePrivacyRegistry::try_from_manifest_registry(&source, &pool).unwrap());
        });
        let borrowed_projection = borrowed_projection.unwrap();
        assert_eq!(borrowed_projection, snapshot_projection.unwrap());
        assert!(borrowed_allocations < snapshot_allocations);
        assert_eq!(std::ptr::from_ref(source.status(lane).unwrap()), original);
        assert!(borrowed_projection.lane(LaneId::new(8)).is_none());
        assert_eq!(
            borrowed_projection.lane(lane).unwrap().dataspace_id(),
            DataSpaceId::new(99)
        );
        borrowed_projection
            .verify(lane, commitment, PrivacyWitness::Merkle(witness))
            .expect("borrowed projection retains the original valid commitment");
    }

    #[test]
    fn borrowed_privacy_statuses_keep_last_nonempty_lane_and_commitment() {
        let pool = AllocationBudget::new(1024 * 1024);
        let lane = LaneId::new(7);
        let id = LaneCommitmentId::new(19);
        let first =
            LanePrivacyCommitment::merkle(id, MerkleCommitment::from_root_bytes([1; 32], 8));
        let last = LanePrivacyCommitment::merkle(id, MerkleCommitment::from_root_bytes([2; 32], 8));
        let statuses = [
            status_with_commitments(lane, DataSpaceId::new(98), vec![first]),
            status_with_commitments(lane, DataSpaceId::new(99), vec![first, last]),
            status_with_commitments(lane, DataSpaceId::new(100), Vec::new()),
        ];
        let registry = LanePrivacyRegistry::try_from_statuses(statuses.iter(), &pool).unwrap();
        let entry = registry.lane(lane).unwrap();
        assert_eq!(entry.dataspace_id(), DataSpaceId::new(99));
        assert_eq!(entry.get(id), Some(last));
        assert_eq!(entry.commitments().count(), 1);
    }
}
