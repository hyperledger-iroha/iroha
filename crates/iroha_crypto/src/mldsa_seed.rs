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

        use pqcrypto_mldsa::mldsa65;
        #[cfg(feature = "rand")]
        use rand_core::{TryCryptoRng, TryRngCore};

        use super::*;

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
