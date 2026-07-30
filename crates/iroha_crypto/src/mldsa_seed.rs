//! Safe ML-DSA-65 key derivation adapters.
//!
//! All key generation and secret-key validation goes through `soranet_pq`.
//! This module deliberately does not bind PQClean's private polynomial ABI:
//! only canonical FIPS 204 byte encodings cross the crate boundary.

/// ML-DSA-65 key derivation and public-key recovery.
pub mod mldsa65 {
    use hkdf::Hkdf;
    use pqcrypto_traits::sign::SecretKey as _;
    #[cfg(feature = "rand")]
    use rand::rngs::OsRng;
    #[cfg(feature = "rand")]
    use rand_core::TryCryptoRng;
    use sha2::Sha512;
    use soranet_pq::{
        MlDsaKeyPair, MlDsaSuite, generate_mldsa_keypair_from_fips_seed,
        mldsa_public_key_from_secret_key,
    };
    use zeroize::Zeroizing;

    use crate::{Algorithm, Error, PrivateKey, PublicKey};

    const FIPS_SEED_BYTES: usize = 32;
    const HKDF_SALT: &[u8] = b"iroha:ml-dsa:keygen:v1";
    const HKDF_INFO: &[u8] = b"iroha:ml-dsa:fips204:keypair";

    /// Derive a canonical ML-DSA-65 keypair from arbitrary-length secret seed
    /// material.
    pub fn keypair_from_seed(seed: &[u8]) -> Result<(PublicKey, PrivateKey), Error> {
        validate_seed_material_not_all_zero(seed)?;
        let seed_material = derive_seed_material(seed)?;
        keypair_from_fips_seed(&seed_material)
    }

    /// Generate a canonical ML-DSA-65 keypair using the operating-system RNG.
    #[cfg(feature = "rand")]
    pub fn random_keypair() -> Result<(PublicKey, PrivateKey), Error> {
        random_keypair_from_rng(&mut OsRng)
    }

    #[cfg(feature = "rand")]
    fn random_keypair_from_rng<R>(rng: &mut R) -> Result<(PublicKey, PrivateKey), Error>
    where
        R: TryCryptoRng,
    {
        let mut seed_material = Zeroizing::new([0u8; FIPS_SEED_BYTES]);
        rng.try_fill_bytes(seed_material.as_mut())
            .map_err(|err| Error::KeyGen(format!("ML-DSA OS RNG failed: {err}")))?;
        validate_seed_material_not_all_zero(seed_material.as_slice())?;
        keypair_from_fips_seed(&seed_material)
    }

    /// Reconstruct and validate the public key committed by a canonical
    /// ML-DSA-65 secret key.
    pub fn public_key_from_secret(
        secret_key: &pqcrypto_mldsa::mldsa65::SecretKey,
    ) -> Result<PublicKey, Error> {
        let public_key =
            mldsa_public_key_from_secret_key(MlDsaSuite::MlDsa65, secret_key.as_bytes())
                .map_err(|err| Error::KeyGen(err.to_string()))?;

        PublicKey::from_bytes(Algorithm::MlDsa, &public_key)
            .map_err(|err| Error::KeyGen(err.to_string()))
    }

    fn derive_seed_material(seed: &[u8]) -> Result<Zeroizing<[u8; FIPS_SEED_BYTES]>, Error> {
        let kdf = Hkdf::<Sha512>::new(Some(HKDF_SALT), seed);
        let mut out = Zeroizing::new([0u8; FIPS_SEED_BYTES]);
        kdf.expand(HKDF_INFO, out.as_mut())
            .map_err(|_| Error::KeyGen(String::from("ML-DSA HKDF seed expansion failed")))?;
        Ok(out)
    }

    fn validate_seed_material_not_all_zero(seed: &[u8]) -> Result<(), Error> {
        if !seed.is_empty() && seed.iter().all(|&byte| byte == 0) {
            return Err(Error::KeyGen(String::from(
                "ML-DSA seed material must not be all zero",
            )));
        }
        Ok(())
    }

    fn keypair_from_fips_seed(
        seed_material: &[u8; FIPS_SEED_BYTES],
    ) -> Result<(PublicKey, PrivateKey), Error> {
        let keypair = generate_mldsa_keypair_from_fips_seed(MlDsaSuite::MlDsa65, seed_material)
            .map_err(|err| Error::KeyGen(err.to_string()))?;
        convert_keypair(&keypair)
    }

    fn convert_keypair(keypair: &MlDsaKeyPair) -> Result<(PublicKey, PrivateKey), Error> {
        let public_key = PublicKey::from_bytes(Algorithm::MlDsa, keypair.public_key())
            .map_err(|err| Error::KeyGen(err.to_string()))?;
        let private_key = PrivateKey::from_bytes(Algorithm::MlDsa, keypair.secret_key())
            .map_err(|err| Error::KeyGen(err.to_string()))?;

        Ok((public_key, private_key))
    }

    #[cfg(test)]
    mod tests {
        #[cfg(feature = "rand")]
        use core::fmt;

        use pqcrypto_mldsa::{mldsa44, mldsa65, mldsa87};
        use pqcrypto_traits::sign::{
            DetachedSignature as _, PublicKey as _, SecretKey as _, VerificationError,
        };
        use rand_core::RngCore as _;
        #[cfg(feature = "rand")]
        use rand_core::{TryCryptoRng, TryRngCore};

        use super::*;

        const RELEASE_KAT_FIPS_SEED: [u8; 32] =
            hex_literal::hex!("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");
        const RELEASE_KAT_SIGNING_SEED: [u8; 32] =
            hex_literal::hex!("a0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebf");
        const RELEASE_KAT_SIGNING_COINS: [u8; 32] =
            hex_literal::hex!("e43f5d01a367368b5db60f4e328dc0a4fb64a3563a9a6e0cc2dd80e02e7c0b5d");
        const RELEASE_KAT_SIGNING_PERSONALIZATION: &[u8] =
            b"iroha-crypto:mldsa-native-release-kat:signing-coins:v1";
        const RELEASE_KAT_CONTEXT: &[u8] = b"iroha-crypto:mldsa-native-release-kat:v1";
        const RELEASE_KAT_MESSAGE: &[u8] = b"native Rust ML-DSA and PQClean interoperability";
        const ML_DSA_SECRET_ETA_OFFSET: usize = 128;
        const RELEASE_KAT_DIGESTS: [(MlDsaSuite, [u8; 32], [u8; 32], [u8; 32]); 3] = [
            (
                MlDsaSuite::MlDsa44,
                hex_literal::hex!(
                    "9f107644c1084526af3bc8098680b05499a2325a644e388fb4f970e058d19d46"
                ),
                hex_literal::hex!(
                    "04bf6b9f579166a627961dfc5c3bf9717df868db88863856356c4668c8b56b0b"
                ),
                hex_literal::hex!(
                    "8292d37f3ecb47b164d6940bd7fe41d87f2216e5e3061c79f9b86f0333148074"
                ),
            ),
            (
                MlDsaSuite::MlDsa65,
                hex_literal::hex!(
                    "d666806e11cee19a7c989f7445f90dd419cf4d2d51db8c0fdb4c0f0a542238c9"
                ),
                hex_literal::hex!(
                    "9f1e24f47795fe50040384e3d6183988047170fa2d866406b70fe0a3f8216063"
                ),
                hex_literal::hex!(
                    "7762d64570cf0c00c27b7f2ed2df2df2f2397a7461d3d5998dd1aae115cf3ba7"
                ),
            ),
            (
                MlDsaSuite::MlDsa87,
                hex_literal::hex!(
                    "91dc389cfaa01470b7f66eee45a4ae9026d154817c754dfe22298b3fa241ffcd"
                ),
                hex_literal::hex!(
                    "764d3e223ed90c07bc91a0ab6ecd170e5c66ffe39f7039298596039a36005435"
                ),
                hex_literal::hex!(
                    "4af9cace582b813f6c8ba071bceaaffc26d6efa79bdc58623a46f79cb0edc90a"
                ),
            ),
        ];

        #[cfg(feature = "rand")]
        struct FailingTryRng;

        #[cfg(feature = "rand")]
        #[derive(Debug)]
        struct FailingTryRngError;

        #[cfg(feature = "rand")]
        impl fmt::Display for FailingTryRngError {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("failing ML-DSA RNG")
            }
        }

        #[cfg(feature = "rand")]
        impl TryRngCore for FailingTryRng {
            type Error = FailingTryRngError;

            fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
                Err(FailingTryRngError)
            }

            fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
                Err(FailingTryRngError)
            }

            fn try_fill_bytes(&mut self, _dest: &mut [u8]) -> Result<(), Self::Error> {
                Err(FailingTryRngError)
            }
        }

        #[cfg(feature = "rand")]
        impl TryCryptoRng for FailingTryRng {}

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

        fn release_kat_keypair(suite: MlDsaSuite) -> MlDsaKeyPair {
            soranet_pq::generate_mldsa_keypair_from_fips_seed(suite, &RELEASE_KAT_FIPS_SEED)
                .expect("fixed nonzero FIPS seed must generate a canonical ML-DSA keypair")
        }

        fn release_kat_signing_rng() -> soranet_pq::HedgedChaCha20Rng {
            soranet_pq::deterministic_chacha20_rng(
                soranet_pq::HedgedRngSeed::from_entropy(RELEASE_KAT_SIGNING_SEED),
                RELEASE_KAT_SIGNING_PERSONALIZATION,
            )
        }

        fn release_kat_signing_coins() -> [u8; 32] {
            let mut rng = release_kat_signing_rng();
            let mut coins = [0_u8; 32];
            rng.fill_bytes(&mut coins);
            coins
        }

        fn release_kat_native_signature(
            suite: MlDsaSuite,
            secret_key: &[u8],
            context: &[u8],
        ) -> soranet_pq::MlDsaSignature {
            let mut rng = release_kat_signing_rng();
            soranet_pq::sign_mldsa(suite, secret_key, context, RELEASE_KAT_MESSAGE, &mut rng)
                .expect("native ML-DSA signing with fixed nonzero coins must succeed")
        }

        fn pqcrypto_verify(
            suite: MlDsaSuite,
            public_key: &[u8],
            context: &[u8],
            message: &[u8],
            signature: &[u8],
        ) -> Result<(), VerificationError> {
            match suite {
                MlDsaSuite::MlDsa44 => {
                    let public_key =
                        mldsa44::PublicKey::from_bytes(public_key).expect("ML-DSA-44 public key");
                    let signature = mldsa44::DetachedSignature::from_bytes(signature)
                        .expect("ML-DSA-44 signature");
                    mldsa44::verify_detached_signature_ctx(
                        &signature,
                        message,
                        context,
                        &public_key,
                    )
                }
                MlDsaSuite::MlDsa65 => {
                    let public_key =
                        mldsa65::PublicKey::from_bytes(public_key).expect("ML-DSA-65 public key");
                    let signature = mldsa65::DetachedSignature::from_bytes(signature)
                        .expect("ML-DSA-65 signature");
                    mldsa65::verify_detached_signature_ctx(
                        &signature,
                        message,
                        context,
                        &public_key,
                    )
                }
                MlDsaSuite::MlDsa87 => {
                    let public_key =
                        mldsa87::PublicKey::from_bytes(public_key).expect("ML-DSA-87 public key");
                    let signature = mldsa87::DetachedSignature::from_bytes(signature)
                        .expect("ML-DSA-87 signature");
                    mldsa87::verify_detached_signature_ctx(
                        &signature,
                        message,
                        context,
                        &public_key,
                    )
                }
            }
        }

        fn pqcrypto_sign(
            suite: MlDsaSuite,
            secret_key: &[u8],
            context: &[u8],
            message: &[u8],
        ) -> Vec<u8> {
            match suite {
                MlDsaSuite::MlDsa44 => {
                    let secret_key =
                        mldsa44::SecretKey::from_bytes(secret_key).expect("ML-DSA-44 secret key");
                    mldsa44::detached_sign_ctx(message, context, &secret_key)
                        .as_bytes()
                        .to_vec()
                }
                MlDsaSuite::MlDsa65 => {
                    let secret_key =
                        mldsa65::SecretKey::from_bytes(secret_key).expect("ML-DSA-65 secret key");
                    mldsa65::detached_sign_ctx(message, context, &secret_key)
                        .as_bytes()
                        .to_vec()
                }
                MlDsaSuite::MlDsa87 => {
                    let secret_key =
                        mldsa87::SecretKey::from_bytes(secret_key).expect("ML-DSA-87 secret key");
                    mldsa87::detached_sign_ctx(message, context, &secret_key)
                        .as_bytes()
                        .to_vec()
                }
            }
        }

        fn assert_malformed_eta_rejected(error: soranet_pq::MlDsaError, suite: MlDsaSuite) {
            assert!(
                matches!(
                    &error,
                    soranet_pq::MlDsaError::SecretKeyMismatch {
                        suite: actual,
                        kind,
                    } if actual.suite_id() == suite.suite_id()
                        && kind.contains("s1 or s2")
                ),
                "expected {suite:?} noncanonical eta encoding rejection, got {error:?}"
            );
        }

        #[test]
        fn native_backend_release_kats_are_exact_and_pqcrypto_verifiable() {
            assert_eq!(
                release_kat_signing_coins(),
                RELEASE_KAT_SIGNING_COINS,
                "the deterministic signing RNG must yield the pinned FIPS 204 coins"
            );
            for (suite, expected_public, expected_secret, expected_signature) in RELEASE_KAT_DIGESTS
            {
                let keypair = release_kat_keypair(suite);
                let signature =
                    release_kat_native_signature(suite, keypair.secret_key(), RELEASE_KAT_CONTEXT);
                assert_eq!(
                    crate::sha256(keypair.public_key()),
                    expected_public,
                    "{suite:?} public-key SHA-256 drifted"
                );
                assert_eq!(
                    crate::sha256(keypair.secret_key()),
                    expected_secret,
                    "{suite:?} secret-key SHA-256 drifted"
                );
                assert_eq!(
                    crate::sha256(signature.as_bytes()),
                    expected_signature,
                    "{suite:?} signature SHA-256 drifted"
                );
                pqcrypto_verify(
                    suite,
                    keypair.public_key(),
                    RELEASE_KAT_CONTEXT,
                    RELEASE_KAT_MESSAGE,
                    signature.as_bytes(),
                )
                .expect("native release KAT signature must verify with pqcrypto");
            }
        }

        #[test]
        fn native_and_pqcrypto_signers_interoperate_for_every_parameter_set() {
            for suite in [
                MlDsaSuite::MlDsa44,
                MlDsaSuite::MlDsa65,
                MlDsaSuite::MlDsa87,
            ] {
                let keypair = release_kat_keypair(suite);
                let native_signature =
                    release_kat_native_signature(suite, keypair.secret_key(), RELEASE_KAT_CONTEXT);
                pqcrypto_verify(
                    suite,
                    keypair.public_key(),
                    RELEASE_KAT_CONTEXT,
                    RELEASE_KAT_MESSAGE,
                    native_signature.as_bytes(),
                )
                .unwrap_or_else(|error| {
                    panic!("{suite:?} native signature failed pqcrypto verification: {error}")
                });

                let pqcrypto_signature = pqcrypto_sign(
                    suite,
                    keypair.secret_key(),
                    RELEASE_KAT_CONTEXT,
                    RELEASE_KAT_MESSAGE,
                );
                soranet_pq::verify_mldsa(
                    suite,
                    keypair.public_key(),
                    RELEASE_KAT_CONTEXT,
                    RELEASE_KAT_MESSAGE,
                    &pqcrypto_signature,
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "{suite:?} pqcrypto signature failed native-wrapper verification: {error}"
                    )
                });
            }
        }

        #[test]
        fn noncanonical_eta_secret_encodings_fail_closed_for_every_parameter_set() {
            for suite in [
                MlDsaSuite::MlDsa44,
                MlDsaSuite::MlDsa65,
                MlDsaSuite::MlDsa87,
            ] {
                let keypair = release_kat_keypair(suite);
                let mut malformed = keypair.secret_key().to_vec();
                malformed[ML_DSA_SECRET_ETA_OFFSET] = 0xff;
                assert_eq!(malformed.len(), suite.secret_key_len());

                let error = soranet_pq::validate_mldsa_secret_key(suite, &malformed)
                    .expect_err("unused eta encodings must fail strict secret-key validation");
                assert_malformed_eta_rejected(error, suite);

                let error = soranet_pq::mldsa_public_key_from_secret_key(suite, &malformed)
                    .expect_err("unused eta encodings must not reconstruct a public key");
                assert_malformed_eta_rejected(error, suite);

                let mut rng = release_kat_signing_rng();
                let error = soranet_pq::sign_mldsa(
                    suite,
                    &malformed,
                    RELEASE_KAT_CONTEXT,
                    RELEASE_KAT_MESSAGE,
                    &mut rng,
                )
                .expect_err("unused eta encodings must fail before signing");
                assert_malformed_eta_rejected(error, suite);
            }
        }

        #[test]
        fn context_and_parameter_sets_are_domain_separated() {
            const OTHER_CONTEXT: &[u8] = b"iroha-crypto:mldsa-native-release-kat:other-purpose:v1";

            let mut artifacts = Vec::new();
            for suite in [
                MlDsaSuite::MlDsa44,
                MlDsaSuite::MlDsa65,
                MlDsaSuite::MlDsa87,
            ] {
                let keypair = release_kat_keypair(suite);
                let primary =
                    release_kat_native_signature(suite, keypair.secret_key(), RELEASE_KAT_CONTEXT);
                let other =
                    release_kat_native_signature(suite, keypair.secret_key(), OTHER_CONTEXT);

                assert_ne!(
                    primary.as_bytes(),
                    other.as_bytes(),
                    "{suite:?} must bind its FIPS 204 context even with identical signing coins"
                );
                soranet_pq::verify_mldsa(
                    suite,
                    keypair.public_key(),
                    RELEASE_KAT_CONTEXT,
                    RELEASE_KAT_MESSAGE,
                    primary.as_bytes(),
                )
                .expect("signature verifies for its declared purpose");
                soranet_pq::verify_mldsa(
                    suite,
                    keypair.public_key(),
                    OTHER_CONTEXT,
                    RELEASE_KAT_MESSAGE,
                    primary.as_bytes(),
                )
                .expect_err("signature must not verify for a different purpose");

                artifacts.push((
                    suite,
                    keypair.public_key().to_vec(),
                    primary.as_bytes().to_vec(),
                ));
            }

            for (index, (suite, public_key, signature)) in artifacts.iter().enumerate() {
                for (other_index, (other_suite, _, _)) in artifacts.iter().enumerate() {
                    if index == other_index {
                        continue;
                    }
                    assert!(
                        soranet_pq::verify_mldsa(
                            *other_suite,
                            public_key,
                            RELEASE_KAT_CONTEXT,
                            RELEASE_KAT_MESSAGE,
                            signature,
                        )
                        .is_err(),
                        "{suite:?} material must not verify as {other_suite:?}"
                    );
                }
            }

            let public_key_digests: Vec<_> = artifacts
                .iter()
                .map(|(_, public_key, _)| crate::sha256(public_key))
                .collect();
            assert_ne!(public_key_digests[0], public_key_digests[1]);
            assert_ne!(public_key_digests[0], public_key_digests[2]);
            assert_ne!(public_key_digests[1], public_key_digests[2]);
        }

        #[test]
        fn seeded_public_key_recovers_from_secret_key() {
            let (public, private) =
                keypair_from_seed(b"iroha:ml-dsa-seed:recover").expect("seeded keypair");
            let secret = mldsa65::SecretKey::from_bytes(&private.to_bytes().1)
                .expect("valid ML-DSA secret bytes");

            let recovered = public_key_from_secret(&secret).expect("recover public key");

            assert_eq!(public, recovered);
        }

        #[test]
        fn seeded_keypair_is_deterministic_and_seed_separated() {
            let first =
                keypair_from_seed(b"iroha:ml-dsa-seed:first").expect("first seeded keypair");
            let first_replay =
                keypair_from_seed(b"iroha:ml-dsa-seed:first").expect("replayed seeded keypair");
            let second =
                keypair_from_seed(b"iroha:ml-dsa-seed:second").expect("second seeded keypair");

            assert_eq!(first, first_replay);
            assert_ne!(first.0, second.0);
            assert_ne!(first.1, second.1);
        }

        #[test]
        fn seeded_adapter_matches_canonical_fips_backend() {
            let seed = b"iroha:ml-dsa-seed:canonical-backend";
            let derived = derive_seed_material(seed).expect("derive FIPS key seed");
            let expected = generate_mldsa_keypair_from_fips_seed(MlDsaSuite::MlDsa65, &derived)
                .expect("canonical backend keypair");
            let actual = keypair_from_seed(seed).expect("adapter keypair");

            assert_eq!(actual.0.to_bytes().1, expected.public_key());
            let actual_secret = actual.1.to_bytes().1;
            assert_eq!(actual_secret.as_slice(), expected.secret_key());
        }

        #[test]
        fn seeded_keypair_rejects_all_zero_seed_material() {
            match keypair_from_seed(&[0u8; 32]) {
                Err(Error::KeyGen(message)) => assert!(message.contains("all zero")),
                Err(err) => panic!("expected all-zero seed KeyGen error, got {err:?}"),
                Ok(_) => panic!("all-zero ML-DSA seed material must fail"),
            }
        }

        #[cfg(feature = "rand")]
        #[test]
        fn random_keypair_from_rng_reports_rng_failure() {
            let mut rng = FailingTryRng;

            match random_keypair_from_rng(&mut rng) {
                Err(Error::KeyGen(message)) => assert!(message.contains("failing ML-DSA RNG")),
                Err(err) => panic!("expected RNG KeyGen error, got {err:?}"),
                Ok(_) => panic!("ML-DSA RNG failure must fail key generation"),
            }
        }

        #[cfg(feature = "rand")]
        #[test]
        fn random_keypair_from_rng_rejects_all_zero_seed_material() {
            let mut rng = FixedTryRng { byte: 0 };

            match random_keypair_from_rng(&mut rng) {
                Err(Error::KeyGen(message)) => assert!(message.contains("all zero")),
                Err(err) => panic!("expected all-zero seed KeyGen error, got {err:?}"),
                Ok(_) => panic!("all-zero ML-DSA random seed material must fail"),
            }
        }

        #[cfg(feature = "rand")]
        #[test]
        fn random_keypair_from_rng_accepts_nonzero_seed_material() {
            let mut rng = FixedTryRng { byte: 0x42 };
            let (public, private) =
                random_keypair_from_rng(&mut rng).expect("nonzero ML-DSA random seed material");
            let secret = mldsa65::SecretKey::from_bytes(&private.to_bytes().1)
                .expect("valid ML-DSA secret bytes");

            let recovered = public_key_from_secret(&secret).expect("recover public key");

            assert_eq!(public, recovered);
        }

        #[test]
        fn public_key_from_secret_rejects_tampered_secret_components() {
            let (_, private) =
                keypair_from_seed(b"iroha:ml-dsa-seed:tamper").expect("seeded keypair");
            let mut secret_bytes = private.to_bytes().1;
            let last = secret_bytes
                .last_mut()
                .expect("ML-DSA secret key has at least one byte");
            *last ^= 0x01;
            let secret = mldsa65::SecretKey::from_bytes(&secret_bytes)
                .expect("length-valid ML-DSA secret bytes");

            let err = public_key_from_secret(&secret).expect_err("tampered secret is inconsistent");

            assert!(matches!(err, Error::KeyGen(message) if message.contains("inconsistent")));
        }

        #[test]
        fn public_key_from_secret_rejects_all_zero_secret_material() {
            let secret_bytes = vec![0u8; MlDsaSuite::MlDsa65.secret_key_len()];
            let secret = mldsa65::SecretKey::from_bytes(&secret_bytes)
                .expect("length-valid all-zero ML-DSA secret bytes");

            let err = public_key_from_secret(&secret)
                .expect_err("all-zero ML-DSA secret material must fail");

            assert!(matches!(err, Error::KeyGen(message) if message.contains("all zero")));
        }

        #[test]
        fn public_key_from_secret_rejects_tampered_public_hash() {
            let (_, private) =
                keypair_from_seed(b"iroha:ml-dsa-seed:tamper-tr").expect("seeded keypair");
            let mut secret_bytes = private.to_bytes().1;
            let tr_offset = 2 * FIPS_SEED_BYTES;
            secret_bytes[tr_offset] ^= 0x01;
            let secret = mldsa65::SecretKey::from_bytes(&secret_bytes)
                .expect("length-valid ML-DSA secret bytes");

            let err = public_key_from_secret(&secret).expect_err("tampered tr is inconsistent");

            assert!(matches!(err, Error::KeyGen(message) if message.contains("tr does not match")));
        }

        #[test]
        fn seed_material_changes_with_seed_input() {
            let first =
                derive_seed_material(b"iroha:ml-dsa-seed:first").expect("derive first seed");
            let second =
                derive_seed_material(b"iroha:ml-dsa-seed:second").expect("derive second seed");

            assert_ne!(first.as_ref(), second.as_ref());
        }
    }
}
