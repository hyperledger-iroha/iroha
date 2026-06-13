//! Regression tests for ML-DSA key pair construction.

mod mldsa_tests {
    use iroha_crypto::{
        Algorithm, Error, ExposedPrivateKey, HashOf, KeyPair, PrivateKey, PublicKey, Signature,
        SignatureOf,
    };
    use pqcrypto_mldsa::mldsa65;
    use pqcrypto_traits::sign::{PublicKey as _, SecretKey as _};

    fn checked_mldsa_public_key_payload(keypair: &KeyPair) -> &[u8] {
        let (algorithm, payload) = keypair
            .public_key()
            .try_to_bytes()
            .expect("fixture ML-DSA public key must be well-formed");
        assert_eq!(algorithm, Algorithm::MlDsa);
        payload
    }

    fn checked_mldsa_keypair_from_seed(label: &[u8]) -> KeyPair {
        KeyPair::try_from_seed(label.to_vec(), Algorithm::MlDsa)
            .expect("generate checked seeded ML-DSA keypair")
    }

    fn checked_mldsa_signature(keypair: &KeyPair, message: &[u8]) -> Signature {
        Signature::try_new(keypair.private_key(), message).expect("sign checked ML-DSA fixture")
    }

    fn seeded_pair(label: &[u8]) -> (mldsa65::PublicKey, mldsa65::SecretKey) {
        let kp = checked_mldsa_keypair_from_seed(label);
        let pk_bytes = checked_mldsa_public_key_payload(&kp);
        let sk_bytes = kp.private_key().to_bytes().1;
        let pk = mldsa65::PublicKey::from_bytes(pk_bytes)
            .expect("seeded ML-DSA public key bytes should decode");
        let sk = mldsa65::SecretKey::from_bytes(&sk_bytes)
            .expect("seeded ML-DSA secret key bytes should decode");
        (pk, sk)
    }

    #[test]
    fn keypair_new_accepts_matching_mldsa_keys() {
        let (pk, sk) = seeded_pair(b"iroha:ml-dsa:keypair:accept");
        let public_key = PublicKey::from_bytes(Algorithm::MlDsa, pk.as_bytes())
            .expect("valid ML-DSA public key");
        let private_key = PrivateKey::from_bytes(Algorithm::MlDsa, sk.as_bytes())
            .expect("valid ML-DSA private key");

        let pair = KeyPair::new(public_key, private_key);

        assert!(pair.is_ok(), "expected matching ML-DSA keys to succeed");
    }

    #[test]
    fn keypair_new_rejects_mismatched_mldsa_keys() {
        let (_, sk) = seeded_pair(b"iroha:ml-dsa:keypair:mismatch:sk");
        let (other_pk, _) = seeded_pair(b"iroha:ml-dsa:keypair:mismatch:pk");

        let public_key = PublicKey::from_bytes(Algorithm::MlDsa, other_pk.as_bytes())
            .expect("valid ML-DSA public key");
        let private_key = PrivateKey::from_bytes(Algorithm::MlDsa, sk.as_bytes())
            .expect("valid ML-DSA private key");

        let result = KeyPair::new(public_key, private_key);

        assert!(
            matches!(result, Err(Error::KeyGen(_))),
            "expected ML-DSA mismatch to error"
        );
    }

    #[test]
    fn keypair_from_seed_is_deterministic() {
        let seed = b"deterministic-ml-dsa-seed".to_vec();
        let kp_a = KeyPair::from_seed(seed.clone(), Algorithm::MlDsa);
        let kp_b = KeyPair::from_seed(seed, Algorithm::MlDsa);

        assert_eq!(kp_a.public_key(), kp_b.public_key());

        let (_, first_secret_bytes) = kp_a.private_key().to_bytes();
        let (_, second_secret_bytes) = kp_b.private_key().to_bytes();
        assert_eq!(first_secret_bytes, second_secret_bytes);
    }

    #[test]
    fn fallible_keypair_from_seed_matches_compat_constructor() {
        let seed = b"fallible-deterministic-ml-dsa-seed".to_vec();
        let infallible = KeyPair::from_seed(seed.clone(), Algorithm::MlDsa);
        let fallible =
            KeyPair::try_from_seed(seed, Algorithm::MlDsa).expect("fallible seeded keypair");

        assert_eq!(fallible.public_key(), infallible.public_key());
        assert_eq!(fallible.private_key(), infallible.private_key());
    }

    #[cfg(feature = "rand")]
    #[test]
    fn fallible_random_keypair_signs_and_verifies() {
        let kp = KeyPair::try_random_with_algorithm(Algorithm::MlDsa)
            .expect("fallible random ML-DSA keypair");
        let message = b"ml-dsa fallible random keypair";

        let signature =
            Signature::try_new(kp.private_key(), message).expect("random ML-DSA keypair signs");

        signature
            .verify(kp.public_key(), message)
            .expect("random ML-DSA signature verifies");
    }

    #[test]
    fn keypair_from_seed_changes_with_seed() {
        let first = KeyPair::from_seed(b"ml-dsa-seed-a".to_vec(), Algorithm::MlDsa);
        let second = KeyPair::from_seed(b"ml-dsa-seed-b".to_vec(), Algorithm::MlDsa);

        assert_ne!(first.public_key(), second.public_key());

        let (_, first_secret_bytes) = first.private_key().to_bytes();
        let (_, second_secret_bytes) = second.private_key().to_bytes();
        assert_ne!(first_secret_bytes, second_secret_bytes);
    }

    #[test]
    fn keypair_from_seed_signs_and_verifies() {
        let kp = checked_mldsa_keypair_from_seed(b"ml-dsa-sign");
        let message = b"ml-dsa signing smoke test";

        let signature = checked_mldsa_signature(&kp, message);

        assert!(
            signature.verify(kp.public_key(), message).is_ok(),
            "ML-DSA signature generated from seeded key should verify"
        );
    }

    #[test]
    fn fallible_signature_constructor_signs_and_verifies() {
        let kp = checked_mldsa_keypair_from_seed(b"ml-dsa-try-sign");
        let message = b"ml-dsa fallible signing smoke test";

        let signature =
            Signature::try_new(kp.private_key(), message).expect("fallible signing should pass");

        signature
            .verify(kp.public_key(), message)
            .expect("fallible signature should verify");
    }

    #[test]
    fn typed_fallible_signature_constructors_sign_and_verify() {
        let kp = checked_mldsa_keypair_from_seed(b"ml-dsa-typed-try-sign");
        let value = ();

        let signature =
            SignatureOf::<()>::try_new(kp.private_key(), &value).expect("typed signing passes");
        signature
            .verify(kp.public_key(), &value)
            .expect("typed signature verifies");

        let from_hash = SignatureOf::<()>::try_from_hash(kp.private_key(), HashOf::new(&value))
            .expect("typed hash signing passes");
        from_hash
            .verify(kp.public_key(), &value)
            .expect("typed hash signature verifies");
    }

    #[test]
    fn signature_rejects_modified_message() {
        let kp = checked_mldsa_keypair_from_seed(b"ml-dsa-modified-message");
        let message = b"ml-dsa original message";
        let signature = checked_mldsa_signature(&kp, message);

        let result = signature.verify(kp.public_key(), b"ml-dsa modified message");

        assert!(matches!(result, Err(Error::BadSignature)));
    }

    #[test]
    fn signature_rejects_different_mldsa_public_key() {
        let signer = checked_mldsa_keypair_from_seed(b"ml-dsa-signer");
        let verifier = checked_mldsa_keypair_from_seed(b"ml-dsa-verifier");
        let message = b"ml-dsa public key mismatch";
        let signature = checked_mldsa_signature(&signer, message);

        let result = signature.verify(verifier.public_key(), message);

        assert!(matches!(result, Err(Error::BadSignature)));
    }

    #[test]
    fn signature_rejects_invalid_mldsa_signature_length() {
        let kp = checked_mldsa_keypair_from_seed(b"ml-dsa-short-signature");
        let signature = Signature::from_bytes(&[0u8; 8]);

        let result = signature.verify(kp.public_key(), b"message");

        assert!(matches!(result, Err(Error::BadSignature)));
    }

    #[test]
    fn mldsa_signature_payload_bytes_roundtrip() {
        let kp = checked_mldsa_keypair_from_seed(b"ml-dsa-signature-payload-roundtrip");
        let message = b"ml-dsa signature payload roundtrip";
        let signature = checked_mldsa_signature(&kp, message);

        let decoded = Signature::from_bytes(signature.payload());

        assert_eq!(decoded.payload(), signature.payload());
        assert!(decoded.verify(kp.public_key(), message).is_ok());
    }

    #[test]
    fn mldsa_key_parsers_reject_invalid_lengths() {
        let public = PublicKey::from_bytes(Algorithm::MlDsa, &[0u8; 8]);
        let private = PrivateKey::from_bytes(Algorithm::MlDsa, &[0u8; 8]);

        assert!(public.is_err());
        assert!(private.is_err());
    }

    #[test]
    fn mldsa_prefixed_public_key_roundtrips() {
        let kp = KeyPair::from_seed(b"ml-dsa-prefixed-public-key".to_vec(), Algorithm::MlDsa);
        let encoded = kp
            .public_key()
            .try_to_prefixed_string()
            .expect("prefixed ML-DSA public key");

        let decoded: PublicKey = encoded.parse().expect("prefixed ML-DSA public key");

        assert_eq!(decoded, kp.public_key().clone());
        assert!(encoded.starts_with("ml-dsa:"));
    }

    #[test]
    fn fallible_multihash_formatters_roundtrip() {
        let kp = KeyPair::from_seed(b"ml-dsa-fallible-multihash".to_vec(), Algorithm::MlDsa);
        let public_bare = kp
            .public_key()
            .try_to_multihash_string()
            .expect("public key multihash");
        let public_prefixed = kp
            .public_key()
            .try_to_prefixed_string()
            .expect("prefixed public key multihash");

        let public_from_bare: PublicKey = public_bare.parse().expect("bare public key multihash");
        let public_from_prefixed: PublicKey = public_prefixed
            .parse()
            .expect("prefixed public key multihash");
        assert_eq!(public_from_bare, kp.public_key().clone());
        assert_eq!(public_from_prefixed, kp.public_key().clone());
        assert!(public_prefixed.starts_with("ml-dsa:"));

        let exposed = ExposedPrivateKey(kp.private_key().clone());
        let private_bare = exposed
            .try_to_multihash_string()
            .expect("private key multihash");
        let private_prefixed = exposed
            .try_to_prefixed_string()
            .expect("prefixed private key multihash");

        let private_from_bare: ExposedPrivateKey =
            private_bare.parse().expect("bare private key multihash");
        let private_from_prefixed: ExposedPrivateKey = private_prefixed
            .parse()
            .expect("prefixed private key multihash");
        assert_eq!(private_from_bare.0, kp.private_key().clone());
        assert_eq!(private_from_prefixed.0, kp.private_key().clone());
        assert!(private_prefixed.starts_with("ml-dsa:"));
    }

    #[test]
    fn mldsa_public_key_bytes_roundtrip() {
        let kp = KeyPair::from_seed(b"ml-dsa-public-key-bytes".to_vec(), Algorithm::MlDsa);
        let (algorithm, payload) = kp
            .public_key()
            .try_to_bytes()
            .expect("fixture ML-DSA public key must be well-formed");

        let decoded = PublicKey::from_bytes(algorithm, payload).expect("valid ML-DSA public key");

        assert_eq!(decoded, kp.public_key().clone());
    }

    #[test]
    fn malformed_mldsa_prefixed_public_key_is_rejected() {
        let parsed = "ml-dsa:not-hex".parse::<PublicKey>();

        assert!(parsed.is_err());
    }

    #[test]
    fn mldsa_private_key_bytes_roundtrip() {
        let kp = KeyPair::from_seed(b"ml-dsa-private-key-roundtrip".to_vec(), Algorithm::MlDsa);
        let (algorithm, secret_bytes) = kp.private_key().to_bytes();

        let decoded =
            PrivateKey::from_bytes(algorithm, &secret_bytes).expect("valid ML-DSA private key");

        assert_eq!(decoded, kp.private_key().clone());
    }

    #[test]
    fn mldsa_public_and_private_key_hex_roundtrip() {
        let kp = KeyPair::from_seed(b"ml-dsa-key-hex-roundtrip".to_vec(), Algorithm::MlDsa);
        let (public_algorithm, public_bytes) = kp
            .public_key()
            .try_to_bytes()
            .expect("fixture ML-DSA public key must be well-formed");
        let (private_algorithm, private_bytes) = kp.private_key().to_bytes();

        let public =
            PublicKey::from_hex(public_algorithm, hex::encode(public_bytes)).expect("public hex");
        let private = PrivateKey::from_hex(private_algorithm, hex::encode(&private_bytes))
            .expect("private hex");

        assert_eq!(public, kp.public_key().clone());
        assert_eq!(private, kp.private_key().clone());
    }

    #[test]
    fn public_key_from_private_key_matches_original() {
        let (pk, sk) = seeded_pair(b"iroha:ml-dsa:pk-from-sk");
        let expected_public =
            PublicKey::from_bytes(Algorithm::MlDsa, pk.as_bytes()).expect("valid public key");
        let private =
            PrivateKey::from_bytes(Algorithm::MlDsa, sk.as_bytes()).expect("valid private key");

        let derived: PublicKey = private.clone().into();

        assert_eq!(
            expected_public, derived,
            "PublicKey::from(PrivateKey) should reconstruct the original ML-DSA public key"
        );
    }

    #[test]
    fn keypair_from_private_key_restores_public_material() {
        let (pk, sk) = seeded_pair(b"iroha:ml-dsa:keypair-from-private");
        let expected_public =
            PublicKey::from_bytes(Algorithm::MlDsa, pk.as_bytes()).expect("valid public key");
        let private =
            PrivateKey::from_bytes(Algorithm::MlDsa, sk.as_bytes()).expect("valid private key");

        let keypair: KeyPair = private.clone().into();

        assert_eq!(
            expected_public,
            keypair.public_key().clone(),
            "KeyPair::from(PrivateKey) should recover the ML-DSA public key"
        );
        assert_eq!(
            private,
            keypair.private_key().clone(),
            "KeyPair::from(PrivateKey) should preserve the original ML-DSA private key"
        );
    }

    #[test]
    fn private_key_import_rejects_inconsistent_private_key() {
        let (_, sk) = seeded_pair(b"iroha:ml-dsa:keypair-from-private:tampered");
        let mut secret_bytes = sk.as_bytes().to_vec();
        let last = secret_bytes
            .last_mut()
            .expect("ML-DSA secret key has at least one byte");
        *last ^= 0x01;

        let parse_err = PrivateKey::from_bytes(Algorithm::MlDsa, &secret_bytes)
            .expect_err("tampered secret must fail during import");

        assert!(
            parse_err.to_string().contains("Inconsistent"),
            "unexpected private-key import error: {parse_err:?}"
        );
    }
}
