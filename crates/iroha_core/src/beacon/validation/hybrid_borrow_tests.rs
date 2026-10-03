//! Signed DKG verification borrows hybrid buffers while preserving canonical rejection.

use super::*;
use crate::{beacon::fixtures::adaptive_beacon_fixture, test_allocations::allocations_during};

#[test]
fn signed_dkg_hybrid_validation_allocates_only_its_exact_signature_preimage() {
    let fixture = adaptive_beacon_fixture();
    let dkg = &fixture.session.record().adaptive_dkg;
    for key in &dkg.recipient_keys {
        let mut verified = None;
        assert_eq!(
            allocations_during(|| {
                verified = Some(verify_global_threshold_beacon_dkg_recipient_key_v1(
                    &dkg.session,
                    key,
                ));
            }),
            1,
            "only the signed preimage owns new backing, never the borrowed ML-KEM key"
        );
        verified.unwrap().unwrap();
    }
    for edge in &dkg.encrypted_shares {
        let dealer = &dkg.dealer_commitments[usize::from(edge.dealer_index - 1)];
        let dealer_key = &dkg.recipient_keys[usize::from(edge.dealer_index - 1)];
        let recipient_key = &dkg.recipient_keys[usize::from(edge.recipient_index - 1)];
        let mut verified = None;
        assert_eq!(
            allocations_during(|| {
                verified = Some(verify_global_threshold_beacon_dkg_encrypted_share_v1(
                    &dkg.session,
                    dealer,
                    dealer_key,
                    recipient_key,
                    edge,
                ));
            }),
            1,
            "only the signed preimage owns new backing, never the borrowed ML-KEM ciphertext"
        );
        verified.unwrap().unwrap();
    }
}

#[test]
fn malformed_hybrid_dkg_fields_reject_before_preimage_allocation_and_keep_signature_checks() {
    let fixture = adaptive_beacon_fixture();
    let dkg = &fixture.session.record().adaptive_dkg;
    let mut key = dkg.recipient_keys[0].clone();
    key.mlkem768_public_key[0] = 0xff;
    key.mlkem768_public_key[1] |= 0x0f;
    let mut failure = None;
    assert_eq!(
        allocations_during(|| {
            failure = Some(verify_global_threshold_beacon_dkg_recipient_key_v1(
                &dkg.session,
                &key,
            ));
        }),
        0
    );
    assert!(matches!(
        failure.unwrap(),
        Err(GlobalThresholdBeaconError::InvalidDkgRecipientKey)
    ));
    let mut edge = dkg.encrypted_shares[0].clone();
    edge.mlkem768_ciphertext.fill(0);
    assert_eq!(
        allocations_during(|| {
            failure = Some(verify_global_threshold_beacon_dkg_encrypted_share_v1(
                &dkg.session,
                &dkg.dealer_commitments[0],
                &dkg.recipient_keys[0],
                &dkg.recipient_keys[0],
                &edge,
            ));
        }),
        0
    );
    assert!(matches!(
        failure.unwrap(),
        Err(GlobalThresholdBeaconError::InvalidDkgEncryptedShare)
    ));
    key = dkg.recipient_keys[0].clone();
    key.signature = iroha_crypto::Signature::from_bytes(&[0; 96]);
    assert_eq!(
        allocations_during(|| {
            failure = Some(verify_global_threshold_beacon_dkg_recipient_key_v1(
                &dkg.session,
                &key,
            ));
        }),
        1
    );
    assert!(matches!(
        failure.unwrap(),
        Err(GlobalThresholdBeaconError::InvalidDkgRecipientKey)
    ));
    edge = dkg.encrypted_shares[0].clone();
    edge.signature = iroha_crypto::Signature::from_bytes(&[0; 96]);
    assert_eq!(
        allocations_during(|| {
            failure = Some(verify_global_threshold_beacon_dkg_encrypted_share_v1(
                &dkg.session,
                &dkg.dealer_commitments[0],
                &dkg.recipient_keys[0],
                &dkg.recipient_keys[0],
                &edge,
            ));
        }),
        1
    );
    assert!(matches!(
        failure.unwrap(),
        Err(GlobalThresholdBeaconError::InvalidDkgEncryptedShare)
    ));
}
