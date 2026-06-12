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
    Algorithm, Error, HashOf, PrivateKey, PublicKey, PublicKeyFull, error::ParseError, ffi,
    hex_decode,
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
const ED25519_PUBLIC_KEY_FULL_FAST_CACHE_SIZE: usize = 16_384;

struct PublicKeyFullCacheEntry {
    algorithm: u8,
    payload: Vec<u8>,
    full: PublicKeyFull,
}

#[derive(Clone, Copy)]
struct Ed25519PublicKeyFullFastEntry {
    payload: [u8; 32],
    full: ed25519::PublicKey,
}

struct PublicKeyFullFastCache {
    ed25519: Box<[Option<Ed25519PublicKeyFullFastEntry>]>,
    #[cfg(test)]
    ed25519_hits: usize,
    #[cfg(test)]
    ed25519_misses: usize,
    #[cfg(test)]
    ed25519_inserts: usize,
}

impl PublicKeyFullFastCache {
    fn new() -> Self {
        Self {
            ed25519: vec![None; ED25519_PUBLIC_KEY_FULL_FAST_CACHE_SIZE].into_boxed_slice(),
            #[cfg(test)]
            ed25519_hits: 0,
            #[cfg(test)]
            ed25519_misses: 0,
            #[cfg(test)]
            ed25519_inserts: 0,
        }
    }

    fn get_ed25519(&mut self, payload: &[u8]) -> Option<ed25519::PublicKey> {
        let payload: [u8; 32] = payload.try_into().ok()?;
        let slot = ed25519_public_key_full_fast_index(&payload);
        if let Some(entry) = self.ed25519[slot]
            && entry.payload == payload
        {
            #[cfg(test)]
            {
                self.ed25519_hits = self.ed25519_hits.saturating_add(1);
            }
            return Some(entry.full);
        }
        #[cfg(test)]
        {
            self.ed25519_misses = self.ed25519_misses.saturating_add(1);
        }
        None
    }

    fn insert_ed25519(&mut self, payload: [u8; 32], full: ed25519::PublicKey) {
        let slot = ed25519_public_key_full_fast_index(&payload);
        self.ed25519[slot] = Some(Ed25519PublicKeyFullFastEntry { payload, full });
        #[cfg(test)]
        {
            self.ed25519_inserts = self.ed25519_inserts.saturating_add(1);
        }
    }

    #[cfg(test)]
    fn reset(&mut self) {
        self.ed25519.fill(None);
        self.ed25519_hits = 0;
        self.ed25519_misses = 0;
        self.ed25519_inserts = 0;
    }

    #[cfg(test)]
    fn stats(&self) -> PublicKeyFullFastCacheStats {
        PublicKeyFullFastCacheStats {
            hits: self.ed25519_hits,
            misses: self.ed25519_misses,
            inserts: self.ed25519_inserts,
        }
    }
}

#[cfg(test)]
#[derive(Debug, Default, PartialEq, Eq)]
struct PublicKeyFullFastCacheStats {
    hits: usize,
    misses: usize,
    inserts: usize,
}

thread_local! {
    static PUBLIC_KEY_FULL_FAST_CACHE: RefCell<PublicKeyFullFastCache> =
        RefCell::new(PublicKeyFullFastCache::new());
    static PUBLIC_KEY_FULL_CACHE: RefCell<Vec<PublicKeyFullCacheEntry>> =
        const { RefCell::new(Vec::new()) };
}

#[inline]
fn ed25519_public_key_full_fast_index(payload: &[u8; 32]) -> usize {
    ed25519_public_key_full_fast_index_for_size(payload, ED25519_PUBLIC_KEY_FULL_FAST_CACHE_SIZE)
}

#[inline]
fn ed25519_public_key_full_fast_index_for_size(payload: &[u8; 32], cache_size: usize) -> usize {
    let Some(a) = le_u64_chunk(payload, 0) else {
        return 0;
    };
    let Some(b) = le_u64_chunk(payload, 8) else {
        return 0;
    };
    let Some(c) = le_u64_chunk(payload, 16) else {
        return 0;
    };
    let Some(d) = le_u64_chunk(payload, 24) else {
        return 0;
    };
    let Some(mask) = cache_size
        .checked_sub(1)
        .and_then(|mask| u64::try_from(mask).ok())
    else {
        return 0;
    };
    let mixed = a ^ b.rotate_left(17) ^ c.rotate_left(31) ^ d.rotate_left(47);
    usize::try_from(mixed & mask).unwrap_or(0)
}

#[inline]
fn le_u64_chunk(payload: &[u8; 32], start: usize) -> Option<u64> {
    let end = start.checked_add(8)?;
    let chunk = payload.get(start..end)?;
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(chunk);
    Some(u64::from_le_bytes(bytes))
}

#[cfg(test)]
fn reset_public_key_full_fast_cache_for_tests() {
    PUBLIC_KEY_FULL_FAST_CACHE.with(|cache| cache.borrow_mut().reset());
}

#[cfg(test)]
fn public_key_full_fast_cache_stats_for_tests() -> PublicKeyFullFastCacheStats {
    PUBLIC_KEY_FULL_FAST_CACHE.with(|cache| cache.borrow().stats())
}

pub(crate) fn public_key_full_cached(public_key: &PublicKey) -> Result<PublicKeyFull, Error> {
    let (algorithm, payload) = public_key.try_to_bytes()?;
    if algorithm == Algorithm::Ed25519 {
        if let Some(full) =
            PUBLIC_KEY_FULL_FAST_CACHE.with(|cache| cache.borrow_mut().get_ed25519(payload))
        {
            return Ok(PublicKeyFull::Ed25519(full));
        }
        let payload_bytes: [u8; 32] = payload
            .try_into()
            .map_err(|_| ParseError("invalid Ed25519 public key length".to_owned()))?;
        let full = ed25519::Ed25519Sha512::parse_public_key(&payload_bytes)?;
        PUBLIC_KEY_FULL_FAST_CACHE.with(|cache| {
            cache.borrow_mut().insert_ed25519(payload_bytes, full);
        });
        return Ok(PublicKeyFull::Ed25519(full));
    }

    let algorithm_tag = algorithm as u8;
    PUBLIC_KEY_FULL_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(pos) = cache.iter().position(|entry| {
            entry.algorithm == algorithm_tag && entry.payload.as_slice() == payload
        }) {
            let entry = cache.remove(pos);
            let full = entry.full.clone();
            cache.push(entry);
            return Ok(full);
        }
        let full = PublicKeyFull::from_bytes(algorithm, payload)?;
        cache.push(PublicKeyFullCacheEntry {
            algorithm: algorithm_tag,
            payload: payload.to_vec(),
            full: full.clone(),
        });
        if cache.len() > PUBLIC_KEY_FULL_CACHE_LIMIT {
            let drain = cache.len() - PUBLIC_KEY_FULL_CACHE_LIMIT;
            cache.drain(0..drain);
        }
        Ok(full)
    })
}

impl Signature {
    /// Creates new signature by signing payload via [`crate::KeyPair::private_key`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Signing`] when an algorithm-specific signing backend
    /// rejects the private-key material or message.
    pub fn try_new(private_key: &PrivateKey, payload: &[u8]) -> Result<Self, Error> {
        use crate::secrecy::ExposeSecret;

        let signature = match private_key.0.expose_secret() {
            crate::PrivateKeyInner::Ed25519(sk) => ed25519::Ed25519Sha512::sign(payload, sk),
            crate::PrivateKeyInner::Secp256k1(sk) => {
                secp256k1::EcdsaSecp256k1Sha256::try_sign(payload, sk)?
            }
            crate::PrivateKeyInner::MlDsa(sk) => sk.try_sign(payload)?,
            #[cfg(feature = "gost")]
            crate::PrivateKeyInner::Gost { algorithm, secret } => {
                gost::sign(*algorithm, payload, secret)?
            }
            #[cfg(feature = "bls")]
            crate::PrivateKeyInner::BlsSmall(sk) => bls::BlsSmall::try_sign(payload, sk)?,
            #[cfg(feature = "bls")]
            crate::PrivateKeyInner::BlsNormal(sk) => bls::BlsNormal::try_sign(payload, sk)?,
            #[cfg(feature = "sm")]
            crate::PrivateKeyInner::Sm2(sk) => sk.try_sign(payload)?.as_bytes().to_vec(),
        };

        Ok(Self {
            payload: ConstVec::new(signature),
        })
    }

    /// Creates new signature by signing payload via [`crate::KeyPair::private_key`].
    pub fn new(private_key: &PrivateKey, payload: &[u8]) -> Self {
        Self::try_new(private_key, payload)
            .expect("signing should succeed for a valid private key and payload")
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
        let public_key_full = public_key_full_cached(public_key)?;
        if signature_payload_is_all_zero(&self.payload) {
            return Err(Error::BadSignature);
        }
        match &public_key_full {
            PublicKeyFull::Ed25519(pk) => {
                ed25519::Ed25519Sha512::verify(payload, &self.payload, pk)
            }
            PublicKeyFull::Secp256k1(pk) => {
                secp256k1::EcdsaSecp256k1Sha256::verify(payload, &self.payload, pk)
            }
            PublicKeyFull::MlDsa(pk_bytes) => {
                use pqcrypto_mldsa::mldsa65 as dilithium;
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
            #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
            PublicKeyFull::BlsSmall { key, .. } => {
                bls::BlsSmall::verify(payload, &self.payload, key)
            }
            #[cfg(all(feature = "bls", not(feature = "bls-backend-blstrs")))]
            PublicKeyFull::BlsNormal { key, .. } => {
                bls::BlsNormal::verify(payload, &self.payload, key)
            }
            #[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
            PublicKeyFull::BlsSmall(pk) => bls::BlsSmall::verify(payload, &self.payload, pk),
            #[cfg(all(feature = "bls", feature = "bls-backend-blstrs"))]
            PublicKeyFull::BlsNormal(pk) => bls::BlsNormal::verify(payload, &self.payload, pk),
            #[cfg(feature = "sm")]
            PublicKeyFull::Sm2(pk) => {
                if self.payload.len() != Sm2Signature::LENGTH {
                    return Err(Error::BadSignature);
                }
                let mut raw = [0u8; Sm2Signature::LENGTH];
                raw.copy_from_slice(self.payload.as_ref());
                let signature = Sm2Signature::from_bytes(&raw).map_err(|_| Error::BadSignature)?;
                pk.verify(payload, &signature)
            }
        }?;

        Ok(())
    }
}

fn signature_payload_is_all_zero(payload: &[u8]) -> bool {
    !payload.is_empty() && payload.iter().all(|&byte| byte == 0)
}

fn decode_signature_payload_unpacked(bytes: &[u8]) -> Result<ConstVec<u8>, ncore::Error> {
    if bytes.len() < 8 {
        return Err(ncore::Error::LengthMismatch);
    }

    let mut count_bytes = [0u8; 8];
    count_bytes.copy_from_slice(&bytes[..8]);
    let count = usize::try_from(u64::from_le_bytes(count_bytes))
        .map_err(|_| ncore::Error::LengthMismatch)?;
    let raw_start = 8usize;
    if bytes.len() == raw_start.saturating_add(count) {
        return Ok(ConstVec::from(bytes[raw_start..].to_vec()));
    }

    let mut offset = raw_start;
    let mut payload = Vec::new();
    payload
        .try_reserve(count)
        .map_err(|_| ncore::Error::LengthMismatch)?;
    for _ in 0..count {
        let (elem_len, header_len) =
            ncore::read_len_from_slice(bytes.get(offset..).ok_or(ncore::Error::LengthMismatch)?)?;
        if elem_len != 1 {
            return Err(ncore::Error::LengthMismatch);
        }
        offset = offset
            .checked_add(header_len)
            .ok_or(ncore::Error::LengthMismatch)?;
        let byte = *bytes.get(offset).ok_or(ncore::Error::LengthMismatch)?;
        payload.push(byte);
        offset = offset
            .checked_add(elem_len)
            .ok_or(ncore::Error::LengthMismatch)?;
    }
    if offset != bytes.len() {
        return Err(ncore::Error::LengthMismatch);
    }
    Ok(ConstVec::from(payload))
}

fn decode_signature_payload_from_slice(
    bytes: &[u8],
) -> Result<(ConstVec<u8>, usize), ncore::Error> {
    <ConstVec<u8> as DecodeFromSlice>::decode_from_slice(bytes)
        .or_else(|_| decode_signature_payload_unpacked(bytes).map(|payload| (payload, bytes.len())))
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
        let payload_bytes =
            ncore::payload_slice_from_ptr(core::ptr::from_ref(archived).cast::<u8>()).ok();
        let payload =
            ConstVec::<u8>::try_deserialize(archived.cast::<ConstVec<u8>>()).or_else(|err| {
                let bytes = payload_bytes.ok_or(err)?;
                let payload = decode_signature_payload_unpacked(bytes)?;
                ncore::note_payload_access(bytes, bytes.len());
                Ok::<_, ncore::Error>(payload)
            })?;
        Ok(Signature { payload })
    }
}

// Use default Norito derives for SignatureOf<T> provided by the crate macros.
impl<'a> norito::core::DecodeFromSlice<'a> for Signature {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (payload, used) = decode_signature_payload_from_slice(bytes)?;
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
        Self::try_deserialize(archived).expect("SignatureOf decode")
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let signature = Signature::try_deserialize(archived.cast::<Signature>())?;
        Ok(SignatureOf(signature, PhantomData))
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
    /// Fallibly create [`SignatureOf`] from the given hash with
    /// [`crate::KeyPair::private_key`].
    ///
    /// # Errors
    ///
    /// Returns [`Error::Signing`] when an algorithm-specific signing backend
    /// rejects the private-key material or hash payload.
    #[inline]
    pub fn try_from_hash(private_key: &PrivateKey, hash: HashOf<T>) -> Result<Self, Error> {
        Signature::try_new(private_key, hash.as_ref()).map(|signature| Self(signature, PhantomData))
    }

    /// Create [`SignatureOf`] from the given hash with [`crate::KeyPair::private_key`].
    #[inline]
    pub fn from_hash(private_key: &PrivateKey, hash: HashOf<T>) -> Self {
        Self::try_from_hash(private_key, hash)
            .expect("signing should succeed for a valid private key and hash payload")
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
    /// Fallibly create [`SignatureOf`] by signing the given value with
    /// [`crate::KeyPair::private_key`].
    /// The value provided will be hashed before being signed. If you already have the
    /// hash of the value you can sign it with [`SignatureOf::try_from_hash`] instead.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Signing`] when an algorithm-specific signing backend
    /// rejects the private-key material or hash payload.
    #[inline]
    pub fn try_new(private_key: &PrivateKey, value: &T) -> Result<Self, Error> {
        let h = HashOf::new(value);
        Self::try_from_hash(private_key, h)
    }

    /// Create [`SignatureOf`] by signing the given value with [`crate::KeyPair::private_key`].
    /// The value provided will be hashed before being signed. If you already have the
    /// hash of the value you can sign it with [`SignatureOf::from_hash`] instead.
    #[inline]
    pub fn new(private_key: &PrivateKey, value: &T) -> Self {
        Self::try_new(private_key, value)
            .expect("signing should succeed for a valid private key and value")
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
    use crate::{Algorithm, HashOf, KeyGenOption, KeyPair, PublicKeyCompact};

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
    fn create_signature_secp256k1_checked_path() {
        let key_pair = KeyPair::try_from_seed(vec![0x51; 32], Algorithm::Secp256k1)
            .expect("seeded secp256k1 keypair");
        let message = b"Test message to sign with checked secp256k1.";
        let signature = Signature::try_new(key_pair.private_key(), message)
            .expect("checked secp256k1 signature");

        signature.verify(key_pair.public_key(), message).unwrap();
    }

    #[test]
    #[cfg(feature = "sm")]
    fn create_signature_sm2_checked_path() {
        let key_pair =
            KeyPair::try_from_seed(vec![0x42; 32], Algorithm::Sm2).expect("seeded SM2 keypair");
        let message = b"Test message to sign with checked SM2.";
        let signature =
            Signature::try_new(key_pair.private_key(), message).expect("checked SM2 signature");
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
    fn ed25519_public_key_full_fast_cache_hits_after_first_lookup() {
        let (raw_public, _) = ed25519::Ed25519Sha512::keypair(KeyGenOption::UseSeed(vec![7u8; 32]));
        let public_key = PublicKey::new(PublicKeyFull::Ed25519(raw_public));
        reset_public_key_full_fast_cache_for_tests();

        let first = public_key_full_cached(&public_key).expect("public key parses");
        assert!(matches!(first, PublicKeyFull::Ed25519(_)));
        assert_eq!(
            public_key_full_fast_cache_stats_for_tests(),
            PublicKeyFullFastCacheStats {
                hits: 0,
                misses: 1,
                inserts: 1,
            }
        );

        let second = public_key_full_cached(&public_key).expect("public key parses");
        assert!(matches!(second, PublicKeyFull::Ed25519(_)));
        assert_eq!(
            public_key_full_fast_cache_stats_for_tests(),
            PublicKeyFullFastCacheStats {
                hits: 1,
                misses: 1,
                inserts: 1,
            }
        );
    }

    #[test]
    fn ed25519_public_key_full_fast_index_is_total() {
        let payload = [0xA5; 32];

        assert_eq!(ed25519_public_key_full_fast_index_for_size(&payload, 0), 0);
        assert_eq!(ed25519_public_key_full_fast_index_for_size(&payload, 1), 0);

        let index = ed25519_public_key_full_fast_index(&payload);
        assert!(index < ED25519_PUBLIC_KEY_FULL_FAST_CACHE_SIZE);
        assert_eq!(index, ed25519_public_key_full_fast_index(&payload));
    }

    #[test]
    fn signature_verify_rejects_malformed_cached_ed25519_public_key_without_panic() {
        let malformed = PublicKey(PublicKeyCompact::new(Algorithm::Ed25519, &[]));
        let signature = Signature::from_bytes(&[0u8; 64]);

        let err = signature
            .verify(&malformed, b"message")
            .expect_err("malformed public key must fail verification");

        assert!(matches!(err, Error::Parse(_)));
    }

    #[test]
    fn signature_verify_rejects_all_zero_payload_before_backend() {
        let key_pair = KeyPair::try_from_seed(vec![0x44; 32], Algorithm::Ed25519)
            .expect("seeded Ed25519 keypair");
        let signature = Signature::from_bytes(&[0u8; 64]);

        let err = signature
            .verify(key_pair.public_key(), b"message")
            .expect_err("all-zero signature payload must fail closed");

        assert!(matches!(err, Error::BadSignature));
    }

    #[test]
    #[cfg(feature = "sm")]
    fn signature_verify_rejects_malformed_sm2_payload_as_bad_signature() {
        let key_pair =
            KeyPair::try_from_seed(vec![0x45; 32], Algorithm::Sm2).expect("seeded SM2 keypair");
        let mut payload = [0u8; crate::Sm2Signature::LENGTH];
        payload[crate::Sm2Signature::LENGTH - 1] = 1;
        let signature = Signature::from_bytes(&payload);

        let err = signature
            .verify(key_pair.public_key(), b"message")
            .expect_err("malformed SM2 signature payload must fail closed");

        assert!(matches!(err, Error::BadSignature));
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
    fn signature_of_try_deserialize_preserves_compact_const_vec_payload() {
        let payload = (0u8..64).collect::<Vec<_>>();
        let typed = SignatureOf::<()>::from_signature(Signature::from_bytes(&payload));
        let framed = norito::core::to_bytes(&typed).expect("frame typed signature");
        let archived =
            norito::from_bytes::<SignatureOf<()>>(&framed).expect("archived typed signature");

        let decoded =
            <SignatureOf<()> as norito::core::NoritoDeserialize>::try_deserialize(archived)
                .expect("typed signature decodes");

        assert_eq!(decoded, typed);
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
