//! This module contains structures and implementations related to the cryptographic parts of the Iroha.
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(not(feature = "std"))]
extern crate alloc;

#[cfg(not(feature = "ffi_import"))]
pub mod encryption;
mod hash;
#[cfg(not(feature = "ffi_import"))]
pub mod kex;
mod merkle;
#[cfg(not(feature = "ffi_import"))]
mod multihash;
mod signature;
#[cfg(not(feature = "ffi_import"))]
mod varint;

#[cfg(not(feature = "std"))]
use alloc::{
    borrow::ToOwned as _,
    boxed::Box,
    format,
    string::{String, ToString as _},
    vec,
    vec::Vec,
};
use core::{borrow::Borrow, fmt, str::FromStr};

#[cfg(feature = "base64")]
pub use base64;
#[cfg(not(feature = "ffi_import"))]
pub use blake2;
use derive_more::Display;
use error::{Error, NoSuchAlgorithm, ParseError};
use getset::Getters;
pub use hash::*;
use iroha_macro::ffi_impl_opaque;
use iroha_primitives::const_vec::ConstVec;
use iroha_schema::{Declaration, IntoSchema, MetaMap, Metadata, NamedFieldsMeta, TypeId};
pub use merkle::MerkleTree;
#[cfg(not(feature = "ffi_import"))]
use parity_scale_codec::{Decode, Encode};
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};
use w3f_bls::SerializableToBytes;

pub use self::signature::*;

// Hiding constants is a bad idea. For one, you're forcing the user to
// create temporaries, but also you're not actually hiding any
// information that can be used in malicious ways. If you want to hide
// these, I'd prefer inlining them instead.

/// String algorithm representation
pub const ED_25519: &str = "ed25519";
/// String algorithm representation
pub const SECP_256_K1: &str = "secp256k1";
/// String algorithm representation
pub const BLS_NORMAL: &str = "bls_normal";
/// String algorithm representation
pub const BLS_SMALL: &str = "bls_small";

ffi::ffi_item! {
    /// Algorithm for hashing & signing
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, DeserializeFromStr, SerializeDisplay, Decode, Encode, IntoSchema)]
    #[repr(u8)]
    pub enum Algorithm {
        #[default]
        #[allow(missing_docs)]
        Ed25519,
        #[allow(missing_docs)]
        Secp256k1,
        #[allow(missing_docs)]
        BlsNormal,
        #[allow(missing_docs)]
        BlsSmall,
    }
}

impl Algorithm {
    /// Maps the algorithm to its static string representation
    pub const fn as_static_str(self) -> &'static str {
        match self {
            Self::Ed25519 => ED_25519,
            Self::Secp256k1 => SECP_256_K1,
            Self::BlsNormal => BLS_NORMAL,
            Self::BlsSmall => BLS_SMALL,
        }
    }
}

impl fmt::Display for Algorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_static_str())
    }
}

impl FromStr for Algorithm {
    type Err = NoSuchAlgorithm;

    fn from_str(algorithm: &str) -> Result<Self, Self::Err> {
        match algorithm {
            ED_25519 => Ok(Algorithm::Ed25519),
            SECP_256_K1 => Ok(Algorithm::Secp256k1),
            BLS_NORMAL => Ok(Algorithm::BlsNormal),
            BLS_SMALL => Ok(Algorithm::BlsSmall),
            _ => Err(NoSuchAlgorithm),
        }
    }
}

/// Options for key generation
#[cfg(not(feature = "ffi_import"))]
#[derive(Debug, Clone)]
pub enum KeyGenOption<K = PrivateKey> {
    /// Use random number generator
    #[cfg(feature = "rand")]
    Random,
    /// Use seed
    UseSeed(Vec<u8>),
    /// Derive from private key
    FromPrivateKey(K),
}

ffi::ffi_item! {
    /// Configuration of key generation
    #[derive(Clone)]
    #[cfg_attr(not(feature="ffi_import"), derive(Debug))]
    pub struct KeyGenConfiguration {
        /// Options
        key_gen_option: KeyGenOption,
        /// Algorithm
        algorithm: Algorithm,
    }
}

#[ffi_impl_opaque]
impl KeyGenConfiguration {
    /// Construct using random number generation with [`Ed25519`](Algorithm::Ed25519) algorithm
    #[cfg(feature = "rand")]
    #[must_use]
    pub fn from_random() -> Self {
        Self {
            key_gen_option: KeyGenOption::Random,
            algorithm: Algorithm::default(),
        }
    }

    /// Construct using seed with [`Ed25519`](Algorithm::Ed25519) algorithm
    #[must_use]
    pub fn from_seed(seed: Vec<u8>) -> Self {
        Self {
            key_gen_option: KeyGenOption::UseSeed(seed),
            algorithm: Algorithm::default(),
        }
    }

    /// Construct using private key with [`Ed25519`](Algorithm::Ed25519) algorithm
    #[must_use]
    pub fn from_private_key(private_key: impl Into<PrivateKey>) -> Self {
        Self {
            key_gen_option: KeyGenOption::FromPrivateKey(private_key.into()),
            algorithm: Algorithm::default(),
        }
    }

    /// With algorithm
    #[must_use]
    pub fn with_algorithm(mut self, algorithm: Algorithm) -> Self {
        self.algorithm = algorithm;
        self
    }
}

ffi::ffi_item! {
    /// Pair of Public and Private keys.
    #[derive(Clone, PartialEq, Eq, Getters)]
    #[cfg_attr(not(feature="ffi_import"), derive(Debug, Serialize))]
    #[getset(get = "pub")]
    pub struct KeyPair {
        /// Public key.
        public_key: PublicKey,
        /// Private key.
        private_key: PrivateKey,
    }
}

impl KeyPair {
    /// Generates a pair of Public and Private key with [`Algorithm::default()`] selected as generation algorithm.
    #[cfg(feature = "rand")]
    pub fn generate() -> Self {
        Self::generate_with_configuration(
            KeyGenConfiguration::from_random().with_algorithm(Algorithm::Ed25519),
        )
    }
}

#[ffi_impl_opaque]
impl KeyPair {
    /// Algorithm
    pub fn algorithm(&self) -> Algorithm {
        self.private_key.algorithm()
    }

    /// Construct a [`KeyPair`]
    /// # Errors
    /// If public and private key don't match, i.e. if they don't make a pair
    pub fn new(public_key: PublicKey, private_key: PrivateKey) -> Result<Self, Error> {
        let algorithm = private_key.algorithm();

        if algorithm != public_key.algorithm() {
            return Err(Error::KeyGen("Mismatch of key algorithms".to_owned()));
        }

        if PublicKey::from(private_key.clone()) != public_key {
            return Err(Error::KeyGen(String::from("Key pair mismatch")));
        }

        Ok(Self {
            public_key,
            private_key,
        })
    }

    /// Generates a pair of Public and Private key with the corresponding [`KeyGenConfiguration`].
    pub fn generate_with_configuration(
        KeyGenConfiguration {
            key_gen_option,
            algorithm,
        }: KeyGenConfiguration,
    ) -> Self {
        match algorithm {
            Algorithm::Ed25519 => signature::ed25519::Ed25519Sha512::keypair(key_gen_option).into(),
            Algorithm::Secp256k1 => {
                signature::secp256k1::EcdsaSecp256k1Sha256::keypair(key_gen_option).into()
            }
            Algorithm::BlsNormal => signature::bls::BlsNormal::keypair(key_gen_option).into(),
            Algorithm::BlsSmall => signature::bls::BlsSmall::keypair(key_gen_option).into(),
        }
    }
}

impl From<(ed25519::PublicKey, ed25519::PrivateKey)> for KeyPair {
    fn from((public_key, private_key): (ed25519::PublicKey, ed25519::PrivateKey)) -> Self {
        Self {
            public_key: PublicKey(Box::new(PublicKeyInner::Ed25519(public_key))),
            private_key: PrivateKey(Box::new(PrivateKeyInner::Ed25519(private_key))),
        }
    }
}

impl From<(secp256k1::PublicKey, secp256k1::PrivateKey)> for KeyPair {
    fn from((public_key, private_key): (secp256k1::PublicKey, secp256k1::PrivateKey)) -> Self {
        Self {
            public_key: PublicKey(Box::new(PublicKeyInner::Secp256k1(public_key))),
            private_key: PrivateKey(Box::new(PrivateKeyInner::Secp256k1(private_key))),
        }
    }
}

impl From<(bls::BlsNormalPublicKey, bls::BlsNormalPrivateKey)> for KeyPair {
    fn from(
        (public_key, private_key): (bls::BlsNormalPublicKey, bls::BlsNormalPrivateKey),
    ) -> Self {
        Self {
            public_key: PublicKey(Box::new(PublicKeyInner::BlsNormal(public_key))),
            private_key: PrivateKey(Box::new(PrivateKeyInner::BlsNormal(private_key))),
        }
    }
}

impl From<(bls::BlsSmallPublicKey, bls::BlsSmallPrivateKey)> for KeyPair {
    fn from((public_key, private_key): (bls::BlsSmallPublicKey, bls::BlsSmallPrivateKey)) -> Self {
        Self {
            public_key: PublicKey(Box::new(PublicKeyInner::BlsSmall(public_key))),
            private_key: PrivateKey(Box::new(PrivateKeyInner::BlsSmall(private_key))),
        }
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<'de> Deserialize<'de> for KeyPair {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;

        #[derive(Deserialize)]
        struct KeyPairCandidate {
            public_key: PublicKey,
            private_key: PrivateKey,
        }

        // NOTE: Verify that key pair is valid
        let key_pair = KeyPairCandidate::deserialize(deserializer)?;
        Self::new(key_pair.public_key, key_pair.private_key).map_err(D::Error::custom)
    }
}

// TODO: enable in ffi_import?
#[cfg(not(feature = "ffi_import"))]
impl From<KeyPair> for (PublicKey, PrivateKey) {
    fn from(key_pair: KeyPair) -> Self {
        (key_pair.public_key, key_pair.private_key)
    }
}

#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(
    not(feature = "ffi_import"),
    derive(DeserializeFromStr, SerializeDisplay)
)]
#[allow(missing_docs, variant_size_differences)]
enum PublicKeyInner {
    Ed25519(ed25519::PublicKey),
    Secp256k1(secp256k1::PublicKey),
    BlsNormal(bls::BlsNormalPublicKey),
    BlsSmall(bls::BlsSmallPublicKey),
}

#[cfg(not(feature = "ffi_import"))]
impl fmt::Debug for PublicKeyInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple(self.algorithm().as_static_str())
            .field(&self.normalize())
            .finish()
    }
}

#[cfg(not(feature = "ffi_import"))]
impl fmt::Display for PublicKeyInner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.normalize())
    }
}

#[cfg(not(feature = "ffi_import"))]
impl FromStr for PublicKeyInner {
    type Err = ParseError;

    fn from_str(key: &str) -> Result<Self, Self::Err> {
        let bytes = hex_decode(key)?;

        multihash::Multihash::try_from(bytes).map(Into::into)
    }
}

#[cfg(not(feature = "ffi_import"))]
impl PublicKeyInner {
    fn normalize(&self) -> String {
        let multihash: &multihash::Multihash = &self.clone().into();
        let bytes = Vec::try_from(multihash).expect("Failed to convert multihash to bytes.");

        let mut bytes_iter = bytes.into_iter();
        let fn_code = hex::encode(bytes_iter.by_ref().take(2).collect::<Vec<_>>());
        let dig_size = hex::encode(bytes_iter.by_ref().take(1).collect::<Vec<_>>());
        let key = hex::encode_upper(bytes_iter.by_ref().collect::<Vec<_>>());

        format!("{fn_code}{dig_size}{key}")
    }
}

impl PublicKeyInner {
    fn to_raw(&self) -> (Algorithm, Vec<u8>) {
        (self.algorithm(), self.payload())
    }

    /// Key payload
    fn payload(&self) -> Vec<u8> {
        use w3f_bls::SerializableToBytes as _;

        match self {
            Self::Ed25519(key) => key.as_bytes().to_vec(),
            Self::Secp256k1(key) => key.to_sec1_bytes().to_vec(),
            Self::BlsNormal(key) => key.to_bytes(),
            Self::BlsSmall(key) => key.to_bytes(),
        }
    }

    fn algorithm(&self) -> Algorithm {
        match self {
            Self::Ed25519(_) => Algorithm::Ed25519,
            Self::Secp256k1(_) => Algorithm::Secp256k1,
            Self::BlsNormal(_) => Algorithm::BlsNormal,
            Self::BlsSmall(_) => Algorithm::BlsSmall,
        }
    }
}

ffi::ffi_item! {
    /// Public Key used in signatures.
    #[derive(Debug, Clone, PartialEq, Eq, TypeId)]
    #[cfg_attr(not(feature="ffi_import"), derive(Deserialize, Serialize, derive_more::Display))]
    #[cfg_attr(not(feature="ffi_import"), display(fmt = "{_0}"))]
    #[cfg_attr(all(feature = "ffi_export", not(feature = "ffi_import")), ffi_type(opaque))]
    #[allow(missing_docs)]
    pub struct PublicKey(Box<PublicKeyInner>);
}

#[ffi_impl_opaque]
impl PublicKey {
    /// Creates a new public key from raw bytes received from elsewhere
    ///
    /// # Errors
    ///
    /// Fails if public key parsing fails
    pub fn from_raw(algorithm: Algorithm, payload: &[u8]) -> Result<Self, ParseError> {
        match algorithm {
            Algorithm::Ed25519 => {
                ed25519::Ed25519Sha512::parse_public_key(payload).map(PublicKeyInner::Ed25519)
            }
            Algorithm::Secp256k1 => secp256k1::EcdsaSecp256k1Sha256::parse_public_key(payload)
                .map(PublicKeyInner::Secp256k1),
            Algorithm::BlsNormal => {
                bls::BlsNormal::parse_public_key(payload).map(PublicKeyInner::BlsNormal)
            }
            Algorithm::BlsSmall => {
                bls::BlsSmall::parse_public_key(payload).map(PublicKeyInner::BlsSmall)
            }
        }
        .map(Box::new)
        .map(PublicKey)
    }

    /// Extracts the raw bytes from public key, copying the payload.
    ///
    /// `into_raw()` without copying is not provided because underlying crypto
    /// libraries do not provide move functionality.
    pub fn to_raw(&self) -> (Algorithm, Vec<u8>) {
        self.0.to_raw()
    }

    /// Construct [`PublicKey`] from hex encoded string
    /// # Errors
    ///
    /// - If the given payload is not hex encoded
    /// - If the given payload is not a valid private key
    pub fn from_hex(digest_function: Algorithm, payload: &str) -> Result<Self, ParseError> {
        let payload = hex_decode(payload)?;

        Self::from_raw(digest_function, &payload)
    }

    /// Get the digital signature algorithm of the public key
    pub fn algorithm(&self) -> Algorithm {
        self.0.algorithm()
    }
}

#[cfg(not(feature = "ffi_import"))]
impl core::hash::Hash for PublicKey {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        (self.to_raw()).hash(state)
    }
}

impl PartialOrd for PublicKey {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PublicKey {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.to_raw().cmp(&other.to_raw())
    }
}

#[cfg(not(feature = "ffi_import"))]
impl Encode for PublicKey {
    fn size_hint(&self) -> usize {
        self.to_raw().size_hint()
    }

    fn encode_to<W: parity_scale_codec::Output + ?Sized>(&self, dest: &mut W) {
        self.to_raw().encode_to(dest);
    }
}

#[cfg(not(feature = "ffi_import"))]
impl Decode for PublicKey {
    fn decode<I: parity_scale_codec::Input>(
        input: &mut I,
    ) -> Result<Self, parity_scale_codec::Error> {
        let algorithm = Algorithm::decode(input)?;
        let payload = Vec::decode(input)?;
        Self::from_raw(algorithm, &payload).map_err(|_| {
            parity_scale_codec::Error::from(
                "Failed to construct public key from digest function and payload",
            )
        })
    }
}

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

impl FromStr for PublicKey {
    type Err = ParseError;

    fn from_str(key: &str) -> Result<Self, Self::Err> {
        PublicKeyInner::from_str(key).map(Box::new).map(Self)
    }
}

// TODO: Enable in ffi_import
#[cfg(not(feature = "ffi_import"))]
impl From<PrivateKey> for PublicKey {
    fn from(private_key: PrivateKey) -> Self {
        let algorithm = private_key.algorithm();
        let key_gen_option = KeyGenOption::FromPrivateKey(private_key);

        let inner = match algorithm {
            Algorithm::Ed25519 => {
                PublicKeyInner::Ed25519(ed25519::Ed25519Sha512::keypair(key_gen_option).0)
            }
            Algorithm::Secp256k1 => PublicKeyInner::Secp256k1(
                secp256k1::EcdsaSecp256k1Sha256::keypair(key_gen_option).0,
            ),
            Algorithm::BlsNormal => {
                PublicKeyInner::BlsNormal(bls::BlsNormal::keypair(key_gen_option).0)
            }
            Algorithm::BlsSmall => {
                PublicKeyInner::BlsSmall(bls::BlsSmall::keypair(key_gen_option).0)
            }
        };
        PublicKey(Box::new(inner))
    }
}

#[derive(Clone)]
#[allow(missing_docs, variant_size_differences)]
enum PrivateKeyInner {
    Ed25519(ed25519::PrivateKey),
    Secp256k1(secp256k1::PrivateKey),
    BlsNormal(bls::BlsNormalPrivateKey),
    BlsSmall(bls::BlsSmallPrivateKey),
}

ffi::ffi_item! {
    /// Private Key used in signatures.
    #[derive(Clone)]
    #[cfg_attr(all(feature = "ffi_export", not(feature = "ffi_import")), ffi_type(opaque))]
    #[allow(missing_docs, variant_size_differences)]
    pub struct PrivateKey(Box<PrivateKeyInner>);
}

impl PartialEq for PrivateKey {
    fn eq(&self, other: &Self) -> bool {
        match (self.0.borrow(), other.0.borrow()) {
            (PrivateKeyInner::Ed25519(l), PrivateKeyInner::Ed25519(r)) => l == r,
            (PrivateKeyInner::Secp256k1(l), PrivateKeyInner::Secp256k1(r)) => l == r,
            (PrivateKeyInner::BlsNormal(l), PrivateKeyInner::BlsNormal(r)) => {
                l.to_bytes() == r.to_bytes()
            }
            (PrivateKeyInner::BlsSmall(l), PrivateKeyInner::BlsSmall(r)) => {
                l.to_bytes() == r.to_bytes()
            }
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
    pub fn from_raw(algorithm: Algorithm, payload: &[u8]) -> Result<Self, ParseError> {
        match algorithm {
            Algorithm::Ed25519 => {
                ed25519::Ed25519Sha512::parse_private_key(payload).map(PrivateKeyInner::Ed25519)
            }
            Algorithm::Secp256k1 => secp256k1::EcdsaSecp256k1Sha256::parse_private_key(payload)
                .map(PrivateKeyInner::Secp256k1),
            Algorithm::BlsNormal => {
                bls::BlsNormal::parse_private_key(payload).map(PrivateKeyInner::BlsNormal)
            }
            Algorithm::BlsSmall => {
                bls::BlsSmall::parse_private_key(payload).map(PrivateKeyInner::BlsSmall)
            }
        }
        .map(Box::new)
        .map(PrivateKey)
    }

    /// Construct [`PrivateKey`] from hex encoded string
    ///
    /// # Errors
    ///
    /// - If the given payload is not hex encoded
    /// - If the given payload is not a valid private key
    pub fn from_hex(algorithm: Algorithm, payload: &str) -> Result<Self, ParseError> {
        let payload = hex_decode(payload)?;

        Self::from_raw(algorithm, &payload)
    }

    /// Get the digital signature algorithm of the private key
    pub fn algorithm(&self) -> Algorithm {
        match self.0.borrow() {
            PrivateKeyInner::Ed25519(_) => Algorithm::Ed25519,
            PrivateKeyInner::Secp256k1(_) => Algorithm::Secp256k1,
            PrivateKeyInner::BlsNormal(_) => Algorithm::BlsNormal,
            PrivateKeyInner::BlsSmall(_) => Algorithm::BlsSmall,
        }
    }

    /// Key payload
    fn payload(&self) -> Vec<u8> {
        match self.0.borrow() {
            PrivateKeyInner::Ed25519(key) => key.to_keypair_bytes().to_vec(),
            PrivateKeyInner::Secp256k1(key) => key.to_bytes().to_vec(),
            PrivateKeyInner::BlsNormal(key) => key.to_bytes(),
            PrivateKeyInner::BlsSmall(key) => key.to_bytes(),
        }
    }

    /// Extracts the raw bytes from the private key, copying the payload.
    ///
    /// `into_raw()` without copying is not provided because underlying crypto
    /// libraries do not provide move functionality.
    pub fn to_raw(&self) -> (Algorithm, Vec<u8>) {
        (self.algorithm(), self.payload())
    }
}

#[cfg(not(feature = "ffi_import"))]
impl core::fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple(self.algorithm().as_static_str())
            .field(&hex::encode_upper(self.payload()))
            .finish()
    }
}

#[cfg(not(feature = "ffi_import"))]
impl core::fmt::Display for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&hex::encode_upper(self.payload()))
    }
}

#[cfg(not(feature = "ffi_import"))]
impl Serialize for PrivateKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("PublicKey", 2)?;
        state.serialize_field("digest_function", &self.algorithm())?;
        state.serialize_field("payload", &hex::encode(self.payload()))?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for PrivateKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;

        #[derive(Deserialize)]
        struct PrivateKeyCandidate {
            digest_function: Algorithm,
            payload: String,
        }

        // NOTE: Verify that private key is valid
        let private_key = PrivateKeyCandidate::deserialize(deserializer)?;
        Self::from_hex(private_key.digest_function, private_key.payload.as_ref())
            .map_err(D::Error::custom)
    }
}

/// A session key derived from a key exchange. Will usually be used for a symmetric encryption afterwards
pub struct SessionKey(ConstVec<u8>);

impl SessionKey {
    /// Expose the raw bytes of the session key
    pub fn payload(&self) -> &[u8] {
        self.0.as_ref()
    }
}

/// Shim for decoding hexadecimal strings
pub(crate) fn hex_decode<T: AsRef<[u8]> + ?Sized>(payload: &T) -> Result<Vec<u8>, ParseError> {
    hex::decode(payload).map_err(|err| ParseError(err.to_string()))
}

pub mod error {
    //! Module containing errors
    use super::*;

    /// Error indicating algorithm could not be found
    #[derive(Debug, Display, Clone, Copy)]
    #[display(fmt = "Algorithm not supported")]
    pub struct NoSuchAlgorithm;

    #[cfg(feature = "std")]
    impl std::error::Error for NoSuchAlgorithm {}

    /// Error parsing a key
    #[derive(Debug, Display, Clone, serde::Deserialize, PartialEq, Eq)]
    #[display(fmt = "{_0}")]
    pub struct ParseError(pub(crate) String);

    #[cfg(feature = "std")]
    impl std::error::Error for ParseError {}

    /// Error when dealing with cryptographic functions
    #[derive(Debug, Display, serde::Deserialize, PartialEq, Eq)]
    pub enum Error {
        /// Returned when trying to create an algorithm which does not exist
        #[display(fmt = "Algorithm doesn't exist")] // TODO: which algorithm
        NoSuchAlgorithm(String),
        /// Occurs during deserialization of a private or public key
        #[display(fmt = "Key could not be parsed. {_0}")]
        Parse(ParseError),
        /// Returned when an error occurs during the signing process
        #[display(fmt = "Signing failed. {_0}")]
        Signing(String),
        /// Returned when an error occurs during the signature verification process
        #[display(fmt = "Signature verification failed")]
        BadSignature,
        /// Returned when an error occurs during key generation
        #[display(fmt = "Key generation failed. {_0}")]
        KeyGen(String),
        /// Returned when an error occurs during digest generation
        #[display(fmt = "Digest generation failed. {_0}")]
        DigestGen(String),
        /// Returned when an error occurs during creation of [`SignaturesOf`]
        #[display(fmt = "`SignaturesOf` must contain at least one signature")]
        EmptySignatureIter,
        /// A General purpose error message that doesn't fit in any category
        #[display(fmt = "General error. {_0}")] // This is going to cause a headache
        Other(String),
    }

    impl From<NoSuchAlgorithm> for Error {
        fn from(source: NoSuchAlgorithm) -> Self {
            Self::NoSuchAlgorithm(source.to_string())
        }
    }

    impl From<ParseError> for Error {
        fn from(source: ParseError) -> Self {
            Self::Parse(source)
        }
    }

    #[cfg(feature = "std")]
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
        KeyGenConfiguration,
        PublicKey,
        PrivateKey,
        KeyPair,
        Signature,
    }

    #[cfg(feature = "ffi_import")]
    iroha_ffi::decl_ffi_fns! { link_prefix="iroha_crypto" Drop, Clone, Eq, Ord, Default }
    #[cfg(all(feature = "ffi_export", not(feature = "ffi_import")))]
    iroha_ffi::def_ffi_fns! { link_prefix="iroha_crypto"
        Drop: { KeyGenConfiguration, PublicKey, PrivateKey, KeyPair, Signature },
        Clone: { KeyGenConfiguration, PublicKey, PrivateKey, KeyPair, Signature },
        Eq: { PublicKey, PrivateKey, KeyPair, Signature },
        Ord: { PublicKey, Signature },
    }

    // NOTE: Makes sure that only one `dealloc` is exported per generated dynamic library
    #[cfg(any(crate_type = "dylib", crate_type = "cdylib"))]
    #[cfg(all(feature = "ffi_export", not(feature = "ffi_import")))]
    mod dylib {
        #[cfg(not(feature = "std"))]
        use alloc::alloc;
        #[cfg(feature = "std")]
        use std::alloc;

        iroha_ffi::def_ffi_fns! {dealloc}
    }

    pub(crate) use ffi_item;
}

/// The prelude re-exports most commonly used traits, structs and macros from this crate.
pub mod prelude {
    pub use super::{Algorithm, Hash, KeyPair, PrivateKey, PublicKey, Signature};
}

#[cfg(test)]
mod tests {
    use parity_scale_codec::{Decode, Encode};
    #[cfg(not(feature = "ffi_import"))]
    use serde::Deserialize;

    use super::*;

    #[test]
    fn algorithm_serialize_deserialize_consistent() {
        for algorithm in [
            Algorithm::Ed25519,
            Algorithm::Secp256k1,
            Algorithm::BlsNormal,
            Algorithm::BlsSmall,
        ] {
            assert_eq!(
                algorithm,
                serde_json::to_string(&algorithm)
                    .and_then(|algorithm| serde_json::from_str(&algorithm))
                    .unwrap_or_else(|_| panic!("Failed to de/serialize key {:?}", &algorithm))
            );
        }
    }

    #[test]
    #[cfg(feature = "rand")]
    fn key_pair_serialize_deserialize_consistent() {
        for algorithm in [
            Algorithm::Ed25519,
            Algorithm::Secp256k1,
            Algorithm::BlsNormal,
            Algorithm::BlsSmall,
        ] {
            let key_pair = KeyPair::generate_with_configuration(
                KeyGenConfiguration::from_random().with_algorithm(algorithm),
            );

            assert_eq!(
                key_pair,
                serde_json::to_string(&key_pair)
                    .and_then(|key_pair| serde_json::from_str(&key_pair))
                    .unwrap_or_else(|_| panic!("Failed to de/serialize key {:?}", &key_pair))
            );
        }
    }

    #[test]
    fn encode_decode_algorithm_consistent() {
        for algorithm in [
            Algorithm::Ed25519,
            Algorithm::Secp256k1,
            Algorithm::BlsNormal,
            Algorithm::BlsSmall,
        ] {
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
        KeyPair::new("ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
            .parse()
            .expect("Public key not in mulithash format"),
        PrivateKey::from_hex(
            Algorithm::Ed25519,
            "93CA389FC2979F3F7D2A7F8B76C70DE6D5EAF5FA58D4F93CB8B0FB298D398ACC59C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
        ).expect("Private key not hex encoded")).unwrap();

        KeyPair::new("ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
            .parse()
            .expect("Public key not in multihash format"),
        PrivateKey::from_hex(
            Algorithm::BlsNormal,
            "1ca347641228c3b79aa43839dedc85fa51c0e8b9b6a00f6b0d6b0423e902973f",
        ).expect("Private key not hex encoded")).unwrap();
    }

    #[test]
    #[cfg(feature = "rand")]
    fn encode_decode_public_key_consistent() {
        for algorithm in [
            Algorithm::Ed25519,
            Algorithm::Secp256k1,
            Algorithm::BlsNormal,
            Algorithm::BlsSmall,
        ] {
            let key_pair = KeyPair::generate_with_configuration(
                KeyGenConfiguration::from_random().with_algorithm(algorithm),
            );
            let (public_key, _) = key_pair.into();

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
    fn invalid_private_key() {
        assert!(PrivateKey::from_hex(
            Algorithm::Ed25519,
            "0000000000000000000000000000000049BF70187154C57B97AF913163E8E875733B4EAF1F3F0689B31CE392129493E9"
        ).is_err());

        assert!(
            PrivateKey::from_hex(
                Algorithm::BlsNormal,
                "93CA389FC2979F3F7D2A7F8B76C70DE6D5EAF5FA58D4F93CB8B0FB298D398ACC59C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
            ).is_err());
    }

    #[test]
    fn key_pair_mismatch() {
        KeyPair::new("ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774"
            .parse()
            .expect("Public key not in mulithash format"),
        PrivateKey::from_hex(
            Algorithm::Ed25519,
            "3A7991AF1ABB77F3FD27CC148404A6AE4439D095A63591B77C788D53F708A02A1509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        ).expect("Private key not valid")).unwrap_err();

        KeyPair::new("ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
            .parse()
            .expect("Public key not in mulithash format"),
        PrivateKey::from_hex(
            Algorithm::BlsNormal,
            "CC176E44C41AA144FD1BEE4E0BCD2EF43F06D0C7BC2988E89A799951D240E503",
        ).expect("Private key not valid")).unwrap_err();
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
    #[cfg(not(feature = "ffi_import"))]
    #[derive(Debug, PartialEq, Deserialize, Serialize)]
    struct TestJson {
        public_key: PublicKey,
        private_key: PrivateKey,
    }

    #[test]
    #[cfg(not(feature = "ffi_import"))]
    fn deserialize_keys_ed25519() {
        assert_eq!(
            serde_json::from_str::<'_, TestJson>("{
                \"public_key\": \"ed01201509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4\",
                \"private_key\": {
                    \"digest_function\": \"ed25519\",
                    \"payload\": \"3A7991AF1ABB77F3FD27CC148404A6AE4439D095A63591B77C788D53F708A02A1509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4\"
                }
            }").expect("Failed to deserialize."),
            TestJson {
                public_key: PublicKey::from_hex(
                    Algorithm::Ed25519,
                    "1509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
                ).unwrap(),
                private_key: PrivateKey::from_hex(
                    Algorithm::Ed25519,
                    "3A7991AF1ABB77F3FD27CC148404A6AE4439D095A63591B77C788D53F708A02A1509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4",
                ).unwrap()
            }
        );
    }

    #[test]
    #[cfg(not(feature = "ffi_import"))]
    fn deserialize_keys_secp256k1() {
        assert_eq!(
            serde_json::from_str::<'_, TestJson>("{
                \"public_key\": \"e701210312273E8810581E58948D3FB8F9E8AD53AAA21492EBB8703915BBB565A21B7FCC\",
                \"private_key\": {
                    \"digest_function\": \"secp256k1\",
                    \"payload\": \"4DF4FCA10762D4B529FE40A2188A60CA4469D2C50A825B5F33ADC2CB78C69445\"
                }
            }").expect("Failed to deserialize."),
            TestJson {
                public_key: PublicKey::from_hex(
                    Algorithm::Secp256k1,
                    "0312273E8810581E58948D3FB8F9E8AD53AAA21492EBB8703915BBB565A21B7FCC"
                ).unwrap(),
                private_key: PrivateKey::from_hex(
                    Algorithm::Secp256k1,
                    "4DF4FCA10762D4B529FE40A2188A60CA4469D2C50A825B5F33ADC2CB78C69445",
                ).unwrap()
            }
        );
    }

    #[test]
    #[cfg(not(feature = "ffi_import"))]
    fn deserialize_keys_bls() {
        assert_eq!(
            serde_json::from_str::<'_, TestJson>("{
                \"public_key\": \"ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2\",
                \"private_key\": {
                    \"digest_function\": \"bls_normal\",
                    \"payload\": \"1ca347641228c3b79aa43839dedc85fa51c0e8b9b6a00f6b0d6b0423e902973f\"
                }
            }").expect("Failed to deserialize."),
            TestJson {
                public_key: PublicKey::from_hex(
                    Algorithm::BlsNormal,
                    "9060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2",
                ).unwrap(),
                private_key: PrivateKey::from_hex(
                    Algorithm::BlsNormal,
                    "1ca347641228c3b79aa43839dedc85fa51c0e8b9b6a00f6b0d6b0423e902973f",
                ).unwrap()
            }
        );
        assert_eq!(
            serde_json::from_str::<'_, TestJson>("{
                \"public_key\": \"eb01609051D4A9C69402423413EBBA4C00BC82A0102AA2B783057BD7BCEE4DD17B37DE5D719EE84BE43783F2AE47A673A74B8315DD3E595ED1FBDFAC17DA1D7A36F642B423ED18275FAFD671B1D331439D22F12FB6EB436A47E8656F182A78DF29D310\",
                \"private_key\": {
                    \"digest_function\": \"bls_small\",
                    \"payload\": \"8cb95072914cdd8e4cf682fdbe1189cdf4fc54d445e760b3446f896dbdbf5b2b\"
                }
            }").expect("Failed to deserialize."),
            TestJson {
                public_key: PublicKey::from_hex(
                    Algorithm::BlsSmall,
                    "9051D4A9C69402423413EBBA4C00BC82A0102AA2B783057BD7BCEE4DD17B37DE5D719EE84BE43783F2AE47A673A74B8315DD3E595ED1FBDFAC17DA1D7A36F642B423ED18275FAFD671B1D331439D22F12FB6EB436A47E8656F182A78DF29D310",
                ).unwrap(),
                private_key: PrivateKey::from_hex(
                    Algorithm::BlsSmall,
                    "8cb95072914cdd8e4cf682fdbe1189cdf4fc54d445e760b3446f896dbdbf5b2b",
                ).unwrap()
            }
        );
    }
}
