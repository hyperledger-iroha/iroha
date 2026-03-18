//! Regression tests for ML-DSA key pair construction.

mod mldsa_tests {
    use iroha_crypto::{Algorithm, Error, KeyPair, PrivateKey, PublicKey, Signature};
    use pqcrypto_dilithium::dilithium3 as dilithium;
    use pqcrypto_traits::sign::{PublicKey as _, SecretKey as _};

    fn seeded_pair(label: &[u8]) -> (dilithium::PublicKey, dilithium::SecretKey) {
        let kp = KeyPair::from_seed(label.to_vec(), Algorithm::MlDsa);
        let pk_bytes = kp.public_key().to_bytes().1;
        let sk_bytes = kp.private_key().to_bytes().1;
        let pk = dilithium::PublicKey::from_bytes(pk_bytes)
            .expect("seeded ML-DSA public key bytes should decode");
        let sk = dilithium::SecretKey::from_bytes(&sk_bytes)
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
    fn keypair_from_seed_signs_and_verifies() {
        let kp = KeyPair::from_seed(b"ml-dsa-sign".to_vec(), Algorithm::MlDsa);
        let message = b"ml-dsa signing smoke test";

        let signature = Signature::new(kp.private_key(), message);

        assert!(
            signature.verify(kp.public_key(), message).is_ok(),
            "ML-DSA signature generated from seeded key should verify"
        );
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
}
