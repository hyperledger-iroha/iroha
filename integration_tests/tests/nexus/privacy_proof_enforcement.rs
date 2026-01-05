//! End-to-end check that privacy proofs attached to transactions can satisfy
//! lane compliance policies requiring advertised commitments.

use std::{collections::BTreeSet, str::FromStr, sync::Arc};

use eyre::Result;
use iroha_core::{
    compliance::{LaneComplianceContext, LaneComplianceEngine, LaneComplianceEvaluation},
    governance::manifest::LaneManifestStatus,
    interlane::{LanePrivacyRegistry, verify_lane_privacy_proofs},
};
use iroha_crypto::{
    Hash, MerkleTree,
    privacy::{LaneCommitmentId, LanePrivacyCommitment, MerkleCommitment},
};
use iroha_data_model::{
    account::AccountId,
    nexus::{
        DataSpaceId, JurisdictionSet, LaneCompliancePolicy, LaneCompliancePolicyId,
        LaneComplianceRule, LaneId, LaneStorageProfile, LaneVisibility, ParticipantSelector,
        LanePrivacyMerkleWitness, LanePrivacyProof, LanePrivacyWitness,
    },
};
use iroha_test_samples::ALICE_ID;

#[test]
fn lane_privacy_proof_allows_compliance() -> Result<()> {
    let lane_id = LaneId::new(5);

    // Build a manifest status with a registered Merkle commitment.
    let mut leaves = Vec::new();
    leaves.extend_from_slice(&[0xAA_u8; 32]);
    leaves.extend_from_slice(&[0xBB_u8; 32]);
    let tree = MerkleTree::<[u8; 32]>::from_byte_chunks(&leaves, 32).expect("valid chunk");
    let merkle_root = tree.root().expect("merkle root");
    let merkle_proof = tree.get_proof(0).expect("merkle proof");
    let first_leaf_hash: [u8; 32] = tree
        .leaves()
        .next()
        .expect("leaf hash")
        .as_ref()
        .as_ref()
        .try_into()
        .expect("hash length");

    let commitment_id = LaneCommitmentId::new(9);
    let manifest = LaneManifestStatus {
        lane: lane_id,
        alias: "confidential-lane".to_string(),
        dataspace: DataSpaceId::GLOBAL,
        visibility: LaneVisibility::Public,
        storage: LaneStorageProfile::CommitmentOnly,
        governance: None,
        manifest_path: None,
        governance_rules: None,
        privacy_commitments: vec![LanePrivacyCommitment::merkle(
            commitment_id,
            MerkleCommitment::new(merkle_root, 8),
        )],
    };
    let registry = LanePrivacyRegistry::from_statuses(&[manifest]);

    // Attach a matching privacy proof and verify it against the registry.
    let proof = LanePrivacyProof {
        commitment_id,
        witness: LanePrivacyWitness::Merkle(LanePrivacyMerkleWitness {
            leaf: first_leaf_hash,
            proof: merkle_proof,
        }),
    };
    let verified = verify_lane_privacy_proofs(&registry, lane_id, &[proof])?;
    assert!(
        verified.contains(&commitment_id),
        "expected verified commitments to include the attachment id"
    );

    // Build a lane compliance policy that requires the same commitment id.
    let policy = LaneCompliancePolicy {
        id: LaneCompliancePolicyId::new(Hash::prehashed([0x11; 32])),
        version: 1,
        lane_id,
        dataspace_id: DataSpaceId::GLOBAL,
        jurisdiction: JurisdictionSet::default(),
        deny: Vec::new(),
        allow: vec![LaneComplianceRule {
            selector: ParticipantSelector {
                account: Some(AccountId::from_str(&ALICE_ID.to_string())?),
                privacy_commitments_any_of: vec![commitment_id],
                ..ParticipantSelector::default()
            },
            reason_code: Some("privacy proof attached".to_string()),
            jurisdiction_override: JurisdictionSet::default(),
        }],
        transfer_limits: Vec::new(),
        audit_controls: Default::default(),
        metadata: Default::default(),
    };
    let engine = LaneComplianceEngine::from_policies(vec![policy], false)?;

    // Evaluate with a verified commitment.
    let verified_set = verified;
    let ctx = LaneComplianceContext {
        lane_id,
        dataspace_id: DataSpaceId::GLOBAL,
        authority: &ALICE_ID,
        uaid: None,
        capability_tags: &[],
        lane_privacy_registry: Some(Arc::new(registry.clone())),
        verified_privacy_commitments: &verified_set,
    };
    assert!(matches!(
        engine.evaluate(&ctx),
        LaneComplianceEvaluation::Allowed(_)
    ));

    // Evaluate with no proof to ensure the policy denies the transaction.
    let empty_verified = BTreeSet::new();
    let ctx_missing_proof = LaneComplianceContext {
        verified_privacy_commitments: &empty_verified,
        ..ctx
    };
    assert!(matches!(
        engine.evaluate(&ctx_missing_proof),
        LaneComplianceEvaluation::Denied(_)
    ));

    Ok(())
}
