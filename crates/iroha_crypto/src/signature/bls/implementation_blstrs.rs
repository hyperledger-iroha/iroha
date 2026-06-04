use core::marker::PhantomData;
#[cfg(test)]
use std::sync::Arc;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
    vec::Vec,
};

#[cfg(feature = "rand")]
use crate::rng::os_rng;
#[cfg(test)]
use blstrs::G2Prepared;
use blstrs::{G1Affine, G2Affine};
use group::prime::PrimeCurveAffine;
use parking_lot::Mutex;
use w3f_bls::{
    EngineBLS, PublicKey as W3fPublicKey, SerializableToBytes as _, Signature as W3fSignature,
};
use zeroize::Zeroize as _;

pub(super) const MESSAGE_CONTEXT: &[u8; 20] = b"for signing messages";

use crate::{Algorithm, Error, KeyGenOption, ParseError};

pub trait BlsConfiguration {
    const ALGORITHM: Algorithm;
    // true: Normal (pk in G1, sig in G2); false: Small (pk in G2, sig in G1)
    const NORMAL: bool;
}

#[doc(hidden)]
#[cfg(test)]
pub trait PreparedPublicKeyCacheAccess: BlsConfiguration {}

#[cfg(test)]
impl<C: BlsConfiguration> PreparedPublicKeyCacheAccess for C {}

// Public key wrapper stores compressed bytes; orientation depends on C::NORMAL
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicKey<C: BlsConfiguration> {
    bytes: Vec<u8>,
    _m: PhantomData<C>,
}
impl<C: BlsConfiguration> PublicKey<C> {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }
}

// Private key wrapper holds the scalar
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecretKey<C: BlsConfiguration> {
    bytes: [u8; 32], // stable on-wire layout (w3f-compatible)
    _m: PhantomData<C>,
}
impl<C: BlsConfiguration> SecretKey<C> {
    pub fn to_bytes(&self) -> [u8; 32] {
        self.bytes
    }
    fn from_bytes(bytes: [u8; 32]) -> Self {
        Self {
            bytes,
            _m: PhantomData,
        }
    }
}
impl<C: BlsConfiguration> zeroize::Zeroize for SecretKey<C> {
    fn zeroize(&mut self) {
        self.bytes.fill(0);
    }
}

pub struct BlsImpl<C: BlsConfiguration + ?Sized>(PhantomData<C>);

impl<C: BlsConfiguration> BlsImpl<C> {
    #[allow(clippy::similar_names)]
    pub fn keypair(
        option: KeyGenOption<SecretKey<C>>,
    ) -> Result<(PublicKey<C>, SecretKey<C>), Error> {
        Self::try_keypair(option)
    }

    #[allow(clippy::similar_names)]
    pub fn try_keypair(
        mut option: KeyGenOption<SecretKey<C>>,
    ) -> Result<(PublicKey<C>, SecretKey<C>), Error> {
        let sk = match option {
            #[cfg(feature = "rand")]
            KeyGenOption::Random => {
                let bytes = if C::NORMAL {
                    w3f_bls::SecretKeyVT::<w3f_bls::ZBLS>::generate(os_rng()).to_bytes()
                } else {
                    w3f_bls::SecretKeyVT::<w3f_bls::TinyBLS381>::generate(os_rng()).to_bytes()
                };
                Self::secret_key_from_generated_bytes(&bytes)?
            }
            KeyGenOption::UseSeed(ref mut seed) => {
                let bytes = if C::NORMAL {
                    w3f_bls::SecretKeyVT::<w3f_bls::ZBLS>::from_seed(seed).to_bytes()
                } else {
                    w3f_bls::SecretKeyVT::<w3f_bls::TinyBLS381>::from_seed(seed).to_bytes()
                };
                seed.zeroize();
                Self::secret_key_from_generated_bytes(&bytes)?
            }
            KeyGenOption::FromPrivateKey(key) => key,
        };

        let public_key =
            Self::derive_public_key(&sk).map_err(|err| Error::KeyGen(err.to_string()))?;
        Ok((public_key, sk))
    }

    fn secret_key_from_generated_bytes(bytes: &[u8]) -> Result<SecretKey<C>, Error> {
        let mut arr = [0u8; 32];
        if bytes.len() != arr.len() {
            return Err(Error::KeyGen(
                "invalid generated BLS secret key length".into(),
            ));
        }
        arr.copy_from_slice(bytes);
        Ok(SecretKey::from_bytes(arr))
    }

    pub fn sign(message: &[u8], sk: &SecretKey<C>) -> Result<Vec<u8>, Error> {
        Self::try_sign(message, sk)
    }

    pub fn try_sign(message: &[u8], sk: &SecretKey<C>) -> Result<Vec<u8>, Error> {
        // Produce signature with w3f to match canonical encoding exactly.
        let msg = w3f_bls::Message::new(MESSAGE_CONTEXT, message);
        if C::NORMAL {
            let sk_w = w3f_bls::SecretKeyVT::<w3f_bls::ZBLS>::from_bytes(&sk.bytes)
                .map_err(|err| Error::Signing(err.to_string()))?;
            Ok(sk_w.sign(&msg).to_bytes())
        } else {
            let sk_w = w3f_bls::SecretKeyVT::<w3f_bls::TinyBLS381>::from_bytes(&sk.bytes)
                .map_err(|err| Error::Signing(err.to_string()))?;
            Ok(sk_w.sign(&msg).to_bytes())
        }
    }

    pub fn derive_public_key(sk: &SecretKey<C>) -> Result<PublicKey<C>, ParseError> {
        // Public key depends on orientation; derive via w3f to ensure stable encoding
        let pk_bytes = if C::NORMAL {
            let sk_w = w3f_bls::SecretKeyVT::<w3f_bls::ZBLS>::from_bytes(&sk.bytes)
                .map_err(|err| ParseError(err.to_string()))?;
            sk_w.into_public().to_bytes()
        } else {
            let sk_w = w3f_bls::SecretKeyVT::<w3f_bls::TinyBLS381>::from_bytes(&sk.bytes)
                .map_err(|err| ParseError(err.to_string()))?;
            sk_w.into_public().to_bytes()
        };
        Ok(PublicKey {
            bytes: pk_bytes,
            _m: PhantomData,
        })
    }

    pub fn verify(message: &[u8], signature: &[u8], pk: &PublicKey<C>) -> Result<(), Error> {
        if C::NORMAL {
            verify_w3f::<w3f_bls::ZBLS>(message, signature, &pk.bytes)
        } else {
            verify_w3f::<w3f_bls::TinyBLS381>(message, signature, &pk.bytes)
        }
    }

    pub fn verify_aggregate_same_message(
        message: &[u8],
        signatures: &[&[u8]],
        public_keys: &[&[u8]],
    ) -> Result<(), Error> {
        if C::NORMAL {
            verify_aggregate_same_message_w3f::<w3f_bls::ZBLS>(message, signatures, public_keys)
        } else {
            verify_aggregate_same_message_w3f::<w3f_bls::TinyBLS381>(
                message,
                signatures,
                public_keys,
            )
        }
    }

    /// Aggregate a sequence of BLS signatures (same-message context) into a single signature.
    /// The caller is responsible for ensuring all signatures are valid and from the same suite.
    /// Rejects aggregates that cancel to the identity element.
    pub fn aggregate_signatures(signatures: &[&[u8]]) -> Result<Vec<u8>, Error> {
        if C::NORMAL {
            aggregate_w3f_signatures::<w3f_bls::ZBLS>(signatures)
                .map(|signature| signature.to_bytes())
        } else {
            aggregate_w3f_signatures::<w3f_bls::TinyBLS381>(signatures)
                .map(|signature| signature.to_bytes())
        }
    }

    /// Verify a pre-aggregated signature for the same-message case.
    pub fn verify_preaggregated_same_message(
        message: &[u8],
        aggregated_signature: &[u8],
        public_keys: &[&[u8]],
    ) -> Result<(), Error> {
        if C::NORMAL {
            verify_preaggregated_same_message_w3f::<w3f_bls::ZBLS>(
                message,
                aggregated_signature,
                public_keys,
            )
        } else {
            verify_preaggregated_same_message_w3f::<w3f_bls::TinyBLS381>(
                message,
                aggregated_signature,
                public_keys,
            )
        }
    }

    pub fn verify_aggregate_multi_message(
        messages: &[&[u8]],
        signatures: &[&[u8]],
        public_keys: &[&[u8]],
    ) -> Result<(), Error> {
        if C::NORMAL {
            verify_aggregate_multi_message_w3f::<w3f_bls::ZBLS>(messages, signatures, public_keys)
        } else {
            verify_aggregate_multi_message_w3f::<w3f_bls::TinyBLS381>(
                messages,
                signatures,
                public_keys,
            )
        }
    }

    pub fn parse_public_key(payload: &[u8]) -> Result<PublicKey<C>, ParseError> {
        // Just validate compression length and decompress once
        if C::NORMAL {
            to_g1_public_key(payload)
                .ok_or_else(|| ParseError("invalid G1 public key".to_string()))?;
        } else {
            to_g2_public_key(payload)
                .ok_or_else(|| ParseError("invalid G2 public key".to_string()))?;
        }
        Ok(PublicKey {
            bytes: payload.to_vec(),
            _m: PhantomData,
        })
    }

    pub fn parse_private_key(payload: &[u8]) -> Result<SecretKey<C>, ParseError> {
        if payload.len() != 32 {
            return Err(ParseError("invalid BLS secret key length".to_string()));
        }
        if payload.iter().all(|&b| b == 0) {
            return Err(ParseError("BLS secret key is zero".to_string()));
        }
        // Validate via w3f backend to match compat acceptance window
        if C::NORMAL {
            w3f_bls::SecretKeyVT::<w3f_bls::ZBLS>::from_bytes(payload)
                .map_err(|_| ParseError("invalid BLS secret key".to_string()))?;
        } else {
            w3f_bls::SecretKeyVT::<w3f_bls::TinyBLS381>::from_bytes(payload)
                .map_err(|_| ParseError("invalid BLS secret key".to_string()))?;
        }
        let mut arr = [0u8; 32];
        arr.copy_from_slice(payload);
        Ok(SecretKey::from_bytes(arr))
    }
}

fn parse_w3f_signature<E: EngineBLS>(bytes: &[u8]) -> Result<W3fSignature<E>, Error> {
    let signature = W3fSignature::<E>::from_bytes(bytes)
        .map_err(|_| ParseError("Failed to parse signature.".to_string()))?;
    let canonical = signature.to_bytes();
    if canonical.as_slice() != bytes {
        return Err(ParseError("non-canonical BLS signature encoding".to_string()).into());
    }
    let identity = W3fSignature::<E>(Default::default()).to_bytes();
    if canonical == identity {
        return Err(ParseError("BLS signature is identity".to_string()).into());
    }
    Ok(signature)
}

fn parse_w3f_public_key<E: EngineBLS>(bytes: &[u8]) -> Result<W3fPublicKey<E>, Error> {
    let public_key =
        W3fPublicKey::<E>::from_bytes(bytes).map_err(|err| ParseError(err.to_string()))?;
    let canonical = public_key.to_bytes();
    if canonical.as_slice() != bytes {
        return Err(ParseError("non-canonical BLS public key encoding".to_string()).into());
    }
    let identity = W3fPublicKey::<E>(Default::default()).to_bytes();
    if canonical == identity {
        return Err(ParseError("BLS public key is identity".to_string()).into());
    }
    Ok(public_key)
}

fn aggregate_w3f_signatures<E: EngineBLS>(signatures: &[&[u8]]) -> Result<W3fSignature<E>, Error> {
    use core::ops::AddAssign as _;

    let mut signatures = signatures.iter();
    let first = signatures.next().ok_or(Error::BadSignature)?;
    let first = parse_w3f_signature::<E>(first)?;
    let mut aggregate = first.0;
    for signature in signatures {
        let signature = parse_w3f_signature::<E>(signature)?;
        aggregate.add_assign(&signature.0);
    }
    let aggregate = W3fSignature::<E>(aggregate);
    let identity = W3fSignature::<E>(Default::default()).to_bytes();
    if aggregate.to_bytes() == identity {
        return Err(Error::BadSignature);
    }
    Ok(aggregate)
}

fn aggregate_w3f_public_keys<E: EngineBLS>(
    public_keys: &[&[u8]],
) -> Result<W3fPublicKey<E>, Error> {
    use core::ops::AddAssign as _;

    let mut seen = BTreeSet::new();
    let mut public_keys = public_keys.iter();
    let first_bytes = public_keys.next().ok_or(Error::BadSignature)?;
    let first = parse_w3f_public_key::<E>(first_bytes)?;
    if !seen.insert(*first_bytes) {
        return Err(Error::BadSignature);
    }
    let mut aggregate = first.0;
    for public_key_bytes in public_keys {
        let public_key = parse_w3f_public_key::<E>(public_key_bytes)?;
        if !seen.insert(*public_key_bytes) {
            return Err(Error::BadSignature);
        }
        aggregate.add_assign(&public_key.0);
    }
    let aggregate = W3fPublicKey::<E>(aggregate);
    let identity = W3fPublicKey::<E>(Default::default()).to_bytes();
    if aggregate.to_bytes() == identity {
        return Err(Error::BadSignature);
    }
    Ok(aggregate)
}

fn verify_w3f<E: EngineBLS>(
    message: &[u8],
    signature: &[u8],
    public_key: &[u8],
) -> Result<(), Error> {
    let signature = parse_w3f_signature::<E>(signature)?;
    let public_key = parse_w3f_public_key::<E>(public_key)?;
    let message = w3f_bls::Message::new(MESSAGE_CONTEXT, message);
    if signature.verify(&message, &public_key) {
        Ok(())
    } else {
        Err(Error::BadSignature)
    }
}

fn verify_aggregate_same_message_w3f<E: EngineBLS>(
    message: &[u8],
    signatures: &[&[u8]],
    public_keys: &[&[u8]],
) -> Result<(), Error> {
    if signatures.is_empty() || signatures.len() != public_keys.len() {
        return Err(Error::BadSignature);
    }
    let signature = aggregate_w3f_signatures::<E>(signatures)?;
    let public_key = aggregate_w3f_public_keys::<E>(public_keys)?;
    let message = w3f_bls::Message::new(MESSAGE_CONTEXT, message);
    if signature.verify(&message, &public_key) {
        Ok(())
    } else {
        Err(Error::BadSignature)
    }
}

fn verify_preaggregated_same_message_w3f<E: EngineBLS>(
    message: &[u8],
    aggregated_signature: &[u8],
    public_keys: &[&[u8]],
) -> Result<(), Error> {
    if public_keys.is_empty() {
        return Err(Error::BadSignature);
    }
    let signature = parse_w3f_signature::<E>(aggregated_signature)?;
    let public_key = aggregate_w3f_public_keys::<E>(public_keys)?;
    let message = w3f_bls::Message::new(MESSAGE_CONTEXT, message);
    if signature.verify(&message, &public_key) {
        Ok(())
    } else {
        Err(Error::BadSignature)
    }
}

fn ensure_distinct_messages(messages: &[&[u8]]) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for &message in messages {
        if !seen.insert(message) {
            return Err(Error::BadSignature);
        }
    }
    Ok(())
}

fn verify_aggregate_multi_message_w3f<E: EngineBLS>(
    messages: &[&[u8]],
    signatures: &[&[u8]],
    public_keys: &[&[u8]],
) -> Result<(), Error> {
    if !(messages.len() == signatures.len() && signatures.len() == public_keys.len())
        || messages.is_empty()
    {
        return Err(Error::BadSignature);
    }
    ensure_distinct_messages(messages)?;

    let signature = aggregate_w3f_signatures::<E>(signatures)?;
    let mut decoded_messages = Vec::with_capacity(messages.len());
    let mut decoded_public_keys = Vec::with_capacity(messages.len());
    for (message, public_key) in messages.iter().zip(public_keys.iter()) {
        decoded_messages.push(w3f_bls::Message::new(MESSAGE_CONTEXT, message));
        decoded_public_keys.push(parse_w3f_public_key::<E>(public_key)?);
    }

    let batch = MultiMessageBatch {
        signature,
        messages: decoded_messages,
        public_keys: decoded_public_keys,
    };

    if w3f_bls::verifiers::verify_with_distinct_messages(&batch, false) {
        Ok(())
    } else {
        Err(Error::BadSignature)
    }
}

struct MultiMessageBatch<E: EngineBLS> {
    signature: W3fSignature<E>,
    messages: Vec<w3f_bls::Message>,
    public_keys: Vec<W3fPublicKey<E>>,
}

impl<'a, E: EngineBLS> w3f_bls::Signed for &'a MultiMessageBatch<E> {
    type E = E;
    type M = &'a w3f_bls::Message;
    type PKG = &'a W3fPublicKey<E>;
    type PKnM = std::iter::Zip<
        std::slice::Iter<'a, w3f_bls::Message>,
        std::slice::Iter<'a, W3fPublicKey<E>>,
    >;

    fn signature(&self) -> W3fSignature<E> {
        W3fSignature(self.signature.0)
    }

    fn messages_and_publickeys(self) -> Self::PKnM {
        self.messages.iter().zip(self.public_keys.iter())
    }
}

const PUBKEY_CACHE_MAX: usize = 4096;

fn g1_pubkey_cache() -> &'static Mutex<BTreeMap<Vec<u8>, G1Affine>> {
    static CACHE: OnceLock<Mutex<BTreeMap<Vec<u8>, G1Affine>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn g2_pubkey_cache() -> &'static Mutex<BTreeMap<Vec<u8>, G2Affine>> {
    static CACHE: OnceLock<Mutex<BTreeMap<Vec<u8>, G2Affine>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

#[cfg(test)]
fn g2_prepared_cache() -> &'static Mutex<BTreeMap<Vec<u8>, Arc<G2Prepared>>> {
    static CACHE: OnceLock<Mutex<BTreeMap<Vec<u8>, Arc<G2Prepared>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

#[cfg(test)]
fn g2_prepared_generator() -> &'static Arc<G2Prepared> {
    static GENERATOR: OnceLock<Arc<G2Prepared>> = OnceLock::new();
    GENERATOR.get_or_init(|| Arc::new(G2Prepared::from(G2Affine::generator())))
}

fn to_g1_public_key(bytes: &[u8]) -> Option<G1Affine> {
    if let Some(point) = g1_pubkey_cache().lock().get(bytes).copied() {
        return Some(point);
    }
    let point = to_g1(bytes)?;
    let mut cache = g1_pubkey_cache().lock();
    if cache.len() >= PUBKEY_CACHE_MAX {
        cache.clear();
    }
    cache.insert(bytes.to_vec(), point);
    Some(point)
}

fn to_g2_public_key(bytes: &[u8]) -> Option<G2Affine> {
    if let Some(point) = g2_pubkey_cache().lock().get(bytes).copied() {
        return Some(point);
    }
    let point = to_g2(bytes)?;
    let mut cache = g2_pubkey_cache().lock();
    if cache.len() >= PUBKEY_CACHE_MAX {
        cache.clear();
    }
    cache.insert(bytes.to_vec(), point);
    Some(point)
}

#[cfg(test)]
fn to_g2_prepared(bytes: &[u8]) -> Option<Arc<G2Prepared>> {
    if let Some(point) = g2_prepared_cache().lock().get(bytes).cloned() {
        return Some(point);
    }
    let point = to_g2_public_key(bytes)?;
    let prepared = Arc::new(G2Prepared::from(point));
    let mut cache = g2_prepared_cache().lock();
    if cache.len() >= PUBKEY_CACHE_MAX {
        cache.clear();
    }
    cache.insert(bytes.to_vec(), Arc::clone(&prepared));
    Some(prepared)
}

fn to_g1(bytes: &[u8]) -> Option<G1Affine> {
    if bytes.len() != 48 {
        return None;
    }
    let mut arr = [0u8; 48];
    arr.copy_from_slice(bytes);
    let point = G1Affine::from_compressed(&arr).into_option()?;
    if bool::from(point.is_identity()) {
        return None;
    }
    if point.to_compressed() != arr {
        return None;
    }
    Some(point)
}
fn to_g2(bytes: &[u8]) -> Option<G2Affine> {
    if bytes.len() != 96 {
        return None;
    }
    let mut arr = [0u8; 96];
    arr.copy_from_slice(bytes);
    let point = G2Affine::from_compressed(&arr).into_option()?;
    if bool::from(point.is_identity()) {
        return None;
    }
    if point.to_compressed() != arr {
        return None;
    }
    Some(point)
}

#[cfg(test)]
pub(super) fn detect_variant_normal(
    message: &[u8],
    signature: &[u8],
    pk_bytes: &[u8],
) -> (bool, bool) {
    // Parse via w3f-bls to match bytes and ciphersuite exactly
    let sig = if let Ok(s) = w3f_bls::Signature::<w3f_bls::ZBLS>::from_bytes(signature) {
        s
    } else {
        return (false, false);
    };
    let pk = if let Ok(p) = w3f_bls::PublicKey::<w3f_bls::ZBLS>::from_bytes(pk_bytes) {
        p
    } else {
        return (false, false);
    };

    // CONCAT: Message::new(context, message)
    let ok_concat = {
        let msg = w3f_bls::Message::new(MESSAGE_CONTEXT, message);
        sig.verify(&msg, &pk)
    };
    // AUG: approximate by pre-pending pk to message; should fail under our ciphersuite
    let ok_aug = {
        let mut buf = Vec::with_capacity(pk_bytes.len() + message.len());
        buf.extend_from_slice(pk_bytes);
        buf.extend_from_slice(message);
        let msg = w3f_bls::Message::new(MESSAGE_CONTEXT, &buf);
        sig.verify(&msg, &pk)
    };
    (ok_concat, ok_aug)
}

#[cfg(test)]
pub(super) fn detect_variant_small(
    message: &[u8],
    signature: &[u8],
    pk_bytes: &[u8],
) -> (bool, bool) {
    // Parse via w3f-bls tiny engine to match bytes and ciphersuite exactly
    let sig = if let Ok(s) = w3f_bls::Signature::<w3f_bls::TinyBLS381>::from_bytes(signature) {
        s
    } else {
        return (false, false);
    };
    let pk = if let Ok(p) = w3f_bls::PublicKey::<w3f_bls::TinyBLS381>::from_bytes(pk_bytes) {
        p
    } else {
        return (false, false);
    };

    let ok_concat = {
        let msg = w3f_bls::Message::new(MESSAGE_CONTEXT, message);
        sig.verify(&msg, &pk)
    };
    let ok_aug = {
        let mut buf = Vec::with_capacity(pk_bytes.len() + message.len());
        buf.extend_from_slice(pk_bytes);
        buf.extend_from_slice(message);
        let msg = w3f_bls::Message::new(MESSAGE_CONTEXT, &buf);
        sig.verify(&msg, &pk)
    };
    (ok_concat, ok_aug)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    // Simple self-test to ensure keypair/sign/verify cycle works for both orientations
    #[derive(Debug, Clone, Copy)]
    struct CNormal;
    impl BlsConfiguration for CNormal {
        const ALGORITHM: Algorithm = Algorithm::BlsNormal;
        const NORMAL: bool = true;
    }
    #[derive(Debug, Clone, Copy)]
    struct CSmall;
    impl BlsConfiguration for CSmall {
        const ALGORITHM: Algorithm = Algorithm::BlsSmall;
        const NORMAL: bool = false;
    }

    #[test]
    fn smoke_normal() {
        let (pk, sk) =
            BlsImpl::<CNormal>::keypair(KeyGenOption::UseSeed(vec![7; 10])).expect("BLS keypair");
        let sig = BlsImpl::<CNormal>::sign(b"abc", &sk).expect("BLS sign");
        assert!(BlsImpl::<CNormal>::verify(b"abc", &sig, &pk).is_ok());
    }

    #[test]
    fn smoke_small() {
        let (pk, sk) =
            BlsImpl::<CSmall>::keypair(KeyGenOption::UseSeed(vec![9; 16])).expect("BLS keypair");
        let sig = BlsImpl::<CSmall>::sign(b"xyz", &sk).expect("BLS sign");
        assert!(BlsImpl::<CSmall>::verify(b"xyz", &sig, &pk).is_ok());
    }

    #[test]
    fn public_key_cache_roundtrip_normal() {
        let (pk, _sk) =
            BlsImpl::<CNormal>::keypair(KeyGenOption::UseSeed(vec![1; 8])).expect("BLS keypair");
        let bytes = pk.to_bytes();
        let parsed = to_g1_public_key(&bytes).expect("valid public key");
        let cached = to_g1_public_key(&bytes).expect("cached public key");
        assert_eq!(parsed.to_compressed(), cached.to_compressed());
    }

    #[test]
    fn public_key_cache_roundtrip_small() {
        let (pk, _sk) =
            BlsImpl::<CSmall>::keypair(KeyGenOption::UseSeed(vec![2; 8])).expect("BLS keypair");
        let bytes = pk.to_bytes();
        let parsed = to_g2_public_key(&bytes).expect("valid public key");
        let cached = to_g2_public_key(&bytes).expect("cached public key");
        assert_eq!(parsed.to_compressed(), cached.to_compressed());
    }

    #[test]
    fn prepared_generator_is_cached() {
        let first = g2_prepared_generator();
        let second = g2_prepared_generator();
        assert!(Arc::ptr_eq(first, second));
    }

    #[test]
    fn prepared_public_key_cache_roundtrip_small() {
        let (pk, _sk) =
            BlsImpl::<CSmall>::keypair(KeyGenOption::UseSeed(vec![3; 8])).expect("BLS keypair");
        let bytes = pk.to_bytes();
        let prepared = to_g2_prepared(&bytes).expect("valid prepared key");
        let cached = to_g2_prepared(&bytes).expect("cached prepared key");
        assert!(Arc::ptr_eq(&prepared, &cached));
    }

    #[test]
    fn compressed_point_decoders_reject_invalid_encodings() {
        let mut invalid_g1 = [0xFF; 48];
        let mut invalid_g2 = [0xFF; 96];
        invalid_g1[0] = 0x00;
        invalid_g2[0] = 0x00;

        assert!(to_g1_public_key(&invalid_g1).is_none());
        assert!(to_g2_public_key(&invalid_g2).is_none());
    }
}
