//! This module contains structures and implementations related to the cryptographic parts of the Iroha.
#![allow(unexpected_cfgs)]

mod algorithm;
mod confidential;
#[cfg(not(feature = "ffi_import"))]
/// Symmetric/asymmetric encryption utilities.
pub mod encryption;
mod hash;
#[cfg(not(feature = "ffi_import"))]
/// Hybrid KEM/DEM helpers used by SoraFS payload envelopes.
pub mod hybrid;
#[cfg(not(feature = "ffi_import"))]
/// Key exchange protocols.
pub mod kex;
mod merkle;
mod mldsa_seed;
#[cfg(not(feature = "ffi_import"))]
mod multihash;
#[cfg(not(feature = "ffi_import"))]
/// Lane privacy commitment registry (NX-10).
pub mod privacy;
pub(crate) mod rng;
mod secrecy;
mod signature;
#[cfg(not(feature = "ffi_import"))]
pub mod sorafs;
#[cfg(not(feature = "ffi_import"))]
pub mod soranet;
#[cfg(not(feature = "ffi_import"))]
pub mod streaming;
#[cfg(not(feature = "ffi_import"))]
mod varint;
#[cfg(feature = "bls")]
/// Verifiable Random Function (VRF) based on BLS12-381 signatures.
///
/// This module exposes a simple VRF interface where the proof is a BLS
/// signature over a pre-hashed input with an Iroha-specific domain tag and the
/// output is a 32-byte Blake2b hash of the signature bytes with a distinct
/// domain tag. The construction avoids cross-protocol collisions with regular
/// signatures and keeps verification simple and efficient.
///
/// Determinism and cross-arch stability:
/// - The message prehash uses raw Blake2b-256 over
///   `b"iroha:vrf:v1:input|" || chain_id || "|" || input`.
/// - Proofs are BLS signatures produced by the canonical BLS implementation in
///   this crate; verification accepts the same bytes.
/// - Outputs are computed as raw Blake2b-256 over
///   `b"iroha:vrf:v1:output" || proof_bytes`.
pub mod vrf;

#[cfg(feature = "sm")]
pub mod sm;

use core::{fmt, str::FromStr};
use std::sync::Arc;
#[cfg(feature = "bls")]
use std::sync::{Mutex, OnceLock};
use std::{
    borrow::ToOwned as _,
    boxed::Box,
    format,
    string::{String, ToString as _},
    vec,
    vec::Vec,
};

#[cfg(not(feature = "ffi_import"))]
pub use blake2;

#[cfg(feature = "bls")]
pub use self::signature::bls::{
    BlsNormal, BlsNormalPrivateKey, BlsNormalPublicKey, BlsSmall, BlsSmallPrivateKey,
    BlsSmallPublicKey,
};
/// Convenience alias for the historical Blake2b-256 digest type which was
/// previously exported directly from the `blake2` crate. The upstream crate
/// removed this alias in 0.10, so we offer it here to keep the existing API
/// surface for downstream users.
pub type Blake2b256 = blake2::Blake2b<blake2::digest::consts::U32>;
pub use confidential::{
    ConfidentialKeyError, ConfidentialKeyset, derive_keyset, derive_keyset_from_slice,
    generate_keyset,
};
use derive_more::Display;
pub use error::Error;
use error::ParseError;
use getset::Getters;
pub use hash::*;
#[cfg(not(feature = "ffi_import"))]
pub use hybrid::{
    DerivedSecret as HybridDerivedSecret, HybridError, HybridKemCiphertext, HybridKeyPair,
    HybridPublicKey, HybridSecretKey, HybridSuite, decapsulate as hybrid_decapsulate,
    encapsulate as hybrid_encapsulate,
};
use iroha_macro::ffi_impl_opaque;
use iroha_primitives::const_vec::{ConstVec, ToConstVec};
use iroha_schema::{Declaration, IntoSchema, MetaMap, Metadata, NamedFieldsMeta, TypeId};
pub use merkle::{CompactMerkleProof, MerkleError, MerkleProof, MerkleTree};
#[cfg(not(feature = "ffi_import"))]
pub use privacy::{
    CommitmentScheme, CommitmentSchemeKind, LaneCommitmentId, LanePrivacyCommitment,
    MerkleCommitment, MerkleWitness, PrivacyError, PrivacyWitness, PrivacyWitnessKind,
    SnarkCircuit, SnarkCircuitId, SnarkWitness, hash_proof, hash_public_inputs,
};
#[cfg(feature = "sm")]
pub use sm::{Sm2PrivateKey, Sm2PublicKey, Sm2Signature, Sm3Digest, Sm4Key};
#[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
use w3f_bls::SerializableToBytes;
// Zeroize trait is only required under configurations that use it.
use zeroize::{Zeroize, ZeroizeOnDrop};

#[cfg(not(feature = "ffi_import"))]
pub use self::signature::secp256k1::EcdsaSecp256k1Sha256;
pub use self::signature::*;

#[cfg(feature = "gost")]
pub mod gost {
    //! Public wrapper exposing the GOST signature helpers.
    pub use super::signature::gost::*;
}
pub use algorithm::{Algorithm, ED_25519, SECP_256_K1};
#[cfg(feature = "bls")]
pub use algorithm::{BLS_NORMAL, BLS_SMALL};

use crate::secrecy::Secret;

/// Domain separator for BLS Proof-of-Possession over a validator public key.
/// Message = Hash("iroha:bls:pop:v1" || `pk_bytes`)
#[cfg(feature = "bls")]
const POP_DST: &str = "iroha:bls:pop:v1";

/// Key pair generation option. Passed to a specific algorithm.
#[derive(Debug)]
pub enum KeyGenOption<K> {
    /// Use random number generator
    #[cfg(feature = "rand")]
    Random,
    /// Use seed
    UseSeed(Vec<u8>),
    /// Derive from a private key
    FromPrivateKey(K),
}

ffi::ffi_item! {
    /// Pair of Public and Private keys.
    #[derive(Clone, PartialEq, Eq, Getters)]
    #[cfg_attr(not(feature="ffi_import"), derive(Debug))]
    #[getset(get = "pub")]
    pub struct KeyPair {
        /// Public key.
        public_key: PublicKey,
        /// Private key.
        private_key: PrivateKey,
    }
}

#[cfg(feature = "rand")]
impl KeyPair {
    /// Generate a random key pair using a default [`Algorithm`].
    pub fn random() -> Self {
        Self::random_with_algorithm(Algorithm::default())
    }

    /// Generate a random key pair
    pub fn random_with_algorithm(algorithm: Algorithm) -> Self {
        match algorithm {
            Algorithm::Ed25519 => ed25519::Ed25519Sha512::keypair(KeyGenOption::Random).into(),
            Algorithm::Secp256k1 => {
                secp256k1::EcdsaSecp256k1Sha256::keypair(KeyGenOption::Random).into()
            }
            Algorithm::MlDsa => {
                use pqcrypto_dilithium::dilithium3 as dilithium;
                use pqcrypto_traits::sign::PublicKey as _;
                let (pk, sk) = dilithium::keypair();
                let public_key = PublicKey::from_bytes(Algorithm::MlDsa, pk.as_bytes())
                    .expect("valid mldsa public key bytes");
                let private_key = PrivateKey(Box::new(Secret::new(PrivateKeyInner::MlDsa(
                    MlDsaSecretKey::new(&sk),
                ))));
                KeyPair {
                    public_key,
                    private_key,
                }
            }
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_256ParamSetA
            | Algorithm::Gost3410_2012_256ParamSetB
            | Algorithm::Gost3410_2012_256ParamSetC
            | Algorithm::Gost3410_2012_512ParamSetA
            | Algorithm::Gost3410_2012_512ParamSetB => {
                let (public, secret) = signature::gost::generate_random_keypair(algorithm)
                    .expect("random GOST key generation must succeed for supported parameter sets");
                let public_key = PublicKey::new(PublicKeyFull::Gost {
                    algorithm,
                    key: public,
                });
                let private_key = PrivateKey(Box::new(Secret::new(PrivateKeyInner::Gost {
                    algorithm,
                    secret,
                })));
                KeyPair::new(public_key, private_key)
                    .expect("freshly generated GOST key pair should be internally consistent")
            }
            #[cfg(feature = "bls")]
            Algorithm::BlsNormal => bls::BlsNormal::keypair(KeyGenOption::Random).into(),
            #[cfg(feature = "bls")]
            Algorithm::BlsSmall => bls::BlsSmall::keypair(KeyGenOption::Random).into(),
            #[cfg(feature = "sm")]
            Algorithm::Sm2 => {
                let mut rng = sm2::elliptic_curve::rand_core::OsRng;
                let private =
                    sm::Sm2PrivateKey::random(sm::Sm2PublicKey::default_distid(), &mut rng)
                        .expect("sm2 default distid must be valid");
                let public_key = PublicKey::new(PublicKeyFull::Sm2(private.public_key()));
                let private_key = PrivateKey(Box::new(Secret::new(PrivateKeyInner::Sm2(private))));
                KeyPair {
                    public_key,
                    private_key,
                }
            }
        }
    }
}

#[ffi_impl_opaque]
impl KeyPair {
    /// Derive a key pair from seed material.
    ///
    /// Ed25519 uses the seed directly when 32 bytes are provided; other lengths are
    /// hashed with SHA-256 to obtain a canonical 32-byte seed.
    pub fn from_seed(seed: Vec<u8>, algorithm: Algorithm) -> Self {
        match algorithm {
            Algorithm::Ed25519 => {
                ed25519::Ed25519Sha512::keypair(KeyGenOption::UseSeed(seed)).into()
            }
            Algorithm::Secp256k1 => {
                secp256k1::EcdsaSecp256k1Sha256::keypair(KeyGenOption::UseSeed(seed)).into()
            }
            Algorithm::MlDsa => {
                let (public, private) = mldsa_seed::dilithium3::keypair_from_seed(&seed)
                    .expect("seeded ML-DSA key generation must produce a valid key pair");
                KeyPair::new(public, private)
                    .expect("seeded ML-DSA key pair should pass validation")
            }
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_256ParamSetA
            | Algorithm::Gost3410_2012_256ParamSetB
            | Algorithm::Gost3410_2012_256ParamSetC
            | Algorithm::Gost3410_2012_512ParamSetA
            | Algorithm::Gost3410_2012_512ParamSetB => {
                let (public, secret) = signature::gost::generate_seeded_keypair(algorithm, &seed)
                    .expect("seeded GOST key generation must succeed for supported parameter sets");
                let public_key = PublicKey::new(PublicKeyFull::Gost {
                    algorithm,
                    key: public,
                });
                let private_key = PrivateKey(Box::new(Secret::new(PrivateKeyInner::Gost {
                    algorithm,
                    secret,
                })));
                KeyPair::new(public_key, private_key)
                    .expect("seed-derived GOST key pair should be internally consistent")
            }
            #[cfg(feature = "bls")]
            Algorithm::BlsNormal => bls::BlsNormal::keypair(KeyGenOption::UseSeed(seed)).into(),
            #[cfg(feature = "bls")]
            Algorithm::BlsSmall => bls::BlsSmall::keypair(KeyGenOption::UseSeed(seed)).into(),
            #[cfg(feature = "sm")]
            Algorithm::Sm2 => {
                let private_inner =
                    sm::Sm2PrivateKey::from_seed(Sm2PublicKey::default_distid(), &seed)
                        .expect("SM2 seed derivation must succeed");
                let public_key = PublicKey::new(PublicKeyFull::Sm2(private_inner.public_key()));
                let private_key =
                    PrivateKey(Box::new(Secret::new(PrivateKeyInner::Sm2(private_inner))));
                KeyPair::new(public_key, private_key)
                    .expect("seed-derived SM2 key pair should be internally consistent")
            }
        }
    }

    /// Algorithm
    pub fn algorithm(&self) -> Algorithm {
        self.private_key.algorithm()
    }

    /// Construct a [`KeyPair`].
    ///
    /// See [`Self::into_parts`] for an opposite conversion.
    ///
    /// # Errors
    /// If public and private keys don't match, i.e. if they don't make a pair
    pub fn new(public_key: PublicKey, private_key: PrivateKey) -> Result<Self, Error> {
        let algorithm = private_key.algorithm();

        if algorithm != public_key.algorithm() {
            return Err(Error::KeyGen("Mismatch of key algorithms".to_owned()));
        }

        #[cfg(feature = "gost")]
        if matches!(
            algorithm,
            Algorithm::Gost3410_2012_256ParamSetA
                | Algorithm::Gost3410_2012_256ParamSetB
                | Algorithm::Gost3410_2012_256ParamSetC
                | Algorithm::Gost3410_2012_512ParamSetA
                | Algorithm::Gost3410_2012_512ParamSetB
        ) {
            use crate::secrecy::ExposeSecret;
            let public_full = PublicKeyFull::from(&public_key.0);
            let gost_public = match &public_full {
                PublicKeyFull::Gost { key, .. } => key,
                _ => unreachable!("algorithm indicates GOST"),
            };
            let gost_private = match private_key.0.expose_secret() {
                PrivateKeyInner::Gost { secret, .. } => secret,
                _ => unreachable!("algorithm indicates GOST"),
            };
            signature::gost::validate_key_pair(algorithm, gost_public, gost_private)
                .map_err(|err| Error::KeyGen(err.to_string()))?;
        }

        if algorithm == Algorithm::MlDsa {
            use pqcrypto_dilithium::dilithium3 as dilithium;
            use pqcrypto_traits::sign::PublicKey as _;

            use crate::secrecy::ExposeSecret;

            let pk_bytes = public_key.to_bytes().1;
            let dilithium_pk = dilithium::PublicKey::from_bytes(pk_bytes)
                .map_err(|_| Error::KeyGen(String::from("Invalid ML-DSA public key")))?;

            let secret_bytes = match private_key.0.expose_secret() {
                PrivateKeyInner::MlDsa(bytes) => bytes,
                _ => unreachable!("Algorithm is ML-DSA"),
            };

            let dilithium_secret_key = secret_bytes.as_secret();

            let probe_message = b"iroha:ml-dsa:keypair-check";
            let probe_signature = dilithium::detached_sign(probe_message, dilithium_secret_key);
            if dilithium::verify_detached_signature(&probe_signature, probe_message, &dilithium_pk)
                .is_err()
            {
                return Err(Error::KeyGen(String::from("Key pair mismatch")));
            }

            return Ok(Self {
                public_key,
                private_key,
            });
        }

        if PublicKey::from(private_key.clone()) != public_key {
            return Err(Error::KeyGen(String::from("Key pair mismatch")));
        }

        Ok(Self {
            public_key,
            private_key,
        })
    }

    /// Get [`PublicKey`] and [`PrivateKey`] contained in the [`KeyPair`].
    pub fn into_parts(self) -> (PublicKey, PrivateKey) {
        (self.public_key, self.private_key)
    }

    /// Construct a [`KeyPair`] from a [`PrivateKey`] by deriving the matching [`PublicKey`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::KeyGen`] if the derived public key does not correspond to the provided
    /// private key material.
    pub fn from_private_key(private_key: PrivateKey) -> Result<Self, Error> {
        #[cfg(feature = "gost")]
        if matches!(
            private_key.algorithm(),
            Algorithm::Gost3410_2012_256ParamSetA
                | Algorithm::Gost3410_2012_256ParamSetB
                | Algorithm::Gost3410_2012_256ParamSetC
                | Algorithm::Gost3410_2012_512ParamSetA
                | Algorithm::Gost3410_2012_512ParamSetB
        ) {
            use crate::secrecy::ExposeSecret;
            let algorithm = private_key.algorithm();
            let gost_private = match private_key.0.expose_secret() {
                PrivateKeyInner::Gost { secret, .. } => secret,
                _ => unreachable!("algorithm indicates GOST"),
            };
            let derived = signature::gost::derive_public_key(algorithm, gost_private)
                .map_err(|err| Error::KeyGen(err.to_string()))?;
            let public_key = PublicKey::new(PublicKeyFull::Gost {
                algorithm,
                key: derived,
            });
            return KeyPair::new(public_key, private_key);
        }

        let public_key = PublicKey::from(private_key.clone());
        Self::new(public_key, private_key)
    }
}

/// Derives full [`KeyPair`] from its [`PrivateKey`] only.
impl From<PrivateKey> for KeyPair {
    fn from(value: PrivateKey) -> Self {
        KeyPair::from_private_key(value).expect(
            "deriving a key pair from a private key should succeed for supported algorithms",
        )
    }
}

impl From<(ed25519::PublicKey, ed25519::PrivateKey)> for KeyPair {
    fn from((public_key, private_key): (ed25519::PublicKey, ed25519::PrivateKey)) -> Self {
        Self {
            public_key: PublicKey::new(PublicKeyFull::Ed25519(public_key)),
            private_key: PrivateKey(Box::new(Secret::new(PrivateKeyInner::Ed25519(private_key)))),
        }
    }
}

impl From<(secp256k1::PublicKey, secp256k1::PrivateKey)> for KeyPair {
    fn from((public_key, private_key): (secp256k1::PublicKey, secp256k1::PrivateKey)) -> Self {
        Self {
            public_key: PublicKey::new(PublicKeyFull::Secp256k1(public_key)),
            private_key: PrivateKey(Box::new(Secret::new(PrivateKeyInner::Secp256k1(
                private_key,
            )))),
        }
    }
}

#[cfg(feature = "bls")]
impl From<(bls::BlsNormalPublicKey, bls::BlsNormalPrivateKey)> for KeyPair {
    fn from(
        (public_key, private_key): (bls::BlsNormalPublicKey, bls::BlsNormalPrivateKey),
    ) -> Self {
        Self {
            public_key: PublicKey::new(PublicKeyFull::BlsNormal(public_key)),
            private_key: PrivateKey(Box::new(Secret::new(PrivateKeyInner::BlsNormal(
                private_key,
            )))),
        }
    }
}

#[cfg(feature = "bls")]
impl From<(bls::BlsSmallPublicKey, bls::BlsSmallPrivateKey)> for KeyPair {
    fn from((public_key, private_key): (bls::BlsSmallPublicKey, bls::BlsSmallPrivateKey)) -> Self {
        Self {
            public_key: PublicKey::new(PublicKeyFull::BlsSmall(public_key)),
            private_key: PrivateKey(Box::new(Secret::new(PrivateKeyInner::BlsSmall(
                private_key,
            )))),
        }
    }
}

/// Decoded version of public key (requires more memory).
/// Used only for signature verification.
#[derive(Clone)]
enum PublicKeyFull {
    Ed25519(ed25519::PublicKey),
    Secp256k1(secp256k1::PublicKey),
    MlDsa(Vec<u8>),
    #[cfg(feature = "gost")]
    Gost {
        algorithm: Algorithm,
        key: signature::gost::PublicKey,
    },
    #[cfg(feature = "bls")]
    BlsNormal(bls::BlsNormalPublicKey),
    #[cfg(feature = "bls")]
    BlsSmall(bls::BlsSmallPublicKey),
    #[cfg(feature = "sm")]
    Sm2(Sm2PublicKey),
}

impl PublicKeyFull {
    fn from_bytes(algorithm: Algorithm, payload: &[u8]) -> Result<Self, ParseError> {
        match algorithm {
            Algorithm::Ed25519 => {
                ed25519::Ed25519Sha512::parse_public_key(payload).map(PublicKeyFull::Ed25519)
            }
            Algorithm::Secp256k1 => secp256k1::EcdsaSecp256k1Sha256::parse_public_key(payload)
                .map(PublicKeyFull::Secp256k1),
            Algorithm::MlDsa => {
                use pqcrypto_dilithium::dilithium3 as dilithium;
                use pqcrypto_traits::sign::PublicKey as _;
                if payload.len() != dilithium::public_key_bytes() {
                    return Err(ParseError("invalid ML-DSA public key length".to_string()));
                }
                let pk = dilithium::PublicKey::from_bytes(payload)
                    .map_err(|_| ParseError("invalid ML-DSA public key".to_string()))?;
                Ok(PublicKeyFull::MlDsa(pk.as_bytes().to_vec()))
            }
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_256ParamSetA
            | Algorithm::Gost3410_2012_256ParamSetB
            | Algorithm::Gost3410_2012_256ParamSetC
            | Algorithm::Gost3410_2012_512ParamSetA
            | Algorithm::Gost3410_2012_512ParamSetB => {
                signature::gost::parse_public_key(algorithm, payload)
                    .map(|key| PublicKeyFull::Gost { algorithm, key })
            }
            #[cfg(feature = "bls")]
            Algorithm::BlsNormal => {
                bls::BlsNormal::parse_public_key(payload).map(PublicKeyFull::BlsNormal)
            }
            #[cfg(feature = "bls")]
            Algorithm::BlsSmall => {
                bls::BlsSmall::parse_public_key(payload).map(PublicKeyFull::BlsSmall)
            }
            #[cfg(feature = "sm")]
            Algorithm::Sm2 => sm::decode_sm2_public_key_payload(payload).map(PublicKeyFull::Sm2),
        }
    }

    /// Key payload
    fn payload(&self) -> ConstVec<u8> {
        match self {
            Self::Ed25519(key) => key.as_bytes().to_const_vec(),
            Self::Secp256k1(key) => key.to_sec1_bytes().to_const_vec(),
            Self::MlDsa(key) => key.clone().to_const_vec(),
            #[cfg(feature = "gost")]
            Self::Gost { key, .. } => key.as_bytes().to_const_vec(),
            #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
            Self::BlsNormal(key) => key.to_bytes().to_const_vec(),
            #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
            Self::BlsSmall(key) => key.to_bytes().to_const_vec(),
            #[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
            Self::BlsNormal(key) => key.to_bytes().to_const_vec(),
            #[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
            Self::BlsSmall(key) => key.to_bytes().to_const_vec(),
            #[cfg(feature = "sm")]
            Self::Sm2(key) => {
                sm::encode_sm2_public_key_payload(key.distid(), &key.to_sec1_bytes(false))
                    .expect("SM2 public key payload must be encodable")
                    .to_const_vec()
            }
        }
    }

    fn algorithm(&self) -> Algorithm {
        match self {
            Self::Ed25519(_) => Algorithm::Ed25519,
            Self::Secp256k1(_) => Algorithm::Secp256k1,
            Self::MlDsa(_) => Algorithm::MlDsa,
            #[cfg(feature = "gost")]
            Self::Gost { algorithm, .. } => *algorithm,
            #[cfg(feature = "bls")]
            Self::BlsNormal(_) => Algorithm::BlsNormal,
            #[cfg(feature = "bls")]
            Self::BlsSmall(_) => Algorithm::BlsSmall,
            #[cfg(feature = "sm")]
            Self::Sm2(_) => Algorithm::Sm2,
        }
    }
}

impl From<&PublicKeyCompact> for PublicKeyFull {
    fn from(public_key: &PublicKeyCompact) -> Self {
        Self::from_bytes(public_key.algorithm(), public_key.payload())
            .expect("`PublicKeyEncoded` must contain valid payload")
    }
}

/// Encoded version of public key (requires less memory).
/// Any public keys should be stored in such form to reduce memory consumption.
/// In case signature verification is needed, it will be decoded.
///
/// Invariant: `payload` is valid, that is conversion to full form must not give error.
#[derive(Clone, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct PublicKeyCompact {
    // First byte corresponds to algorithm
    // Other bytes are payload
    algorithm_and_payload: ConstVec<u8>,
    // This is non-optimized version of this struct:
    // algorithm: Algorithm,
    // payload: ConstVec<u8>,
}

// Batch verification helpers (deterministic), exposed for admission-time grouping
// across transaction signatures.

/// Deterministic Ed25519 batch verification wrapper (per-signature).
/// The `seed32` parameter is reserved for API compatibility and is ignored.
/// # Errors
/// Returns `Err(Error::BadSignature)` if any `(message, signature, public_key)` tuple fails verification,
/// if the input slices have mismatched lengths, or if the input is empty.
pub fn ed25519_verify_batch_deterministic(
    messages: &[&[u8]],
    signatures: &[&[u8]],
    public_keys: &[&[u8]],
    seed32: [u8; 32],
) -> Result<(), Error> {
    signature::ed25519::Ed25519Sha512::verify_batch_deterministic(
        messages,
        signatures,
        public_keys,
        seed32,
    )
}

/// Deterministic secp256k1 (ECDSA) batch verification wrapper.
/// Currently verifies each signature independently in the given order.
/// The `seed32` parameter is reserved for future deterministic MSM batching.
/// # Errors
/// Returns `Err(Error::BadSignature)` if any `(message, signature, public_key)` tuple fails verification
/// or if the input slices have mismatched lengths or are empty.
pub fn secp256k1_verify_batch_deterministic(
    messages: &[&[u8]],
    signatures: &[&[u8]],
    public_keys: &[&[u8]],
    seed32: [u8; 32],
) -> Result<(), Error> {
    #[cfg(feature = "secp256k1-msm-batch")]
    {
        signature::secp256k1::EcdsaSecp256k1Sha256::verify_batch_deterministic(
            messages,
            signatures,
            public_keys,
            seed32,
        )
    }
    #[cfg(not(feature = "secp256k1-msm-batch"))]
    {
        signature::secp256k1::EcdsaSecp256k1Sha256::verify_batch_deterministic(
            messages,
            signatures,
            public_keys,
            seed32,
        )
    }
}

/// Deterministic PQC (ML‑DSA Dilithium3) batch verification wrapper.
/// Verifies each signature independently.
///
/// # Errors
/// Returns [`Error::BadSignature`] when the input is empty, the input lengths differ,
/// or when signature validation fails.
pub fn pqc_verify_batch_deterministic(
    messages: &[&[u8]],
    signatures: &[&[u8]],
    public_keys: &[&[u8]],
    _seed32: [u8; 32],
) -> Result<(), Error> {
    use pqcrypto_dilithium::dilithium3 as dilithium;
    use pqcrypto_traits::sign::{DetachedSignature as _, PublicKey as _};

    if messages.is_empty()
        || !(messages.len() == signatures.len() && signatures.len() == public_keys.len())
    {
        return Err(Error::BadSignature);
    }
    let exp_sig = dilithium::signature_bytes();
    let exp_pk = dilithium::public_key_bytes();
    for ((m, s), pk) in messages
        .iter()
        .zip(signatures.iter())
        .zip(public_keys.iter())
    {
        if s.len() != exp_sig || pk.len() != exp_pk {
            return Err(Error::BadSignature);
        }
        let sig = match dilithium::DetachedSignature::from_bytes(s) {
            Ok(v) => v,
            Err(_) => return Err(Error::BadSignature),
        };
        let vk = match dilithium::PublicKey::from_bytes(pk) {
            Ok(v) => v,
            Err(_) => return Err(Error::BadSignature),
        };
        if dilithium::verify_detached_signature(&sig, m, &vk).is_err() {
            return Err(Error::BadSignature);
        }
    }
    Ok(())
}

/// Deterministic BLS (normal) batch verification wrapper.
/// Verifies each signature independently using `w3f_bls` (public key in G1, signature in G2).
///
/// # Errors
/// Returns `Err(Error::BadSignature)` on empty input, length mismatches, or if any
/// signature or public key fails to parse or verify.
#[cfg(feature = "bls")]
pub fn bls_normal_verify_batch_deterministic(
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
    for ((m, s), pk) in messages
        .iter()
        .zip(signatures.iter())
        .zip(public_keys.iter())
    {
        let vk = signature::bls::BlsNormal::parse_public_key(pk)?;
        signature::bls::BlsNormal::verify(m, s, &vk)?;
    }
    Ok(())
}

/// Deterministic BLS (small) batch verification wrapper.
/// Verifies each signature independently using `w3f_bls` tiny variant.
///
/// # Errors
/// Returns `Err(Error::BadSignature)` on empty input, length mismatches, or if any
/// signature or public key fails to parse or verify.
#[cfg(feature = "bls")]
pub fn bls_small_verify_batch_deterministic(
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
    for ((m, s), pk) in messages
        .iter()
        .zip(signatures.iter())
        .zip(public_keys.iter())
    {
        let vk = signature::bls::BlsSmall::parse_public_key(pk)?;
        signature::bls::BlsSmall::verify(m, s, &vk)?;
    }
    Ok(())
}

#[cfg(feature = "bls")]
fn bls_collect_pks_with_pop<'a>(
    public_keys: &[&'a PublicKey],
    pops: &[&'a [u8]],
    algorithm: Algorithm,
    pop_verify: fn(&PublicKey, &[u8]) -> Result<(), Error>,
) -> Result<Vec<&'a [u8]>, Error> {
    use std::collections::BTreeSet;

    fn pop_cache_key(pk_bytes: &[u8], pop: &[u8]) -> Hash {
        let mut buf = Vec::with_capacity(pk_bytes.len() + pop.len());
        buf.extend_from_slice(pk_bytes);
        buf.extend_from_slice(pop);
        Hash::new(&buf)
    }

    fn pop_cache() -> &'static Mutex<std::collections::BTreeSet<Hash>> {
        static CACHE: OnceLock<Mutex<std::collections::BTreeSet<Hash>>> = OnceLock::new();
        CACHE.get_or_init(|| Mutex::new(std::collections::BTreeSet::new()))
    }

    if public_keys.len() != pops.len() || public_keys.is_empty() {
        return Err(Error::BadSignature);
    }
    let mut seen = BTreeSet::new();
    let mut pk_bytes = Vec::with_capacity(public_keys.len());
    for (pk, pop) in public_keys.iter().zip(pops.iter()) {
        if pk.algorithm() != algorithm {
            return Err(Error::BadSignature);
        }
        let (_, bytes) = pk.to_bytes();
        let cache_key = pop_cache_key(bytes, pop);
        let cached = pop_cache()
            .lock()
            .ok()
            .map_or(false, |cache| cache.contains(&cache_key));
        if !cached {
            pop_verify(pk, pop)?;
            if let Ok(mut cache) = pop_cache().lock() {
                cache.insert(cache_key);
            }
        }
        if !seen.insert(bytes) {
            return Err(Error::BadSignature);
        }
        pk_bytes.push(bytes);
    }
    Ok(pk_bytes)
}

/// Attempt aggregate verification for BLS (normal) when all messages are identical.
/// Requires a valid Proof-of-Possession (`PoP`) per public key to prevent rogue-key attacks.
/// Fallback: per-signature verify inside this function. Deterministic and hardware-stable.
///
/// # Errors
/// Returns `Err(Error::BadSignature)` if any signature/public key fails to parse or verify,
/// or if input slice lengths are inconsistent. Public keys must be unique.
#[cfg(feature = "bls")]
pub fn bls_normal_verify_aggregate_same_message(
    message: &[u8],
    signatures: &[&[u8]],
    public_keys: &[&PublicKey],
    pops: &[&[u8]],
) -> Result<(), Error> {
    if signatures.len() != public_keys.len() || signatures.is_empty() {
        return Err(Error::BadSignature);
    }
    let pk_bytes = bls_collect_pks_with_pop(
        public_keys,
        pops,
        Algorithm::BlsNormal,
        bls_normal_pop_verify,
    )?;
    // Preferred: aggregate-style helper; internal path may be per-sig today.
    if let Ok(()) =
        signature::bls::verify_aggregate_same_message_normal(message, signatures, &pk_bytes)
    {
        Ok(())
    } else {
        // Fallback: per-signature
        for (s, pk) in signatures.iter().zip(pk_bytes.iter()) {
            let vk = signature::bls::BlsNormal::parse_public_key(pk)?;
            signature::bls::BlsNormal::verify(message, s, &vk)?;
        }
        Ok(())
    }
}

/// Attempt aggregate verification for BLS (normal) when all messages are identical.
/// Requires a valid Proof-of-Possession (`PoP`) per public key to prevent rogue-key attacks.
/// Fast path: aggregate-only check, no per-signature fallback.
///
/// # Errors
/// Returns `Err(Error::BadSignature)` if any signature/public key fails to parse or verify,
/// or if input slice lengths are inconsistent. Public keys must be unique.
#[cfg(feature = "bls")]
pub fn bls_normal_verify_aggregate_same_message_fast(
    message: &[u8],
    signatures: &[&[u8]],
    public_keys: &[&PublicKey],
    pops: &[&[u8]],
) -> Result<(), Error> {
    if signatures.len() != public_keys.len() || signatures.is_empty() {
        return Err(Error::BadSignature);
    }
    let pk_bytes = bls_collect_pks_with_pop(
        public_keys,
        pops,
        Algorithm::BlsNormal,
        bls_normal_pop_verify,
    )?;
    signature::bls::verify_aggregate_same_message_normal(message, signatures, &pk_bytes)
}

/// Aggregate verification across distinct messages (multi-message) for BLS (normal).
/// Attempts the shared aggregate verifier and falls back to per-signature checks when it rejects.
///
/// # Errors
/// Returns `Err(Error::BadSignature)` if any signature/public key fails to parse or verify,
/// or if input slice lengths are inconsistent.
#[cfg(all(feature = "bls", not(feature = "bls-multi-pairing")))]
pub fn bls_normal_verify_aggregate_multi_message(
    messages: &[&[u8]],
    signatures: &[&[u8]],
    public_keys: &[&[u8]],
) -> Result<(), Error> {
    {
        use std::collections::BTreeSet;
        if messages.len() != signatures.len()
            || signatures.len() != public_keys.len()
            || messages.is_empty()
        {
            return Err(Error::BadSignature);
        }
        let mut seen = BTreeSet::new();
        for &msg in messages {
            if !seen.insert(msg) {
                return Err(Error::BadSignature);
            }
        }
    }
    if let Ok(()) =
        signature::bls::verify_aggregate_multi_message_normal(messages, signatures, public_keys)
    {
        return Ok(());
    }

    for ((m, s), pk) in messages
        .iter()
        .zip(signatures.iter())
        .zip(public_keys.iter())
    {
        let vk = signature::bls::BlsNormal::parse_public_key(pk)?;
        signature::bls::BlsNormal::verify(m, s, &vk)?;
    }
    Ok(())
}

/// Aggregate verification across distinct messages (multi-message) for BLS (normal).
/// Uses a single pairing-product when `bls-multi-pairing` is enabled; falls back
/// to per-signature verification on any parsing/hashing failure to preserve correctness.
///
/// # Errors
/// Returns `Err(Error::BadSignature)` if any signature/public key fails to parse or verify,
/// or if input slice lengths are inconsistent.
#[cfg(all(feature = "bls", feature = "bls-multi-pairing"))]
pub fn bls_normal_verify_aggregate_multi_message(
    messages: &[&[u8]],
    signatures: &[&[u8]],
    public_keys: &[&[u8]],
) -> Result<(), Error> {
    {
        use std::collections::BTreeSet;
        if messages.len() != signatures.len()
            || signatures.len() != public_keys.len()
            || messages.is_empty()
        {
            return Err(Error::BadSignature);
        }
        let mut seen = BTreeSet::new();
        for &msg in messages {
            if !seen.insert(msg) {
                return Err(Error::BadSignature);
            }
        }
    }
    // Prefer pairing-product helper; on failure, fall back to per-signature verify
    if let Ok(()) =
        signature::bls::verify_aggregate_multi_message_normal(messages, signatures, public_keys)
    {
        Ok(())
    } else {
        for ((m, s), pk) in messages
            .iter()
            .zip(signatures.iter())
            .zip(public_keys.iter())
        {
            let vk = signature::bls::BlsNormal::parse_public_key(pk)?;
            signature::bls::BlsNormal::verify(m, s, &vk)?;
        }
        Ok(())
    }
}

/// Aggregate verification across distinct messages (multi-message) for BLS (small).
/// With the basic `bls` feature (without `bls-multi-pairing`) this helper verifies each
/// signature independently; a multi-pairing batch path is tracked for the `bls-small`
/// backend and will land alongside the multi-pairing feature.
///
/// # Errors
/// Returns `Err(Error::BadSignature)` if any signature/public key fails to parse or verify,
/// or if input slice lengths are inconsistent.
#[cfg(all(feature = "bls", not(feature = "bls-multi-pairing")))]
pub fn bls_small_verify_aggregate_multi_message(
    messages: &[&[u8]],
    signatures: &[&[u8]],
    public_keys: &[&[u8]],
) -> Result<(), Error> {
    {
        use std::collections::BTreeSet;
        if messages.len() != signatures.len()
            || signatures.len() != public_keys.len()
            || messages.is_empty()
        {
            return Err(Error::BadSignature);
        }
        let mut seen = BTreeSet::new();
        for &msg in messages {
            if !seen.insert(msg) {
                return Err(Error::BadSignature);
            }
        }
    }
    for ((m, s), pk) in messages
        .iter()
        .zip(signatures.iter())
        .zip(public_keys.iter())
    {
        let vk = signature::bls::BlsSmall::parse_public_key(pk)?;
        signature::bls::BlsSmall::verify(m, s, &vk)?;
    }
    Ok(())
}

/// Aggregate verification across distinct messages (multi-message) for BLS (small).
/// Uses a single pairing-product when `bls-multi-pairing` is enabled; falls back
/// to per-signature verification on any parsing/hashing failure to preserve correctness.
///
/// # Errors
/// Returns `Err(Error::BadSignature)` if any signature/public key fails to parse or verify,
/// or if input slice lengths are inconsistent.
#[cfg(all(feature = "bls", feature = "bls-multi-pairing"))]
pub fn bls_small_verify_aggregate_multi_message(
    messages: &[&[u8]],
    signatures: &[&[u8]],
    public_keys: &[&[u8]],
) -> Result<(), Error> {
    {
        use std::collections::BTreeSet;
        if messages.len() != signatures.len()
            || signatures.len() != public_keys.len()
            || messages.is_empty()
        {
            return Err(Error::BadSignature);
        }
        let mut seen = BTreeSet::new();
        for &msg in messages {
            if !seen.insert(msg) {
                return Err(Error::BadSignature);
            }
        }
    }
    if let Ok(()) =
        signature::bls::verify_aggregate_multi_message_small(messages, signatures, public_keys)
    {
        Ok(())
    } else {
        for ((m, s), pk) in messages
            .iter()
            .zip(signatures.iter())
            .zip(public_keys.iter())
        {
            let vk = signature::bls::BlsSmall::parse_public_key(pk)?;
            signature::bls::BlsSmall::verify(m, s, &vk)?;
        }
        Ok(())
    }
}
/// Attempt aggregate verification for BLS (small) when all messages are identical.
/// Requires a valid Proof-of-Possession (`PoP`) per public key to prevent rogue-key attacks.
/// Fallback: per-signature verify inside this function.
///
/// # Errors
/// Returns `Err(Error::BadSignature)` if any signature/public key fails to parse or verify,
/// or if input slice lengths are inconsistent. Public keys must be unique.
#[cfg(feature = "bls")]
pub fn bls_small_verify_aggregate_same_message(
    message: &[u8],
    signatures: &[&[u8]],
    public_keys: &[&PublicKey],
    pops: &[&[u8]],
) -> Result<(), Error> {
    if signatures.len() != public_keys.len() || signatures.is_empty() {
        return Err(Error::BadSignature);
    }
    let pk_bytes =
        bls_collect_pks_with_pop(public_keys, pops, Algorithm::BlsSmall, bls_small_pop_verify)?;
    if let Ok(()) =
        signature::bls::verify_aggregate_same_message_small(message, signatures, &pk_bytes)
    {
        Ok(())
    } else {
        for (s, pk) in signatures.iter().zip(pk_bytes.iter()) {
            let vk = signature::bls::BlsSmall::parse_public_key(pk)?;
            signature::bls::BlsSmall::verify(message, s, &vk)?;
        }
        Ok(())
    }
}

/// Attempt aggregate verification for BLS (small) when all messages are identical.
/// Requires a valid Proof-of-Possession (`PoP`) per public key to prevent rogue-key attacks.
/// Fast path: aggregate-only check, no per-signature fallback.
///
/// # Errors
/// Returns `Err(Error::BadSignature)` if any signature/public key fails to parse or verify,
/// or if input slice lengths are inconsistent. Public keys must be unique.
#[cfg(feature = "bls")]
pub fn bls_small_verify_aggregate_same_message_fast(
    message: &[u8],
    signatures: &[&[u8]],
    public_keys: &[&PublicKey],
    pops: &[&[u8]],
) -> Result<(), Error> {
    if signatures.len() != public_keys.len() || signatures.is_empty() {
        return Err(Error::BadSignature);
    }
    let pk_bytes =
        bls_collect_pks_with_pop(public_keys, pops, Algorithm::BlsSmall, bls_small_pop_verify)?;
    signature::bls::verify_aggregate_same_message_small(message, signatures, &pk_bytes)
}

/// Aggregate BLS (normal) signatures (same-message) into a single signature.
///
/// # Errors
/// Returns `Err(Error::BadSignature)` if aggregation fails.
#[cfg(feature = "bls")]
pub fn bls_normal_aggregate_signatures(signatures: &[&[u8]]) -> Result<Vec<u8>, Error> {
    signature::bls::aggregate_same_message_normal(signatures)
}

/// Verify a pre-aggregated BLS (normal) signature for the same-message case against a set of public keys.
/// Requires a valid Proof-of-Possession (`PoP`) per public key to prevent rogue-key attacks.
///
/// # Errors
/// Returns `Err(Error::BadSignature)` on parse/verify failure.
#[cfg(feature = "bls")]
pub fn bls_normal_verify_preaggregated_same_message(
    message: &[u8],
    aggregated_signature: &[u8],
    public_keys: &[&PublicKey],
    pops: &[&[u8]],
) -> Result<(), Error> {
    let pk_bytes = bls_collect_pks_with_pop(
        public_keys,
        pops,
        Algorithm::BlsNormal,
        bls_normal_pop_verify,
    )?;
    signature::bls::verify_preaggregated_same_message_normal(
        message,
        aggregated_signature,
        &pk_bytes,
    )
}

// Note: small-variant pre-aggregated helpers are not exposed; consensus uses BLS-normal only.

/// Verify BLS-Normal Proof-of-Possession (`PoP`) for a given public key.
/// The `PoP` is a BLS signature over `Hash(POP_DST` || `pk_bytes`) using the same
/// BLS-normal ciphersuite as regular signatures.
/// # Errors
/// Returns `Err(Error::BadSignature)` if the public key is not BLS-normal, the proof
/// cannot be parsed, or verification fails.
#[cfg(feature = "bls")]
pub fn bls_normal_pop_verify(pk: &PublicKey, pop: &[u8]) -> Result<(), Error> {
    if pk.algorithm() != Algorithm::BlsNormal {
        return Err(Error::BadSignature);
    }
    let (_alg, pk_bytes) = pk.to_bytes();
    let msg_hashed: [u8; 32] = {
        let mut buf = Vec::with_capacity(POP_DST.len() + pk_bytes.len());
        buf.extend_from_slice(POP_DST.as_bytes());
        buf.extend_from_slice(pk_bytes);
        Hash::new(&buf).into()
    };
    let (alg, payload) = pk.to_bytes();
    if alg != Algorithm::BlsNormal {
        return Err(Error::BadSignature);
    }
    let vk = signature::bls::BlsNormal::parse_public_key(payload)?;
    signature::bls::BlsNormal::verify(&msg_hashed, pop, &vk)
}

/// Create BLS-Normal Proof-of-Possession for the corresponding public key.
/// This signs `Hash(POP_DST` || `pk_bytes`) with the provided private key.
///
/// # Errors
/// Returns `Err(Error::BadSignature)` if the private key is not BLS-normal.
#[cfg(feature = "bls")]
pub fn bls_normal_pop_prove(sk: &PrivateKey) -> Result<Vec<u8>, Error> {
    use crate::secrecy::ExposeSecret as _;
    match sk.0.expose_secret() {
        PrivateKeyInner::BlsNormal(_inner) => {
            let pk = PublicKey::from(sk.clone());
            let (_alg, pk_bytes) = pk.to_bytes();
            let mut msg = Vec::with_capacity(POP_DST.len() + pk_bytes.len());
            msg.extend_from_slice(POP_DST.as_bytes());
            msg.extend_from_slice(pk_bytes);
            let msg_h: [u8; 32] = Hash::new(&msg).into();
            Ok(signature::bls::BlsNormal::sign(
                &msg_h,
                match sk.0.expose_secret() {
                    PrivateKeyInner::BlsNormal(v) => v,
                    _ => unreachable!(),
                },
            ))
        }
        _ => Err(Error::BadSignature),
    }
}

/// Verify BLS-Small Proof-of-Possession (`PoP`) for a given public key.
/// The `PoP` is a BLS signature over `Hash(POP_DST` || `pk_bytes`) using the BLS-small ciphersuite.
///
/// # Errors
/// Returns [`Error::BadSignature`] if the key is not BLS-Small, the signature is malformed,
/// or verification fails.
#[cfg(feature = "bls")]
pub fn bls_small_pop_verify(pk: &PublicKey, pop: &[u8]) -> Result<(), Error> {
    if pk.algorithm() != Algorithm::BlsSmall {
        return Err(Error::BadSignature);
    }
    let (_alg, pk_bytes) = pk.to_bytes();
    let msg_h: [u8; 32] = {
        let mut buf = Vec::with_capacity(POP_DST.len() + pk_bytes.len());
        buf.extend_from_slice(POP_DST.as_bytes());
        buf.extend_from_slice(pk_bytes);
        Hash::new(&buf).into()
    };
    let (alg, payload) = pk.to_bytes();
    if alg != Algorithm::BlsSmall {
        return Err(Error::BadSignature);
    }
    let vk = signature::bls::BlsSmall::parse_public_key(payload)?;
    signature::bls::BlsSmall::verify(&msg_h, pop, &vk)
}

/// Create BLS-Small Proof-of-Possession for the corresponding public key.
/// This signs `Hash(POP_DST` || `pk_bytes`) with the provided private key.
///
/// # Errors
/// Returns [`Error::BadSignature`] if the private key is not BLS-Small.
#[cfg(feature = "bls")]
pub fn bls_small_pop_prove(sk: &PrivateKey) -> Result<Vec<u8>, Error> {
    use crate::secrecy::ExposeSecret as _;
    match sk.0.expose_secret() {
        PrivateKeyInner::BlsSmall(_inner) => {
            let pk = PublicKey::from(sk.clone());
            let (_alg, pk_bytes) = pk.to_bytes();
            let mut msg = Vec::with_capacity(POP_DST.len() + pk_bytes.len());
            msg.extend_from_slice(POP_DST.as_bytes());
            msg.extend_from_slice(pk_bytes);
            let msg_h: [u8; 32] = Hash::new(&msg).into();
            Ok(signature::bls::BlsSmall::sign(
                &msg_h,
                match sk.0.expose_secret() {
                    PrivateKeyInner::BlsSmall(v) => v,
                    _ => unreachable!(),
                },
            ))
        }
        _ => Err(Error::BadSignature),
    }
}

/// Aggregate-style check for Ed25519: per-signature verification wrapper.
/// # Errors
/// Returns `Err(Error::BadSignature)` if aggregate verification fails for any tuple or the input is empty.
pub fn ed25519_verify_aggregate(
    messages: &[&[u8]],
    signatures: &[&[u8]],
    public_keys: &[&[u8]],
) -> Result<(), Error> {
    ed25519_verify_batch_deterministic(messages, signatures, public_keys, [0u8; 32])
}

/// Aggregate-style check for ML‑DSA (Dilithium3): verifies each signature on the shared or unique message.
///
/// # Errors
/// Returns [`Error::BadSignature`] when the inputs are inconsistent or any signature fails verification.
pub fn pqc_verify_aggregate(
    messages: &[&[u8]],
    signatures: &[&[u8]],
    public_keys: &[&[u8]],
) -> Result<(), Error> {
    // For Dilithium3 there is no standard aggregate signature; fall back to per-sig check.
    if !(messages.len() == signatures.len() && signatures.len() == public_keys.len()) {
        return Err(Error::BadSignature);
    }
    for ((m, s), pk) in messages
        .iter()
        .zip(signatures.iter())
        .zip(public_keys.iter())
    {
        pqc_verify_batch_deterministic(
            core::slice::from_ref(m),
            core::slice::from_ref(s),
            core::slice::from_ref(pk),
            [0u8; 32],
        )?;
    }
    Ok(())
}

impl PublicKeyCompact {
    fn new(algorithm: Algorithm, payload: &[u8]) -> Self {
        // Use stable discriminants matching `Algorithm::try_from(u8)` below.
        let algorithm: u8 = match algorithm {
            Algorithm::Ed25519 => 0,
            Algorithm::Secp256k1 => 1,
            Algorithm::MlDsa => 4,
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_256ParamSetA => 5,
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_256ParamSetB => 6,
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_256ParamSetC => 7,
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_512ParamSetA => 8,
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_512ParamSetB => 9,
            #[cfg(feature = "bls")]
            Algorithm::BlsNormal => 2,
            #[cfg(feature = "bls")]
            Algorithm::BlsSmall => 3,
            #[cfg(feature = "sm")]
            Algorithm::Sm2 => 10,
        };
        let mut bytes = Vec::with_capacity(1 + payload.len());
        bytes.push(algorithm);
        bytes.extend_from_slice(payload);
        Self {
            algorithm_and_payload: ConstVec::new(bytes),
        }
    }

    fn algorithm(&self) -> Algorithm {
        let algorithm = self.algorithm_and_payload[0];
        Algorithm::try_from(algorithm).expect("Invalid PublicKeyCompact::algorithm_and_payload")
    }

    fn payload(&self) -> &[u8] {
        &self.algorithm_and_payload[1..]
    }
}

impl From<PublicKeyFull> for PublicKeyCompact {
    fn from(public_key: PublicKeyFull) -> Self {
        Self::new(public_key.algorithm(), &public_key.payload())
    }
}

#[cfg(not(feature = "ffi_import"))]
impl norito::core::NoritoSerialize for PublicKeyCompact {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        <ConstVec<u8> as norito::core::NoritoSerialize>::serialize(
            &self.algorithm_and_payload,
            writer,
        )
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        <ConstVec<u8> as norito::core::NoritoSerialize>::encoded_len_hint(
            &self.algorithm_and_payload,
        )
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        <ConstVec<u8> as norito::core::NoritoSerialize>::encoded_len_exact(
            &self.algorithm_and_payload,
        )
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<'de> norito::core::NoritoDeserialize<'de> for PublicKeyCompact {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("PublicKeyCompact decode")
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let archived_bytes = archived.cast::<ConstVec<u8>>();
        let payload = ConstVec::<u8>::try_deserialize(archived_bytes)?;
        #[cfg(debug_assertions)]
        {
            let preview_len = core::cmp::min(payload.len(), 16);
            eprintln!(
                "PublicKeyCompact::try_deserialize payload_len={} preview={:?}",
                payload.len(),
                &payload[..preview_len]
            );
        }
        if payload.is_empty() {
            return Err(norito::core::Error::length_mismatch_detail(
                "PublicKeyCompact::try_deserialize",
                0,
                1,
                0,
            ));
        }
        let tag = payload[0];
        let algorithm = Algorithm::try_from(tag)
            .map_err(|()| norito::core::Error::invalid_tag("PublicKeyCompact::algorithm", tag))?;
        PublicKeyFull::from_bytes(algorithm, &payload[1..])
            .map_err(|err| norito::core::Error::Message(err.to_string()))?;
        Ok(Self {
            algorithm_and_payload: payload,
        })
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<'a> norito::core::DecodeFromSlice<'a> for PublicKeyCompact {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (payload, used) =
            <ConstVec<u8> as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
        if payload.is_empty() {
            return Err(norito::core::Error::length_mismatch_detail(
                "PublicKeyCompact::decode_from_slice",
                0,
                1,
                0,
            ));
        }
        let tag = payload[0];
        let algorithm = Algorithm::try_from(tag)
            .map_err(|()| norito::core::Error::invalid_tag("PublicKeyCompact::algorithm", tag))?;
        PublicKeyFull::from_bytes(algorithm, &payload[1..])
            .map_err(|err| norito::core::Error::Message(err.to_string()))?;
        Ok((
            Self {
                algorithm_and_payload: payload,
            },
            used,
        ))
    }
}

ffi::ffi_item! {
    /// Public key used in signatures.
    ///
    /// Its serialized form (via serde `Serialize`/`Deserialize`, plus [`Display`] and [`FromStr`]) is
    /// represented as a [multihash](https://www.multiformats.io/multihash/) string.
    /// [`FromStr`] also accepts an algorithm-prefixed form like
    /// `"ed25519:<multihash-hex>"` for clarity in JSON. [`Display`] returns
    /// the bare multihash hex. Multihash hex is canonical: varint bytes are
    /// lowercase hex and payload bytes are uppercase hex; parsing rejects
    /// non-canonical casing and `0x` prefixes.
    /// For example:
    ///
    /// ```
    /// use iroha_crypto::{PublicKey, Algorithm};
    ///
    /// let key = PublicKey::from_hex(
    ///     Algorithm::Ed25519,
    ///     "1509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4",
    /// )
    /// .unwrap();
    ///
    /// assert_eq!(
    ///     format!("{key}"),
    ///     "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
    /// );
    /// ```
    #[derive(Clone, PartialEq, Eq, TypeId)]
    #[repr(transparent)]
    #[cfg_attr(all(feature = "ffi_export", not(feature = "ffi_import")), ffi_type(opaque))]
    pub struct PublicKey(PublicKeyCompact);
}

#[ffi_impl_opaque]
impl PublicKey {
    fn new(inner: PublicKeyFull) -> Self {
        Self(inner.into())
    }

    /// Creates a new public key from raw bytes received from elsewhere
    ///
    /// # Errors
    ///
    /// Fails if public key parsing fails
    pub fn from_bytes(algorithm: Algorithm, payload: &[u8]) -> Result<Self, ParseError> {
        // Validate that `payload` is valid before constructing the key.
        let inner = PublicKeyFull::from_bytes(algorithm, payload)?;
        Ok(Self::new(inner))
    }

    /// Extracts raw bytes from the public key, copying the payload.
    ///
    /// `into_bytes()` without copying is not provided because underlying crypto
    /// libraries do not provide move functionality.
    pub fn to_bytes(&self) -> (Algorithm, &[u8]) {
        (self.0.algorithm(), self.0.payload())
    }

    /// Construct from hex encoded string. A shorthand over [`Self::from_bytes`].
    ///
    /// # Errors
    ///
    /// - If the given payload is not hex encoded
    /// - If the given payload is not a valid private key
    pub fn from_hex(algorithm: Algorithm, payload: impl AsRef<str>) -> Result<Self, ParseError> {
        let payload = hex_decode(payload.as_ref())?;

        Self::from_bytes(algorithm, &payload)
    }

    /// Get the digital signature algorithm of the public key
    pub fn algorithm(&self) -> Algorithm {
        self.0.algorithm()
    }
}

#[cfg(not(feature = "ffi_import"))]
impl PublicKey {
    fn normalize(&self) -> String {
        let (algorithm, payload) = self.to_bytes();
        let bytes = multihash::encode_public_key(algorithm, payload)
            .expect("Failed to convert multihash to bytes.");

        multihash::multihash_to_hex_string(&bytes)
            .expect("Failed to convert multihash to hex string.")
    }

    #[cfg(not(feature = "ffi_import"))]
    /// Format as an algorithm-prefixed multihash string (e.g., "ed25519:...").
    pub fn to_prefixed_string(&self) -> String {
        let (algorithm, payload) = self.to_bytes();
        multihash::encode_public_key_prefixed(algorithm, payload)
            .expect("Failed to convert multihash to prefixed string.")
    }
}

#[cfg(not(feature = "ffi_import"))]
impl core::hash::Hash for PublicKey {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        (self.to_bytes()).hash(state)
    }
}

impl PartialOrd for PublicKey {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PublicKey {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.to_bytes().cmp(&other.to_bytes())
    }
}

// Bridge Norito slice-based decoding for PublicKey to the codec decoder.
// This allows containers (Vec/Option) using PublicKey elements to decode via
// `norito::core::DecodeFromSlice` without re-implementing deep decoders.
impl<'a> norito::core::DecodeFromSlice<'a> for PublicKey {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (normalized, used) =
            <String as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
        let key = normalized
            .parse::<Self>()
            .map_err(|err: ParseError| norito::core::Error::Message(err.to_string()))?;
        Ok((key, used))
    }
}

#[cfg(not(feature = "ffi_import"))]
impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // This could be simplified using `f.field_with` when `debug_closure_helpers` feature become stable
        struct Helper {
            algorithm: Algorithm,
            normalized: String,
        }
        impl fmt::Debug for Helper {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(self.algorithm.as_static_str())
                    .field(&self.normalized)
                    .finish()
            }
        }

        let helper = Helper {
            algorithm: self.algorithm(),
            normalized: self.normalize(),
        };
        f.debug_tuple("PublicKey").field(&helper).finish()
    }
}

#[cfg(not(feature = "ffi_import"))]
impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.normalize())
    }
}

#[cfg(not(feature = "ffi_import"))]
impl norito::json::JsonSerialize for PublicKey {
    fn json_serialize(&self, out: &mut String) {
        let normalized = self.normalize();
        norito::json::JsonSerialize::json_serialize(&normalized, out);
    }
}

#[cfg(not(feature = "ffi_import"))]
impl norito::json::JsonDeserialize for PublicKey {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value: String = norito::json::JsonDeserialize::json_deserialize(parser)?;
        value
            .parse()
            .map_err(|err: ParseError| norito::json::Error::Message(err.to_string()))
    }
}

#[cfg(not(feature = "ffi_import"))]
impl norito::json::JsonDeserialize for PrivateKey {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value: String = norito::json::JsonDeserialize::json_deserialize(parser)?;
        value
            .parse()
            .map_err(|err: ParseError| norito::json::Error::Message(err.to_string()))
    }
}

#[cfg(not(feature = "ffi_import"))]
impl FromStr for PublicKey {
    type Err = ParseError;

    fn from_str(key: &str) -> Result<Self, Self::Err> {
        let (algorithm, payload) = multihash::decode_public_key_str(key)?;
        Self::from_bytes(algorithm, &payload)
    }
}

#[cfg(not(feature = "ffi_import"))]
impl norito::core::NoritoSerialize for PublicKey {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        let normalized = self.normalize();
        norito::core::NoritoSerialize::serialize(&normalized, writer)
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<'de> norito::core::NoritoDeserialize<'de> for PublicKey {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("PublicKey decode")
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        #[allow(unsafe_code)]
        let archived_str =
            unsafe { &*core::ptr::from_ref(archived).cast::<norito::core::Archived<String>>() };
        let normalized = String::try_deserialize(archived_str)?;
        normalized
            .parse::<Self>()
            .map_err(|err: ParseError| norito::core::Error::Message(err.to_string()))
    }
}

#[cfg(not(feature = "ffi_import"))]
#[cfg(not(feature = "ffi_import"))]
impl IntoSchema for PublicKey {
    fn type_name() -> String {
        Self::id()
    }

    fn update_schema_map(metamap: &mut MetaMap) {
        if !metamap.contains_key::<Self>() {
            if !metamap.contains_key::<Algorithm>() {
                <Algorithm as iroha_schema::IntoSchema>::update_schema_map(metamap);
            }
            if !metamap.contains_key::<ConstVec<u8>>() {
                <ConstVec<u8> as iroha_schema::IntoSchema>::update_schema_map(metamap);
            }

            metamap.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: vec![
                    Declaration {
                        name: String::from("algorithm"),
                        ty: core::any::TypeId::of::<Algorithm>(),
                    },
                    Declaration {
                        name: String::from("payload"),
                        ty: core::any::TypeId::of::<ConstVec<u8>>(),
                    },
                ],
            }));
        }
    }
}

/// Deriving a public key from a private key is currently disabled when the `ffi_import` feature is active.
#[cfg(not(feature = "ffi_import"))]
impl From<PrivateKey> for PublicKey {
    fn from(private_key: PrivateKey) -> Self {
        use crate::secrecy::ExposeSecret;
        if private_key.algorithm() == Algorithm::MlDsa {
            let derived = {
                let secret = match private_key.0.expose_secret() {
                    PrivateKeyInner::MlDsa(sk) => sk.as_secret(),
                    _ => unreachable!("algorithm() returned MlDsa"),
                };
                mldsa_seed::dilithium3::public_key_from_secret(secret)
                    .expect("ML-DSA secret key should derive a valid public key")
            };
            return derived;
        }

        let inner = match private_key.0.expose_secret() {
            PrivateKeyInner::Ed25519(secret) => PublicKeyFull::Ed25519(
                ed25519::Ed25519Sha512::keypair(KeyGenOption::FromPrivateKey(secret.clone())).0,
            ),
            PrivateKeyInner::Secp256k1(secret) => PublicKeyFull::Secp256k1(
                secp256k1::EcdsaSecp256k1Sha256::keypair(KeyGenOption::FromPrivateKey(
                    secret.clone(),
                ))
                .0,
            ),
            PrivateKeyInner::MlDsa(_) => unreachable!("handled ML-DSA earlier"),
            #[cfg(feature = "gost")]
            PrivateKeyInner::Gost { algorithm, secret } => {
                let derived = signature::gost::derive_public_key(*algorithm, secret)
                    .expect("GOST public key derivation must succeed for valid private keys");
                PublicKeyFull::Gost {
                    algorithm: *algorithm,
                    key: derived,
                }
            }
            #[cfg(feature = "bls")]
            PrivateKeyInner::BlsNormal(secret) => PublicKeyFull::BlsNormal(
                bls::BlsNormal::keypair(KeyGenOption::FromPrivateKey(secret.clone())).0,
            ),
            #[cfg(feature = "bls")]
            PrivateKeyInner::BlsSmall(secret) => PublicKeyFull::BlsSmall(
                bls::BlsSmall::keypair(KeyGenOption::FromPrivateKey(secret.clone())).0,
            ),
            #[cfg(feature = "sm")]
            PrivateKeyInner::Sm2(key) => PublicKeyFull::Sm2(key.public_key()),
        };

        Self::new(inner)
    }
}

#[derive(Clone)]
struct MlDsaSecretKey {
    inner: Arc<MlDsaSecretKeyInner>,
}

struct MlDsaSecretKeyInner {
    secret: pqcrypto_dilithium::dilithium3::SecretKey,
}

impl MlDsaSecretKey {
    fn new(inner: &pqcrypto_dilithium::dilithium3::SecretKey) -> Self {
        Self {
            inner: Arc::new(MlDsaSecretKeyInner { secret: *inner }),
        }
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
        use pqcrypto_traits::sign::SecretKey as _;
        let inner = pqcrypto_dilithium::dilithium3::SecretKey::from_bytes(bytes)
            .map_err(|err| ParseError(err.to_string()))?;
        Ok(Self::new(&inner))
    }

    fn as_secret(&self) -> &pqcrypto_dilithium::dilithium3::SecretKey {
        &self.inner.secret
    }

    fn to_vec(&self) -> Vec<u8> {
        use pqcrypto_traits::sign::SecretKey as _;
        self.inner.secret.as_bytes().to_vec()
    }

    fn sign(&self, payload: &[u8]) -> Vec<u8> {
        use pqcrypto_dilithium::dilithium3 as dilithium;
        use pqcrypto_traits::sign::DetachedSignature as _;

        let sig = dilithium::detached_sign(payload, self.as_secret());
        sig.as_bytes().to_vec()
    }

    #[cfg(test)]
    fn strong_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }
}

impl PartialEq for MlDsaSecretKey {
    fn eq(&self, other: &Self) -> bool {
        use pqcrypto_traits::sign::SecretKey as _;
        self.inner.secret.as_bytes() == other.inner.secret.as_bytes()
    }
}

impl Eq for MlDsaSecretKey {}

impl ZeroizeOnDrop for MlDsaSecretKey {}

#[allow(unsafe_code)]
impl Drop for MlDsaSecretKeyInner {
    fn drop(&mut self) {
        use core::{mem, ptr};

        let byte_ptr = ptr::addr_of_mut!(self.secret).cast::<u8>();
        unsafe {
            ptr::write_bytes(
                byte_ptr,
                0,
                mem::size_of::<pqcrypto_dilithium::dilithium3::SecretKey>(),
            );
        }
    }
}

#[derive(Clone)]
#[allow(variant_size_differences, clippy::large_enum_variant)]
enum PrivateKeyInner {
    Ed25519(ed25519::PrivateKey),
    Secp256k1(secp256k1::PrivateKey),
    MlDsa(MlDsaSecretKey),
    #[cfg(feature = "gost")]
    Gost {
        algorithm: Algorithm,
        secret: signature::gost::PrivateKey,
    },
    #[cfg(feature = "sm")]
    Sm2(sm::Sm2PrivateKey),
    #[cfg(feature = "bls")]
    BlsNormal(bls::BlsNormalPrivateKey),
    #[cfg(feature = "bls")]
    BlsSmall(bls::BlsSmallPrivateKey),
}

ffi::ffi_item! {
    /// Private Key used in signatures.
    #[derive(Clone)]
    #[cfg_attr(all(feature = "ffi_export", not(feature = "ffi_import")), ffi_type(opaque))]
    #[allow(variant_size_differences)]
    pub struct PrivateKey(Box<Secret<PrivateKeyInner>>);
}

#[allow(unsafe_code)]
unsafe impl Send for PrivateKey {}

#[allow(unsafe_code)]
unsafe impl Sync for PrivateKey {}

impl PartialEq for PrivateKey {
    fn eq(&self, other: &Self) -> bool {
        use crate::secrecy::ExposeSecret;
        match (self.0.expose_secret(), other.0.expose_secret()) {
            (PrivateKeyInner::Ed25519(l), PrivateKeyInner::Ed25519(r)) => l == r,
            (PrivateKeyInner::Secp256k1(l), PrivateKeyInner::Secp256k1(r)) => l == r,
            (PrivateKeyInner::MlDsa(l), PrivateKeyInner::MlDsa(r)) => l == r,
            #[cfg(feature = "gost")]
            (
                PrivateKeyInner::Gost {
                    algorithm: la,
                    secret: ls,
                },
                PrivateKeyInner::Gost {
                    algorithm: ra,
                    secret: rs,
                },
            ) => la == ra && ls == rs,
            #[cfg(feature = "bls")]
            (PrivateKeyInner::BlsNormal(l), PrivateKeyInner::BlsNormal(r)) => {
                l.to_bytes() == r.to_bytes()
            }
            #[cfg(feature = "bls")]
            (PrivateKeyInner::BlsSmall(l), PrivateKeyInner::BlsSmall(r)) => {
                l.to_bytes() == r.to_bytes()
            }
            #[cfg(feature = "sm")]
            (PrivateKeyInner::Sm2(l), PrivateKeyInner::Sm2(r)) => l == r,
            _ => false,
        }
    }
}

impl Eq for PrivateKey {}

impl PrivateKey {
    /// Creates a new public key from raw bytes received from elsewhere
    ///
    /// # Errors
    ///
    /// - If the given payload is not a valid private key for the given digest function
    pub fn from_bytes(algorithm: Algorithm, payload: &[u8]) -> Result<Self, ParseError> {
        match algorithm {
            Algorithm::Ed25519 => {
                ed25519::Ed25519Sha512::parse_private_key(payload).map(PrivateKeyInner::Ed25519)
            }
            Algorithm::Secp256k1 => secp256k1::EcdsaSecp256k1Sha256::parse_private_key(payload)
                .map(PrivateKeyInner::Secp256k1),
            Algorithm::MlDsa => MlDsaSecretKey::from_bytes(payload).map(PrivateKeyInner::MlDsa),
            #[cfg(feature = "gost")]
            Algorithm::Gost3410_2012_256ParamSetA
            | Algorithm::Gost3410_2012_256ParamSetB
            | Algorithm::Gost3410_2012_256ParamSetC
            | Algorithm::Gost3410_2012_512ParamSetA
            | Algorithm::Gost3410_2012_512ParamSetB => {
                signature::gost::parse_private_key(algorithm, payload)
                    .map(|secret| PrivateKeyInner::Gost { algorithm, secret })
            }
            #[cfg(feature = "sm")]
            Algorithm::Sm2 => sm::decode_sm2_private_key_payload(payload).map(PrivateKeyInner::Sm2),
            #[cfg(feature = "bls")]
            Algorithm::BlsNormal => {
                bls::BlsNormal::parse_private_key(payload).map(PrivateKeyInner::BlsNormal)
            }
            #[cfg(feature = "bls")]
            Algorithm::BlsSmall => {
                bls::BlsSmall::parse_private_key(payload).map(PrivateKeyInner::BlsSmall)
            }
        }
        .map(Secret::new)
        .map(Box::new)
        .map(PrivateKey)
    }

    /// Construct [`PrivateKey`] from hex encoded string.
    /// A shorthand over [`PrivateKey::from_bytes`]
    ///
    /// # Errors
    ///
    /// - If the given payload is not hex encoded
    /// - If the given payload is not a valid private key
    pub fn from_hex(algorithm: Algorithm, payload: impl AsRef<str>) -> Result<Self, ParseError> {
        let payload = hex_decode(payload.as_ref())?;

        Self::from_bytes(algorithm, &payload)
    }

    /// Get the digital signature algorithm of the private key
    pub fn algorithm(&self) -> Algorithm {
        use crate::secrecy::ExposeSecret;
        match self.0.expose_secret() {
            PrivateKeyInner::Ed25519(_) => Algorithm::Ed25519,
            PrivateKeyInner::Secp256k1(_) => Algorithm::Secp256k1,
            PrivateKeyInner::MlDsa(_) => Algorithm::MlDsa,
            #[cfg(feature = "gost")]
            PrivateKeyInner::Gost { algorithm, .. } => *algorithm,
            #[cfg(feature = "sm")]
            PrivateKeyInner::Sm2(_) => Algorithm::Sm2,
            #[cfg(feature = "bls")]
            PrivateKeyInner::BlsNormal(_) => Algorithm::BlsNormal,
            #[cfg(feature = "bls")]
            PrivateKeyInner::BlsSmall(_) => Algorithm::BlsSmall,
        }
    }

    /// Key payload
    fn payload(&self) -> Vec<u8> {
        use crate::secrecy::ExposeSecret;
        match self.0.expose_secret() {
            PrivateKeyInner::Ed25519(key) => key.to_bytes().to_vec(),
            PrivateKeyInner::Secp256k1(key) => key.to_bytes().to_vec(),
            PrivateKeyInner::MlDsa(key) => key.to_vec(),
            #[cfg(feature = "gost")]
            PrivateKeyInner::Gost { secret, .. } => secret.as_bytes().to_vec(),
            #[cfg(feature = "sm")]
            PrivateKeyInner::Sm2(key) => {
                sm::encode_sm2_private_key_payload(key.distid(), &key.secret_bytes())
                    .expect("SM2 private key payload must be encodable")
            }
            #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
            PrivateKeyInner::BlsNormal(key) => key.to_bytes().clone(),
            #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
            PrivateKeyInner::BlsSmall(key) => key.to_bytes().clone(),
            #[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
            PrivateKeyInner::BlsNormal(key) => key.to_bytes().to_vec(),
            #[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
            PrivateKeyInner::BlsSmall(key) => key.to_bytes().to_vec(),
        }
    }

    /// Extracts the raw bytes from the private key, copying the payload.
    ///
    /// `into_bytes()` without copying is not provided because underlying crypto
    /// libraries do not provide move functionality.
    pub fn to_bytes(&self) -> (Algorithm, Vec<u8>) {
        (self.algorithm(), self.payload())
    }
}

#[cfg(not(feature = "ffi_import"))]
impl norito::json::JsonDeserialize for KeyPair {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let mut map = norito::json::MapVisitor::new(parser)?;
        let mut public_key: Option<PublicKey> = None;
        let mut private_key: Option<ExposedPrivateKey> = None;

        while let Some(field) = map.next_key()? {
            match field.as_str() {
                "public_key" => {
                    if public_key.is_some() {
                        return Err(norito::json::MapVisitor::duplicate_field("public_key"));
                    }
                    public_key = Some(map.parse_value()?);
                }
                "private_key" => {
                    if private_key.is_some() {
                        return Err(norito::json::MapVisitor::duplicate_field("private_key"));
                    }
                    private_key = Some(map.parse_value()?);
                }
                _ => map.skip_value()?,
            }
        }

        map.finish()?;

        let public_key =
            public_key.ok_or_else(|| norito::json::MapVisitor::missing_field("public_key"))?;
        let exposed =
            private_key.ok_or_else(|| norito::json::MapVisitor::missing_field("private_key"))?;

        KeyPair::new(public_key, exposed.0.clone())
            .map_err(|err| norito::json::Error::Message(err.to_string()))
    }
}

#[cfg(not(feature = "ffi_import"))]
impl norito::json::JsonSerialize for KeyPair {
    fn json_serialize(&self, out: &mut String) {
        use norito::json::{self, Value};

        let mut map = norito::json::Map::new();
        map.insert(
            "public_key".into(),
            json::to_value(&self.public_key).expect("serialize public key"),
        );
        map.insert(
            "private_key".into(),
            json::to_value(&self.private_key).expect("serialize private key"),
        );
        json::JsonSerialize::json_serialize(&Value::Object(map), out);
    }
}

impl FromStr for PrivateKey {
    type Err = ParseError;

    fn from_str(key: &str) -> Result<Self, Self::Err> {
        let (algorithm, payload) = multihash::decode_private_key_str(key)?;
        PrivateKey::from_bytes(algorithm, &payload)
    }
}

impl ZeroizeOnDrop for PrivateKeyInner {}

impl Drop for PrivateKeyInner {
    fn drop(&mut self) {
        fn assert_will_zeroize_on_drop(_value: &mut impl ZeroizeOnDrop) {
            // checks that `zeroize` feature of `ed25519-dalek` crate is enabled
            // actual zeroize will be in `impl Drop` for nested key
        }
        match self {
            PrivateKeyInner::Ed25519(key) => {
                assert_will_zeroize_on_drop(key);
            }
            PrivateKeyInner::Secp256k1(key) => {
                assert_will_zeroize_on_drop(key);
            }
            PrivateKeyInner::MlDsa(key) => {
                assert_will_zeroize_on_drop(key);
            }
            #[cfg(feature = "gost")]
            PrivateKeyInner::Gost { secret, .. } => {
                assert_will_zeroize_on_drop(secret);
            }
            #[cfg(feature = "sm")]
            PrivateKeyInner::Sm2(key) => {
                assert_will_zeroize_on_drop(key);
            }
            #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
            PrivateKeyInner::BlsNormal(key) => {
                key.zeroize();
            }
            #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
            PrivateKeyInner::BlsSmall(key) => {
                key.zeroize();
            }
            #[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
            PrivateKeyInner::BlsNormal(key) => {
                key.zeroize();
            }
            #[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
            PrivateKeyInner::BlsSmall(key) => {
                key.zeroize();
            }
        }
    }
}

const PRIVATE_KEY_REDACTED: &str = "[REDACTED PrivateKey]";

#[cfg(not(feature = "ffi_import"))]
impl core::fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        PRIVATE_KEY_REDACTED.fmt(f)
    }
}

#[cfg(not(feature = "ffi_import"))]
impl core::fmt::Display for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        PRIVATE_KEY_REDACTED.fmt(f)
    }
}

#[cfg(not(feature = "ffi_import"))]
impl norito::json::JsonSerialize for PrivateKey {
    fn json_serialize(&self, out: &mut String) {
        let redacted = PRIVATE_KEY_REDACTED.to_string();
        norito::json::JsonSerialize::json_serialize(&redacted, out);
    }
}

/// Use when you need to format/serialize private key (e.g in kagami)
///
/// [`FromStr`] accepts both a bare multihash hex string and an algorithm-prefixed
/// variant such as `"ml-dsa:<multihash-hex>"`. The default [`Display`] formatting returns
/// the bare multihash hex. Multihash hex is canonical: varint bytes are lowercase hex
/// and payload bytes are uppercase hex; parsing rejects non-canonical casing and
/// `0x` prefixes.
#[derive(Clone, Eq, PartialEq)]
pub struct ExposedPrivateKey(pub PrivateKey);

impl FromStr for ExposedPrivateKey {
    type Err = ParseError;

    fn from_str(key: &str) -> Result<Self, Self::Err> {
        let private_key = key.parse()?;
        Ok(ExposedPrivateKey(private_key))
    }
}

impl ExposedPrivateKey {
    fn normalize(&self) -> String {
        let (algorithm, payload) = self.0.to_bytes();
        let bytes = multihash::encode_private_key(algorithm, &payload)
            .expect("Failed to convert multihash to bytes.");

        multihash::multihash_to_hex_string(&bytes)
            .expect("Failed to convert multihash to hex string.")
    }

    #[cfg(not(feature = "ffi_import"))]
    /// Format as an algorithm-prefixed multihash string (e.g., "ml-dsa:...").
    pub fn to_prefixed_string(&self) -> String {
        let (algorithm, payload) = self.0.to_bytes();
        multihash::encode_private_key_prefixed(algorithm, &payload)
            .expect("Failed to convert multihash to prefixed string.")
    }
}

#[cfg(not(feature = "ffi_import"))]
impl fmt::Debug for ExposedPrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple(self.0.algorithm().as_static_str())
            .field(&self.normalize())
            .finish()
    }
}

#[cfg(not(feature = "ffi_import"))]
impl norito::json::JsonSerialize for ExposedPrivateKey {
    fn json_serialize(&self, out: &mut String) {
        let normalized = self.normalize();
        norito::json::JsonSerialize::json_serialize(&normalized, out);
    }
}

#[cfg(not(feature = "ffi_import"))]
impl norito::json::JsonDeserialize for ExposedPrivateKey {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value: String = norito::json::JsonDeserialize::json_deserialize(parser)?;
        value
            .parse::<ExposedPrivateKey>()
            .map_err(|err| norito::json::Error::Message(err.to_string()))
    }
}

impl norito::core::NoritoSerialize for ExposedPrivateKey {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        let normalized = self.normalize();
        norito::core::NoritoSerialize::serialize(&normalized, writer)
    }
}

impl<'de> norito::core::NoritoDeserialize<'de> for ExposedPrivateKey {
    fn deserialize(archived: &'de norito::core::Archived<ExposedPrivateKey>) -> Self {
        Self::try_deserialize(archived).expect("ExposedPrivateKey normalization")
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<ExposedPrivateKey>,
    ) -> Result<Self, norito::core::Error> {
        let archived_str: &norito::core::Archived<String> = archived.cast();
        let raw = <String as norito::core::NoritoDeserialize>::try_deserialize(archived_str)?;
        raw.parse::<ExposedPrivateKey>()
            .map_err(|err| norito::core::Error::Message(err.to_string()))
    }
}

#[cfg(not(feature = "ffi_import"))]
impl fmt::Display for ExposedPrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.normalize())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ZeroizingConstVec(ConstVec<u8>);

impl ZeroizingConstVec {
    fn new(bytes: Vec<u8>) -> Self {
        Self(ConstVec::from(bytes))
    }

    fn as_slice(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl Zeroize for ZeroizingConstVec {
    fn zeroize(&mut self) {
        let mut bytes = core::mem::take(&mut self.0).into_vec();
        bytes.fill(0);
        record_session_key_zeroization(&bytes);
        self.0 = ConstVec::from(bytes);
    }
}

impl ZeroizeOnDrop for ZeroizingConstVec {}

impl Drop for ZeroizingConstVec {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl AsRef<[u8]> for ZeroizingConstVec {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

fn session_key_zeroization_log() -> &'static std::sync::Mutex<Vec<u8>> {
    use std::sync::{Mutex, OnceLock};

    static LOG: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();
    LOG.get_or_init(|| Mutex::new(Vec::new()))
}

fn record_session_key_zeroization(bytes: &[u8]) {
    let mut guard = session_key_zeroization_log()
        .lock()
        .expect("session key zeroization log poisoned");
    guard.clear();
    guard.extend_from_slice(bytes);
}

#[doc(hidden)]
pub fn __debug_last_zeroized_session_key() -> Vec<u8> {
    session_key_zeroization_log()
        .lock()
        .expect("session key zeroization log poisoned")
        .clone()
}

#[doc(hidden)]
pub fn __debug_clear_last_zeroized_session_key() {
    session_key_zeroization_log()
        .lock()
        .expect("session key zeroization log poisoned")
        .clear();
}

/// A session key derived from a key exchange. Will usually be used for a symmetric encryption afterwards
pub struct SessionKey(ZeroizingConstVec);

impl SessionKey {
    /// Create a new [`SessionKey`] from raw key material.
    pub fn new(payload: Vec<u8>) -> Self {
        Self(ZeroizingConstVec::new(payload))
    }

    /// Expose the raw bytes of the session key
    pub fn payload(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl From<Vec<u8>> for SessionKey {
    fn from(payload: Vec<u8>) -> Self {
        Self::new(payload)
    }
}

/// Shim for decoding hexadecimal strings
pub(crate) fn hex_decode<T: AsRef<[u8]> + ?Sized>(payload: &T) -> Result<Vec<u8>, ParseError> {
    let bytes = payload.as_ref();
    let trimmed = if bytes.len() >= 2 && bytes[0] == b'0' && matches!(bytes[1], b'x' | b'X') {
        &bytes[2..]
    } else {
        bytes
    };
    hex::decode(trimmed).map_err(|err| ParseError(err.to_string()))
}

pub mod error {
    //! Module containing errors
    use super::*;

    /// Error indicating algorithm could not be found
    #[derive(Debug, Display, Clone, Copy)]
    #[display("Algorithm not supported")]
    pub struct NoSuchAlgorithm;

    impl std::error::Error for NoSuchAlgorithm {}

    /// Error parsing a key
    #[derive(Debug, Display, Clone, PartialEq, Eq)]
    #[display("{_0}")]
    pub struct ParseError(pub(crate) String);

    impl std::error::Error for ParseError {}

    #[cfg(all(feature = "ffi_export", not(feature = "ffi_import")))]
    impl iroha_ffi::IntoFfiReturn for ParseError {
        fn into_ffi_return(self) -> iroha_ffi::FfiReturn {
            let _ = self;
            iroha_ffi::FfiReturn::ExecutionFail
        }
    }

    /// Error when dealing with cryptographic functions
    #[derive(Debug, Display, PartialEq, Eq)]
    pub enum Error {
        /// Returned when trying to create an algorithm which does not exist
        #[display("Algorithm '{_0}' is not supported")]
        NoSuchAlgorithm(String),
        /// Occurs during deserialization of a private or public key
        #[display("Key could not be parsed. {_0}")]
        Parse(ParseError),
        /// Returned when an error occurs during the signing process
        #[display("Signing failed. {_0}")]
        Signing(String),
        /// Returned when an error occurs during the signature verification process
        #[display("Signature verification failed")]
        BadSignature,
        /// Returned when an error occurs during key generation
        #[display("Key generation failed. {_0}")]
        KeyGen(String),
        /// A General purpose error message that doesn't fit in any category
        #[display("General error. {_0}")] // This is going to cause a headache
        Other(String),
    }

    impl From<NoSuchAlgorithm> for Error {
        fn from(source: NoSuchAlgorithm) -> Self {
            Self::NoSuchAlgorithm(source.to_string())
        }
    }

    #[cfg(all(feature = "ffi_export", not(feature = "ffi_import")))]
    impl iroha_ffi::IntoFfiReturn for Error {
        fn into_ffi_return(self) -> iroha_ffi::FfiReturn {
            let _ = self;
            iroha_ffi::FfiReturn::ExecutionFail
        }
    }

    impl From<ParseError> for Error {
        fn from(source: ParseError) -> Self {
            Self::Parse(source)
        }
    }

    impl std::error::Error for Error {}
}

mod ffi {
    //! Definitions and implementations of FFI related functionalities

    #[cfg(any(feature = "ffi_export", feature = "ffi_import"))]
    use super::*;

    macro_rules! ffi_item {
        ($it: item $($attr: meta)?) => {
            #[cfg(all(not(feature = "ffi_export"), not(feature = "ffi_import")))]
            $it

            #[cfg(all(feature = "ffi_export", not(feature = "ffi_import")))]
            #[derive(iroha_ffi::FfiType)]
            #[iroha_ffi::ffi_export]
            $(#[$attr])?
            $it

            #[cfg(feature = "ffi_import")]
            iroha_ffi::ffi! {
                #[iroha_ffi::ffi_import]
                $(#[$attr])?
                $it
            }
        };
    }

    #[cfg(any(feature = "ffi_export", feature = "ffi_import"))]
    iroha_ffi::handles! {
        PublicKey,
        PrivateKey,
        KeyPair,
        Signature,
    }

    #[cfg(feature = "ffi_import")]
    iroha_ffi::decl_ffi_fns! { link_prefix="iroha_crypto" Drop, Clone, Eq, Ord, Default }
    #[cfg(all(feature = "ffi_export", not(feature = "ffi_import")))]
    iroha_ffi::def_ffi_fns! { link_prefix="iroha_crypto"
        Drop: { PublicKey, PrivateKey, KeyPair, Signature },
        Clone: { PublicKey, PrivateKey, KeyPair, Signature },
        Eq: { PublicKey, PrivateKey, KeyPair, Signature },
        Ord: { PublicKey, Signature },
    }

    // NOTE: Makes sure that only one `dealloc` is exported per generated dynamic library
    #[cfg(all(feature = "ffi_export", not(feature = "ffi_import")))]
    mod dylib {
        iroha_ffi::def_ffi_fns! {dealloc}
    }

    pub(crate) use ffi_item;
}

#[cfg(test)]
mod tests {
    use norito::codec::{Decode, Encode};

    use super::*;

    fn supported_algorithms() -> Vec<Algorithm> {
        let base = [Algorithm::Ed25519, Algorithm::Secp256k1];
        #[cfg(feature = "gost")]
        let gost_algorithms = [
            Algorithm::Gost3410_2012_256ParamSetA,
            Algorithm::Gost3410_2012_256ParamSetB,
            Algorithm::Gost3410_2012_256ParamSetC,
            Algorithm::Gost3410_2012_512ParamSetA,
            Algorithm::Gost3410_2012_512ParamSetB,
        ];
        #[cfg(not(feature = "gost"))]
        let gost_algorithms: [Algorithm; 0] = [];

        let ml_dsa_algorithms = [Algorithm::MlDsa];

        #[cfg(feature = "bls")]
        let bls_algorithms = [Algorithm::BlsNormal, Algorithm::BlsSmall];
        #[cfg(not(feature = "bls"))]
        let bls_algorithms: [Algorithm; 0] = [];

        #[cfg(feature = "sm")]
        let sm_algorithms = [Algorithm::Sm2];
        #[cfg(not(feature = "sm"))]
        let sm_algorithms: [Algorithm; 0] = [];

        base.iter()
            .chain(gost_algorithms.iter())
            .chain(ml_dsa_algorithms.iter())
            .chain(bls_algorithms.iter())
            .chain(sm_algorithms.iter())
            .copied()
            .collect()
    }

    #[test]
    fn algorithm_serialize_deserialize_consistent() {
        for algorithm in supported_algorithms() {
            let ser = norito::json::to_json(&algorithm)
                .unwrap_or_else(|_| panic!("Failed to serialize algorithm {:?}", &algorithm));
            let de: Algorithm = norito::json::from_str(&ser)
                .unwrap_or_else(|_| panic!("Failed to deserialize algorithm {:?}", &algorithm));
            assert_eq!(algorithm, de);
        }
    }

    #[cfg(feature = "bls")]
    #[test]
    fn bls_normal_aggregate_fast_accepts_valid_and_rejects_bad() {
        let msg = b"bls-normal-fast";
        let kp1 = KeyPair::from_seed(vec![1; 32], Algorithm::BlsNormal);
        let kp2 = KeyPair::from_seed(vec![2; 32], Algorithm::BlsNormal);
        let (pk1, sk1) = kp1.into_parts();
        let (pk2, sk2) = kp2.into_parts();
        let sig1 = Signature::new(&sk1, msg);
        let sig2 = Signature::new(&sk2, msg);
        let signatures: Vec<&[u8]> = vec![sig1.payload(), sig2.payload()];
        let public_keys: Vec<&PublicKey> = vec![&pk1, &pk2];
        let pops = vec![
            bls_normal_pop_prove(&sk1).expect("pop"),
            bls_normal_pop_prove(&sk2).expect("pop"),
        ];
        let pop_refs: Vec<&[u8]> = pops.iter().map(|pop| pop.as_slice()).collect();

        bls_normal_verify_aggregate_same_message_fast(msg, &signatures, &public_keys, &pop_refs)
            .expect("aggregate fast ok");

        let mut bad_sig = sig1.payload().to_vec();
        bad_sig[0] ^= 0x01;
        let bad_signatures: Vec<&[u8]> = vec![bad_sig.as_slice(), sig2.payload()];
        assert!(
            bls_normal_verify_aggregate_same_message_fast(
                msg,
                &bad_signatures,
                &public_keys,
                &pop_refs,
            )
            .is_err(),
            "bad signature must be rejected"
        );
    }

    #[cfg(feature = "bls")]
    #[test]
    fn bls_small_aggregate_fast_accepts_valid_and_rejects_bad() {
        let msg = b"bls-small-fast";
        let kp1 = KeyPair::from_seed(vec![3; 32], Algorithm::BlsSmall);
        let kp2 = KeyPair::from_seed(vec![4; 32], Algorithm::BlsSmall);
        let (pk1, sk1) = kp1.into_parts();
        let (pk2, sk2) = kp2.into_parts();
        let sig1 = Signature::new(&sk1, msg);
        let sig2 = Signature::new(&sk2, msg);
        let signatures: Vec<&[u8]> = vec![sig1.payload(), sig2.payload()];
        let public_keys: Vec<&PublicKey> = vec![&pk1, &pk2];
        let pops = vec![
            bls_small_pop_prove(&sk1).expect("pop"),
            bls_small_pop_prove(&sk2).expect("pop"),
        ];
        let pop_refs: Vec<&[u8]> = pops.iter().map(|pop| pop.as_slice()).collect();

        bls_small_verify_aggregate_same_message_fast(msg, &signatures, &public_keys, &pop_refs)
            .expect("aggregate fast ok");

        let mut bad_sig = sig2.payload().to_vec();
        bad_sig[0] ^= 0x01;
        let bad_signatures: Vec<&[u8]> = vec![sig1.payload(), bad_sig.as_slice()];
        assert!(
            bls_small_verify_aggregate_same_message_fast(
                msg,
                &bad_signatures,
                &public_keys,
                &pop_refs,
            )
            .is_err(),
            "bad signature must be rejected"
        );
    }

    #[test]
    fn no_such_algorithm_error_displays_identifier() {
        let missing = "rot13-ed25519";
        let err = Error::NoSuchAlgorithm(missing.to_string());

        let rendered = err.to_string();
        assert!(
            rendered.contains(missing),
            "Error Display output `{rendered}` should mention `{missing}`"
        );
        assert!(
            rendered.starts_with("Algorithm"),
            "Display output should remain human readable"
        );
    }

    #[test]
    fn ml_dsa_secret_key_clone_shares_inner_arc() {
        use pqcrypto_traits::sign::SecretKey as _;

        use crate::mldsa_seed::dilithium3 as seeded;

        let (_, private) =
            seeded::keypair_from_seed(b"iroha:ml-dsa:strong-count").expect("seeded ML-DSA keypair");
        let raw_secret =
            pqcrypto_dilithium::dilithium3::SecretKey::from_bytes(&private.to_bytes().1)
                .expect("valid ML-DSA secret bytes");
        let key = MlDsaSecretKey::new(&raw_secret);
        assert_eq!(key.strong_count(), 1, "initial strong count must be 1");

        let cloned = key.clone();
        assert_eq!(key.strong_count(), 2, "cloning increments strong count");

        let message = b"iroha:ml-dsa:test-arc-sharing";
        let sig_original = key.sign(message);
        let sig_clone = cloned.sign(message);
        assert_eq!(sig_original, sig_clone, "cloned key must sign identically");

        drop(cloned);
        assert_eq!(key.strong_count(), 1, "dropping clone decrements count");
    }

    #[test]
    fn ml_dsa_public_key_parse_rejects_invalid_length() {
        use pqcrypto_dilithium::dilithium3 as dilithium;
        use pqcrypto_traits::sign::PublicKey as _;

        let (pk, _) = dilithium::keypair();
        let parsed = PublicKey::from_bytes(Algorithm::MlDsa, pk.as_bytes());
        assert!(parsed.is_ok(), "expected valid ML-DSA public key bytes");

        let mut bad = pk.as_bytes().to_vec();
        bad.push(0x00);
        let err = PublicKey::from_bytes(Algorithm::MlDsa, &bad).unwrap_err();
        assert!(
            err.0.contains("invalid ML-DSA public key length"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    #[cfg(feature = "rand")]
    fn key_pair_serialize_deserialize_consistent() {
        struct ExposedKeyPair {
            public_key: PublicKey,
            private_key: ExposedPrivateKey,
        }

        impl norito::json::JsonSerialize for ExposedKeyPair {
            fn json_serialize(&self, out: &mut String) {
                use norito::json::{self, Map, Value};
                let mut map = Map::new();
                map.insert(
                    "public_key".into(),
                    json::to_value(&self.public_key).expect("serialize public key"),
                );
                map.insert(
                    "private_key".into(),
                    json::to_value(&self.private_key).expect("serialize private key"),
                );
                norito::json::JsonSerialize::json_serialize(&Value::Object(map), out);
            }
        }

        for algorithm in supported_algorithms() {
            let key_pair = KeyPair::random_with_algorithm(algorithm);
            let exposed_key_pair = ExposedKeyPair {
                public_key: key_pair.public_key.clone(),
                private_key: ExposedPrivateKey(key_pair.private_key.clone()),
            };

            let ser = norito::json::to_json(&exposed_key_pair)
                .unwrap_or_else(|_| panic!("Failed to serialize key pair {:?}", &key_pair));
            let de: KeyPair = norito::json::from_str(&ser)
                .unwrap_or_else(|_| panic!("Failed to deserialize key pair {:?}", &key_pair));
            assert_eq!(key_pair, de);
        }
    }

    #[test]
    #[cfg(feature = "bls")]
    fn bls_pop_prove_and_verify_roundtrip() {
        // Generate a BLS-normal key pair
        let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        // Prove possession
        let pop = bls_normal_pop_prove(kp.private_key()).expect("pop prove");
        // Verify
        bls_normal_pop_verify(kp.public_key(), &pop).expect("pop verify");
        // Negative: wrong key should fail
        let other = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        assert!(bls_normal_pop_verify(other.public_key(), &pop).is_err());
    }

    #[test]
    #[cfg(feature = "bls")]
    fn bls_pop_rejects_unhashed_message() {
        use crate::secrecy::ExposeSecret;

        // PoP signed over POP_DST || pk (unhashed) must be rejected.
        let kp = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let (_alg, pk_bytes) = kp.public_key().to_bytes();
        let mut unhashed_msg = Vec::with_capacity(POP_DST.len() + pk_bytes.len());
        unhashed_msg.extend_from_slice(POP_DST.as_bytes());
        unhashed_msg.extend_from_slice(pk_bytes);
        let pop = signature::bls::BlsNormal::sign(
            &unhashed_msg,
            match kp.private_key().0.expose_secret() {
                PrivateKeyInner::BlsNormal(sk) => sk,
                _ => unreachable!(),
            },
        );
        assert!(bls_normal_pop_verify(kp.public_key(), &pop).is_err());
    }

    #[test]
    #[cfg(feature = "bls")]
    fn bls_small_pop_roundtrip() {
        let kp = KeyPair::random_with_algorithm(Algorithm::BlsSmall);
        let pop = bls_small_pop_prove(kp.private_key()).expect("small pop prove");
        bls_small_pop_verify(kp.public_key(), &pop).expect("small pop verify");
        let other = KeyPair::random_with_algorithm(Algorithm::BlsSmall);
        assert!(bls_small_pop_verify(other.public_key(), &pop).is_err());
    }

    #[test]
    fn private_key_format_or_serialize_redacted() {
        let key_pair = KeyPair::random();
        let (_, private_key) = key_pair.into_parts();

        assert_eq!(
            norito::json::to_json(&private_key).expect("Couldn't serialize key"),
            format!("\"{PRIVATE_KEY_REDACTED}\"")
        );
        assert_eq!(format!("{}", &private_key), PRIVATE_KEY_REDACTED);
    }

    #[test]
    fn encode_decode_algorithm_consistent() {
        for algorithm in supported_algorithms() {
            let encoded_algorithm = algorithm.encode();

            let decoded_algorithm =
                Algorithm::decode(&mut encoded_algorithm.as_slice()).expect("Failed to decode");
            assert_eq!(
                algorithm, decoded_algorithm,
                "Failed to decode encoded {:?}",
                &algorithm
            );
        }
    }

    #[test]
    fn key_pair_match() {
        KeyPair::new(
            "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
                .parse()
                .expect("Public key not in mulithash format"),
            "80262093CA389FC2979F3F7D2A7F8B76C70DE6D5EAF5FA58D4F93CB8B0FB298D398ACC"
                .parse()
                .expect("Private key not in mulithash format"),
        )
        .unwrap();

        #[cfg(feature = "bls")]
        {
            KeyPair::new("ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
                .parse()
                .expect("Public key not in mulithash format"),
                "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F"
                .parse()
                .expect("Private key not in mulithash format")
            ).unwrap();
        }

        #[cfg(feature = "gost")]
        {
            for algorithm in [
                Algorithm::Gost3410_2012_256ParamSetA,
                Algorithm::Gost3410_2012_256ParamSetB,
                Algorithm::Gost3410_2012_256ParamSetC,
                Algorithm::Gost3410_2012_512ParamSetA,
                Algorithm::Gost3410_2012_512ParamSetB,
            ] {
                let key_pair = KeyPair::random_with_algorithm(algorithm);
                let public: PublicKey = key_pair
                    .public_key()
                    .to_string()
                    .parse()
                    .expect("public multihash should parse");
                let private: PrivateKey = ExposedPrivateKey(key_pair.private_key().clone())
                    .to_string()
                    .parse()
                    .expect("private multihash should parse");
                KeyPair::new(public, private).expect("GOST multihash roundtrip");
            }
        }
    }

    #[test]
    #[cfg(all(feature = "bls", not(feature = "bls-multi-pairing"), feature = "rand"))]
    fn bls_normal_multi_message_duplicates_fallbacks_to_per_sig() {
        let (pk1, sk1) = signature::bls::BlsNormal::keypair(super::KeyGenOption::Random);
        let (pk2, sk2) = signature::bls::BlsNormal::keypair(super::KeyGenOption::Random);
        let message = b"duplicate-multi-msg".to_vec();
        let sig1 = signature::bls::BlsNormal::sign(&message, &sk1);
        let sig2 = signature::bls::BlsNormal::sign(&message, &sk2);
        let pk1_bytes = pk1.to_bytes();
        let pk2_bytes = pk2.to_bytes();

        let msgs: Vec<&[u8]> = vec![message.as_slice(), message.as_slice()];
        let signature_refs: Vec<&[u8]> = vec![sig1.as_slice(), sig2.as_slice()];
        let pk_refs: Vec<&[u8]> = vec![pk1_bytes.as_slice(), pk2_bytes.as_slice()];

        super::signature::bls::verify_aggregate_multi_message_normal(
            &msgs,
            &signature_refs,
            &pk_refs,
        )
        .expect("aggregate verifier should accept duplicate messages");

        super::bls_normal_verify_aggregate_multi_message(&msgs, &signature_refs, &pk_refs)
            .expect("wrapper should accept duplicate messages");
    }

    #[test]
    #[cfg(feature = "rand")]
    fn encode_decode_public_key_consistent() {
        for algorithm in supported_algorithms() {
            let key_pair = KeyPair::random_with_algorithm(algorithm);
            let (public_key, _) = key_pair.into_parts();

            let encoded_public_key = public_key.encode();

            let decoded_public_key =
                PublicKey::decode(&mut encoded_public_key.as_slice()).expect("Failed to decode");
            assert_eq!(
                public_key, decoded_public_key,
                "Failed to decode encoded Public Key{:?}",
                &public_key
            );
        }
    }

    #[test]
    fn public_key_norito_roundtrip() {
        let pk: PublicKey =
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
                .parse()
                .expect("public key");

        let mut buf = Vec::new();
        norito::core::NoritoSerialize::serialize(&pk, &mut buf).expect("serialize public key");
        let (decoded, used) = <PublicKey as norito::core::DecodeFromSlice>::decode_from_slice(&buf)
            .expect("decode public key slice");
        assert_eq!(used, buf.len());
        assert_eq!(decoded, pk);

        let codec_bytes = pk.encode();
        let mut cursor = codec_bytes.as_slice();
        let codec_decoded =
            <PublicKey as Decode>::decode(&mut cursor).expect("codec decode public key");
        assert_eq!(codec_decoded, pk);
    }

    #[cfg(feature = "sm")]
    #[test]
    fn sm2_public_key_multihash_and_prefixed_roundtrip() {
        let private = Sm2PrivateKey::new(Sm2PublicKey::DEFAULT_DISTID, [0x42; 32])
            .expect("construct SM2 private key");
        let sm2_pk = private.public_key();
        let sec1 = sm2_pk.to_sec1_bytes(false);
        let payload = sm::encode_sm2_public_key_payload(sm2_pk.distid(), &sec1).expect("payload");
        let pk = PublicKey::from_bytes(Algorithm::Sm2, &payload).expect("construct SM2 key");

        let canonical = pk.to_string();
        let payload_hex = hex::encode_upper(&payload);
        assert!(
            canonical.starts_with("8626"),
            "SM2 multihash should start with algorithm prefix"
        );
        assert!(
            canonical.ends_with(&payload_hex),
            "SM2 multihash should embed payload bytes"
        );

        let prefixed = pk.to_prefixed_string();
        assert_eq!(prefixed, format!("sm2:{canonical}"));

        let parsed_prefixed: PublicKey = prefixed.parse().expect("parse prefixed key");
        assert_eq!(parsed_prefixed, pk);

        let parsed: PublicKey = canonical.parse().expect("parse bare multihash");
        assert_eq!(parsed, pk);
    }

    #[test]
    #[cfg(not(feature = "ffi_import"))]
    fn public_key_compact_roundtrip_via_canonical_decode() {
        let pk: PublicKey =
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
                .parse()
                .expect("public key");
        let compact = pk.0.clone();
        let mut payload = Vec::new();
        <PublicKeyCompact as norito::core::NoritoSerialize>::serialize(&compact, &mut payload)
            .expect("serialize compact");

        let (decoded, used) = norito::core::decode_field_canonical::<PublicKeyCompact>(&payload)
            .expect("decode compact");
        assert!(used <= payload.len());
        let from_decoded = PublicKey(decoded.clone());
        let from_original = PublicKey(compact.clone());
        assert_eq!(from_decoded, from_original);
    }

    #[test]
    #[cfg(not(feature = "ffi_import"))]
    fn public_key_compact_try_deserialize_rejects_invalid_payload() {
        let compact = PublicKeyCompact::new(Algorithm::Ed25519, &[]);
        let framed = norito::core::to_bytes(&compact).expect("encode compact");
        let archived = norito::from_bytes::<PublicKeyCompact>(&framed).expect("archive");
        let err = <PublicKeyCompact as norito::core::NoritoDeserialize>::try_deserialize(archived)
            .expect_err("invalid compact payload");
        assert!(matches!(err, norito::core::Error::Message(_)));
    }

    #[test]
    #[cfg(not(feature = "ffi_import"))]
    fn public_key_compact_decode_from_slice_rejects_invalid_payload() {
        let compact = PublicKeyCompact::new(Algorithm::Ed25519, &[]);
        let mut payload = Vec::new();
        <PublicKeyCompact as norito::core::NoritoSerialize>::serialize(&compact, &mut payload)
            .expect("serialize compact");
        let err = <PublicKeyCompact as norito::core::DecodeFromSlice>::decode_from_slice(&payload)
            .expect_err("invalid compact payload");
        assert!(matches!(err, norito::core::Error::Message(_)));
    }

    #[test]
    fn public_key_norito_golden_archive() {
        let pk: PublicKey =
            "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245"
                .parse()
                .expect("public key");
        let framed = norito::core::to_bytes(&pk).expect("encode public key");
        let actual_hex = hex::encode(&framed);
        let expected_hex = "4e5254300000308ea40f1c2e0d24308ea40f1c2e0d24004e00000000000000b86570116ac45e8f00460000000000000065643031323045444636443742353243373033324430334145433639364632303638424435333130313532384633433742363038314246463035413136363244374643323435";
        assert_eq!(
            actual_hex, expected_hex,
            "public key Norito archive changed"
        );
    }

    #[test]
    #[cfg(not(feature = "ffi_import"))]
    fn public_key_try_deserialize_rejects_invalid_payload() {
        let bogus = String::from("not-a-public-key");
        let (payload, flags) = norito::codec::encode_with_header_flags(&bogus);
        let framed = norito::core::frame_bare_with_header_flags::<PublicKey>(&payload, flags)
            .expect("frame");
        let archived = norito::from_bytes::<PublicKey>(&framed).expect("archive");
        let err = <PublicKey as norito::core::NoritoDeserialize>::try_deserialize(archived)
            .expect_err("invalid key");
        assert!(matches!(err, norito::core::Error::Message(_)));
    }

    #[test]
    #[cfg(feature = "rand")]
    fn public_key_from_bytes_roundtrip() {
        let key_pair = KeyPair::random();
        let (public_key, _) = key_pair.into_parts();
        let (alg, bytes) = public_key.to_bytes();
        let reconstructed = PublicKey::from_bytes(alg, bytes).expect("Should decode");
        assert_eq!(public_key, reconstructed);
    }

    #[test]
    fn invalid_private_key() {
        assert!(PrivateKey::from_hex(
            Algorithm::Ed25519,
            "0000000000000000000000000000000049BF70187154C57B97AF913163E8E875733B4EAF1F3F0689B31CE392129493E9"
        ).is_err());

        #[cfg(feature = "bls")]
        assert!(
            PrivateKey::from_hex(
                Algorithm::BlsNormal,
                "93CA389FC2979F3F7D2A7F8B76C70DE6D5EAF5FA58D4F93CB8B0FB298D398ACC59C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
            ).is_err());
    }

    #[test]
    fn key_pair_mismatch() {
        KeyPair::new(
            "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
                .parse()
                .expect("Public key not in mulithash format"),
            "8026203A7991AF1ABB77F3FD27CC148404A6AE4439D095A63591B77C788D53F708A02A"
                .parse()
                .expect("Public key not in mulithash format"),
        )
        .unwrap_err();

        #[cfg(feature = "bls")]
        {
            KeyPair::new("ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
                .parse()
                .expect("Public key not in mulithash format"),
                "892620CC176E44C41AA144FD1BEE4E0BCD2EF43F06D0C7BC2988E89A799951D240E503"
                .parse()
                .expect("Private key not in mulithash format"),
                ).unwrap_err();
        }
    }

    #[test]
    #[cfg(not(feature = "ffi_import"))]
    fn display_public_key() {
        assert_eq!(
            format!(
                "{}",
                PublicKey::from_hex(
                    Algorithm::Ed25519,
                    "1509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
                )
                .unwrap()
            ),
            "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        );
        assert_eq!(
            format!(
                "{}",
                PublicKey::from_hex(
                    Algorithm::Secp256k1,
                    "0312273E8810581E58948D3FB8F9E8AD53AAA21492EBB8703915BBB565A21B7FCC"
                )
                .unwrap()
            ),
            "e701210312273E8810581E58948D3FB8F9E8AD53AAA21492EBB8703915BBB565A21B7FCC"
        );
        #[cfg(feature = "bls")]
        {
            assert_eq!(
                format!(
                    "{}",
                    PublicKey::from_hex(
                        Algorithm::BlsNormal,
                        "9060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2",
                    ).unwrap()
                ),
                "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2",
            );
            assert_eq!(
                format!(
                    "{}",
                    PublicKey::from_hex(
                        Algorithm::BlsSmall,
                        "9051D4A9C69402423413EBBA4C00BC82A0102AA2B783057BD7BCEE4DD17B37DE5D719EE84BE43783F2AE47A673A74B8315DD3E595ED1FBDFAC17DA1D7A36F642B423ED18275FAFD671B1D331439D22F12FB6EB436A47E8656F182A78DF29D310",
                    ).unwrap()
                ),
                "eb01609051D4A9C69402423413EBBA4C00BC82A0102AA2B783057BD7BCEE4DD17B37DE5D719EE84BE43783F2AE47A673A74B8315DD3E595ED1FBDFAC17DA1D7A36F642B423ED18275FAFD671B1D331439D22F12FB6EB436A47E8656F182A78DF29D310",
            );
        }
    }
    #[cfg(not(feature = "ffi_import"))]
    #[derive(Debug, PartialEq)]
    struct TestJson {
        public_key: PublicKey,
        private_key: ExposedPrivateKey,
    }

    #[cfg(not(feature = "ffi_import"))]
    impl norito::json::JsonSerialize for TestJson {
        fn json_serialize(&self, out: &mut String) {
            use norito::json::{self, Map, Value};
            let mut map = Map::new();
            map.insert(
                "public_key".into(),
                json::to_value(&self.public_key).expect("serialize public key"),
            );
            map.insert(
                "private_key".into(),
                json::to_value(&self.private_key).expect("serialize private key"),
            );
            norito::json::JsonSerialize::json_serialize(&Value::Object(map), out);
        }
    }

    #[cfg(not(feature = "ffi_import"))]
    impl norito::json::JsonDeserialize for TestJson {
        fn json_deserialize(
            parser: &mut norito::json::Parser<'_>,
        ) -> Result<Self, norito::json::Error> {
            let mut map = norito::json::MapVisitor::new(parser)?;
            let mut public_key = None;
            let mut private_key = None;

            while let Some(key) = map.next_key()? {
                match key.as_str() {
                    "public_key" => {
                        if public_key.is_some() {
                            return Err(norito::json::MapVisitor::duplicate_field("public_key"));
                        }
                        public_key = Some(map.parse_value()?);
                    }
                    "private_key" => {
                        if private_key.is_some() {
                            return Err(norito::json::MapVisitor::duplicate_field("private_key"));
                        }
                        private_key = Some(map.parse_value()?);
                    }
                    _ => map.skip_value()?,
                }
            }

            map.finish()?;

            let public_key =
                public_key.ok_or_else(|| norito::json::MapVisitor::missing_field("public_key"))?;
            let private_key = private_key
                .ok_or_else(|| norito::json::MapVisitor::missing_field("private_key"))?;

            Ok(Self {
                public_key,
                private_key,
            })
        }
    }

    macro_rules! assert_test_json_serde {
        ($json:expr, $actual:expr) => {
            assert_eq!(
                norito::json::from_value::<TestJson>($json.clone()).expect("failed to deserialize"),
                $actual
            );
            assert_eq!(
                norito::json::to_value(&$actual).expect("failed to serialize"),
                $json
            );
        };
    }

    #[test]
    #[cfg(not(feature = "ffi_import"))]
    fn serde_keys_ed25519() {
        assert_test_json_serde!(
            norito::json!({
                "public_key": "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4",
                "private_key": "8026203A7991AF1ABB77F3FD27CC148404A6AE4439D095A63591B77C788D53F708A02A"
            }),
            TestJson {
                public_key: PublicKey::from_hex(
                    Algorithm::Ed25519,
                    "1509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
                )
                .unwrap(),
                private_key: ExposedPrivateKey(
                    PrivateKey::from_hex(
                        Algorithm::Ed25519,
                        "3a7991af1abb77f3fd27cc148404a6ae4439d095a63591b77c788d53f708a02a",
                    )
                    .unwrap()
                )
            }
        );
    }

    #[test]
    #[cfg(not(feature = "ffi_import"))]
    fn serde_keys_secp256k1() {
        assert_test_json_serde!(
            norito::json!({
                "public_key": "e701210312273E8810581E58948D3FB8F9E8AD53AAA21492EBB8703915BBB565A21B7FCC",
                "private_key": "8126204DF4FCA10762D4B529FE40A2188A60CA4469D2C50A825B5F33ADC2CB78C69445"
            }),
            TestJson {
                public_key: PublicKey::from_hex(
                    Algorithm::Secp256k1,
                    "0312273E8810581E58948D3FB8F9E8AD53AAA21492EBB8703915BBB565A21B7FCC"
                )
                .unwrap(),
                private_key: ExposedPrivateKey(
                    PrivateKey::from_hex(
                        Algorithm::Secp256k1,
                        "4DF4FCA10762D4B529FE40A2188A60CA4469D2C50A825B5F33ADC2CB78C69445",
                    )
                    .unwrap()
                )
            }
        );
    }

    #[test]
    #[cfg(not(feature = "ffi_import"))]
    fn fromstr_accepts_algo_prefixed_public_key() {
        // Ed25519 example from existing tests
        let mh_hex = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4";
        let prefixed = format!("ed25519:{mh_hex}");
        let pk1: PublicKey = mh_hex.parse().expect("bare multihash parses");
        let pk2: PublicKey = prefixed.parse().expect("prefixed multihash parses");
        assert_eq!(pk1, pk2);
    }

    #[test]
    #[cfg(not(feature = "ffi_import"))]
    fn fromstr_accepts_algo_prefixed_private_key() {
        // Ed25519 example from existing tests
        let mh_hex = "8026203A7991AF1ABB77F3FD27CC148404A6AE4439D095A63591B77C788D53F708A02A";
        let prefixed = format!("ed25519:{mh_hex}");
        let sk1: PrivateKey = mh_hex.parse().expect("bare multihash parses");
        let sk2: PrivateKey = prefixed.parse().expect("prefixed multihash parses");
        assert_eq!(sk1.to_bytes().1, sk2.to_bytes().1);
        assert_eq!(sk1.algorithm(), sk2.algorithm());
    }

    #[test]
    #[cfg(not(feature = "ffi_import"))]
    fn prefixed_mismatch_is_error() {
        // Public key: try wrong prefix
        let mh_hex = "ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4";
        let wrong = format!("secp256k1:{mh_hex}");
        assert!(wrong.parse::<PublicKey>().is_err());
        // Private key: wrong prefix
        let mh_hex_sk = "8126204DF4FCA10762D4B529FE40A2188A60CA4469D2C50A825B5F33ADC2CB78C69445";
        let wrong_sk = format!("ed25519:{mh_hex_sk}");
        // This private key multihash above is secp256k1 in serde_keys_secp256k1 test
        assert!(wrong_sk.parse::<PrivateKey>().is_err());
    }
    #[test]
    #[cfg(all(feature = "bls", not(feature = "ffi_import")))]
    fn serde_keys_bls() {
        assert_test_json_serde!(
            norito::json!({
                "public_key": "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2",
                "private_key": "8926201CA347641228C3B79AA43839DEDC85FA51C0E8B9B6A00F6B0D6B0423E902973F"
            }),
            TestJson {
                public_key: PublicKey::from_hex(
                    Algorithm::BlsNormal,
                    "9060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2",
                ).unwrap(),
                private_key: ExposedPrivateKey(PrivateKey::from_hex(
                    Algorithm::BlsNormal,
                    "1ca347641228c3b79aa43839dedc85fa51c0e8b9b6a00f6b0d6b0423e902973f",
                ).unwrap())
            }
        );
        assert_test_json_serde!(
            norito::json!({
                "public_key": "eb01609051D4A9C69402423413EBBA4C00BC82A0102AA2B783057BD7BCEE4DD17B37DE5D719EE84BE43783F2AE47A673A74B8315DD3E595ED1FBDFAC17DA1D7A36F642B423ED18275FAFD671B1D331439D22F12FB6EB436A47E8656F182A78DF29D310",
                "private_key": "8a26208CB95072914CDD8E4CF682FDBE1189CDF4FC54D445E760B3446F896DBDBF5B2B"
            }),
            TestJson {
                public_key: PublicKey::from_hex(
                    Algorithm::BlsSmall,
                    "9051D4A9C69402423413EBBA4C00BC82A0102AA2B783057BD7BCEE4DD17B37DE5D719EE84BE43783F2AE47A673A74B8315DD3E595ED1FBDFAC17DA1D7A36F642B423ED18275FAFD671B1D331439D22F12FB6EB436A47E8656F182A78DF29D310",
                ).unwrap(),
                private_key: ExposedPrivateKey(PrivateKey::from_hex(
                    Algorithm::BlsSmall,
                    "8cb95072914cdd8e4cf682fdbe1189cdf4fc54d445e760b3446f896dbdbf5b2b",
                ).unwrap())
            }
        );
    }
}
