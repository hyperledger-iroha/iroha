//! Faucet proof-of-work and adjacent onboarding error-classification tests.

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{NetworkId, block::BlockHeader};

use super::{adaptive_faucet_pow_extra_bits, faucet_pow_challenge};

#[test]
fn adaptive_faucet_pow_extra_bits_scales_and_caps() {
    assert_eq!(adaptive_faucet_pow_extra_bits(0, 4, 6), 0);
    assert_eq!(adaptive_faucet_pow_extra_bits(3, 4, 6), 0);
    assert_eq!(adaptive_faucet_pow_extra_bits(4, 4, 6), 1);
    assert_eq!(adaptive_faucet_pow_extra_bits(12, 4, 6), 3);
    assert_eq!(adaptive_faucet_pow_extra_bits(999, 4, 6), 6);
    assert_eq!(adaptive_faucet_pow_extra_bits(10, 0, 6), 0);
    assert_eq!(adaptive_faucet_pow_extra_bits(10, 4, 0), 0);
}

#[test]
fn faucet_pow_challenge_rejects_same_label_different_genesis_replay() {
    let account_id = iroha_test_samples::ALICE_ID.clone();
    let anchor_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"same anchor"));
    let first_network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"same-label first genesis",
    )));
    let second_network = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
        b"same-label second genesis",
    )));

    let first = faucet_pow_challenge(&first_network, &account_id, 7, &anchor_hash, None);
    let second = faucet_pow_challenge(&second_network, &account_id, 7, &anchor_hash, None);

    assert_ne!(first, second);
}

#[test]
fn faucet_invalid_request_returns_specific_app_error() {
    let err = super::faucet_invalid_request("invalid account id literal");
    match err {
        crate::Error::AppQueryValidation { code, message } => {
            assert_eq!(code, "invalid_account_id");
            assert_eq!(message, "invalid account id literal");
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}

#[test]
fn onboarding_error_metadata_classifies_noncanonical_account_id() {
    let (code, hint) = super::onboarding_error_metadata(
        "account_id must use the canonical domainless representation",
    );
    assert_eq!(code, "noncanonical_account_id");
    assert!(hint.is_some());
}

#[test]
fn onboarding_error_metadata_classifies_disallowed_permission() {
    let (code, hint) = super::onboarding_error_metadata(
        "requested permission is not in account_onboarding.additional_permissions",
    );
    assert_eq!(code, "requested_permission_not_allowed");
    assert!(hint.is_some());
}

#[test]
fn onboarding_invalid_request_preserves_structured_code() {
    let err = super::onboarding_invalid_request("invalid canonical account_id");
    match err {
        crate::Error::AccountOnboardingValidation { code, hint, .. } => {
            assert_eq!(code, "invalid_account_id");
            assert!(hint.is_some());
        }
        other => panic!("unexpected error variant: {other:?}"),
    }
}
