//! Exact typed standby retirement, with canonical originals and closed refusals.

use super::*;
use crate::isi::governance::ProposeKagemushaVerifierReleaseRetireV1;

fn retirement_proposal() -> KagemushaVerifierReleaseRetireProposalV1 {
    let activate = activate_proposal();
    KagemushaVerifierReleaseRetireProposalV1 {
        proposal_operator: activate.proposal_operator,
        network_id: activate.network_id,
        expected_predecessor: activate.expected_predecessor,
        standby_release_id: activate.successor_release_id,
    }
}

#[test]
fn exact_standby_retirement_preserves_registry_and_roundtrips_canonical_wire() {
    let proposal = retirement_proposal();
    let before = proposal.expected_predecessor.clone();
    let successor = proposal.successor().unwrap();
    assert_eq!(proposal.expected_predecessor, before);
    let mut expected = before.clone();
    expected
        .retire_standby(proposal.standby_release_id)
        .unwrap();
    assert_eq!(successor, expected);
    assert_eq!(successor.authority_policy, before.authority_policy);
    assert_eq!(successor.active_release_id, before.active_release_id);
    assert!(successor.releases.is_empty());
    let kind = ProposalKind::KagemushaVerifierReleaseRetire(proposal.clone());
    assert_eq!(&kind.encode()[..4], &13_u32.to_le_bytes());
    assert_eq!(kind.first_release_exact_json_u64_invariant_error(), None);
    assert_eq!(
        kind.proposal_operator_v1(),
        Some(&proposal.proposal_operator)
    );
    let frame = norito::encode_canonical(&kind).unwrap();
    assert_eq!(
        norito::decode_canonical::<ProposalKind>(&frame).unwrap(),
        kind
    );
    let json = norito::json::to_json(&kind).unwrap();
    assert_eq!(
        norito::json::from_json::<ProposalKind>(&json).unwrap(),
        kind
    );
    let instruction = ProposeKagemushaVerifierReleaseRetireV1 { proposal };
    let frame = norito::encode_canonical(&instruction).unwrap();
    assert_eq!(
        norito::decode_canonical::<ProposeKagemushaVerifierReleaseRetireV1>(&frame).unwrap(),
        instruction
    );
    let boxed: crate::isi::InstructionBox = instruction.into();
    let frame = norito::encode_canonical(&boxed).unwrap();
    assert_eq!(
        norito::decode_canonical::<crate::isi::InstructionBox>(&frame).unwrap(),
        boxed
    );
}

#[test]
fn retirement_rejects_absent_active_historical_ungoverned_malformed_and_replayed_rows() {
    let original = retirement_proposal();
    let mut missing = original.clone();
    missing.standby_release_id[0] ^= 1;
    assert!(missing.validate().is_err());
    let mut active = original.clone();
    active
        .expected_predecessor
        .activate_standby(None, original.standby_release_id)
        .unwrap();
    assert!(active.validate().is_err());
    // A genuine formerly active row remains valid only beside its current active successor.
    let mut historical = authenticated_populated_retirement_proposal();
    historical.standby_release_id = historical
        .expected_predecessor
        .releases
        .iter()
        .find(|row| row.status == crate::kagemusha::KAGEMUSHA_RELEASE_VERIFICATION_ONLY_V1)
        .unwrap()
        .release_id;
    historical.expected_predecessor.validate().unwrap();
    assert!(historical.validate().is_err());
    let mut ungoverned = original.clone();
    ungoverned.expected_predecessor = KagemushaGovernedVerifierRegistryV1::default();
    assert!(ungoverned.validate().is_err());
    let mut malformed = original.clone();
    malformed.expected_predecessor.version += 1;
    assert!(malformed.validate().is_err());
    let mut replay = original.clone();
    replay.expected_predecessor = original.successor().unwrap();
    assert!(replay.validate().is_err());
}

/// Authenticate each added public release with the original fixture signer policy.
/// Reports remain synthetic test reports and never establish release qualification.
fn authenticated_populated_retirement_proposal() -> KagemushaVerifierReleaseRetireProposalV1 {
    use crate::kagemusha::{
        KAGEMUSHA_WIRE_VERSION_V1, KagemushaReleaseApprovalV1, KagemushaReleaseAttestationV1,
    };
    use iroha_crypto::{Algorithm, SignatureOf};
    let install = install_proposal();
    let mut registry = install.successor().unwrap();
    let first = install.manifest.release_id;
    registry.activate_standby(None, first).unwrap();
    let mut keys: Vec<_> = [0x41_u8, 0x42, 0x43]
        .into_iter()
        .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).unwrap())
        .collect();
    keys.sort_by(|a, b| a.public_key().cmp(b.public_key()));
    let policy = registry.authority_policy.as_ref().unwrap().clone();
    assert_eq!(policy.threshold, 2);
    assert_eq!(
        policy.authorized_signers,
        keys.iter()
            .map(|key| key.public_key().clone())
            .collect::<Vec<_>>()
    );
    let mut standby = [0; 32];
    for (i, mask) in [0x5a, 0xa5].into_iter().enumerate() {
        let mut receipt = install.receipt.clone();
        receipt.source_tree_digest[0] ^= mask;
        let mut manifest = install.manifest.clone();
        manifest.source_tree_digest = receipt.source_tree_digest;
        manifest.validation_receipt_digest = receipt.canonical_digest().unwrap();
        let manifest = manifest.seal().unwrap();
        assert!(
            registry
                .clone()
                .install_authenticated_release(&manifest, &receipt, &install.attestation)
                .is_err()
        );
        let subject = manifest
            .release_attestation_subject(&receipt, &policy)
            .unwrap();
        let payload = subject.approval_payload();
        let attestation = KagemushaReleaseAttestationV1 {
            version: KAGEMUSHA_WIRE_VERSION_V1,
            subject,
            approvals: keys[..2]
                .iter()
                .map(|key| KagemushaReleaseApprovalV1 {
                    public_key: key.public_key().clone(),
                    signature: SignatureOf::try_new(key.private_key(), &payload).unwrap(),
                })
                .collect(),
        };
        registry
            .install_authenticated_release(&manifest, &receipt, &attestation)
            .unwrap();
        if i == 0 {
            registry
                .activate_standby(Some(first), manifest.release_id)
                .unwrap();
        } else {
            standby = manifest.release_id;
        }
    }
    assert_eq!(registry.releases.len(), 3);
    KagemushaVerifierReleaseRetireProposalV1 {
        proposal_operator: install.proposal_operator,
        network_id: install.network_id,
        expected_predecessor: registry,
        standby_release_id: standby,
    }
}

#[test]
fn retirement_cannot_remove_other_authenticated_active_or_historical_registry_authority() {
    let proposal = authenticated_populated_retirement_proposal();
    let successor = proposal.successor().unwrap();
    assert_eq!(
        successor.active_release_id,
        proposal.expected_predecessor.active_release_id
    );
    assert_eq!(
        successor.authority_policy,
        proposal.expected_predecessor.authority_policy
    );
    assert_eq!(
        successor.releases,
        proposal
            .expected_predecessor
            .releases
            .iter()
            .filter(|row| row.release_id != proposal.standby_release_id)
            .cloned()
            .collect::<Vec<_>>()
    );
    assert_eq!(successor.releases.len(), 2);
    for row in &successor.releases {
        assert_ne!(row.status, crate::kagemusha::KAGEMUSHA_RELEASE_STANDBY_V1);
        let mut forbidden = proposal.clone();
        forbidden.standby_release_id = row.release_id;
        assert!(forbidden.validate().is_err());
    }
}

#[test]
fn retirement_binds_each_original_field_and_distinct_proposal_and_effect_domains() {
    let proposal = retirement_proposal();
    let original = ProposalKind::KagemushaVerifierReleaseRetire(proposal.clone());
    let activation = ProposalKind::KagemushaVerifierReleaseActivate(activate_proposal());
    assert_eq!(
        original.governed_subject_id_v1().unwrap(),
        activation.governed_subject_id_v1().unwrap()
    );
    assert_ne!(original.fingerprint(), activation.fingerprint());
    assert_ne!(
        original.effect_preimage_hash_v1(),
        activation.effect_preimage_hash_v1()
    );
    assert_ne!(original.effect_preimage_hash_v1(), original.fingerprint());
    for mutation in 0..4 {
        let mut changed = proposal.clone();
        match mutation {
            0 => changed.proposal_operator = checked_account_id(),
            1 => changed.network_id = kagemusha_policy_install_proposal().network_id,
            2 => changed.expected_predecessor.releases.clear(),
            _ => changed.standby_release_id[0] ^= 1,
        }
        assert_ne!(changed, proposal);
        let changed = ProposalKind::KagemushaVerifierReleaseRetire(changed);
        assert_ne!(changed.fingerprint(), original.fingerprint());
        assert_ne!(
            changed.effect_preimage_hash_v1(),
            original.effect_preimage_hash_v1()
        );
    }
    let json = norito::json::to_json(&original).unwrap();
    let mut value: norito::json::Value = norito::json::from_json(&json).unwrap();
    let object = value
        .as_object_mut()
        .unwrap()
        .get_mut("payload")
        .unwrap()
        .as_object_mut()
        .unwrap();
    object.insert("unknown_authority".into(), norito::json::Value::Bool(true));
    assert!(norito::json::from_value::<ProposalKind>(value).is_err());
}
