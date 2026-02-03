#[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
use w3f_bls::SerializableToBytes as _;

use super::{
    implementation::{BlsConfiguration, BlsImpl},
    normal::NormalConfiguration,
    small::SmallConfiguration,
};
use crate::KeyGenOption;

const MESSAGE_1: &[u8; 22] = b"This is a test message";
const MESSAGE_2: &[u8; 20] = b"Another test message";
const SEED: &[u8; 10] = &[1u8; 10];

#[allow(clippy::similar_names)]
fn test_keypair_generation_from_seed<C: BlsConfiguration>() {
    let (pk_1, sk_1) = BlsImpl::<C>::keypair(KeyGenOption::UseSeed(SEED.to_vec()));
    let (pk_2, sk_2) = BlsImpl::<C>::keypair(KeyGenOption::UseSeed(SEED.to_vec()));

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

fn test_signature_verification<C: BlsConfiguration>() {
    let (pk, sk) = BlsImpl::<C>::keypair(KeyGenOption::Random);

    let signature_1 = BlsImpl::<C>::sign(MESSAGE_1, &sk);
    BlsImpl::<C>::verify(MESSAGE_1, &signature_1, &pk)
        .expect("Signature verification should succeed");
}

fn test_signature_verification_different_messages<C: BlsConfiguration>() {
    let (pk, sk) = BlsImpl::<C>::keypair(KeyGenOption::Random);

    let signature = BlsImpl::<C>::sign(MESSAGE_1, &sk);
    BlsImpl::<C>::verify(MESSAGE_2, &signature, &pk)
        .expect_err("Signature verification for wrong message should fail");
}

#[allow(clippy::similar_names)]
fn test_signature_verification_different_keys<C: BlsConfiguration>() {
    let (_pk_1, sk_1) = BlsImpl::<C>::keypair(KeyGenOption::Random);
    let (pk_2, _sk_2) = BlsImpl::<C>::keypair(KeyGenOption::Random);

    let signature = BlsImpl::<C>::sign(MESSAGE_1, &sk_1);
    BlsImpl::<C>::verify(MESSAGE_1, &signature, &pk_2)
        .expect_err("Signature verification for wrong public key should fail");
}

mod normal {
    use super::*;
    #[cfg(feature = "bls-backend-blstrs")]
    use crate::signature::bls::implementation;
    use blstrs::{G1Affine, G2Affine, Scalar};
    use group::prime::PrimeCurveAffine;
    #[cfg(feature = "bls-backend-blstrs")]
    #[test]
    fn detect_hash_variant_normal_matches_concat() {
        let (pk, sk) = BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random);
        let msg = b"diagnostic-normal";
        let sig = BlsImpl::<NormalConfiguration>::sign(msg, &sk);
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
    fn signature_verification() {
        test_signature_verification::<NormalConfiguration>();
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
    fn verify_rejects_identity_signature_as_parse_error() {
        let (pk, _sk) = BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random);
        let sig = G2Affine::identity().to_compressed();
        let err = BlsImpl::<NormalConfiguration>::verify(b"identity", sig.as_ref(), &pk)
            .expect_err("identity signature must be rejected");
        assert!(matches!(err, crate::Error::Parse(_)));
    }

    #[test]
    fn aggregate_same_message_roundtrip() {
        let (pk1, sk1) = BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random);
        let (pk2, sk2) = BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random);
        let msg = b"aggregate-same-message";
        let sig1 = BlsImpl::<NormalConfiguration>::sign(msg, &sk1);
        let sig2 = BlsImpl::<NormalConfiguration>::sign(msg, &sk2);
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
        let (pk, sk) = BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random);
        let msg = b"aggregate-duplicate-pk";
        let sig = BlsImpl::<NormalConfiguration>::sign(msg, &sk);
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
    fn parse_public_key_rejects_identity() {
        let identity = G1Affine::identity().to_compressed();
        assert!(BlsImpl::<NormalConfiguration>::parse_public_key(identity.as_ref()).is_err());
    }

    #[test]
    fn parse_private_key_rejects_zero() {
        let zero = [0u8; 32];
        assert!(BlsImpl::<NormalConfiguration>::parse_private_key(&zero).is_err());
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

        let (pk, sk) = BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random);
        let sk = Arc::new(sk);
        let msg = b"concurrent-signing";
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let sk = Arc::clone(&sk);
                std::thread::spawn(move || BlsImpl::<NormalConfiguration>::sign(msg, &sk))
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
    }

    #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
    #[test]
    fn aggregate_multi_message_verification() {
        let (pk1, sk1) = BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random);
        let (pk2, sk2) = BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random);

        let msg1 = b"aggregate-m1";
        let msg2 = b"aggregate-m2";
        let sig1 = BlsImpl::<NormalConfiguration>::sign(msg1, &sk1);
        let sig2 = BlsImpl::<NormalConfiguration>::sign(msg2, &sk2);

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
        let (pk1, sk1) = BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random);
        let (pk2, sk2) = BlsImpl::<NormalConfiguration>::keypair(KeyGenOption::Random);
        let msg1 = b"duplicate-msg".to_vec();
        let msg2 = msg1.clone();
        let sig1 = BlsImpl::<NormalConfiguration>::sign(&msg1, &sk1);
        let sig2 = BlsImpl::<NormalConfiguration>::sign(&msg2, &sk2);
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
}

mod small {
    use super::*;
    #[cfg(feature = "bls-backend-blstrs")]
    use crate::signature::bls::implementation;
    use blstrs::{G1Affine, G2Affine, Scalar};
    use group::prime::PrimeCurveAffine;
    #[cfg(feature = "bls-backend-blstrs")]
    #[test]
    fn detect_hash_variant_small_matches_concat() {
        let (pk, sk) = BlsImpl::<SmallConfiguration>::keypair(KeyGenOption::Random);
        let msg = b"diagnostic-small";
        let sig = BlsImpl::<SmallConfiguration>::sign(msg, &sk);
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
    fn signature_verification() {
        test_signature_verification::<SmallConfiguration>();
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
    fn verify_rejects_identity_signature_as_parse_error() {
        let (pk, _sk) = BlsImpl::<SmallConfiguration>::keypair(KeyGenOption::Random);
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
    fn parse_private_key_rejects_zero() {
        let zero = [0u8; 32];
        assert!(BlsImpl::<SmallConfiguration>::parse_private_key(&zero).is_err());
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
        let (pk, sk) = BlsImpl::<SmallConfiguration>::keypair(KeyGenOption::Random);
        let msg = b"aggregate-duplicate-pk-small";
        let sig = BlsImpl::<SmallConfiguration>::sign(msg, &sk);
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
    }

    #[test]
    fn aggregate_multi_message_rejects_duplicate_messages() {
        let (pk1, sk1) = BlsImpl::<SmallConfiguration>::keypair(KeyGenOption::Random);
        let (pk2, sk2) = BlsImpl::<SmallConfiguration>::keypair(KeyGenOption::Random);
        let msg1 = b"duplicate-msg-small".to_vec();
        let msg2 = msg1.clone();
        let sig1 = BlsImpl::<SmallConfiguration>::sign(&msg1, &sk1);
        let sig2 = BlsImpl::<SmallConfiguration>::sign(&msg2, &sk2);
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
}
