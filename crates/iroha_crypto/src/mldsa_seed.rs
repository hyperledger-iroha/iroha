#![allow(clippy::too_many_arguments)]

pub mod mldsa65 {
    use core::{
        array,
        ptr::{addr_of, addr_of_mut},
    };

    use hkdf::Hkdf;
    use pqcrypto_mldsa::ffi;
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
        let seed_material = Zeroizing::new(derive_seed_material(seed)?);
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

    fn derive_seed_material(seed: &[u8]) -> Result<[u8; SEEDBYTES], Error> {
        let kdf = Hkdf::<Sha512>::new(Some(HKDF_SALT), seed);
        let mut out = [0u8; SEEDBYTES];
        kdf.expand(HKDF_INFO, &mut out)
            .map_err(|_| Error::KeyGen(String::from("ML-DSA HKDF seed expansion failed")))?;
        Ok(out)
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
        use pqcrypto_mldsa::mldsa65;
        use pqcrypto_traits::sign::SecretKey as _;

        use super::*;

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

            assert_ne!(first, second);
        }
    }
}
