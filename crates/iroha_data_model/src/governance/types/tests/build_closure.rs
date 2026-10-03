//! Typed verifier proposals retain authenticated release and registry bindings.

use super::*;
use crate::isi::governance::{
    ProposeKagemushaVerifierReleaseActivateV1, ProposeKagemushaVerifierReleaseInstallV1,
};

fn install_proposal() -> KagemushaVerifierReleaseInstallProposalV1 {
    norito::decode_canonical::<ProposeKagemushaVerifierReleaseInstallV1>(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/governance/kagemusha_verifier_release_install_v1.bin"
    )))
    .expect("canonical threshold-authenticated release fixture")
    .proposal
}

fn activate_proposal() -> KagemushaVerifierReleaseActivateProposalV1 {
    norito::decode_canonical::<ProposeKagemushaVerifierReleaseActivateV1>(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/governance/kagemusha_verifier_release_activate_v1.bin"
    )))
    .expect("canonical exact standby activation fixture")
    .proposal
}

#[test]
fn release_install_authenticates_original_evidence_and_preserves_predecessor() {
    let proposal = install_proposal();
    proposal.validate().expect("authenticated release");
    let before = proposal.expected_predecessor.clone();
    let successor = proposal.successor().expect("standby successor");
    assert_eq!(proposal.expected_predecessor, before);
    assert_eq!(successor.active_release_id, None);
    let release = successor
        .releases
        .iter()
        .find(|row| row.release_id == proposal.manifest.release_id)
        .expect("the exact authenticated release is retained");
    assert_eq!(
        release.status,
        crate::kagemusha::KAGEMUSHA_RELEASE_STANDBY_V1
    );
    assert_eq!(
        proposal.first_release_exact_json_u64_invariant_error(u64::MAX),
        None
    );
    assert!(
        proposal
            .first_release_exact_json_u64_invariant_error(0)
            .is_some()
    );
    let kind = ProposalKind::KagemushaVerifierReleaseInstall(proposal.clone());
    let wire = norito::encode_canonical(&kind).unwrap();
    assert_eq!(
        norito::decode_canonical::<ProposalKind>(&wire).unwrap(),
        kind
    );
    let json = norito::json::to_json(&kind).unwrap();
    assert_eq!(
        norito::json::from_json::<ProposalKind>(&json).unwrap(),
        kind
    );

    let mut foreign = proposal.clone();
    foreign.network_id = NetworkId::from_genesis_hash(iroha_crypto::HashOf::<
        crate::block::BlockHeader,
    >::from_untyped_unchecked(
        iroha_crypto::Hash::new(b"foreign verifier release network"),
    ));
    assert_ne!(foreign.network_id, foreign.manifest.network_id);
    assert!(foreign.successor().is_err());
    let mut forged = proposal.clone();
    forged.manifest.source_tree_digest[0] ^= 1;
    assert!(forged.validate().is_err());
    let mut ungoverned = proposal.clone();
    ungoverned.expected_predecessor = KagemushaGovernedVerifierRegistryV1::default();
    assert!(ungoverned.successor().is_err());
    let mut replay = proposal;
    replay.expected_predecessor = successor;
    assert!(replay.successor().is_err());
}

#[test]
fn release_activation_binds_sole_standby_and_rejects_replay() {
    let proposal = activate_proposal();
    proposal.validate().expect("exact first activation");
    let before = proposal.expected_predecessor.clone();
    let successor = proposal.successor().expect("active successor");
    assert_eq!(proposal.expected_predecessor, before);
    assert_eq!(
        successor.active_release_id,
        Some(proposal.successor_release_id)
    );
    assert_eq!(successor.releases.len(), 1);
    assert_eq!(
        successor.releases[0].status,
        crate::kagemusha::KAGEMUSHA_RELEASE_ACTIVE_V1
    );
    let kind = ProposalKind::KagemushaVerifierReleaseActivate(proposal.clone());
    let wire = norito::encode_canonical(&kind).unwrap();
    assert_eq!(
        norito::decode_canonical::<ProposalKind>(&wire).unwrap(),
        kind
    );
    let json = norito::json::to_json(&kind).unwrap();
    assert_eq!(
        norito::json::from_json::<ProposalKind>(&json).unwrap(),
        kind
    );

    let mut absent = proposal.clone();
    absent.successor_release_id[0] ^= 1;
    assert!(absent.validate().is_err());
    let mut ungoverned = proposal.clone();
    ungoverned.expected_predecessor = KagemushaGovernedVerifierRegistryV1::default();
    assert!(ungoverned.successor().is_err());
    let mut replay = proposal;
    replay.expected_predecessor = successor;
    assert!(replay.successor().is_err());
}

#[test]
fn verifier_registry_subject_is_unique_and_shared_by_all_three_proposals() {
    let install = install_proposal();
    let mut policy = kagemusha_policy_install_proposal();
    policy.network_id = install.network_id;
    let mut activate = activate_proposal();
    activate.network_id = install.network_id;
    let subject = GovernanceSubjectPreimageV1::KagemushaVerifierRegistry(install.network_id);
    let encoded = subject.encode();
    assert_eq!(&encoded[..4], &13_u32.to_le_bytes());
    assert_eq!(
        &GovernanceSubjectPreimageV1::SorafsAdmissionCouncil.encode()[..4],
        &11_u32.to_le_bytes()
    );
    assert_eq!(
        &GovernanceSubjectPreimageV1::SorafsProviderAdmission(crate::sorafs::capacity::ProviderId(
            [7; 32]
        ))
        .encode()[..4],
        &12_u32.to_le_bytes()
    );
    let subject = crate::governance_fingerprint::fingerprint(
        crate::governance_fingerprint::GOVERNANCE_SUBJECT_ID_V1,
        &subject,
    );
    for kind in [
        ProposalKind::KagemushaVerifierPolicyInstall(policy),
        ProposalKind::KagemushaVerifierReleaseInstall(install),
        ProposalKind::KagemushaVerifierReleaseActivate(activate),
    ] {
        assert_eq!(kind.governed_subject_id_v1().unwrap(), subject);
        assert!(kind.proposal_operator_v1().is_some());
    }
}

#[path = "retirement.rs"]
mod retirement;
