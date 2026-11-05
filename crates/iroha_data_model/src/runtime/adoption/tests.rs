//! Synthetic cryptographic controls; none supplies governance or imported-State authority.

use super::*;
use crate::{
    NetworkId,
    block::BlockHeader,
    runtime::{
        definition::SelectionId,
        lifecycle::{ADOPTION_RECORD_DOMAIN, ChangeSetId, RootScopeV1, SupportAttestationV1},
    },
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};

fn network(byte: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([byte; 32]),
    ))
}
fn keys(count: u8) -> Vec<KeyPair> {
    (1..=count)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect()
}
fn committee(keys: &[KeyPair]) -> Vec<PublicKey> {
    keys.iter().map(|key| key.public_key().clone()).collect()
}
fn record(keys: &[KeyPair]) -> AdoptionRecordV1 {
    AdoptionRecordV1 {
        network: network(7),
        height: 42,
        block_hash: [1; 32],
        predecessor_world_root: [2; 32],
        imported_state_root: [3; 32],
        committee: committee_digest(&committee(keys)).unwrap(),
        selection: SelectionId([4; 32]),
        authorization: [5; 32],
    }
}
fn statement(record: &AdoptionRecordV1, key: &KeyPair) -> SignedAdoptionAttestationV1 {
    AdoptionAttestationV1 {
        adoption: record.id().unwrap(),
        validator: key.public_key().clone(),
    }
    .sign(key.private_key())
    .unwrap()
}
fn statements(record: &AdoptionRecordV1, keys: &[KeyPair]) -> Vec<SignedAdoptionAttestationV1> {
    keys.iter().map(|key| statement(record, key)).collect()
}

#[test]
fn all_four_sign_exact_record_and_order_is_not_membership() {
    let keys = keys(4);
    let record = record(&keys);
    let mut committee = committee(&keys);
    let mut signed = statements(&record, &keys);
    verify_all_adoption_attestations(&record, &committee, &signed).unwrap();
    committee.rotate_left(1);
    signed.reverse();
    verify_all_adoption_attestations(&record, &committee, &signed).unwrap();
}
#[test]
fn maximum_sumeragi_committee_is_bounded_and_supported() {
    let keys = keys(31);
    let record = record(&keys);
    verify_all_adoption_attestations(&record, &committee(&keys), &statements(&record, &keys))
        .unwrap();
}
#[test]
fn a_quorum_or_superset_is_not_all_n() {
    let keys = keys(4);
    let record = record(&keys);
    let committee = committee(&keys);
    let mut signed = statements(&record, &keys);
    assert!(matches!(
        verify_all_adoption_attestations(&record, &committee, &signed[..3]),
        Err(AdoptionAttestationError::StatementCount)
    ));
    signed.push(signed[0].clone());
    assert!(matches!(
        verify_all_adoption_attestations(&record, &committee, &signed),
        Err(AdoptionAttestationError::StatementCount)
    ));
}
#[test]
fn duplicated_signer_cannot_stand_for_a_missing_validator() {
    let keys = keys(4);
    let record = record(&keys);
    let mut signed = statements(&record, &keys);
    signed[3] = signed[0].clone();
    assert!(matches!(
        verify_all_adoption_attestations(&record, &committee(&keys), &signed),
        Err(AdoptionAttestationError::DuplicateSigner)
    ));
}
#[test]
fn a_valid_foreign_signature_is_not_membership() {
    let keys = keys(4);
    let record = record(&keys);
    let mut signed = statements(&record, &keys);
    let foreign = KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519);
    signed[3] = statement(&record, &foreign);
    signed[3].verify_for(&record).unwrap();
    assert!(matches!(
        verify_all_adoption_attestations(&record, &committee(&keys), &signed),
        Err(AdoptionAttestationError::ForeignSigner)
    ));
}
#[test]
fn duplicate_membership_is_refused_before_set_digest_can_hide_it() {
    let keys = keys(4);
    let record = record(&keys);
    let mut committee = committee(&keys);
    committee[3] = committee[0].clone();
    assert!(matches!(
        verify_all_adoption_attestations(&record, &committee, &statements(&record, &keys)),
        Err(AdoptionAttestationError::DuplicateCommitteeMember)
    ));
}
#[test]
fn unsupported_or_empty_committee_never_vacuously_passes() {
    let keys = keys(34);
    let record = record(&keys);
    let committee = committee(&keys);
    for size in [0, 1, 3, 5, 32, 34] {
        assert!(matches!(
            verify_all_adoption_attestations(&record, &committee[..size], &[]),
            Err(AdoptionAttestationError::CommitteeSize)
        ));
    }
}
#[test]
fn another_committee_is_not_the_records_committee() {
    let keys = keys(4);
    let record = record(&keys);
    let mut committee = committee(&keys);
    committee[3] = KeyPair::from_seed(vec![99; 32], Algorithm::Ed25519)
        .public_key()
        .clone();
    assert!(matches!(
        verify_all_adoption_attestations(&record, &committee, &statements(&record, &keys)),
        Err(AdoptionAttestationError::CommitteeMismatch)
    ));
}
#[test]
fn every_anchor_field_including_governance_is_bound() {
    let keys = keys(4);
    let original = record(&keys);
    let signed = statements(&original, &keys);
    let edits: [fn(&mut AdoptionRecordV1); 8] = [
        |r| r.network = network(8),
        |r| r.height += 1,
        |r| r.block_hash[0] ^= 1,
        |r| r.predecessor_world_root[0] ^= 1,
        |r| r.imported_state_root[0] ^= 1,
        |r| r.committee[0] ^= 1,
        |r| r.selection.0[0] ^= 1,
        |r| r.authorization[0] ^= 1,
    ];
    for edit in edits {
        let mut changed = original;
        edit(&mut changed);
        assert!(signed[0].verify_for(&changed).is_err());
        assert!(verify_all_adoption_attestations(&changed, &committee(&keys), &signed).is_err());
    }
}
#[test]
fn relabeling_another_validators_signature_is_refused() {
    let keys = keys(4);
    let record = record(&keys);
    let mut signed = statement(&record, &keys[0]);
    signed.attestation.validator = keys[1].public_key().clone();
    assert!(matches!(
        signed.verify_for(&record),
        Err(AdoptionAttestationError::Crypto(_))
    ));
}
#[test]
fn malformed_and_wrong_key_signatures_preserve_crypto_refusal() {
    let keys = keys(4);
    let record = record(&keys);
    let mut signed = statement(&record, &keys[0]);
    for bytes in [vec![], vec![0; 64], vec![42; 3]] {
        signed.signature = Signature::from_bytes(&bytes);
        assert!(matches!(
            signed.verify_for(&record),
            Err(AdoptionAttestationError::Crypto(_))
        ));
    }
    let wrong = AdoptionAttestationV1 {
        adoption: record.id().unwrap(),
        validator: keys[0].public_key().clone(),
    }
    .sign(keys[1].private_key())
    .unwrap();
    assert!(matches!(
        wrong.verify_for(&record),
        Err(AdoptionAttestationError::Crypto(_))
    ));
}
#[test]
fn runtime_support_signature_cannot_be_repurposed_as_adoption() {
    let keys = keys(4);
    let record = record(&keys);
    let mut signed = statement(&record, &keys[0]);
    let support = SupportAttestationV1 {
        network: record.network,
        root: RootScopeV1::Universal,
        change_set: ChangeSetId([9; 32]),
        predecessor: SelectionId([8; 32]),
        successor: record.selection,
        committee: record.committee,
        tested_checkpoint: record.height,
        valid_from: 43,
        valid_until: 90,
        release: [7; 32],
        validator: keys[0].public_key().clone(),
    }
    .sign(keys[0].private_key())
    .unwrap();
    support.verify().unwrap();
    signed.signature = support.signature;
    assert!(matches!(
        signed.verify_for(&record),
        Err(AdoptionAttestationError::Crypto(_))
    ));
}
#[test]
fn record_id_domain_and_undomained_signature_are_not_adoption_statements() {
    let keys = keys(4);
    let record = record(&keys);
    let mut signed = statement(&record, &keys[0]);
    let wire = norito::encode_canonical(&signed.attestation).unwrap();
    for payload in [
        wire.clone(),
        [ADOPTION_RECORD_DOMAIN, wire.as_slice()].concat(),
    ] {
        signed.signature = Signature::try_new(keys[0].private_key(), &payload).unwrap();
        assert!(matches!(
            signed.verify_for(&record),
            Err(AdoptionAttestationError::Crypto(_))
        ));
    }
}
#[test]
fn original_signature_bytes_survive_canonical_and_json_roundtrips() {
    let keys = keys(4);
    let record = record(&keys);
    let signed = statement(&record, &keys[0]);
    let wire = norito::encode_canonical(&signed).unwrap();
    let decoded: SignedAdoptionAttestationV1 = norito::decode_canonical(&wire).unwrap();
    assert_eq!(decoded, signed);
    decoded.verify_for(&record).unwrap();
    assert_eq!(norito::encode_canonical(&decoded).unwrap(), wire);
    let json = norito::json::to_json(&signed).unwrap();
    let decoded: SignedAdoptionAttestationV1 = norito::json::from_str(&json).unwrap();
    assert_eq!(decoded, signed);
    decoded.verify_for(&record).unwrap();
}
