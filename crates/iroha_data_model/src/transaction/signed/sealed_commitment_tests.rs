//! Tests for sealed-transaction commitment signing and reveal binding.

use super::*;

#[test]
fn sealed_transaction_commitment_signs_and_reveals_expected_hash() {
    let tx = sample_signed_transaction();
    let private_key: iroha_crypto::PrivateKey =
        "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
            .parse()
            .unwrap();
    let salt = [0xA5; 32];
    let reveal_deadline_height = 42;
    let network_id = tx.network_id().expect("ordinary transaction network id");
    let commitment =
        compute_sealed_transaction_commitment(network_id, &tx, salt, reveal_deadline_height);
    {
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_eq!(
            compute_sealed_transaction_commitment(network_id, &tx, salt, reveal_deadline_height,),
            commitment
        );
    }
    let payload = SealedTransactionCommitmentPayload::new(
        *network_id,
        tx.authority().clone(),
        commitment,
        10,
        reveal_deadline_height,
        core::num::NonZeroU64::new(7),
    );
    let signed = SignedSealedTransactionCommitment::sign(payload.clone(), &private_key);
    signed
        .verify_signature()
        .expect("sealed commitment signature verifies");
    assert_eq!(signed.payload(), &payload);
    assert_eq!(signed.commitment(), &commitment);
    let reveal = SealedTransactionReveal::new(commitment, tx, salt);
    assert_eq!(
        reveal.expected_commitment_with_deadline(reveal_deadline_height),
        commitment
    );
    assert_ne!(
        reveal.expected_commitment_with_deadline(reveal_deadline_height + 1),
        commitment
    );
}
#[test]
fn sealed_transaction_commitment_try_sign_matches_compatibility_sign() {
    let tx = sample_signed_transaction();
    let private_key: iroha_crypto::PrivateKey =
        "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
            .parse()
            .unwrap();
    let network_id = *tx.network_id().expect("ordinary transaction network id");
    let payload = SealedTransactionCommitmentPayload::new(
        network_id,
        tx.authority().clone(),
        compute_sealed_transaction_commitment(&network_id, &tx, [0x5A; 32], 64),
        11,
        64,
        core::num::NonZeroU64::new(9),
    );
    let fallible = SignedSealedTransactionCommitment::try_sign(payload.clone(), &private_key)
        .expect("sealed commitment signing should succeed");
    let compatibility = SignedSealedTransactionCommitment::sign(payload, &private_key);
    assert_eq!(fallible, compatibility);
    fallible
        .verify_signature()
        .expect("fallible sealed commitment signature verifies");
}
