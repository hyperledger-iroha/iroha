// pub(crate) for inner modules it is not redundant, the contents of `signature` module get re-exported at root
#![allow(clippy::redundant_pub_crate)]

#[cfg(all(feature = "bls", not(feature = "ffi_import")))]
pub(crate) mod bls;

#[cfg(not(feature = "ffi_import"))]
pub(crate) mod ed25519;

#[cfg(not(feature = "ffi_import"))]
pub(crate) mod secp256k1;

#[cfg(all(feature = "gost", not(feature = "ffi_import")))]
pub(crate) mod gost;

#[cfg(all(feature = "sm", not(feature = "ffi_import")))]
pub(crate) mod sm;

use core::marker::PhantomData;
use std::{cell::RefCell, format, string::String, vec, vec::Vec};

use derive_more::{Deref, DerefMut};
use iroha_primitives::const_vec::ConstVec;
use iroha_schema::{IntoSchema, TypeId};
use norito::core::{self as ncore, DecodeFromSlice};
#[cfg(feature = "json")]
use norito::json::{self, FastJsonWrite, JsonDeserialize};

#[cfg(feature = "sm")]
use crate::sm::Sm2Signature;
use crate::{
    Error, HashOf, PrivateKey, PublicKey, PublicKeyFull, error::ParseError, ffi, hex_decode,
};

ffi::ffi_item! {
    /// Represents a signature of the data (`Block` or `Transaction` for example).
    #[allow(unexpected_cfgs)]
    #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, getset::Getters)]
    #[cfg_attr(
        not(feature="ffi_import"),
        derive(derive_more::Debug, Hash, IntoSchema)
    )]
    #[cfg_attr(all(feature = "ffi_export", not(feature = "ffi_import")), ffi_type(opaque))]
    #[repr(transparent)]
    #[cfg_attr(not(feature="ffi_import"), debug("{{ {} }}", hex::encode_upper(payload)))]
    pub struct Signature {
        payload: ConstVec<u8>
    }
}

const PUBLIC_KEY_FULL_CACHE_LIMIT: usize = 128;

struct PublicKeyFullCacheEntry {
    algorithm: u8,
    payload: Vec<u8>,
    full: PublicKeyFull,
}

thread_local! {
    static PUBLIC_KEY_FULL_CACHE: RefCell<Vec<PublicKeyFullCacheEntry>> =
        RefCell::new(Vec::new());
}

fn public_key_full_cached(public_key: &PublicKey) -> PublicKeyFull {
    let (algorithm, payload) = public_key.to_bytes();
    let algorithm = algorithm as u8;
    PUBLIC_KEY_FULL_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(pos) = cache
            .iter()
            .position(|entry| entry.algorithm == algorithm && entry.payload.as_slice() == payload)
        {
            let entry = cache.remove(pos);
            let full = entry.full.clone();
            cache.push(entry);
            return full;
        }
        let full: PublicKeyFull = (&public_key.0).into();
        cache.push(PublicKeyFullCacheEntry {
            algorithm,
            payload: payload.to_vec(),
            full: full.clone(),
        });
        if cache.len() > PUBLIC_KEY_FULL_CACHE_LIMIT {
            let drain = cache.len() - PUBLIC_KEY_FULL_CACHE_LIMIT;
            cache.drain(0..drain);
        }
        full
    })
}

impl Signature {
    /// Creates new signature by signing payload via [`crate::KeyPair::private_key`].
    pub fn new(private_key: &PrivateKey, payload: &[u8]) -> Self {
        use crate::secrecy::ExposeSecret;

        let signature = match private_key.0.expose_secret() {
            crate::PrivateKeyInner::Ed25519(sk) => ed25519::Ed25519Sha512::sign(payload, sk),
            crate::PrivateKeyInner::Secp256k1(sk) => {
                secp256k1::EcdsaSecp256k1Sha256::sign(payload, sk)
            }
            crate::PrivateKeyInner::MlDsa(sk) => sk.sign(payload),
            #[cfg(feature = "gost")]
            crate::PrivateKeyInner::Gost { algorithm, secret } => {
                gost::sign(*algorithm, payload, secret)
                    .expect("GOST signing should succeed for a valid private key")
            }
            #[cfg(feature = "bls")]
            crate::PrivateKeyInner::BlsSmall(sk) => bls::BlsSmall::sign(payload, sk),
            #[cfg(feature = "bls")]
            crate::PrivateKeyInner::BlsNormal(sk) => bls::BlsNormal::sign(payload, sk),
            #[cfg(feature = "sm")]
            crate::PrivateKeyInner::Sm2(sk) => sk.sign(payload).as_bytes().to_vec(),
        };

        Self {
            payload: ConstVec::new(signature),
        }
    }

    /// Get the raw payload of the signature.
    pub fn payload(&self) -> &[u8] {
        &self.payload
    }

    /// Creates new signature from its raw payload and public key.
    ///
    /// **This method does not sign the payload.** Use [`Signature::new`] for this purpose.
    ///
    /// This method exists to allow reproducing the signature in a more efficient way than through
    /// deserialization.
    pub fn from_bytes(payload: &[u8]) -> Self {
        Self {
            payload: ConstVec::new(payload),
        }
    }

    /// A shorthand for [`Self::from_bytes`] accepting payload as hex.
    ///
    /// # Errors
    /// If passed string is not a valid hex.
    pub fn from_hex(payload: impl AsRef<str>) -> Result<Self, ParseError> {
        let payload: Vec<u8> = hex_decode(payload.as_ref())?;
        Ok(Self::from_bytes(&payload))
    }

    /// Verify `payload` using signed data and [`crate::KeyPair::public_key`].
    ///
    /// # Errors
    /// Fails if the message doesn't pass verification
    pub fn verify(&self, public_key: &PublicKey, payload: &[u8]) -> Result<(), Error> {
        let public_key_full = public_key_full_cached(public_key);
        match &public_key_full {
            PublicKeyFull::Ed25519(pk) => {
                ed25519::Ed25519Sha512::verify(payload, &self.payload, pk)
            }
            PublicKeyFull::Secp256k1(pk) => {
                secp256k1::EcdsaSecp256k1Sha256::verify(payload, &self.payload, pk)
            }
            PublicKeyFull::MlDsa(pk_bytes) => {
                use pqcrypto_dilithium::dilithium3 as dilithium;
                use pqcrypto_traits::sign::{DetachedSignature as _, PublicKey as _};
                if self.payload.len() != dilithium::signature_bytes() {
                    return Err(Error::BadSignature);
                }
                let sig = dilithium::DetachedSignature::from_bytes(&self.payload)
                    .map_err(|_| Error::BadSignature)?;
                if pk_bytes.len() != dilithium::public_key_bytes() {
                    return Err(Error::BadSignature);
                }
                let pk =
                    dilithium::PublicKey::from_bytes(pk_bytes).map_err(|_| Error::BadSignature)?;
                if dilithium::verify_detached_signature(&sig, payload, &pk).is_err() {
                    return Err(Error::BadSignature);
                }
                Ok(())
            }
            #[cfg(feature = "gost")]
            PublicKeyFull::Gost { algorithm, key } => {
                gost::verify(*algorithm, payload, &self.payload, key)
            }
            #[cfg(feature = "bls")]
            PublicKeyFull::BlsSmall(pk) => bls::BlsSmall::verify(payload, &self.payload, pk),
            #[cfg(feature = "bls")]
            PublicKeyFull::BlsNormal(pk) => bls::BlsNormal::verify(payload, &self.payload, pk),
            #[cfg(feature = "sm")]
            PublicKeyFull::Sm2(pk) => {
                if self.payload.len() != Sm2Signature::LENGTH {
                    return Err(Error::BadSignature);
                }
                let mut raw = [0u8; Sm2Signature::LENGTH];
                raw.copy_from_slice(self.payload.as_ref());
                let signature = Sm2Signature::from_bytes(&raw).map_err(Error::Parse)?;
                pk.verify(payload, &signature)
            }
        }?;

        Ok(())
    }
}

#[cfg(all(feature = "json", not(feature = "ffi_import")))]
impl FastJsonWrite for Signature {
    fn write_json(&self, out: &mut String) {
        let encoded = hex::encode_upper(self.payload());
        json::write_json_string(&encoded, out);
    }
}

#[cfg(all(feature = "json", not(feature = "ffi_import")))]
impl JsonDeserialize for Signature {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let encoded = parser.parse_string()?;
        Signature::from_hex(&encoded).map_err(|err| json::Error::Message(err.to_string()))
    }
}

impl<T> From<SignatureOf<T>> for Signature {
    fn from(SignatureOf(signature, ..): SignatureOf<T>) -> Self {
        signature
    }
}

ffi::ffi_item! {
    /// Represents signature of the data (`Block` or `Transaction` for example).
    // Lint triggers when expanding #[codec(skip)]
    #[allow(clippy::default_trait_access, clippy::unsafe_derive_deserialize)]
    #[derive(Deref, DerefMut, TypeId)]
    // Transmute guard
    #[repr(transparent)]
    pub struct SignatureOf<T>(
        #[deref]
        #[deref_mut]
        Signature,
        PhantomData<T>,
    );

    // SAFETY: `SignatureOf` has no trap representation in `Signature`
    ffi_type(unsafe {robust})
}

#[cfg(not(feature = "ffi_import"))]
impl<T> core::fmt::Debug for SignatureOf<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple(core::any::type_name::<Self>())
            .field(&self.0)
            .finish()
    }
}

impl<T> Clone for SignatureOf<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone(), PhantomData)
    }
}

#[allow(clippy::unconditional_recursion)] // False-positive
impl<T> PartialEq for SignatureOf<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}
impl<T> Eq for SignatureOf<T> {}

impl<T> PartialOrd for SignatureOf<T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for SignatureOf<T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<T> core::hash::Hash for SignatureOf<T> {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<T: IntoSchema> IntoSchema for SignatureOf<T> {
    fn type_name() -> String {
        format!("SignatureOf<{}>", T::type_name())
    }
    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if !map.contains_key::<Self>() {
            map.insert::<Self>(iroha_schema::Metadata::Tuple(
                iroha_schema::UnnamedFieldsMeta {
                    types: vec![core::any::TypeId::of::<Signature>()],
                },
            ));

            Signature::update_schema_map(map);
        }
    }
}

/// Archived representation of [`SignatureOf`].
pub type ArchivedSignatureOf<T> = norito::core::Archived<SignatureOf<T>>;

#[cfg(not(feature = "ffi_import"))]
impl ncore::NoritoSerialize for Signature {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), ncore::Error> {
        self.payload.serialize(writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        self.payload.encoded_len_hint()
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        self.payload.encoded_len_exact()
    }
}

#[cfg(not(feature = "ffi_import"))]
impl<'de> ncore::NoritoDeserialize<'de> for Signature {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        let payload = ConstVec::<u8>::deserialize(archived.cast::<ConstVec<u8>>());
        Signature { payload }
    }

    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let vec = Vec::<u8>::try_deserialize(archived.cast::<Vec<u8>>())?;
        Ok(Signature {
            payload: ConstVec::from(vec),
        })
    }
}

// Use default Norito derives for SignatureOf<T> provided by the crate macros.
impl<'a> norito::core::DecodeFromSlice<'a> for Signature {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (payload, used) =
            <ConstVec<u8> as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
        Ok((Signature { payload }, used))
    }
}

impl<T> norito::core::NoritoSerialize for SignatureOf<T> {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
        // Delegate to inner Signature so SignatureOf has identical on-wire bytes.
        norito::core::NoritoSerialize::serialize(&self.0, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        norito::core::NoritoSerialize::encoded_len_hint(&self.0)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        norito::core::NoritoSerialize::encoded_len_exact(&self.0)
    }
}

impl<'de, T> norito::core::NoritoDeserialize<'de> for SignatureOf<T> {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        let as_sig: &norito::core::Archived<Signature> = archived.cast();
        let sig = <Signature as norito::core::NoritoDeserialize>::deserialize(as_sig);
        SignatureOf(sig, PhantomData)
    }
}

#[cfg(all(feature = "json", not(feature = "ffi_import")))]
impl<T> FastJsonWrite for SignatureOf<T> {
    fn write_json(&self, out: &mut String) {
        self.0.write_json(out);
    }
}

#[cfg(all(feature = "json", not(feature = "ffi_import")))]
impl<T> JsonDeserialize for SignatureOf<T> {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        Signature::json_deserialize(parser).map(|sig| SignatureOf(sig, PhantomData))
    }
}

// Norito already provides blanket `Encode`/`Decode` implementations via the
// `NoritoSerialize`/`NoritoDeserialize` traits. However, packed sequence layouts
// used by `ConstVec<u8>` require the slice decoder to cooperate with the Norito
// payload context, so we keep this focused bridge that simply delegates to the
// inner `Signature`.
impl<'a, T> DecodeFromSlice<'a> for SignatureOf<T> {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (inner, used) = <Signature as DecodeFromSlice>::decode_from_slice(bytes)?;
        Ok((SignatureOf(inner, PhantomData), used))
    }
}

impl<T> SignatureOf<T> {
    /// Create [`SignatureOf`] from the given hash with [`crate::KeyPair::private_key`].
    ///
    /// # Errors
    /// Fails if signing fails
    #[inline]
    pub fn from_hash(private_key: &PrivateKey, hash: HashOf<T>) -> Self {
        Self(Signature::new(private_key, hash.as_ref()), PhantomData)
    }

    /// Construct [`SignatureOf`] from an already-produced [`Signature`].
    #[inline]
    pub fn from_signature(signature: Signature) -> Self {
        Self(signature, PhantomData)
    }

    /// Verify signature for this hash
    ///
    /// # Errors
    ///
    /// Fails if the given hash didn't pass verification
    pub fn verify_hash(&self, public_key: &PublicKey, hash: HashOf<T>) -> Result<(), Error> {
        self.0.verify(public_key, hash.as_ref())
    }
}

impl<T: norito::codec::Encode> SignatureOf<T> {
    /// Create [`SignatureOf`] by signing the given value with [`crate::KeyPair::private_key`].
    /// The value provided will be hashed before being signed. If you already have the
    /// hash of the value you can sign it with [`SignatureOf::from_hash`] instead.
    ///
    /// # Errors
    /// Fails if signing fails
    #[inline]
    pub fn new(private_key: &PrivateKey, value: &T) -> Self {
        let h = HashOf::new(value);
        Self::from_hash(private_key, h)
    }

    /// Verifies signature for this item
    ///
    /// # Errors
    /// Fails if verification fails
    pub fn verify(&self, public_key: &PublicKey, value: &T) -> Result<(), Error> {
        self.verify_hash(public_key, HashOf::new(value))
    }
}

// Provide slice-based decoding for Signature as well (used by hybrid decoders)
#[cfg(not(feature = "ffi_import"))]
#[cfg(test)]
mod tests {

    use super::*;
    use crate::{Algorithm, HashOf, KeyPair};

    #[test]
    #[cfg(feature = "rand")]
    fn create_signature_ed25519() {
        let key_pair = KeyPair::random_with_algorithm(crate::Algorithm::Ed25519);
        let message = b"Test message to sign.";
        let signature = Signature::new(key_pair.private_key(), message);
        signature.verify(key_pair.public_key(), message).unwrap();
    }

    #[test]
    #[cfg(feature = "rand")]
    fn create_signature_secp256k1() {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::Secp256k1);
        let message = b"Test message to sign.";
        let signature = Signature::new(key_pair.private_key(), message);
        signature.verify(key_pair.public_key(), message).unwrap();
    }

    #[test]
    #[cfg(all(feature = "rand", feature = "bls"))]
    fn create_signature_bls_normal() {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let message = b"Test message to sign.";
        let signature = Signature::new(key_pair.private_key(), message);
        signature.verify(key_pair.public_key(), message).unwrap();
    }

    #[test]
    #[cfg(all(feature = "rand", feature = "bls"))]
    fn create_signature_bls_small() {
        let key_pair = KeyPair::random_with_algorithm(Algorithm::BlsSmall);
        let message = b"Test message to sign.";
        let signature = Signature::new(key_pair.private_key(), message);
        signature.verify(key_pair.public_key(), message).unwrap();
    }

    #[test]
    #[cfg(feature = "rand")]
    fn signature_verify_cache_separates_keys() {
        let key_one = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let key_two = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let message = b"Signature verify cache test";
        let signature = Signature::new(key_one.private_key(), message);

        signature.verify(key_one.public_key(), message).unwrap();
        assert!(
            signature.verify(key_two.public_key(), message).is_err(),
            "cache must not mix distinct public keys"
        );
        signature.verify(key_one.public_key(), message).unwrap();
    }

    #[test]
    fn signature_serialized_representation() {
        let input = norito::json!(
            "3A7991AF1ABB77F3FD27CC148404A6AE4439D095A63591B77C788D53F708A02A1509A611AD6D97B01D871E58ED00C8FD7C3917B6CA61A8C2833A19E000AAC2E4"
        );

        let signature: Signature = norito::json::from_value(input.clone()).unwrap();

        assert_eq!(norito::json::to_value(&signature).unwrap(), input);
    }

    #[test]
    fn signature_from_hex_simply_reproduces_the_data() {
        let payload = "3a7991af1abb77f3fd27cc148404a6ae4439d095a63591b77c788d53f708a02a1509a611ad6d97b01d871e58ed00c8fd7c3917b6ca61a8c2833a19e000aac2e4";

        let value = Signature::from_hex(payload).unwrap();
        assert_eq!(value.payload.as_ref(), &hex::decode(payload).unwrap());
    }

    #[test]
    #[cfg(feature = "rand")]
    fn signature_of_roundtrip() {
        use norito::codec::{Decode, Encode};

        let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let hash = HashOf::new(&());
        let sig = SignatureOf::from_hash(key_pair.private_key(), hash);
        let bytes = sig.encode();
        // Decode inner Signature from the same bare codec payload.
        let decoded_sig = Signature::decode(&mut &bytes[..]).expect("decode inner signature");
        let decoded = SignatureOf::<()>(decoded_sig, PhantomData);
        assert_eq!(sig, decoded);
    }

    #[test]
    fn signature_norito_roundtrip_preserves_payload() {
        use norito::{
            NoritoDeserialize,
            codec::{Decode, Encode},
            core::DecodeFromSlice as _,
        };

        let payload = (0u8..32).collect::<Vec<_>>();
        let signature = Signature::from_bytes(&payload);

        let bytes = signature.encode();
        let mut cursor = &bytes[..];
        let decoded_codec = Signature::decode(&mut cursor).expect("codec decode");
        assert_eq!(decoded_codec, signature);

        let framed = norito::core::to_bytes(&signature).expect("frame signature payload");
        let archived = norito::from_bytes::<Signature>(&framed).expect("archived signature");
        let decoded = Signature::deserialize(archived);
        assert_eq!(decoded, signature);

        let inner_payload = &framed[std::mem::size_of::<norito::core::Header>()..];
        let (decoded_from_slice, used) =
            Signature::decode_from_slice(inner_payload).expect("slice decode");
        assert_eq!(used, inner_payload.len());
        assert_eq!(decoded_from_slice, signature);

        norito::core::reset_decode_state();
    }

    #[test]
    fn signature_of_from_signature_wraps_payload() {
        let payload = (0u8..64).collect::<Vec<_>>();
        let signature = Signature::from_bytes(&payload);

        let typed = SignatureOf::<()>::from_signature(signature.clone());

        assert_eq!(Signature::from(typed), signature);
    }

    #[test]
    fn signature_vec_roundtrip_via_norito() {
        use norito::NoritoDeserialize;

        let payload = (0u8..16).collect::<Vec<_>>();
        let signature = Signature::from_bytes(&payload);
        let values = vec![signature.clone()];

        let bytes = norito::core::to_bytes(&values).expect("encode signature vec");
        println!("signature vec encoded bytes {bytes:02X?}");
        let archived = norito::core::from_bytes::<Vec<Signature>>(&bytes)
            .expect("decode signature vec header");
        let decoded = Vec::<Signature>::deserialize(archived);

        let payload = decoded[0].payload();
        println!("decoded signature payload {payload:02X?}");

        assert_eq!(decoded, values);
    }
}
