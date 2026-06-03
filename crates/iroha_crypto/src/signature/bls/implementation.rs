use core::marker::PhantomData;
use std::{
    borrow::ToOwned as _,
    cell::RefCell,
    collections::{BTreeSet, HashSet},
    string::ToString as _,
    vec,
    vec::Vec,
};

use blake2::{Blake2b, digest::consts::U32};
use sha2::Digest as _;
use sha2::Sha256;
use w3f_bls::{
    EngineBLS, PublicKey, SecretKey as W3fSecretKey, SecretKeyVT, SerializableToBytes as _,
    Signature as BlsSignature,
};
use zeroize::Zeroize as _;

use super::{normal::NormalConfiguration, small::SmallConfiguration};

pub(super) const MESSAGE_CONTEXT: &[u8; 20] = b"for signing messages";

const PREPARED_PK_CACHE_LIMIT: usize = 128;
const VERIFY_OK_CACHE_LIMIT: usize = 4096;

#[doc(hidden)]
pub struct PreparedPublicKeyCache<E: EngineBLS> {
    entries: Vec<(Vec<u8>, E::PublicKeyPrepared)>,
}

impl<E: EngineBLS> PreparedPublicKeyCache<E> {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn get_or_insert(&mut self, pk: &PublicKey<E>, pk_bytes: &[u8]) -> E::PublicKeyPrepared {
        if let Some(pos) = self
            .entries
            .iter()
            .position(|(bytes, _)| bytes.as_slice() == pk_bytes)
        {
            let prepared = self.entries[pos].1.clone();
            if pos + 1 != self.entries.len() {
                let entry = self.entries.remove(pos);
                self.entries.push(entry);
            }
            return prepared;
        }
        let prepared = E::prepare_public_key(pk.0);
        self.entries.push((pk_bytes.to_vec(), prepared.clone()));
        if self.entries.len() > PREPARED_PK_CACHE_LIMIT {
            let drain = self.entries.len() - PREPARED_PK_CACHE_LIMIT;
            self.entries.drain(0..drain);
        }
        prepared
    }
}

#[doc(hidden)]
pub struct VerifyOkCache {
    map: HashSet<[u8; 32]>,
}

impl VerifyOkCache {
    fn new() -> Self {
        Self {
            map: HashSet::new(),
        }
    }

    fn contains(&self, key: &[u8; 32]) -> bool {
        self.map.contains(key)
    }

    fn insert(&mut self, key: [u8; 32]) {
        if self.map.len() >= VERIFY_OK_CACHE_LIMIT {
            self.map.clear();
        }
        self.map.insert(key);
    }
}

fn verify_ok_cache_key(pk_bytes: &[u8], message: &[u8], signature: &[u8]) -> [u8; 32] {
    let mut h = Blake2b::<U32>::new();
    h.update(b"iroha:bls:verify_ok_cache:v1");
    h.update(pk_bytes);
    h.update(message);
    h.update(signature);
    h.finalize().into()
}

#[doc(hidden)]
pub trait PreparedPublicKeyCacheAccess: BlsConfiguration {
    fn with_cache<R>(f: impl FnOnce(&mut PreparedPublicKeyCache<Self::Engine>) -> R) -> R;

    fn with_verify_ok_cache<R>(f: impl FnOnce(&mut VerifyOkCache) -> R) -> R;
}

thread_local! {
    static PREPARED_PK_CACHE_NORMAL: RefCell<
        PreparedPublicKeyCache<<NormalConfiguration as BlsConfiguration>::Engine>
    > = RefCell::new(PreparedPublicKeyCache::new());
    static PREPARED_PK_CACHE_SMALL: RefCell<
        PreparedPublicKeyCache<<SmallConfiguration as BlsConfiguration>::Engine>
    > = RefCell::new(PreparedPublicKeyCache::new());
    static VERIFY_OK_CACHE_NORMAL: RefCell<VerifyOkCache> = RefCell::new(VerifyOkCache::new());
    static VERIFY_OK_CACHE_SMALL: RefCell<VerifyOkCache> = RefCell::new(VerifyOkCache::new());
}

impl PreparedPublicKeyCacheAccess for NormalConfiguration {
    fn with_cache<R>(f: impl FnOnce(&mut PreparedPublicKeyCache<Self::Engine>) -> R) -> R {
        PREPARED_PK_CACHE_NORMAL.with(|cache| f(&mut cache.borrow_mut()))
    }

    fn with_verify_ok_cache<R>(f: impl FnOnce(&mut VerifyOkCache) -> R) -> R {
        VERIFY_OK_CACHE_NORMAL.with(|cache| f(&mut cache.borrow_mut()))
    }
}

impl PreparedPublicKeyCacheAccess for SmallConfiguration {
    fn with_cache<R>(f: impl FnOnce(&mut PreparedPublicKeyCache<Self::Engine>) -> R) -> R {
        PREPARED_PK_CACHE_SMALL.with(|cache| f(&mut cache.borrow_mut()))
    }

    fn with_verify_ok_cache<R>(f: impl FnOnce(&mut VerifyOkCache) -> R) -> R {
        VERIFY_OK_CACHE_SMALL.with(|cache| f(&mut cache.borrow_mut()))
    }
}

/// Thread-safe wrapper around the w3f `SecretKey` that allows interior mutability.
pub struct ManagedSecretKey<C: BlsConfiguration + ?Sized> {
    bytes: Vec<u8>,
    _marker: PhantomData<C>,
}

impl<C: BlsConfiguration + ?Sized> Clone for ManagedSecretKey<C> {
    fn clone(&self) -> Self {
        Self {
            bytes: self.bytes.clone(),
            _marker: PhantomData,
        }
    }
}

impl<C: BlsConfiguration + ?Sized> ManagedSecretKey<C> {
    fn new(secret: &W3fSecretKey<C::Engine>) -> Self {
        Self {
            bytes: secret.clone().into_vartime().to_bytes(),
            _marker: PhantomData,
        }
    }

    fn try_load_secret(&self) -> Result<W3fSecretKey<C::Engine>, ParseError> {
        W3fSecretKey::<C::Engine>::from_bytes(&self.bytes)
            .map_err(|err| ParseError(err.to_string()))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    pub fn to_fixed_bytes(&self) -> [u8; 32] {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&self.bytes);
        arr
    }

    pub fn public_key(&self) -> Result<PublicKey<C::Engine>, ParseError> {
        self.try_public_key()
    }

    pub fn try_public_key(&self) -> Result<PublicKey<C::Engine>, ParseError> {
        Ok(self.try_load_secret()?.into_public())
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ParseError> {
        let secret = W3fSecretKey::<C::Engine>::from_bytes(bytes)
            .map_err(|err| ParseError(err.to_string()))?;
        Ok(Self::new(&secret))
    }

    fn sign_bytes(&self, message: &[u8]) -> Result<Vec<u8>, Error> {
        self.try_sign_bytes(message)
    }

    fn try_sign_bytes(&self, message: &[u8]) -> Result<Vec<u8>, Error> {
        let mut guard = self
            .try_load_secret()
            .map_err(|err| Error::Signing(err.to_string()))?;
        let msg = w3f_bls::Message::new(MESSAGE_CONTEXT, message);
        #[cfg(feature = "rand")]
        {
            Ok(guard.sign(&msg, os_rng()).to_bytes())
        }
        #[cfg(not(feature = "rand"))]
        {
            Ok(guard.sign_once(&msg).to_bytes())
        }
    }

    #[cfg(test)]
    pub(crate) fn from_unchecked_bytes_for_test(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            _marker: PhantomData,
        }
    }
}

impl<C: BlsConfiguration + ?Sized> zeroize::Zeroize for ManagedSecretKey<C> {
    fn zeroize(&mut self) {
        let mut zero_seed = vec![0u8; C::Engine::SECRET_KEY_SIZE];
        let new_secret = W3fSecretKey::<C::Engine>::from_seed(&zero_seed);
        zero_seed.zeroize();
        self.bytes = new_secret.into_vartime().to_bytes();
    }
}

#[cfg(feature = "rand")]
use crate::rng::os_rng;
use crate::{Algorithm, Error, KeyGenOption, ParseError};

fn ensure_distinct_messages(messages: &[&[u8]]) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for &msg in messages {
        if !seen.insert(msg) {
            return Err(Error::BadSignature);
        }
    }
    Ok(())
}

pub trait BlsConfiguration {
    const ALGORITHM: Algorithm;
    type Engine: w3f_bls::EngineBLS;
}

pub struct BlsImpl<C: BlsConfiguration + ?Sized>(PhantomData<C>);

impl<C: BlsConfiguration + ?Sized> BlsImpl<C> {
    // the names are from an RFC, not a good idea to change them
    #[allow(clippy::similar_names)]
    pub fn keypair(
        option: KeyGenOption<ManagedSecretKey<C>>,
    ) -> Result<(PublicKey<C::Engine>, ManagedSecretKey<C>), Error> {
        Self::try_keypair(option)
    }

    #[allow(clippy::similar_names)]
    pub fn try_keypair(
        mut option: KeyGenOption<ManagedSecretKey<C>>,
    ) -> Result<(PublicKey<C::Engine>, ManagedSecretKey<C>), Error> {
        let private_key = match option {
            #[cfg(feature = "rand")]
            KeyGenOption::Random => {
                let mut rng = os_rng();
                let secret_vt = SecretKeyVT::<C::Engine>::generate(&mut rng);
                let secret = secret_vt.into_split(&mut rng);
                ManagedSecretKey::new(&secret)
            }
            KeyGenOption::UseSeed(ref mut seed) => {
                let salt = b"BLS-SIG-KEYGEN-SALT-";
                let secret_key_size = u8::try_from(C::Engine::SECRET_KEY_SIZE)
                    .map_err(|_| Error::KeyGen("BLS secret-key size overflow".into()))?;
                let info = [0u8, secret_key_size];
                let mut ikm = vec![0u8; seed.len() + 1];
                ikm[..seed.len()].copy_from_slice(seed);
                seed.zeroize();
                let mut okm = vec![0u8; C::Engine::SECRET_KEY_SIZE];
                let h = hkdf::Hkdf::<Sha256>::new(Some(&salt[..]), &ikm);
                h.expand(&info[..], &mut okm)
                    .map_err(|_| Error::KeyGen("BLS HKDF seed expansion failed".into()))?;
                ikm.zeroize();

                let deterministic_rng = crate::rng::rng_from_seed(okm.clone());
                let secret =
                    SecretKeyVT::<C::Engine>::from_seed(&okm).into_split(deterministic_rng);
                okm.zeroize();
                ManagedSecretKey::new(&secret)
            }
            KeyGenOption::FromPrivateKey(key) => key,
        };
        let public_key = private_key
            .try_public_key()
            .map_err(|err| Error::KeyGen(err.to_string()))?;
        Ok((public_key, private_key))
    }

    pub fn sign(message: &[u8], sk: &ManagedSecretKey<C>) -> Result<Vec<u8>, Error> {
        sk.sign_bytes(message)
    }

    pub fn try_sign(message: &[u8], sk: &ManagedSecretKey<C>) -> Result<Vec<u8>, Error> {
        Self::sign(message, sk)
    }

    pub fn derive_public_key(sk: &ManagedSecretKey<C>) -> Result<PublicKey<C::Engine>, ParseError> {
        sk.try_public_key()
    }

    pub fn verify(
        message: &[u8],
        signature_bytes: &[u8],
        pk: &PublicKey<C::Engine>,
    ) -> Result<(), Error>
    where
        C: PreparedPublicKeyCacheAccess,
    {
        let pk_bytes = pk.to_bytes();
        let cache_key = verify_ok_cache_key(&pk_bytes, message, signature_bytes);
        if C::with_verify_ok_cache(|cache| cache.contains(&cache_key)) {
            return Ok(());
        }
        let identity_pk = PublicKey::<C::Engine>(Default::default());
        if pk_bytes == identity_pk.to_bytes() {
            return Err(ParseError("BLS public key is identity".to_string()).into());
        }

        let signature = w3f_bls::Signature::<C::Engine>::from_bytes(signature_bytes)
            .map_err(|_| ParseError("Failed to parse signature.".to_owned()))?;
        let canonical = signature.to_bytes();
        if canonical.as_slice() != signature_bytes {
            return Err(ParseError("non-canonical BLS signature encoding".to_string()).into());
        }
        let identity_sig = BlsSignature::<C::Engine>(Default::default()).to_bytes();
        if canonical == identity_sig {
            return Err(ParseError("BLS signature is identity".to_string()).into());
        }

        let message = w3f_bls::Message::new(MESSAGE_CONTEXT, message);
        let prepared_pk = C::with_cache(|cache| cache.get_or_insert(pk, &pk_bytes));
        let prepared_message = <C::Engine as EngineBLS>::prepare_signature(
            message.hash_to_signature_curve::<C::Engine>(),
        );
        let prepared_signature = <C::Engine as EngineBLS>::prepare_signature(signature.0);

        if !<C::Engine as EngineBLS>::verify_prepared(
            prepared_signature,
            &[(prepared_pk, prepared_message)],
        ) {
            return Err(Error::BadSignature);
        }

        C::with_verify_ok_cache(|cache| cache.insert(cache_key));
        Ok(())
    }

    /// Aggregate-style verification for the case where all signers signed the same message.
    /// Performs deterministic aggregate verification for the case where all signers share the
    /// same message. When the optimized multi-pairing backend is unavailable this falls back to
    /// w3f's POP-aware aggregator, so callers still pay only a single pairing check.
    /// Rejects aggregates whose combined signature or public key is the identity element.
    pub fn verify_aggregate_same_message(
        message: &[u8],
        signatures: &[&[u8]],
        public_keys: &[&[u8]],
    ) -> Result<(), Error> {
        use core::ops::AddAssign as _;
        if signatures.is_empty() || signatures.len() != public_keys.len() {
            return Err(Error::BadSignature);
        }
        let identity_sig = BlsSignature::<C::Engine>(Default::default()).to_bytes();
        let identity_pk = PublicKey::<C::Engine>(Default::default()).to_bytes();
        let parse_signature = |bytes: &[u8]| -> Result<BlsSignature<C::Engine>, Error> {
            let sig = BlsSignature::<C::Engine>::from_bytes(bytes)
                .map_err(|_| ParseError("Failed to parse signature.".to_string()))?;
            let canonical = sig.to_bytes();
            if canonical.as_slice() != bytes {
                return Err(ParseError("non-canonical BLS signature encoding".to_string()).into());
            }
            if canonical == identity_sig {
                return Err(ParseError("BLS signature is identity".to_string()).into());
            }
            Ok(sig)
        };

        // Parse and aggregate signatures
        let mut sig_it = signatures.iter();
        let first_sig_bytes = sig_it.next().ok_or(Error::BadSignature)?;
        let first_sig = parse_signature(first_sig_bytes)?;
        let mut agg_sig_group = first_sig.0;
        for s in sig_it {
            let sig = parse_signature(s)?;
            agg_sig_group.add_assign(&sig.0);
        }
        let agg_sig = BlsSignature::<C::Engine>(agg_sig_group);
        if agg_sig.to_bytes() == identity_sig {
            return Err(Error::BadSignature);
        }

        // Parse and aggregate public keys; enforce unique signers.
        let mut seen_pks: BTreeSet<Vec<u8>> = BTreeSet::new();
        let mut pk_it = public_keys.iter();
        let first_pk_bytes = pk_it.next().ok_or(Error::BadSignature)?;
        let first_pk = Self::parse_public_key(first_pk_bytes)?;
        if !seen_pks.insert(first_pk.to_bytes()) {
            return Err(Error::BadSignature);
        }
        let mut agg_pk_group = first_pk.0;
        for pk in pk_it {
            let pk = Self::parse_public_key(pk)?;
            if !seen_pks.insert(pk.to_bytes()) {
                return Err(Error::BadSignature);
            }
            agg_pk_group.add_assign(&pk.0);
        }

        let agg_pk = PublicKey::<C::Engine>(agg_pk_group);
        if agg_pk.to_bytes() == identity_pk {
            return Err(Error::BadSignature);
        }
        let message = w3f_bls::Message::new(MESSAGE_CONTEXT, message);
        if !agg_sig.verify(&message, &agg_pk) {
            return Err(Error::BadSignature);
        }
        Ok(())
    }

    /// Aggregate a sequence of BLS signatures (same-message context) into a single signature.
    /// The caller is responsible for ensuring all signatures are valid and belong to the same
    /// scheme/engine variant. Rejects aggregates that cancel to the identity element.
    pub fn aggregate_signatures(signatures: &[&[u8]]) -> Result<Vec<u8>, Error> {
        use core::ops::AddAssign as _;
        if signatures.is_empty() {
            return Err(Error::BadSignature);
        }
        let identity_sig = BlsSignature::<C::Engine>(Default::default()).to_bytes();
        let parse_signature = |bytes: &[u8]| -> Result<BlsSignature<C::Engine>, Error> {
            let sig = BlsSignature::<C::Engine>::from_bytes(bytes)
                .map_err(|_| ParseError("Failed to parse signature.".to_string()))?;
            let canonical = sig.to_bytes();
            if canonical.as_slice() != bytes {
                return Err(ParseError("non-canonical BLS signature encoding".to_string()).into());
            }
            if canonical == identity_sig {
                return Err(ParseError("BLS signature is identity".to_string()).into());
            }
            Ok(sig)
        };
        let mut sig_it = signatures.iter();
        let first_sig_bytes = sig_it.next().ok_or(Error::BadSignature)?;
        let first_sig = parse_signature(first_sig_bytes)?;
        let mut agg_sig_group = first_sig.0;
        for s in sig_it {
            let sig = parse_signature(s)?;
            agg_sig_group.add_assign(&sig.0);
        }
        let agg_sig = BlsSignature::<C::Engine>(agg_sig_group);
        let agg_sig_bytes = agg_sig.to_bytes();
        if agg_sig_bytes == identity_sig {
            return Err(Error::BadSignature);
        }
        Ok(agg_sig_bytes)
    }

    /// Verify a pre-aggregated signature for the case where all signers signed the
    /// same message. Public keys are aggregated inside this function and a single pairing
    /// check is performed.
    pub fn verify_preaggregated_same_message(
        message: &[u8],
        aggregated_signature: &[u8],
        public_keys: &[&[u8]],
    ) -> Result<(), Error> {
        use core::ops::AddAssign as _;
        if public_keys.is_empty() {
            return Err(Error::BadSignature);
        }
        let sig = BlsSignature::<C::Engine>::from_bytes(aggregated_signature)
            .map_err(|_| ParseError("Failed to parse signature.".to_string()))?;
        let canonical = sig.to_bytes();
        if canonical.as_slice() != aggregated_signature {
            return Err(ParseError("non-canonical BLS signature encoding".to_string()).into());
        }
        let identity_sig = BlsSignature::<C::Engine>(Default::default()).to_bytes();
        if canonical == identity_sig {
            return Err(ParseError("BLS signature is identity".to_string()).into());
        }
        let identity_pk = PublicKey::<C::Engine>(Default::default()).to_bytes();
        // Aggregate public keys; enforce unique signers.
        let mut seen_pks: BTreeSet<Vec<u8>> = BTreeSet::new();
        let mut pk_it = public_keys.iter();
        let first_pk_bytes = pk_it.next().ok_or(Error::BadSignature)?;
        let first_pk = Self::parse_public_key(first_pk_bytes)?;
        if !seen_pks.insert(first_pk.to_bytes()) {
            return Err(Error::BadSignature);
        }
        let mut agg_pk_group = first_pk.0;
        for pk in pk_it {
            let pk = Self::parse_public_key(pk)?;
            if !seen_pks.insert(pk.to_bytes()) {
                return Err(Error::BadSignature);
            }
            agg_pk_group.add_assign(&pk.0);
        }
        let agg_pk = PublicKey::<C::Engine>(agg_pk_group);
        if agg_pk.to_bytes() == identity_pk {
            return Err(Error::BadSignature);
        }
        let message = w3f_bls::Message::new(MESSAGE_CONTEXT, message);
        if !sig.verify(&message, &agg_pk) {
            return Err(Error::BadSignature);
        }
        Ok(())
    }

    pub fn parse_public_key(payload: &[u8]) -> Result<PublicKey<C::Engine>, ParseError> {
        let key = PublicKey::from_bytes(payload).map_err(|err| ParseError(err.to_string()))?;
        let canonical = key.to_bytes();
        if canonical.as_slice() != payload {
            return Err(ParseError(
                "non-canonical BLS public key encoding".to_string(),
            ));
        }
        let identity = PublicKey::<C::Engine>(Default::default());
        if canonical == identity.to_bytes() {
            return Err(ParseError("BLS public key is identity".to_string()));
        }
        Ok(key)
    }

    pub fn parse_private_key(payload: &[u8]) -> Result<ManagedSecretKey<C>, ParseError> {
        let key = ManagedSecretKey::from_bytes(payload)?;
        let identity = PublicKey::<C::Engine>(Default::default());
        if key.try_public_key()?.to_bytes() == identity.to_bytes() {
            return Err(ParseError("BLS secret key is zero".to_string()));
        }
        Ok(key)
    }
}

impl<C: BlsConfiguration + ?Sized> BlsImpl<C> {
    /// Aggregate verification across distinct messages using the w3f verifier so hashing and
    /// domain separation match signatures produced by this backend.
    pub fn verify_aggregate_multi_message(
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

        let identity_sig = BlsSignature::<C::Engine>(Default::default()).to_bytes();
        let parse_signature = |bytes: &[u8]| -> Result<BlsSignature<C::Engine>, Error> {
            let sig = BlsSignature::<C::Engine>::from_bytes(bytes)
                .map_err(|_| ParseError("Failed to parse signature.".to_owned()))?;
            let canonical = sig.to_bytes();
            if canonical.as_slice() != bytes {
                return Err(ParseError("non-canonical BLS signature encoding".to_string()).into());
            }
            if canonical == identity_sig {
                return Err(ParseError("BLS signature is identity".to_string()).into());
            }
            Ok(sig)
        };
        let mut aggregated_group = <C::Engine as EngineBLS>::SignatureGroup::default();
        let mut decoded_messages = Vec::with_capacity(messages.len());
        let mut decoded_public_keys = Vec::with_capacity(messages.len());

        for ((message, signature_bytes), public_key_bytes) in messages
            .iter()
            .zip(signatures.iter())
            .zip(public_keys.iter())
        {
            let signature = parse_signature(signature_bytes)?;
            aggregated_group += signature.0;

            let public_key = Self::parse_public_key(public_key_bytes)?;
            decoded_public_keys.push(public_key);
            decoded_messages.push(w3f_bls::Message::new(MESSAGE_CONTEXT, message));
        }

        let aggregated_signature = BlsSignature(aggregated_group);
        if aggregated_signature.to_bytes() == identity_sig {
            return Err(Error::BadSignature);
        }

        let batch = MultiMessageBatch {
            signature: aggregated_signature,
            messages: decoded_messages,
            public_keys: decoded_public_keys,
        };

        if w3f_bls::verifiers::verify_with_distinct_messages(&batch, false) {
            Ok(())
        } else {
            Err(Error::BadSignature)
        }
    }
}

struct MultiMessageBatch<E: EngineBLS> {
    signature: BlsSignature<E>,
    messages: Vec<w3f_bls::Message>,
    public_keys: Vec<PublicKey<E>>,
}

impl<'a, E: EngineBLS> w3f_bls::Signed for &'a MultiMessageBatch<E> {
    type E = E;
    type M = &'a w3f_bls::Message;
    type PKG = &'a PublicKey<E>;
    type PKnM =
        std::iter::Zip<std::slice::Iter<'a, w3f_bls::Message>, std::slice::Iter<'a, PublicKey<E>>>;

    fn signature(&self) -> BlsSignature<E> {
        BlsSignature(self.signature.0)
    }

    fn messages_and_publickeys(self) -> Self::PKnM {
        self.messages.iter().zip(self.public_keys.iter())
    }
}
