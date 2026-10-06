//! Public transcript and signature vectors only; no native/platform qualification.

use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, PrivateKey};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    kagemusha::{
        KagemushaAppOperationApprovalChallengeV1, KagemushaAppOperationApprovalPurposeV1,
        KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1,
        KagemushaHardwareTransitionSelectionV1, KagemushaOperationKindV1,
    },
};
use p256::ecdsa::{Signature, SigningKey, signature::Signer as _};
use sha2::{Digest as _, Sha256};

fn challenge_for(
    operation: KagemushaOperationKindV1,
    tag: u8,
    index: u128,
) -> KagemushaAppOperationApprovalChallengeV1 {
    // Match the wrapper-owned incoming terminal family as well as outgoing subjects.
    let commitments_required = matches!(
        operation,
        KagemushaOperationKindV1::MintFold
            | KagemushaOperationKindV1::SendSplit
            | KagemushaOperationKindV1::ReceiveFold
            | KagemushaOperationKindV1::RedeemSplit
    );
    let subject = KagemushaHardwareTransitionSelectionV1 {
        version: 1,
        release_id: [1; 32],
        provider_policy_root: [2; 32],
        app_policy_digest: [3; 32],
        credential_id: [4; 32],
        network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed(
            [5; 32],
        ))),
        lane_commitment: [6; 32],
        hardware_profile_id: [7; 32],
        policy_epoch: 8,
        hardware_epoch_id: [9; 32],
        hardware_epoch_generation: 10,
        operation_kind: operation,
        transition_statement_digest: [11; 32],
        candidate_envelope_digest: [if commitments_required { 12 } else { 0 }; 32],
        terminal_body_commitment: [if commitments_required { 13 } else { 0 }; 32],
        secure_index_before: index,
        secure_index_after: index + 1,
    };
    let account_key =
        KeyPair::from_private_key(PrivateKey::from_bytes(Algorithm::Ed25519, &[0x42; 32]).unwrap())
            .unwrap();
    let account = AccountId::new(account_key.public_key().clone());
    let mut challenge = KagemushaAppOperationApprovalChallengeV1 {
        version: 1,
        purpose: KagemushaAppOperationApprovalPurposeV1::MonetaryTransition,
        operation_id: [0x20 + tag; 32],
        nonce: [0x22; 32],
        account_binding: KagemushaAppOperationApprovalChallengeV1::account_binding(&account),
        authority_policy_digest: [0x24; 32],
        attested_key_id: [0x25; 32],
        enrollment_digest: [0x26; 32],
        subject_signing_digest: [0; 32],
        normalized_guard_digest: [0x28; 32],
        issued_at_ms: 1000,
        expires_at_ms: 2000,
        subject,
    };
    let s = challenge.canonical_subject_signing_bytes().unwrap();
    challenge.subject_signing_digest = Sha256::digest(&s).into();
    challenge
}
fn challenge() -> KagemushaAppOperationApprovalChallengeV1 {
    challenge_for(KagemushaOperationKindV1::MintFold, 1, 9)
}

fn vector(name: &str) -> Vec<u8> {
    let row = include_str!("../../../fixtures/offline/kagemusha_app_platform_messages_v1.tsv")
        .lines()
        .find_map(|line| {
            line.split_once('\t')
                .filter(|(key, _)| *key == name)
                .map(|(_, value)| value)
        })
        .expect("required public vector");
    hex::decode(row).unwrap()
}

#[test]
fn mounted_rust_serializers_match_all_ten_actual_exported_s_and_w_vectors() {
    for (name, tag, operation) in [
        ("mint_fold", 1, KagemushaOperationKindV1::MintFold),
        ("send_split", 2, KagemushaOperationKindV1::SendSplit),
        ("receive_fold", 3, KagemushaOperationKindV1::ReceiveFold),
        ("redeem_split", 4, KagemushaOperationKindV1::RedeemSplit),
        ("rotate", 5, KagemushaOperationKindV1::Rotate),
    ] {
        for index in [9, u128::MAX - 1] {
            let original = challenge_for(operation, tag, index);
            let name = format!("{name}_{index}");
            let s = original.canonical_subject_signing_bytes().unwrap();
            let w = original.canonical_signing_bytes().unwrap();
            assert_eq!(s, vector(&format!("s_{name}")));
            assert_eq!(w, vector(&format!("w_{name}")));
            assert_eq!(
                Sha256::digest(&s).as_slice(),
                vector(&format!("s_{name}_sha256"))
            );
            assert_eq!(
                Sha256::digest(&w).as_slice(),
                vector(&format!("w_{name}_sha256"))
            );
            assert_eq!(w.len(), 325);
        }
    }
}

#[test]
fn w_refuses_changed_s_or_invalid_original_selector_and_time_shape() {
    let original = challenge();
    assert!(original.canonical_signing_bytes().is_ok());
    let mut changed = original;
    changed.subject.secure_index_after = 9;
    assert!(changed.canonical_signing_bytes().is_err());
    changed = original;
    changed.subject.transition_statement_digest[0] ^= 1;
    assert!(changed.canonical_signing_bytes().is_err());
    changed = original;
    changed.subject_signing_digest = [0; 32];
    assert!(changed.canonical_signing_bytes().is_err());
    changed = original;
    changed.nonce = [0; 32];
    assert!(changed.canonical_signing_bytes().is_err());
    changed = original;
    changed.operation_id = [0; 32];
    assert!(changed.canonical_signing_bytes().is_err());
    changed = original;
    changed.expires_at_ms = changed.issued_at_ms;
    assert!(changed.canonical_signing_bytes().is_err());
    changed = original;
    changed.expires_at_ms = changed.issued_at_ms + 120_001;
    assert!(changed.canonical_signing_bytes().is_err());
}

#[test]
fn original_p256_der_verifies_w_and_not_s_or_any_other_signed_selector() {
    // Synthetic software signer exercises the exact platform equation, not Android custody.
    let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
    let public = KagemushaDevicePublicKeyV1::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .unwrap();
    let original = challenge();
    let w = original.canonical_signing_bytes().unwrap();
    assert_ne!(w, original.canonical_subject_signing_bytes().unwrap());
    let signature: Signature = key.sign(&w);
    let der = signature.to_der();
    let verifier = KagemushaDeviceSignatureV1::from_der_normalizing_low_s(der.as_bytes()).unwrap();
    assert!(verifier.verify(&public, &w).is_ok());
    assert!(
        verifier
            .verify(
                &public,
                &original.canonical_subject_signing_bytes().unwrap()
            )
            .is_err()
    );
    for field in 0..8 {
        let mut changed = original;
        match field {
            0 => changed.operation_id[0] ^= 1,
            1 => changed.nonce[0] ^= 1,
            2 => changed.account_binding[0] ^= 1,
            3 => changed.authority_policy_digest[0] ^= 1,
            4 => changed.attested_key_id[0] ^= 1,
            5 => changed.enrollment_digest[0] ^= 1,
            6 => {
                changed.subject.transition_statement_digest[0] ^= 1;
                changed.subject_signing_digest =
                    Sha256::digest(changed.canonical_subject_signing_bytes().unwrap()).into();
            }
            7 => changed.normalized_guard_digest[0] ^= 1,
            _ => unreachable!(),
        }
        assert!(
            verifier
                .verify(&public, &changed.canonical_signing_bytes().unwrap())
                .is_err(),
            "signed selector {field}"
        );
    }
    let mut changed = original;
    changed.issued_at_ms += 1;
    assert!(
        verifier
            .verify(&public, &changed.canonical_signing_bytes().unwrap())
            .is_err()
    );
    changed = original;
    changed.expires_at_ms -= 1;
    assert!(
        verifier
            .verify(&public, &changed.canonical_signing_bytes().unwrap())
            .is_err()
    );
}

#[test]
fn w_signature_cannot_be_relabelled_as_enrollment_possession() {
    let key = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
    let public = KagemushaDevicePublicKeyV1::from_sec1_bytes(
        key.verifying_key().to_encoded_point(false).as_bytes(),
    )
    .unwrap();
    let original = challenge();
    let w = original.canonical_signing_bytes().unwrap();
    let signature: Signature = key.sign(&w);
    let verifier =
        KagemushaDeviceSignatureV1::from_der_normalizing_low_s(signature.to_der().as_bytes())
            .unwrap();
    assert!(verifier.verify(&public, &w).is_ok());
    let enrollment = vector("e_enrollment_marker11");
    assert_ne!(w, enrollment);
    assert!(verifier.verify(&public, &enrollment).is_err());
}
