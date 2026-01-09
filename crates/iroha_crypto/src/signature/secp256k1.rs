use std::{format, vec::Vec};

use self::ecdsa_secp256k1::EcdsaSecp256k1Impl;
use crate::{Error, KeyGenOption, ParseError};

/// ECDSA over secp256k1 with SHA-256 hashing (used for interoperability).
#[derive(Clone, Copy)]
pub struct EcdsaSecp256k1Sha256;

/// Compressed/uncompressed secp256k1 public key.
pub type PublicKey = k256::PublicKey;
/// SEC1-encoded secp256k1 secret key.
pub type PrivateKey = k256::SecretKey;

impl EcdsaSecp256k1Sha256 {
    /// Generate a secp256k1 keypair using the provided RNG option.
    pub fn keypair(option: KeyGenOption<PrivateKey>) -> (PublicKey, PrivateKey) {
        EcdsaSecp256k1Impl::keypair(option)
    }

    /// Sign a message using the provided secp256k1 private key.
    pub fn sign(message: &[u8], sk: &PrivateKey) -> Vec<u8> {
        EcdsaSecp256k1Impl::sign(message, sk)
    }

    /// Verify a signature using the provided public key.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadSignature`] if verification fails, the signature is non-canonical
    /// (high-S), or the inputs are malformed.
    pub fn verify(message: &[u8], signature: &[u8], pk: &PublicKey) -> Result<(), Error> {
        EcdsaSecp256k1Impl::verify(message, signature, pk)
    }

    /// Parse a SEC1-encoded public key.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when the payload cannot be decoded into a valid key or
    /// when the encoding is non-canonical (must match the canonical SEC1 encoding).
    pub fn parse_public_key(payload: &[u8]) -> Result<PublicKey, ParseError> {
        EcdsaSecp256k1Impl::parse_public_key(payload)
    }

    /// Parse a SEC1-encoded private key.
    ///
    /// # Errors
    ///
    /// Returns [`ParseError`] when the payload is malformed or uses an unsupported format.
    pub fn parse_private_key(payload: &[u8]) -> Result<PrivateKey, ParseError> {
        EcdsaSecp256k1Impl::parse_private_key(payload)
    }

    /// Deterministic batch verification helper.
    ///
    /// Verifies each (message, signature, `public_key`) triple independently in order.
    /// The `seed32` parameter is reserved for future deterministic MSM batching and is unused.
    ///
    /// # Errors
    ///
    /// Returns [`Error::BadSignature`] when the inputs differ in length or any tuple fails
    /// verification, or when the input is empty.
    pub fn verify_batch_deterministic(
        messages: &[&[u8]],
        signatures: &[&[u8]],
        public_keys: &[&[u8]],
        _seed32: [u8; 32],
    ) -> Result<(), Error> {
        if messages.is_empty()
            || !(messages.len() == signatures.len() && signatures.len() == public_keys.len())
        {
            return Err(Error::BadSignature);
        }
        for ((m, s), pk_bytes) in messages
            .iter()
            .zip(signatures.iter())
            .zip(public_keys.iter())
        {
            let pk = match Self::parse_public_key(pk_bytes) {
                Ok(v) => v,
                Err(_) => return Err(Error::BadSignature),
            };
            Self::verify(m, s, &pk)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_batch_deterministic_two_ok() {
        #[cfg(feature = "rand")]
        let (pk1, sk1) = EcdsaSecp256k1Sha256::keypair(KeyGenOption::Random);
        #[cfg(not(feature = "rand"))]
        let (pk1, sk1) = {
            let bytes =
                hex::decode("e4f21b38e005d4f895a29e84948d7cc83eac79041aeb644ee4fab8d9da42f713")
                    .unwrap();
            let sk = EcdsaSecp256k1Sha256::parse_private_key(&bytes).unwrap();
            (sk.public_key(), sk)
        };
        #[cfg(feature = "rand")]
        let (pk2, sk2) = EcdsaSecp256k1Sha256::keypair(KeyGenOption::Random);
        #[cfg(not(feature = "rand"))]
        let (pk2, sk2) = {
            let bytes =
                hex::decode("a7f0b8eff14c2a0ed5f279d22e2f596dccb6b3f8e8f7e6f4161da676b8458cf1")
                    .unwrap();
            let sk = EcdsaSecp256k1Sha256::parse_private_key(&bytes).unwrap();
            (sk.public_key(), sk)
        };

        let m1 = b"m1".as_ref();
        let m2 = b"m2".as_ref();
        let s1 = EcdsaSecp256k1Sha256::sign(m1, &sk1);
        let s2 = EcdsaSecp256k1Sha256::sign(m2, &sk2);
        let msgs: [&[u8]; 2] = [m1, m2];
        let sigs: [&[u8]; 2] = [s1.as_slice(), s2.as_slice()];
        let ep1 = pk1.to_sec1_bytes();
        let ep2 = pk2.to_sec1_bytes();
        let pks_arr: [&[u8]; 2] = [ep1.as_ref(), ep2.as_ref()];
        EcdsaSecp256k1Sha256::verify_batch_deterministic(&msgs, &sigs, &pks_arr, [0u8; 32])
            .expect("ok");
    }

    #[test]
    fn verify_batch_deterministic_rejects_empty() {
        let empty: Vec<&[u8]> = Vec::new();
        assert!(
            EcdsaSecp256k1Sha256::verify_batch_deterministic(&empty, &empty, &empty, [0u8; 32])
                .is_err()
        );
    }
}

mod ecdsa_secp256k1 {
    use std::{format, string::ToString as _, vec::Vec};

    use k256::ecdsa::signature::hazmat::{PrehashSigner as _, PrehashVerifier as _};
    use sha2::Digest as _;

    use super::{PrivateKey, PublicKey};
    #[cfg(feature = "rand")]
    use crate::rng::os_rng;
    use crate::{Error, KeyGenOption, ParseError, rng::rng_from_seed};

    pub struct EcdsaSecp256k1Impl;

    impl EcdsaSecp256k1Impl {
        pub fn keypair(option: KeyGenOption<PrivateKey>) -> (PublicKey, PrivateKey) {
            let signing_key = match option {
                #[cfg(feature = "rand")]
                KeyGenOption::Random => {
                    let mut rng = os_rng();
                    PrivateKey::random(&mut rng)
                }
                KeyGenOption::UseSeed(seed) => {
                    let mut rng = rng_from_seed(seed);
                    PrivateKey::random(&mut rng)
                }
                KeyGenOption::FromPrivateKey(ref s) => s.clone(),
            };

            let public_key = signing_key.public_key();
            (public_key, signing_key)
        }

        pub fn sign(message: &[u8], sk: &PrivateKey) -> Vec<u8> {
            let signing_key = k256::ecdsa::SigningKey::from(sk);
            let digest = sha2::Sha256::digest(message);
            let signature: k256::ecdsa::Signature = signing_key
                .sign_prehash(&digest)
                .expect("sha256 digest length is 32 bytes");
            let signature = signature.normalize_s().unwrap_or(signature);
            signature.to_bytes().to_vec()
        }

        pub fn verify(message: &[u8], signature: &[u8], pk: &PublicKey) -> Result<(), Error> {
            let signature = k256::ecdsa::Signature::from_slice(signature)
                .map_err(|e| Error::Signing(format!("{e:?}")))?;
            if signature.normalize_s().is_some() {
                return Err(Error::BadSignature);
            }

            let verifying_key = k256::ecdsa::VerifyingKey::from(pk);

            let digest = sha2::Sha256::digest(message);
            verifying_key
                .verify_prehash(&digest, &signature)
                .map_err(|_| Error::BadSignature)
        }

        pub fn parse_public_key(payload: &[u8]) -> Result<PublicKey, ParseError> {
            let key =
                PublicKey::from_sec1_bytes(payload).map_err(|err| ParseError(err.to_string()))?;
            let canonical = key.to_sec1_bytes();
            if canonical.as_ref() != payload {
                return Err(ParseError(
                    "non-canonical secp256k1 public key encoding".to_string(),
                ));
            }
            Ok(key)
        }

        pub fn parse_private_key(payload: &[u8]) -> Result<PrivateKey, ParseError> {
            PrivateKey::from_slice(payload).map_err(|err| ParseError(err.to_string()))
        }
    }
}

impl From<elliptic_curve::Error> for Error {
    fn from(error: elliptic_curve::Error) -> Error {
        // RustCrypto doesn't expose any kind of error information =(
        Error::Other(format!("{error}"))
    }
}

#[cfg(test)]
mod test {
    use amcl::secp256k1::ecp;
    use k256::ecdsa::signature::hazmat::PrehashVerifier as _;
    use k256::elliptic_curve::sec1::ToEncodedPoint;
    use openssl::{
        bn::{BigNum, BigNumContext},
        ec::{EcGroup, EcKey, EcPoint},
        ecdsa::EcdsaSig,
        nid::Nid,
    };
    use sha2::Digest;

    use super::*;

    const MESSAGE_1: &[u8] = b"This is a dummy message for use with tests";
    const SIGNATURE_1: &str = "0aab347be3530a3fd7d91c354956561101e6f273b8a1ea3d414f82fbd5939db34b99c54c16c45bf4cde8193b58d718e7efa8c055e7add7d9c9cbe8935e849200";
    const PRIVATE_KEY: &str = "e4f21b38e005d4f895a29e84948d7cc83eac79041aeb644ee4fab8d9da42f713";
    const PUBLIC_KEY: &str = "0242c1e1f775237a26da4fd51b8d75ee2709711f6e90303e511169a324ef0789c0";

    fn private_key() -> PrivateKey {
        let payload = hex::decode(PRIVATE_KEY).unwrap();
        EcdsaSecp256k1Sha256::parse_private_key(&payload).unwrap()
    }

    fn public_key() -> PublicKey {
        let payload = hex::decode(PUBLIC_KEY).unwrap();
        EcdsaSecp256k1Sha256::parse_public_key(&payload).unwrap()
    }

    fn public_key_uncompressed(pk: &PublicKey) -> Vec<u8> {
        const PUBLIC_UNCOMPRESSED_KEY_SIZE: usize = 65;

        let mut uncompressed = [0u8; PUBLIC_UNCOMPRESSED_KEY_SIZE];
        ecp::ECP::frombytes(&pk.to_sec1_bytes()[..]).tobytes(&mut uncompressed, false);
        uncompressed.to_vec()
    }

    #[test]
    fn parse_public_key_rejects_non_canonical_encoding() {
        let pk = public_key();
        let canonical = pk.to_sec1_bytes();
        let compressed = pk.to_encoded_point(true);
        let uncompressed = pk.to_encoded_point(false);
        let non_canonical = if canonical.as_ref() == compressed.as_bytes() {
            uncompressed.as_bytes()
        } else {
            compressed.as_bytes()
        };

        let err = EcdsaSecp256k1Sha256::parse_public_key(non_canonical).unwrap_err();
        assert!(err.0.contains("non-canonical"), "unexpected error: {err:?}");
    }

    #[test]
    fn secp256k1_compatibility() {
        let secret = private_key();
        let (p, s) = EcdsaSecp256k1Sha256::keypair(KeyGenOption::FromPrivateKey(secret));

        let _sk = secp256k1::SecretKey::from_byte_array(s.to_bytes().into()).unwrap();
        let _pk = secp256k1::PublicKey::from_slice(&p.to_sec1_bytes()).unwrap();

        let openssl_group = EcGroup::from_curve_name(Nid::SECP256K1).unwrap();
        let mut ctx = BigNumContext::new().unwrap();
        let _openssl_point =
            EcPoint::from_bytes(&openssl_group, &public_key_uncompressed(&p)[..], &mut ctx)
                .unwrap();
    }

    #[test]
    fn secp256k1_verify() {
        let p = public_key();

        EcdsaSecp256k1Sha256::verify(MESSAGE_1, hex::decode(SIGNATURE_1).unwrap().as_slice(), &p)
            .unwrap();

        let context = secp256k1::Secp256k1::new();
        let pk =
            secp256k1::PublicKey::from_slice(hex::decode(PUBLIC_KEY).unwrap().as_slice()).unwrap();

        let hash_bytes: [u8; 32] = sha2::Sha256::digest(MESSAGE_1).into();
        let msg = secp256k1::Message::from_digest(hash_bytes);

        // Check if signatures produced here can be verified by secp256k1
        let signature =
            secp256k1::ecdsa::Signature::from_compact(&hex::decode(SIGNATURE_1).unwrap()[..])
                .unwrap();
        context.verify_ecdsa(msg, &signature, &pk).unwrap();

        let openssl_group = EcGroup::from_curve_name(Nid::SECP256K1).unwrap();
        let mut ctx = BigNumContext::new().unwrap();
        let openssl_point =
            EcPoint::from_bytes(&openssl_group, &pk.serialize_uncompressed(), &mut ctx).unwrap();
        let openssl_pkey = EcKey::from_public_key(&openssl_group, &openssl_point).unwrap();

        // Check if the signatures produced here can be verified by openssl
        let (r, s) = SIGNATURE_1.split_at(SIGNATURE_1.len() / 2);
        let openssl_r = BigNum::from_hex_str(r).unwrap();
        let openssl_s = BigNum::from_hex_str(s).unwrap();
        let openssl_sig = EcdsaSig::from_private_components(openssl_r, openssl_s).unwrap();
        let openssl_result = openssl_sig.verify(hash_bytes.as_ref(), &openssl_pkey);
        assert!(openssl_result.unwrap());
    }

    #[test]
    fn secp256k1_sign() {
        let secret = private_key();
        let (pk, sk) = EcdsaSecp256k1Sha256::keypair(KeyGenOption::FromPrivateKey(secret));

        let sig = EcdsaSecp256k1Sha256::sign(MESSAGE_1, &sk);
        EcdsaSecp256k1Sha256::verify(MESSAGE_1, &sig, &pk).unwrap();

        assert_eq!(sig.len(), 64);

        // Check if secp256k1 signs the message and this module still can verify it
        // And that private keys can sign with other libraries
        let context = secp256k1::Secp256k1::new();
        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&hex::decode(PRIVATE_KEY).unwrap());
        let sk = secp256k1::SecretKey::from_byte_array(key_bytes).unwrap();

        let hash_bytes: [u8; 32] = sha2::Sha256::digest(MESSAGE_1).into();

        let msg = secp256k1::Message::from_digest(hash_bytes);
        let sig_1 = context.sign_ecdsa(msg, &sk).serialize_compact();

        EcdsaSecp256k1Sha256::verify(MESSAGE_1, &sig_1, &pk).unwrap();

        let openssl_group = EcGroup::from_curve_name(Nid::SECP256K1).unwrap();
        let mut ctx = BigNumContext::new().unwrap();
        let openssl_point =
            EcPoint::from_bytes(&openssl_group, &public_key_uncompressed(&pk), &mut ctx).unwrap();
        let openssl_public_key = EcKey::from_public_key(&openssl_group, &openssl_point).unwrap();
        let openssl_secret_key = EcKey::from_private_components(
            &openssl_group,
            &BigNum::from_hex_str(PRIVATE_KEY).unwrap(),
            &openssl_point,
        )
        .unwrap();

        let openssl_sig = EcdsaSig::sign(hash_bytes.as_ref(), &openssl_secret_key).unwrap();
        let openssl_result = openssl_sig.verify(hash_bytes.as_ref(), &openssl_public_key);
        assert!(openssl_result.unwrap());

        let openssl_sig = {
            use std::ops::{Shr, Sub};

            // ensure the S value is "low" (see BIP-0062) https://github.com/bitcoin/bips/blob/master/bip-0062.mediawiki#user-content-Low_S_values_in_signatures
            // this is required for k256 to successfully verify the signature, as it will fail verification of any signature with a High S value
            // Based on https://github.com/bitcoin/bitcoin/blob/v0.9.3/src/key.cpp#L202-L227
            // this is only required for interoperability with OpenSSL
            // if we are only using signatures from iroha_crypto, all of this dance is not necessary
            let mut s = openssl_sig.s().to_owned().unwrap();
            let mut order = BigNum::new().unwrap();
            openssl_group.order(&mut order, &mut ctx).unwrap();
            let half_order = order.shr(1);

            // if the S is "high" (s > half_order), convert it to "low" form (order - s)
            if s.cmp(&half_order) == std::cmp::Ordering::Greater {
                s = order.sub(&s);
            }

            let r = openssl_sig.r();

            // serialize the key
            let mut res = Vec::new();
            // padding because EcdsaSecp256k1Sha256::verify requires a slice of length 64
            res.extend(r.to_vec_padded(32).expect("r fits into 32 bytes"));
            res.extend(s.to_vec_padded(32).expect("s fits into 32 bytes"));
            res
        };

        EcdsaSecp256k1Sha256::verify(MESSAGE_1, openssl_sig.as_slice(), &pk).unwrap();

        let (p, s) = EcdsaSecp256k1Sha256::keypair(KeyGenOption::Random);
        let signed = EcdsaSecp256k1Sha256::sign(MESSAGE_1, &s);
        EcdsaSecp256k1Sha256::verify(MESSAGE_1, &signed, &p).unwrap();
    }

    #[test]
    fn secp256k1_rejects_high_s_signatures() {
        let secret = private_key();
        let (pk, sk) = EcdsaSecp256k1Sha256::keypair(KeyGenOption::FromPrivateKey(secret));
        let message = b"secp256k1 high-s test";
        let sig = EcdsaSecp256k1Sha256::sign(message, &sk);

        let signature = k256::ecdsa::Signature::from_slice(&sig).expect("signature parse");
        assert!(
            signature.normalize_s().is_none(),
            "signatures must be low-S"
        );

        let high_sig = if signature.normalize_s().is_some() {
            signature
        } else {
            let (r, s) = signature.split_scalars();
            k256::ecdsa::Signature::from_scalars(r, -s).expect("high-s signature")
        };
        assert!(high_sig.normalize_s().is_some());

        let digest = sha2::Sha256::digest(message);
        let verifying_key = k256::ecdsa::VerifyingKey::from(&pk);
        verifying_key
            .verify_prehash(&digest, &high_sig)
            .expect("high-S signature is still valid mathematically");

        let err = EcdsaSecp256k1Sha256::verify(message, high_sig.to_bytes().as_ref(), &pk);
        assert!(matches!(err, Err(Error::BadSignature)));
    }
}
