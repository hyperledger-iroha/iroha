#![allow(clippy::too_many_arguments)]

pub mod mldsa65 {
    use core::{
        array,
        ptr::{addr_of, addr_of_mut},
    };

    use hkdf::Hkdf;
    use pqcrypto_mldsa::ffi;
    #[cfg(feature = "rand")]
    use rand::rngs::OsRng;
    #[cfg(feature = "rand")]
    use rand_core::TryCryptoRng;
    use sha2::Sha512;
    use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

    use crate::{Algorithm, Error, PrivateKey, PublicKey};

    const SEEDBYTES: usize = 32;
    const CRHBYTES: usize = 64;
    const TRBYTES: usize = 64;
    const N: usize = 256;
    const L: usize = 5;
    const K: usize = 6;

    const HKDF_SALT: &[u8] = b"iroha:ml-dsa:keygen:v1";
    // Preserve the original domain label so existing seeded ML-DSA keys remain stable.
    const HKDF_INFO: &[u8] = b"iroha:ml-dsa:dilithium3:keypair";

    #[repr(C)]
    #[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
    struct Poly {
        coeffs: [i32; N],
    }

    impl Default for Poly {
        fn default() -> Self {
            Self { coeffs: [0; N] }
        }
    }

    #[repr(C)]
    #[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
    struct Polyvecl {
        vec: [Poly; L],
    }

    impl Default for Polyvecl {
        fn default() -> Self {
            Self {
                vec: array::from_fn(|_| Poly::default()),
            }
        }
    }

    #[repr(C)]
    #[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
    struct Polyveck {
        vec: [Poly; K],
    }

    impl Default for Polyveck {
        fn default() -> Self {
            Self {
                vec: array::from_fn(|_| Poly::default()),
            }
        }
    }

    pub fn keypair_from_seed(seed: &[u8]) -> Result<(PublicKey, PrivateKey), Error> {
        validate_seed_material_not_all_zero(seed)?;
        let seed_material = derive_seed_material(seed)?;
        keypair_from_seed_material(&seed_material)
    }

    #[cfg(feature = "rand")]
    pub fn random_keypair() -> Result<(PublicKey, PrivateKey), Error> {
        random_keypair_from_rng(&mut OsRng)
    }

    #[cfg(feature = "rand")]
    fn random_keypair_from_rng<R>(rng: &mut R) -> Result<(PublicKey, PrivateKey), Error>
    where
        R: TryCryptoRng,
    {
        let mut seed_material = Zeroizing::new([0u8; SEEDBYTES]);
        rng.try_fill_bytes(seed_material.as_mut())
            .map_err(|err| Error::KeyGen(format!("ML-DSA OS RNG failed: {err}")))?;
        validate_seed_material_not_all_zero(seed_material.as_slice())?;
        keypair_from_seed_material(&seed_material)
    }

    #[allow(unsafe_code)]
    pub fn public_key_from_secret(
        secret_key: &pqcrypto_mldsa::mldsa65::SecretKey,
    ) -> Result<PublicKey, Error> {
        use pqcrypto_traits::sign::SecretKey as _;

        if secret_key.as_bytes().len() != ffi::PQCLEAN_MLDSA65_CLEAN_CRYPTO_SECRETKEYBYTES {
            return Err(Error::KeyGen(String::from(
                "Invalid ML-DSA secret key length",
            )));
        }

        let mut rho = Zeroizing::new([0u8; SEEDBYTES]);
        let mut tr = Zeroizing::new([0u8; TRBYTES]);
        let mut key = Zeroizing::new([0u8; SEEDBYTES]);
        let mut t0 = Polyveck::default();
        let mut s1 = Polyvecl::default();
        let mut s2 = Polyveck::default();

        unsafe {
            PQCLEAN_MLDSA65_CLEAN_unpack_sk(
                rho.as_mut_ptr(),
                tr.as_mut_ptr(),
                key.as_mut_ptr(),
                addr_of_mut!(t0),
                addr_of_mut!(s1),
                addr_of_mut!(s2),
                secret_key.as_bytes().as_ptr(),
            );
        }

        let mut mat = vec![Polyvecl::default(); K];
        unsafe {
            PQCLEAN_MLDSA65_CLEAN_polyvec_matrix_expand(mat.as_mut_ptr(), rho.as_ptr());
        }

        let mut s1hat = s1.clone();
        unsafe {
            PQCLEAN_MLDSA65_CLEAN_polyvecl_ntt(addr_of_mut!(s1hat));
        }

        let mut t = Polyveck::default();
        unsafe {
            PQCLEAN_MLDSA65_CLEAN_polyvec_matrix_pointwise_montgomery(
                addr_of_mut!(t),
                mat.as_ptr(),
                addr_of!(s1hat),
            );
            PQCLEAN_MLDSA65_CLEAN_polyveck_reduce(addr_of_mut!(t));
            PQCLEAN_MLDSA65_CLEAN_polyveck_invntt_tomont(addr_of_mut!(t));
            PQCLEAN_MLDSA65_CLEAN_polyveck_add(addr_of_mut!(t), addr_of!(t), addr_of!(s2));
            PQCLEAN_MLDSA65_CLEAN_polyveck_caddq(addr_of_mut!(t));
        }

        let mut t1 = Polyveck::default();
        let mut t0_check = Polyveck::default();
        unsafe {
            PQCLEAN_MLDSA65_CLEAN_polyveck_power2round(
                addr_of_mut!(t1),
                addr_of_mut!(t0_check),
                addr_of!(t),
            );
        }

        if t0_check != t0 {
            return Err(Error::KeyGen(String::from(
                "Inconsistent ML-DSA secret key components",
            )));
        }

        let mut pk_bytes = [0u8; ffi::PQCLEAN_MLDSA65_CLEAN_CRYPTO_PUBLICKEYBYTES];
        unsafe {
            PQCLEAN_MLDSA65_CLEAN_pack_pk(pk_bytes.as_mut_ptr(), rho.as_ptr(), addr_of!(t1));
        }

        let mut expected_tr = Zeroizing::new([0u8; TRBYTES]);
        unsafe {
            shake256(
                expected_tr.as_mut_ptr(),
                expected_tr.len(),
                pk_bytes.as_ptr(),
                pk_bytes.len(),
            );
        }
        if expected_tr.as_ref() != tr.as_ref() {
            return Err(Error::KeyGen(String::from(
                "Inconsistent ML-DSA secret key public hash",
            )));
        }

        PublicKey::from_bytes(Algorithm::MlDsa, &pk_bytes)
            .map_err(|err| Error::KeyGen(err.to_string()))
    }

    fn derive_seed_material(seed: &[u8]) -> Result<Zeroizing<[u8; SEEDBYTES]>, Error> {
        let kdf = Hkdf::<Sha512>::new(Some(HKDF_SALT), seed);
        let mut out = Zeroizing::new([0u8; SEEDBYTES]);
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

    #[allow(unsafe_code)]
    fn keypair_from_seed_material(
        seed_material: &[u8; SEEDBYTES],
    ) -> Result<(PublicKey, PrivateKey), Error> {
        let mut expanded = Zeroizing::new([0u8; 2 * SEEDBYTES + CRHBYTES]);
        unsafe {
            shake256(
                expanded.as_mut_ptr(),
                expanded.len(),
                seed_material.as_ptr(),
                seed_material.len(),
            );
        }

        let (rho, rest) = expanded.split_at(SEEDBYTES);
        let (rhoprime, key) = rest.split_at(CRHBYTES);

        let mut mat = vec![Polyvecl::default(); K];
        unsafe {
            PQCLEAN_MLDSA65_CLEAN_polyvec_matrix_expand(mat.as_mut_ptr(), rho.as_ptr());
        }

        let mut s1 = Polyvecl::default();
        unsafe {
            PQCLEAN_MLDSA65_CLEAN_polyvecl_uniform_eta(addr_of_mut!(s1), rhoprime.as_ptr(), 0);
        }

        let mut s2 = Polyveck::default();
        let s2_nonce = polyveck_s2_nonce()?;
        unsafe {
            PQCLEAN_MLDSA65_CLEAN_polyveck_uniform_eta(
                addr_of_mut!(s2),
                rhoprime.as_ptr(),
                s2_nonce,
            );
        }

        let mut s1hat = s1.clone();
        unsafe {
            PQCLEAN_MLDSA65_CLEAN_polyvecl_ntt(addr_of_mut!(s1hat));
        }

        let mut t1 = Polyveck::default();
        unsafe {
            PQCLEAN_MLDSA65_CLEAN_polyvec_matrix_pointwise_montgomery(
                addr_of_mut!(t1),
                mat.as_ptr(),
                addr_of!(s1hat),
            );
            PQCLEAN_MLDSA65_CLEAN_polyveck_reduce(addr_of_mut!(t1));
            PQCLEAN_MLDSA65_CLEAN_polyveck_invntt_tomont(addr_of_mut!(t1));
            PQCLEAN_MLDSA65_CLEAN_polyveck_add(addr_of_mut!(t1), addr_of!(t1), addr_of!(s2));
            PQCLEAN_MLDSA65_CLEAN_polyveck_caddq(addr_of_mut!(t1));
        }

        let mut t1_rounded = Polyveck::default();
        let mut t0 = Polyveck::default();
        unsafe {
            PQCLEAN_MLDSA65_CLEAN_polyveck_power2round(
                addr_of_mut!(t1_rounded),
                addr_of_mut!(t0),
                addr_of!(t1),
            );
        }

        let mut pk_bytes = [0u8; ffi::PQCLEAN_MLDSA65_CLEAN_CRYPTO_PUBLICKEYBYTES];
        unsafe {
            PQCLEAN_MLDSA65_CLEAN_pack_pk(
                pk_bytes.as_mut_ptr(),
                rho.as_ptr(),
                addr_of!(t1_rounded),
            );
        }

        let mut tr = Zeroizing::new([0u8; TRBYTES]);
        unsafe {
            shake256(tr.as_mut_ptr(), tr.len(), pk_bytes.as_ptr(), pk_bytes.len());
        }

        let mut sk_bytes = Zeroizing::new([0u8; ffi::PQCLEAN_MLDSA65_CLEAN_CRYPTO_SECRETKEYBYTES]);
        unsafe {
            PQCLEAN_MLDSA65_CLEAN_pack_sk(
                sk_bytes.as_mut_ptr(),
                rho.as_ptr(),
                tr.as_ptr(),
                key.as_ptr(),
                addr_of!(t0),
                addr_of!(s1),
                addr_of!(s2),
            );
        }

        let public_key = PublicKey::from_bytes(Algorithm::MlDsa, &pk_bytes)
            .map_err(|err| Error::KeyGen(err.to_string()))?;
        let private_key = PrivateKey::from_bytes(Algorithm::MlDsa, &sk_bytes[..])
            .map_err(|err| Error::KeyGen(err.to_string()))?;

        Ok((public_key, private_key))
    }

    fn polyveck_s2_nonce() -> Result<u16, Error> {
        u16::try_from(L).map_err(|_| {
            Error::KeyGen(String::from("ML-DSA S2 nonce offset does not fit into u16"))
        })
    }

    #[allow(unsafe_code)]
    unsafe extern "C" {
        fn PQCLEAN_MLDSA65_CLEAN_polyvec_matrix_expand(mat: *mut Polyvecl, rho: *const u8);
        fn PQCLEAN_MLDSA65_CLEAN_polyvecl_uniform_eta(
            v: *mut Polyvecl,
            seed: *const u8,
            nonce: u16,
        );
        fn PQCLEAN_MLDSA65_CLEAN_polyveck_uniform_eta(
            v: *mut Polyveck,
            seed: *const u8,
            nonce: u16,
        );
        fn PQCLEAN_MLDSA65_CLEAN_polyvecl_ntt(v: *mut Polyvecl);
        fn PQCLEAN_MLDSA65_CLEAN_polyvec_matrix_pointwise_montgomery(
            t: *mut Polyveck,
            mat: *const Polyvecl,
            v: *const Polyvecl,
        );
        fn PQCLEAN_MLDSA65_CLEAN_polyveck_reduce(v: *mut Polyveck);
        fn PQCLEAN_MLDSA65_CLEAN_polyveck_invntt_tomont(v: *mut Polyveck);
        fn PQCLEAN_MLDSA65_CLEAN_polyveck_add(
            w: *mut Polyveck,
            u: *const Polyveck,
            v: *const Polyveck,
        );
        fn PQCLEAN_MLDSA65_CLEAN_polyveck_caddq(v: *mut Polyveck);
        fn PQCLEAN_MLDSA65_CLEAN_polyveck_power2round(
            v1: *mut Polyveck,
            v0: *mut Polyveck,
            v: *const Polyveck,
        );
        fn PQCLEAN_MLDSA65_CLEAN_pack_pk(pk: *mut u8, rho: *const u8, t1: *const Polyveck);
        fn PQCLEAN_MLDSA65_CLEAN_pack_sk(
            sk: *mut u8,
            rho: *const u8,
            tr: *const u8,
            key: *const u8,
            t0: *const Polyveck,
            s1: *const Polyvecl,
            s2: *const Polyveck,
        );
        fn PQCLEAN_MLDSA65_CLEAN_unpack_sk(
            rho: *mut u8,
            tr: *mut u8,
            key: *mut u8,
            t0: *mut Polyveck,
            s1: *mut Polyvecl,
            s2: *mut Polyveck,
            sk: *const u8,
        );
        fn shake256(output: *mut u8, outlen: usize, input: *const u8, inlen: usize);
    }

    #[cfg(test)]
    mod tests {
        #[cfg(feature = "rand")]
        use core::fmt;

        use pqcrypto_mldsa::mldsa65;
        use pqcrypto_traits::sign::SecretKey as _;
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
        fn seeded_keygen_s2_nonce_uses_canonical_l_offset() {
            assert_eq!(
                polyveck_s2_nonce().expect("derive S2 nonce offset"),
                u16::try_from(L).expect("test constant fits u16")
            );
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

            assert!(matches!(err, Error::KeyGen(message) if message.contains("Inconsistent")));
        }

        #[test]
        #[allow(unsafe_code)]
        fn public_key_from_secret_rejects_tampered_public_hash() {
            let (_, private) =
                keypair_from_seed(b"iroha:ml-dsa-seed:tamper-tr").expect("seeded keypair");
            let secret_bytes = private.to_bytes().1;

            let mut rho = Zeroizing::new([0u8; SEEDBYTES]);
            let mut tr = Zeroizing::new([0u8; TRBYTES]);
            let mut key = Zeroizing::new([0u8; SEEDBYTES]);
            let mut t0 = Polyveck::default();
            let mut s1 = Polyvecl::default();
            let mut s2 = Polyveck::default();
            unsafe {
                PQCLEAN_MLDSA65_CLEAN_unpack_sk(
                    rho.as_mut_ptr(),
                    tr.as_mut_ptr(),
                    key.as_mut_ptr(),
                    addr_of_mut!(t0),
                    addr_of_mut!(s1),
                    addr_of_mut!(s2),
                    secret_bytes.as_ptr(),
                );
            }
            tr[0] ^= 0x01;

            let mut tampered =
                Zeroizing::new([0u8; ffi::PQCLEAN_MLDSA65_CLEAN_CRYPTO_SECRETKEYBYTES]);
            unsafe {
                PQCLEAN_MLDSA65_CLEAN_pack_sk(
                    tampered.as_mut_ptr(),
                    rho.as_ptr(),
                    tr.as_ptr(),
                    key.as_ptr(),
                    addr_of!(t0),
                    addr_of!(s1),
                    addr_of!(s2),
                );
            }
            let secret = mldsa65::SecretKey::from_bytes(tampered.as_ref())
                .expect("length-valid ML-DSA secret bytes");

            let err = public_key_from_secret(&secret).expect_err("tampered tr is inconsistent");

            assert!(matches!(err, Error::KeyGen(message) if message.contains("public hash")));
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
