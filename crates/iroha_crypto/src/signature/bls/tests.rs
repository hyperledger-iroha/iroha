use super::{
    implementation::{BlsConfiguration, BlsImpl, PreparedPublicKeyCacheAccess},
    normal::NormalConfiguration,
    small::SmallConfiguration,
};
use crate::{Error, KeyGenOption};
#[cfg(feature = "rand")]
use rand_core::{TryCryptoRng, TryRngCore};
#[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
use w3f_bls::SerializableToBytes as _;
const MESSAGE_1: &[u8; 22] = b"This is a test message";
const MESSAGE_2: &[u8; 20] = b"Another test message";
const SEED: &[u8; 10] = &[1u8; 10];
// Canonical compressed encodings of on-curve points outside the prime-order
// subgroups. Tests first validate them with the backend's unchecked decoder so
// a malformed fixture cannot make the subgroup tripwire pass accidentally.
const NON_SUBGROUP_G1: [u8; 48] = hex_literal::hex!(
    "8000000000000000000000000000000000000000000000000000000000000000\
     00000000000000000000000000000000"
);
const NON_SUBGROUP_G2: [u8; 96] = hex_literal::hex!(
    "8158b0083c00046272a9b63583963fff07e147f3f9e6e24174328ad8bc2aa150\
     298f3189a9cf6ed626f461e944bbd3d117762a3b9108c4a74a151b732a6075bf\
     2199bc19c48c393d4ceb92d0a76057be02f08540770fabd60262cea73ea1906c"
);
#[cfg(feature = "rand")]
struct FixedTryRng {
    byte: u8,
}
#[cfg(feature = "rand")]
impl TryRngCore for FixedTryRng {
    type Error = core::convert::Infallible;
    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        Ok(u32::from_le_bytes([self.byte; 4]))
    }
    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        Ok(u64::from_le_bytes([self.byte; 8]))
    }
    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Self::Error> {
        dest.fill(self.byte);
        Ok(())
    }
}
#[cfg(feature = "rand")]
impl TryCryptoRng for FixedTryRng {}
#[allow(clippy::similar_names)]
fn test_keypair_generation_from_seed<C: BlsConfiguration>() {
    let (pk_1, sk_1) =
        BlsImpl::<C>::keypair(KeyGenOption::UseSeed(SEED.to_vec())).expect("BLS keypair");
    let (pk_2, sk_2) =
        BlsImpl::<C>::keypair(KeyGenOption::UseSeed(SEED.to_vec())).expect("BLS keypair");
    #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
    {
        assert!(
            (pk_1, sk_1.to_bytes()) == (pk_2, sk_2.to_bytes()),
            "Keypairs are not equal"
        );
    }
    #[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
    {
        assert!(
            (pk_1.to_bytes(), sk_1.to_bytes()) == (pk_2.to_bytes(), sk_2.to_bytes()),
            "Keypairs are not equal"
        );
    }
}
#[allow(clippy::similar_names)]
fn test_try_keypair_generation_from_seed<C: BlsConfiguration>() {
    let (pk_1, sk_1) =
        BlsImpl::<C>::try_keypair(KeyGenOption::UseSeed(SEED.to_vec())).expect("checked keypair");
    let (pk_2, sk_2) =
        BlsImpl::<C>::keypair(KeyGenOption::UseSeed(SEED.to_vec())).expect("BLS keypair");
    #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
    {
        assert!(
            (pk_1, sk_1.to_bytes()) == (pk_2, sk_2.to_bytes()),
            "Checked keypair does not match compatibility keypair"
        );
    }
    #[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
    {
        assert!(
            (pk_1.to_bytes(), sk_1.to_bytes()) == (pk_2.to_bytes(), sk_2.to_bytes()),
            "Checked keypair does not match compatibility keypair"
        );
    }
}
fn test_try_keypair_rejects_all_zero_seed<C: BlsConfiguration>() {
    match BlsImpl::<C>::try_keypair(KeyGenOption::UseSeed(vec![0u8; 32])) {
        Err(Error::KeyGen(message)) => assert!(message.contains("all zero")),
        Err(err) => panic!("expected all-zero seed KeyGen error, got {err:?}"),
        Ok(_) => panic!("all-zero BLS seed material must fail"),
    }
}
#[cfg(feature = "rand")]
fn test_random_keypair_from_rng_rejects_all_zero_seed<C: BlsConfiguration>() {
    let mut rng = FixedTryRng { byte: 0 };
    match BlsImpl::<C>::random_keypair_from_rng(&mut rng) {
        Err(Error::KeyGen(message)) => assert!(message.contains("all zero")),
        Err(err) => panic!("expected all-zero random seed KeyGen error, got {err:?}"),
        Ok(_) => panic!("all-zero BLS random seed material must fail"),
    }
}
fn test_signature_verification<C: BlsConfiguration + PreparedPublicKeyCacheAccess>() {
    let (pk, sk) = BlsImpl::<C>::keypair(KeyGenOption::Random).expect("BLS keypair");
    let signature_1 = BlsImpl::<C>::sign(MESSAGE_1, &sk).expect("BLS sign");
    BlsImpl::<C>::verify(MESSAGE_1, &signature_1, &pk)
        .expect("Signature verification should succeed");
}
fn test_checked_random_keypair_signs_and_verifies<
    C: BlsConfiguration + PreparedPublicKeyCacheAccess,
>() {
    let (pk, sk) =
        BlsImpl::<C>::try_keypair(KeyGenOption::Random).expect("checked random BLS keypair");
    let signature = BlsImpl::<C>::try_sign(MESSAGE_1, &sk).expect("checked BLS sign");
    BlsImpl::<C>::verify(MESSAGE_1, &signature, &pk)
        .expect("checked random BLS signature should verify");
}
fn test_signature_verification_different_messages<
    C: BlsConfiguration + PreparedPublicKeyCacheAccess,
>() {
    let (pk, sk) = BlsImpl::<C>::keypair(KeyGenOption::Random).expect("BLS keypair");
    let signature = BlsImpl::<C>::sign(MESSAGE_1, &sk).expect("BLS sign");
    BlsImpl::<C>::verify(MESSAGE_2, &signature, &pk)
        .expect_err("Signature verification for wrong message should fail");
}
#[allow(clippy::similar_names)]
fn test_signature_verification_different_keys<
    C: BlsConfiguration + PreparedPublicKeyCacheAccess,
>() {
    let (_pk_1, sk_1) = BlsImpl::<C>::keypair(KeyGenOption::Random).expect("BLS keypair");
    let (pk_2, _sk_2) = BlsImpl::<C>::keypair(KeyGenOption::Random).expect("BLS keypair");
    let signature = BlsImpl::<C>::sign(MESSAGE_1, &sk_1).expect("BLS sign");
    BlsImpl::<C>::verify(MESSAGE_1, &signature, &pk_2)
        .expect_err("Signature verification for wrong public key should fail");
}
fn test_verify_cache_rejects_variable_length_tuple_splice<
    C: BlsConfiguration + PreparedPublicKeyCacheAccess,
>() {
    let (pk, sk) =
        BlsImpl::<C>::keypair(KeyGenOption::UseSeed(vec![0x74; 32])).expect("BLS keypair");
    let message = b"cached BLS tuple";
    let signature = BlsImpl::<C>::sign(message, &sk).expect("BLS sign");
    BlsImpl::<C>::verify(message, &signature, &pk).expect("seed positive verification cache");
    let mut spliced_message = message.to_vec();
    spliced_message.push(signature[0]);
    BlsImpl::<C>::verify(&spliced_message, &signature[1..], &pk)
        .expect_err("malformed signature must not borrow a cached tuple verdict");
}
fn test_verify_rejects_all_zero_signature_material<
    C: BlsConfiguration + PreparedPublicKeyCacheAccess,
>() {
    let (pk, sk) = BlsImpl::<C>::keypair(KeyGenOption::Random).expect("BLS keypair");
    let valid_signature = BlsImpl::<C>::sign(MESSAGE_1, &sk).expect("BLS sign");
    let all_zero_signature = vec![0u8; valid_signature.len()];
    let err = BlsImpl::<C>::verify(MESSAGE_1, &all_zero_signature, &pk)
        .expect_err("all-zero BLS signature material must fail before backend parsing");
    assert!(matches!(err, Error::Parse(_)));
    assert!(
        err.to_string().contains("all zero"),
        "unexpected all-zero BLS signature error: {err:?}"
    );
}
fn test_parse_public_key_rejects_all_zero_material<C: BlsConfiguration>() {
    let (pk, _sk) = BlsImpl::<C>::keypair(KeyGenOption::Random).expect("BLS keypair");
    let all_zero_public_key = vec![0u8; pk.to_bytes().len()];
    let err = match BlsImpl::<C>::parse_public_key(&all_zero_public_key) {
        Ok(_) => panic!("all-zero BLS public key material must fail before backend parsing"),
        Err(err) => err,
    };
    assert!(
        err.to_string().contains("all zero"),
        "unexpected all-zero BLS public-key error: {err:?}"
    );
}
fn test_aggregate_rejects_all_zero_public_key_material<C: BlsConfiguration>() {
    let (pk, sk) = BlsImpl::<C>::keypair(KeyGenOption::Random).expect("BLS keypair");
    let msg = b"aggregate-all-zero-pk";
    let sig = BlsImpl::<C>::sign(msg, &sk).expect("BLS sign");
    let all_zero_public_key = vec![0u8; pk.to_bytes().len()];
    let signatures: [&[u8]; 1] = [sig.as_slice()];
    let public_keys: [&[u8]; 1] = [all_zero_public_key.as_slice()];
    let err = BlsImpl::<C>::verify_aggregate_same_message(msg, &signatures, &public_keys)
        .expect_err("all-zero BLS aggregate public key must fail before backend parsing");
    assert!(
        err.to_string().contains("all zero"),
        "unexpected all-zero BLS aggregate public-key error: {err:?}"
    );
}
fn test_aggregate_rejects_duplicate_public_key_content<C: BlsConfiguration>() {
    let (pk, sk) = BlsImpl::<C>::keypair(KeyGenOption::Random).expect("BLS keypair");
    let msg = b"aggregate-duplicate-pk-content";
    let sig = BlsImpl::<C>::sign(msg, &sk).expect("BLS sign");
    let pk_bytes = pk.to_bytes();
    let pk_bytes_clone = pk_bytes.clone();
    let signatures: Vec<&[u8]> = vec![sig.as_slice(), sig.as_slice()];
    let public_keys: Vec<&[u8]> = vec![pk_bytes.as_slice(), pk_bytes_clone.as_slice()];
    BlsImpl::<C>::verify_aggregate_same_message(msg, &signatures, &public_keys)
        .expect_err("duplicate public-key bytes must reject same-message aggregate");
    let aggregate = BlsImpl::<C>::aggregate_signatures(&[sig.as_slice(), sig.as_slice()])
        .expect("aggregate signatures");
    BlsImpl::<C>::verify_preaggregated_same_message(msg, &aggregate, &public_keys)
        .expect_err("duplicate public-key bytes must reject preaggregated verification");
}
#[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
fn test_fallible_paths_reject_corrupted_stored_secret<C: BlsConfiguration>() {
    let key =
        super::implementation::ManagedSecretKey::<C>::from_unchecked_bytes_for_test(vec![0; 31]);
    assert!(BlsImpl::<C>::try_sign(MESSAGE_1, &key).is_err());
    assert!(BlsImpl::<C>::derive_public_key(&key).is_err());
    assert!(BlsImpl::<C>::try_keypair(KeyGenOption::FromPrivateKey(key)).is_err());
}
mod normal {
    use super::*;
    #[cfg(feature = "bls-backend-blstrs")]
    use crate::signature::bls::implementation;
    use blstrs::{G1Affine, G2Affine, G2Projective, Scalar};
    use group::prime::PrimeCurveAffine;
    #[cfg(feature = "bls-backend-blstrs")]
    #[test]
    fn detect_hash_variant_normal_matches_concat() {
        let (pk, sk) =
            BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random).expect("BLS keypair");
        let msg = b"diagnostic-normal";
        let sig = BlsImpl::<NormalConfiguration>::sign(msg, &sk).expect("BLS sign");
        let (concat_ok, aug_ok) = implementation::detect_variant_normal(msg, &sig, &pk.to_bytes());
        assert!(concat_ok ^ aug_ok, "exactly one variant should succeed");
        assert!(
            concat_ok,
            "expected concat variant to match w3f Message::new semantics"
        );
    }
    #[test]
    fn keypair_generation_from_seed() {
        test_keypair_generation_from_seed::<NormalConfiguration>();
    }
    #[test]
    fn checked_keypair_generation_from_seed() {
        test_try_keypair_generation_from_seed::<NormalConfiguration>();
    }
    #[test]
    fn checked_keypair_rejects_all_zero_seed() {
        test_try_keypair_rejects_all_zero_seed::<NormalConfiguration>();
    }
    #[cfg(feature = "rand")]
    #[test]
    fn random_keypair_from_rng_rejects_all_zero_seed() {
        test_random_keypair_from_rng_rejects_all_zero_seed::<NormalConfiguration>();
    }
    #[test]
    fn signature_verification() {
        test_signature_verification::<NormalConfiguration>();
    }
    #[test]
    fn verify_rejects_all_zero_signature_material() {
        test_verify_rejects_all_zero_signature_material::<NormalConfiguration>();
    }
    #[test]
    fn checked_random_keypair_signs_and_verifies() {
        test_checked_random_keypair_signs_and_verifies::<NormalConfiguration>();
    }
    #[test]
    fn signature_verification_different_messages() {
        test_signature_verification_different_messages::<NormalConfiguration>();
    }
    #[test]
    fn signature_verification_different_keys() {
        test_signature_verification_different_keys::<NormalConfiguration>();
    }
    #[test]
    fn verify_cache_rejects_variable_length_tuple_splice() {
        test_verify_cache_rejects_variable_length_tuple_splice::<NormalConfiguration>();
    }
    #[test]
    fn verify_rejects_identity_signature_as_parse_error() {
        let (pk, _sk) =
            BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random).expect("BLS keypair");
        let sig = G2Affine::identity().to_compressed();
        let err = BlsImpl::<NormalConfiguration>::verify(b"identity", sig.as_ref(), &pk)
            .expect_err("identity signature must be rejected");
        assert!(matches!(err, crate::Error::Parse(_)));
    }
    #[test]
    fn aggregate_same_message_roundtrip() {
        let (pk1, sk1) =
            BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random).expect("BLS keypair");
        let (pk2, sk2) =
            BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random).expect("BLS keypair");
        let msg = b"aggregate-same-message";
        let sig1 = BlsImpl::<NormalConfiguration>::sign(msg, &sk1).expect("BLS sign");
        let sig2 = BlsImpl::<NormalConfiguration>::sign(msg, &sk2).expect("BLS sign");
        let aggregate = BlsImpl::<NormalConfiguration>::aggregate_signatures(&[
            sig1.as_slice(),
            sig2.as_slice(),
        ])
        .expect("aggregate signatures");
        let pk1_bytes = pk1.to_bytes();
        let pk2_bytes = pk2.to_bytes();
        let public_keys: Vec<&[u8]> = vec![pk1_bytes.as_slice(), pk2_bytes.as_slice()];
        BlsImpl::<NormalConfiguration>::verify_preaggregated_same_message(
            msg,
            &aggregate,
            &public_keys,
        )
        .expect("pre-aggregated signature should verify");
        let mut bad = aggregate.clone();
        bad[0] ^= 0x01;
        assert!(
            BlsImpl::<NormalConfiguration>::verify_preaggregated_same_message(
                msg,
                &bad,
                &public_keys,
            )
            .is_err(),
            "corrupted aggregate must be rejected"
        );
    }
    #[test]
    fn aggregate_same_message_rejects_duplicate_public_keys() {
        let (pk, sk) =
            BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random).expect("BLS keypair");
        let msg = b"aggregate-duplicate-pk";
        let sig = BlsImpl::<NormalConfiguration>::sign(msg, &sk).expect("BLS sign");
        let pk_bytes = pk.to_bytes();
        let signatures: Vec<&[u8]> = vec![sig.as_slice(), sig.as_slice()];
        let public_keys: Vec<&[u8]> = vec![pk_bytes.as_slice(), pk_bytes.as_slice()];
        assert!(
            BlsImpl::<NormalConfiguration>::verify_aggregate_same_message(
                msg,
                &signatures,
                &public_keys
            )
            .is_err()
        );
        let aggregate =
            BlsImpl::<NormalConfiguration>::aggregate_signatures(&[sig.as_slice(), sig.as_slice()])
                .expect("aggregate signatures");
        assert!(
            BlsImpl::<NormalConfiguration>::verify_preaggregated_same_message(
                msg,
                &aggregate,
                &public_keys,
            )
            .is_err()
        );
    }
    #[test]
    fn aggregate_same_message_rejects_duplicate_public_key_content() {
        test_aggregate_rejects_duplicate_public_key_content::<NormalConfiguration>();
    }
    #[test]
    fn aggregate_same_message_rejects_all_zero_public_key_material() {
        test_aggregate_rejects_all_zero_public_key_material::<NormalConfiguration>();
    }
    #[test]
    fn parse_public_key_rejects_identity() {
        let identity = G1Affine::identity().to_compressed();
        assert!(BlsImpl::<NormalConfiguration>::parse_public_key(identity.as_ref()).is_err());
    }
    #[test]
    fn parse_public_key_rejects_all_zero_material() {
        test_parse_public_key_rejects_all_zero_material::<NormalConfiguration>();
    }
    #[test]
    fn parse_public_key_rejects_non_subgroup_point() {
        let unchecked = G1Affine::from_compressed_unchecked(&NON_SUBGROUP_G1)
            .into_option()
            .expect("tripwire fixture is a compressed on-curve G1 point");
        assert!(bool::from(unchecked.is_on_curve()));
        assert!(!bool::from(unchecked.is_torsion_free()));
        assert!(
            BlsImpl::<NormalConfiguration>::parse_public_key(&NON_SUBGROUP_G1).is_err(),
            "BLS-normal parser must enforce the G1 prime-order subgroup"
        );
    }
    #[test]
    fn parse_private_key_rejects_zero() {
        let zero = [0u8; 32];
        assert!(BlsImpl::<NormalConfiguration>::parse_private_key(&zero).is_err());
    }
    #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
    #[test]
    fn fallible_paths_reject_corrupted_stored_secret() {
        test_fallible_paths_reject_corrupted_stored_secret::<NormalConfiguration>();
    }
    #[test]
    fn aggregate_same_message_rejects_identity_inputs() {
        let msg = b"aggregate-identity";
        let sig = G2Affine::identity().to_compressed();
        let pk = G1Affine::identity().to_compressed();
        let signatures: [&[u8]; 1] = [sig.as_ref()];
        let public_keys: [&[u8]; 1] = [pk.as_ref()];
        assert!(
            BlsImpl::<NormalConfiguration>::verify_aggregate_same_message(
                msg,
                &signatures,
                &public_keys
            )
            .is_err()
        );
    }
    #[test]
    fn sign_is_thread_safe() {
        use std::sync::Arc;
        let (pk, sk) =
            BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random).expect("BLS keypair");
        let sk = Arc::new(sk);
        let msg = b"concurrent-signing";
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let sk = Arc::clone(&sk);
                std::thread::spawn(move || {
                    BlsImpl::<NormalConfiguration>::sign(msg, &sk).expect("BLS sign")
                })
            })
            .collect();
        for handle in handles {
            let sig = handle.join().expect("sign thread");
            BlsImpl::<NormalConfiguration>::verify(msg, &sig, &pk)
                .expect("signature should verify");
        }
    }
    #[test]
    fn aggregate_same_message_rejects_canceling_pairs() {
        let msg = b"aggregate-canceling";
        let pk = G1Affine::generator() * Scalar::from(7u64);
        let sig = G2Affine::generator() * Scalar::from(11u64);
        let pk_bytes = G1Affine::from(pk).to_compressed();
        let sig_bytes = G2Affine::from(sig).to_compressed();
        let pk_neg_bytes = G1Affine::from(-pk).to_compressed();
        let sig_neg_bytes = G2Affine::from(-sig).to_compressed();
        let signatures: [&[u8]; 2] = [sig_bytes.as_ref(), sig_neg_bytes.as_ref()];
        let public_keys: [&[u8]; 2] = [pk_bytes.as_ref(), pk_neg_bytes.as_ref()];
        assert!(
            BlsImpl::<NormalConfiguration>::verify_aggregate_same_message(
                msg,
                &signatures,
                &public_keys
            )
            .is_err()
        );
        assert!(
            BlsImpl::<NormalConfiguration>::aggregate_signatures(&signatures).is_err(),
            "identity aggregate must be rejected"
        );
        let non_identity_aggregate =
            BlsImpl::<NormalConfiguration>::aggregate_signatures(&[sig_bytes.as_ref()])
                .expect("single non-identity aggregate");
        assert!(
            BlsImpl::<NormalConfiguration>::verify_preaggregated_same_message(
                msg,
                &non_identity_aggregate,
                &public_keys,
            )
            .is_err(),
            "canceling aggregate public key must be rejected"
        );
    }
    #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
    #[test]
    fn aggregate_multi_message_verification() {
        let (pk1, sk1) =
            BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random).expect("BLS keypair");
        let (pk2, sk2) =
            BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random).expect("BLS keypair");
        let msg1 = b"aggregate-m1";
        let msg2 = b"aggregate-m2";
        let sig1 = BlsImpl::<NormalConfiguration>::sign(msg1, &sk1).expect("BLS sign");
        let sig2 = BlsImpl::<NormalConfiguration>::sign(msg2, &sk2).expect("BLS sign");
        let messages: Vec<&[u8]> = vec![msg1.as_ref(), msg2.as_ref()];
        let signature_refs: Vec<&[u8]> = vec![sig1.as_slice(), sig2.as_slice()];
        let pk1_bytes = pk1.to_bytes();
        let pk2_bytes = pk2.to_bytes();
        let public_key_refs: Vec<&[u8]> = vec![pk1_bytes.as_slice(), pk2_bytes.as_slice()];
        BlsImpl::<NormalConfiguration>::verify_aggregate_multi_message(
            &messages,
            &signature_refs,
            &public_key_refs,
        )
        .expect("aggregate verification should succeed");
        let mut bad_sig1 = sig1.clone();
        // Flip one bit to invalidate the first signature
        bad_sig1[0] ^= 0x01;
        let bad_signature_refs: Vec<&[u8]> = vec![bad_sig1.as_slice(), sig2.as_slice()];
        BlsImpl::<NormalConfiguration>::verify_aggregate_multi_message(
            &messages,
            &bad_signature_refs,
            &public_key_refs,
        )
        .expect_err("corrupted aggregate must be rejected");
    }
    #[test]
    fn aggregate_multi_message_rejects_duplicate_messages() {
        let (pk1, sk1) =
            BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random).expect("BLS keypair");
        let (pk2, sk2) =
            BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random).expect("BLS keypair");
        let msg1 = b"duplicate-msg".to_vec();
        let msg2 = msg1.clone();
        let sig1 = BlsImpl::<NormalConfiguration>::sign(&msg1, &sk1).expect("BLS sign");
        let sig2 = BlsImpl::<NormalConfiguration>::sign(&msg2, &sk2).expect("BLS sign");
        let messages: Vec<&[u8]> = vec![msg1.as_slice(), msg2.as_slice()];
        let signature_refs: Vec<&[u8]> = vec![sig1.as_slice(), sig2.as_slice()];
        let pk1_bytes = pk1.to_bytes();
        let pk2_bytes = pk2.to_bytes();
        let public_key_refs: Vec<&[u8]> = vec![pk1_bytes.as_slice(), pk2_bytes.as_slice()];
        assert!(
            BlsImpl::<NormalConfiguration>::verify_aggregate_multi_message(
                &messages,
                &signature_refs,
                &public_key_refs,
            )
            .is_err(),
            "duplicate messages must be rejected"
        );
    }
    #[test]
    fn aggregate_multi_message_rejects_canceling_signatures() {
        let msg1 = b"canceling-multi-message-a";
        let msg2 = b"canceling-multi-message-b";
        let sig = G2Affine::generator() * Scalar::from(11u64);
        let pk1 = G1Affine::generator() * Scalar::from(17u64);
        let pk2 = G1Affine::generator() * Scalar::from(19u64);
        let sig_bytes = G2Affine::from(sig).to_compressed();
        let sig_neg_bytes = G2Affine::from(-sig).to_compressed();
        let pk1_bytes = G1Affine::from(pk1).to_compressed();
        let pk2_bytes = G1Affine::from(pk2).to_compressed();
        let messages: [&[u8]; 2] = [msg1.as_ref(), msg2.as_ref()];
        let signatures: [&[u8]; 2] = [sig_bytes.as_ref(), sig_neg_bytes.as_ref()];
        let public_keys: [&[u8]; 2] = [pk1_bytes.as_ref(), pk2_bytes.as_ref()];
        assert!(
            BlsImpl::<NormalConfiguration>::verify_aggregate_multi_message(
                &messages,
                &signatures,
                &public_keys,
            )
            .is_err(),
            "identity aggregate signature must be rejected before pairing"
        );
    }
    #[test]
    fn multi_message_rejects_balancing_altered_signatures() {
        let (pk1, sk1) =
            BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::UseSeed(vec![0x31; 32]))
                .expect("first BLS keypair");
        let (pk2, sk2) =
            BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::UseSeed(vec![0x32; 32]))
                .expect("second BLS keypair");
        let msg1 = b"independent-normal-a";
        let msg2 = b"independent-normal-b";
        let sig1 = BlsImpl::<NormalConfiguration>::sign(msg1, &sk1).expect("first BLS signature");
        let sig2 = BlsImpl::<NormalConfiguration>::sign(msg2, &sk2).expect("second BLS signature");
        let sig1_encoded: [u8; 96] = sig1.as_slice().try_into().expect("normal signature length");
        let sig2_encoded: [u8; 96] = sig2.as_slice().try_into().expect("normal signature length");
        let sig1_point = G2Affine::from_compressed(&sig1_encoded)
            .into_option()
            .expect("first canonical signature");
        let sig2_point = G2Affine::from_compressed(&sig2_encoded)
            .into_option()
            .expect("second canonical signature");
        let delta = G2Affine::generator() * Scalar::from(41_u64);
        let altered_sig1 = G2Affine::from(G2Projective::from(sig1_point) + delta).to_compressed();
        let altered_sig2 = G2Affine::from(G2Projective::from(sig2_point) - delta).to_compressed();
        BlsImpl::<NormalConfiguration>::verify(msg1, altered_sig1.as_ref(), &pk1)
            .expect_err("first altered signature must fail independently");
        BlsImpl::<NormalConfiguration>::verify(msg2, altered_sig2.as_ref(), &pk2)
            .expect_err("second altered signature must fail independently");
        let pk1_bytes = pk1.to_bytes();
        let pk2_bytes = pk2.to_bytes();
        let messages: [&[u8]; 2] = [msg1.as_ref(), msg2.as_ref()];
        let signatures: [&[u8]; 2] = [altered_sig1.as_ref(), altered_sig2.as_ref()];
        let public_keys: [&[u8]; 2] = [pk1_bytes.as_ref(), pk2_bytes.as_ref()];
        BlsImpl::<NormalConfiguration>::verify_aggregate_multi_message(
            &messages,
            &signatures,
            &public_keys,
        )
        .expect_err("multi-message verification must prove each signature independently");
    }
}
mod small {
    use super::*;
    #[cfg(feature = "bls-backend-blstrs")]
    use crate::signature::bls::implementation;
    use blstrs::{G1Affine, G1Projective, G2Affine, Scalar};
    use group::prime::PrimeCurveAffine;
    #[cfg(feature = "bls-backend-blstrs")]
    #[test]
    fn detect_hash_variant_small_matches_concat() {
        let (pk, sk) =
            BlsImpl::<SmallConfiguration>::keypair(KeyGenOption::Random).expect("BLS keypair");
        let msg = b"diagnostic-small";
        let sig = BlsImpl::<SmallConfiguration>::sign(msg, &sk).expect("BLS sign");
        let (concat_ok, aug_ok) = implementation::detect_variant_small(msg, &sig, &pk.to_bytes());
        assert!(concat_ok ^ aug_ok, "exactly one variant should succeed");
        assert!(
            concat_ok,
            "expected concat variant to match w3f Message::new semantics"
        );
    }
    #[test]
    fn keypair_generation_from_seed() {
        test_keypair_generation_from_seed::<SmallConfiguration>();
    }
    #[test]
    fn checked_keypair_generation_from_seed() {
        test_try_keypair_generation_from_seed::<SmallConfiguration>();
    }
    #[test]
    fn checked_keypair_rejects_all_zero_seed() {
        test_try_keypair_rejects_all_zero_seed::<SmallConfiguration>();
    }
    #[cfg(feature = "rand")]
    #[test]
    fn random_keypair_from_rng_rejects_all_zero_seed() {
        test_random_keypair_from_rng_rejects_all_zero_seed::<SmallConfiguration>();
    }
    #[test]
    fn signature_verification() {
        test_signature_verification::<SmallConfiguration>();
    }
    #[test]
    fn verify_rejects_all_zero_signature_material() {
        test_verify_rejects_all_zero_signature_material::<SmallConfiguration>();
    }
    #[test]
    fn checked_random_keypair_signs_and_verifies() {
        test_checked_random_keypair_signs_and_verifies::<SmallConfiguration>();
    }
    #[test]
    fn signature_verification_different_messages() {
        test_signature_verification_different_messages::<SmallConfiguration>();
    }
    #[test]
    fn signature_verification_different_keys() {
        test_signature_verification_different_keys::<SmallConfiguration>();
    }
    #[test]
    fn verify_cache_rejects_variable_length_tuple_splice() {
        test_verify_cache_rejects_variable_length_tuple_splice::<SmallConfiguration>();
    }
    #[test]
    fn verify_rejects_identity_signature_as_parse_error() {
        let (pk, _sk) =
            BlsImpl::<SmallConfiguration>::keypair(KeyGenOption::Random).expect("BLS keypair");
        let sig = G1Affine::identity().to_compressed();
        let err = BlsImpl::<SmallConfiguration>::verify(b"identity-small", sig.as_ref(), &pk)
            .expect_err("identity signature must be rejected");
        assert!(matches!(err, crate::Error::Parse(_)));
    }
    #[test]
    fn parse_public_key_rejects_identity() {
        let identity = G2Affine::identity().to_compressed();
        assert!(BlsImpl::<SmallConfiguration>::parse_public_key(identity.as_ref()).is_err());
    }
    #[test]
    fn parse_public_key_rejects_all_zero_material() {
        test_parse_public_key_rejects_all_zero_material::<SmallConfiguration>();
    }
    #[test]
    fn parse_public_key_rejects_non_subgroup_point() {
        let unchecked = G2Affine::from_compressed_unchecked(&NON_SUBGROUP_G2)
            .into_option()
            .expect("tripwire fixture is a compressed on-curve G2 point");
        assert!(bool::from(unchecked.is_on_curve()));
        assert!(!bool::from(unchecked.is_torsion_free()));
        assert!(
            BlsImpl::<SmallConfiguration>::parse_public_key(&NON_SUBGROUP_G2).is_err(),
            "BLS-small parser must enforce the G2 prime-order subgroup"
        );
    }
    #[test]
    fn parse_private_key_rejects_zero() {
        let zero = [0u8; 32];
        assert!(BlsImpl::<SmallConfiguration>::parse_private_key(&zero).is_err());
    }
    #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
    #[test]
    fn fallible_paths_reject_corrupted_stored_secret() {
        test_fallible_paths_reject_corrupted_stored_secret::<SmallConfiguration>();
    }
    #[test]
    fn aggregate_same_message_rejects_identity_inputs() {
        let msg = b"aggregate-identity-small";
        let sig = G1Affine::identity().to_compressed();
        let pk = G2Affine::identity().to_compressed();
        let signatures: [&[u8]; 1] = [sig.as_ref()];
        let public_keys: [&[u8]; 1] = [pk.as_ref()];
        assert!(
            BlsImpl::<SmallConfiguration>::verify_aggregate_same_message(
                msg,
                &signatures,
                &public_keys
            )
            .is_err()
        );
    }
    #[test]
    fn aggregate_same_message_rejects_duplicate_public_keys() {
        let (pk, sk) =
            BlsImpl::<SmallConfiguration>::keypair(KeyGenOption::Random).expect("BLS keypair");
        let msg = b"aggregate-duplicate-pk-small";
        let sig = BlsImpl::<SmallConfiguration>::sign(msg, &sk).expect("BLS sign");
        let pk_bytes = pk.to_bytes();
        let signatures: Vec<&[u8]> = vec![sig.as_slice(), sig.as_slice()];
        let public_keys: Vec<&[u8]> = vec![pk_bytes.as_slice(), pk_bytes.as_slice()];
        assert!(
            BlsImpl::<SmallConfiguration>::verify_aggregate_same_message(
                msg,
                &signatures,
                &public_keys
            )
            .is_err()
        );
        let aggregate =
            BlsImpl::<SmallConfiguration>::aggregate_signatures(&[sig.as_slice(), sig.as_slice()])
                .expect("aggregate signatures");
        assert!(
            BlsImpl::<SmallConfiguration>::verify_preaggregated_same_message(
                msg,
                &aggregate,
                &public_keys,
            )
            .is_err()
        );
    }
    #[test]
    fn aggregate_same_message_rejects_duplicate_public_key_content() {
        test_aggregate_rejects_duplicate_public_key_content::<SmallConfiguration>();
    }
    #[test]
    fn aggregate_same_message_rejects_all_zero_public_key_material() {
        test_aggregate_rejects_all_zero_public_key_material::<SmallConfiguration>();
    }
    #[test]
    fn aggregate_same_message_rejects_canceling_pairs() {
        let msg = b"aggregate-canceling-small";
        let pk = G2Affine::generator() * Scalar::from(9u64);
        let sig = G1Affine::generator() * Scalar::from(13u64);
        let pk_bytes = G2Affine::from(pk).to_compressed();
        let sig_bytes = G1Affine::from(sig).to_compressed();
        let pk_neg_bytes = G2Affine::from(-pk).to_compressed();
        let sig_neg_bytes = G1Affine::from(-sig).to_compressed();
        let signatures: [&[u8]; 2] = [sig_bytes.as_ref(), sig_neg_bytes.as_ref()];
        let public_keys: [&[u8]; 2] = [pk_bytes.as_ref(), pk_neg_bytes.as_ref()];
        assert!(
            BlsImpl::<SmallConfiguration>::verify_aggregate_same_message(
                msg,
                &signatures,
                &public_keys
            )
            .is_err()
        );
        assert!(
            BlsImpl::<SmallConfiguration>::aggregate_signatures(&signatures).is_err(),
            "identity aggregate must be rejected"
        );
        let non_identity_aggregate =
            BlsImpl::<SmallConfiguration>::aggregate_signatures(&[sig_bytes.as_ref()])
                .expect("single non-identity aggregate");
        assert!(
            BlsImpl::<SmallConfiguration>::verify_preaggregated_same_message(
                msg,
                &non_identity_aggregate,
                &public_keys,
            )
            .is_err(),
            "canceling aggregate public key must be rejected"
        );
    }
    #[test]
    fn aggregate_multi_message_rejects_duplicate_messages() {
        let (pk1, sk1) =
            BlsImpl::<SmallConfiguration>::keypair(KeyGenOption::Random).expect("BLS keypair");
        let (pk2, sk2) =
            BlsImpl::<SmallConfiguration>::keypair(KeyGenOption::Random).expect("BLS keypair");
        let msg1 = b"duplicate-msg-small".to_vec();
        let msg2 = msg1.clone();
        let sig1 = BlsImpl::<SmallConfiguration>::sign(&msg1, &sk1).expect("BLS sign");
        let sig2 = BlsImpl::<SmallConfiguration>::sign(&msg2, &sk2).expect("BLS sign");
        let messages: Vec<&[u8]> = vec![msg1.as_slice(), msg2.as_slice()];
        let signature_refs: Vec<&[u8]> = vec![sig1.as_slice(), sig2.as_slice()];
        let pk1_bytes = pk1.to_bytes();
        let pk2_bytes = pk2.to_bytes();
        let public_key_refs: Vec<&[u8]> = vec![pk1_bytes.as_slice(), pk2_bytes.as_slice()];
        assert!(
            BlsImpl::<SmallConfiguration>::verify_aggregate_multi_message(
                &messages,
                &signature_refs,
                &public_key_refs,
            )
            .is_err(),
            "duplicate messages must be rejected"
        );
    }
    #[test]
    fn aggregate_multi_message_rejects_canceling_signatures() {
        let msg1 = b"canceling-multi-message-small-a";
        let msg2 = b"canceling-multi-message-small-b";
        let sig = G1Affine::generator() * Scalar::from(13u64);
        let pk1 = G2Affine::generator() * Scalar::from(23u64);
        let pk2 = G2Affine::generator() * Scalar::from(29u64);
        let sig_bytes = G1Affine::from(sig).to_compressed();
        let sig_neg_bytes = G1Affine::from(-sig).to_compressed();
        let pk1_bytes = G2Affine::from(pk1).to_compressed();
        let pk2_bytes = G2Affine::from(pk2).to_compressed();
        let messages: [&[u8]; 2] = [msg1.as_ref(), msg2.as_ref()];
        let signatures: [&[u8]; 2] = [sig_bytes.as_ref(), sig_neg_bytes.as_ref()];
        let public_keys: [&[u8]; 2] = [pk1_bytes.as_ref(), pk2_bytes.as_ref()];
        assert!(
            BlsImpl::<SmallConfiguration>::verify_aggregate_multi_message(
                &messages,
                &signatures,
                &public_keys,
            )
            .is_err(),
            "identity aggregate signature must be rejected before pairing"
        );
    }
    #[test]
    fn multi_message_rejects_balancing_altered_signatures() {
        let (pk1, sk1) =
            BlsImpl::<SmallConfiguration>::keypair(KeyGenOption::UseSeed(vec![0x41; 32]))
                .expect("first compact BLS keypair");
        let (pk2, sk2) =
            BlsImpl::<SmallConfiguration>::keypair(KeyGenOption::UseSeed(vec![0x42; 32]))
                .expect("second compact BLS keypair");
        let msg1 = b"independent-small-a";
        let msg2 = b"independent-small-b";
        let sig1 =
            BlsImpl::<SmallConfiguration>::sign(msg1, &sk1).expect("first compact BLS signature");
        let sig2 =
            BlsImpl::<SmallConfiguration>::sign(msg2, &sk2).expect("second compact BLS signature");
        let sig1_encoded: [u8; 48] = sig1.as_slice().try_into().expect("small signature length");
        let sig2_encoded: [u8; 48] = sig2.as_slice().try_into().expect("small signature length");
        let sig1_point = G1Affine::from_compressed(&sig1_encoded)
            .into_option()
            .expect("first canonical signature");
        let sig2_point = G1Affine::from_compressed(&sig2_encoded)
            .into_option()
            .expect("second canonical signature");
        let delta = G1Affine::generator() * Scalar::from(43_u64);
        let altered_sig1 = G1Affine::from(G1Projective::from(sig1_point) + delta).to_compressed();
        let altered_sig2 = G1Affine::from(G1Projective::from(sig2_point) - delta).to_compressed();
        BlsImpl::<SmallConfiguration>::verify(msg1, altered_sig1.as_ref(), &pk1)
            .expect_err("first altered signature must fail independently");
        BlsImpl::<SmallConfiguration>::verify(msg2, altered_sig2.as_ref(), &pk2)
            .expect_err("second altered signature must fail independently");
        let pk1_bytes = pk1.to_bytes();
        let pk2_bytes = pk2.to_bytes();
        let messages: [&[u8]; 2] = [msg1.as_ref(), msg2.as_ref()];
        let signatures: [&[u8]; 2] = [altered_sig1.as_ref(), altered_sig2.as_ref()];
        let public_keys: [&[u8]; 2] = [pk1_bytes.as_ref(), pk2_bytes.as_ref()];
        BlsImpl::<SmallConfiguration>::verify_aggregate_multi_message(
            &messages,
            &signatures,
            &public_keys,
        )
        .expect_err("multi-message verification must prove each signature independently");
    }
}
