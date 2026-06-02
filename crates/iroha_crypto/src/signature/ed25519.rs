use core::convert::{Infallible, TryFrom};

use blake2::Blake2bVar;
use curve25519_dalek::edwards::CompressedEdwardsY;
use ed25519_dalek::Signature;
use sha2::{Digest, Sha256};
use signature::Signer as _;
use zeroize::Zeroize;

#[cfg(feature = "rand")]
use crate::rng::os_rng;
use crate::{Error, KeyGenOption, ParseError};

pub type PublicKey = ed25519_dalek::VerifyingKey;
pub type PrivateKey = ed25519_dalek::SigningKey;

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    format,
    string::ToString as _,
    vec::Vec,
};

const VERIFY_OK_CACHE_LIMIT: usize = 8192;
const VERIFY_OK_EXACT_CACHE_SIZE: usize = 65536;
const VERIFY_OK_MAP_INITIAL_CAPACITY: usize = VERIFY_OK_CACHE_LIMIT;
const PUBLIC_KEY_PARSE_CACHE_LIMIT: usize = 32768;
const PUBLIC_KEY_PARSE_FAST_CACHE_SIZE: usize = 16384;
const PUBLIC_KEY_PARSE_MAP_INITIAL_CAPACITY: usize = 8192;

#[inline]
fn masked_cache_index(mixed: u64, cache_size: usize) -> usize {
    debug_assert!(cache_size.is_power_of_two());
    let mask = u64::try_from(cache_size - 1).expect("cache mask fits in u64");
    usize::try_from(mixed & mask).expect("masked cache index fits in usize")
}

#[derive(Clone)]
enum PublicKeyParseOutcome {
    Valid(Box<PublicKey>),
    Invalid(ParseError),
}

impl PublicKeyParseOutcome {
    fn valid(key: PublicKey) -> Self {
        Self::Valid(Box::new(key))
    }

    fn invalid(error: ParseError) -> Self {
        Self::Invalid(error)
    }

    fn as_result(&self) -> Result<PublicKey, ParseError> {
        match self {
            Self::Valid(key) => Ok(**key),
            Self::Invalid(error) => Err(error.clone()),
        }
    }
}

#[derive(Clone)]
struct PublicKeyParseEntry {
    bytes: [u8; 32],
    outcome: PublicKeyParseOutcome,
}

struct PublicKeyParseCache {
    fast: Box<[Option<PublicKeyParseEntry>]>,
    map: HashMap<[u8; 32], PublicKeyParseOutcome>,
    #[cfg(test)]
    hits: usize,
    #[cfg(test)]
    misses: usize,
    #[cfg(test)]
    inserts: usize,
}

impl PublicKeyParseCache {
    fn new() -> Self {
        Self {
            fast: vec![None; PUBLIC_KEY_PARSE_FAST_CACHE_SIZE].into_boxed_slice(),
            map: HashMap::with_capacity(PUBLIC_KEY_PARSE_MAP_INITIAL_CAPACITY),
            #[cfg(test)]
            hits: 0,
            #[cfg(test)]
            misses: 0,
            #[cfg(test)]
            inserts: 0,
        }
    }

    fn get(&mut self, bytes: &[u8; 32]) -> Option<Result<PublicKey, ParseError>> {
        let slot = public_key_parse_fast_index(bytes);
        if let Some(entry) = &self.fast[slot]
            && entry.bytes == *bytes
        {
            #[cfg(test)]
            {
                self.hits = self.hits.saturating_add(1);
            }
            return Some(entry.outcome.as_result());
        }

        let outcome = self.map.get(bytes).cloned();
        if let Some(outcome) = &outcome {
            self.fast[slot] = Some(PublicKeyParseEntry {
                bytes: *bytes,
                outcome: outcome.clone(),
            });
        }
        #[cfg(test)]
        {
            if outcome.is_some() {
                self.hits = self.hits.saturating_add(1);
            } else {
                self.misses = self.misses.saturating_add(1);
            }
        }
        outcome.map(|outcome| outcome.as_result())
    }

    fn insert(&mut self, bytes: [u8; 32], outcome: PublicKeyParseOutcome) {
        if self.map.len() >= PUBLIC_KEY_PARSE_CACHE_LIMIT {
            self.map.clear();
            self.fast.fill(None);
        }
        self.fast[public_key_parse_fast_index(&bytes)] = Some(PublicKeyParseEntry {
            bytes,
            outcome: outcome.clone(),
        });
        self.map.insert(bytes, outcome);
        #[cfg(test)]
        {
            self.inserts = self.inserts.saturating_add(1);
        }
    }

    #[cfg(test)]
    fn reset(&mut self) {
        self.fast.fill(None);
        self.map.clear();
        self.hits = 0;
        self.misses = 0;
        self.inserts = 0;
    }

    #[cfg(test)]
    fn stats(&self) -> PublicKeyParseCacheStats {
        PublicKeyParseCacheStats {
            hits: self.hits,
            misses: self.misses,
            inserts: self.inserts,
        }
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct PublicKeyParseCacheStats {
    hits: usize,
    misses: usize,
    inserts: usize,
}

#[inline]
fn public_key_parse_fast_index(bytes: &[u8; 32]) -> usize {
    let a = u64::from_le_bytes(bytes[0..8].try_into().expect("slice length checked"));
    let b = u64::from_le_bytes(bytes[8..16].try_into().expect("slice length checked"));
    let c = u64::from_le_bytes(bytes[16..24].try_into().expect("slice length checked"));
    let d = u64::from_le_bytes(bytes[24..32].try_into().expect("slice length checked"));
    let mixed = a ^ b.rotate_left(17) ^ c.rotate_left(31) ^ d.rotate_left(47);
    masked_cache_index(mixed, PUBLIC_KEY_PARSE_FAST_CACHE_SIZE)
}

#[derive(Clone, Copy)]
struct VerifyOkExactEntry {
    pk: [u8; 32],
    message: [u8; 32],
    signature: [u8; 64],
}

struct VerifyOkCache {
    exact: Box<[Option<VerifyOkExactEntry>]>,
    map: Option<HashSet<[u8; 32]>>,
}

impl VerifyOkCache {
    fn new() -> Self {
        Self {
            exact: vec![None; VERIFY_OK_EXACT_CACHE_SIZE].into_boxed_slice(),
            map: None,
        }
    }

    fn contains_exact_32(&self, pk: &PublicKey, message: &[u8], signature: &[u8]) -> bool {
        let Some(key) = exact_verify_key(pk, message, signature) else {
            return false;
        };
        let Some(entry) = self.exact[verify_ok_exact_index(&key.pk, &key.message, &key.signature)]
        else {
            return false;
        };
        entry.pk == key.pk && entry.message == key.message && entry.signature == key.signature
    }

    fn insert_exact_32(&mut self, pk: &PublicKey, message: &[u8], signature: &[u8]) -> bool {
        let Some(entry) = exact_verify_key(pk, message, signature) else {
            return false;
        };
        let slot = verify_ok_exact_index(&entry.pk, &entry.message, &entry.signature);
        self.exact[slot] = Some(entry);
        true
    }

    fn contains(&self, key: &[u8; 32]) -> bool {
        self.map.as_ref().is_some_and(|cache| cache.contains(key))
    }

    fn insert(&mut self, key: [u8; 32]) {
        let cache = self
            .map
            .get_or_insert_with(|| HashSet::with_capacity(VERIFY_OK_MAP_INITIAL_CAPACITY));
        if cache.len() >= VERIFY_OK_CACHE_LIMIT {
            // Simple bounded cache: clear rather than paying LRU bookkeeping cost.
            cache.clear();
        }
        cache.insert(key);
    }

    #[cfg(test)]
    fn general_cache_allocated(&self) -> bool {
        self.map.is_some()
    }
}

#[inline]
fn exact_verify_key(
    pk: &PublicKey,
    message: &[u8],
    signature: &[u8],
) -> Option<VerifyOkExactEntry> {
    let message: [u8; 32] = message.try_into().ok()?;
    let signature: [u8; 64] = signature.try_into().ok()?;
    Some(VerifyOkExactEntry {
        pk: pk.to_bytes(),
        message,
        signature,
    })
}

#[inline]
fn verify_ok_exact_index(pk: &[u8; 32], message: &[u8; 32], signature: &[u8; 64]) -> usize {
    let pk_a = u64::from_le_bytes(pk[0..8].try_into().expect("slice length checked"));
    let pk_b = u64::from_le_bytes(pk[24..32].try_into().expect("slice length checked"));
    let msg_a = u64::from_le_bytes(message[0..8].try_into().expect("slice length checked"));
    let msg_b = u64::from_le_bytes(message[24..32].try_into().expect("slice length checked"));
    let sig_a = u64::from_le_bytes(signature[0..8].try_into().expect("slice length checked"));
    let sig_b = u64::from_le_bytes(signature[24..32].try_into().expect("slice length checked"));
    let sig_c = u64::from_le_bytes(signature[56..64].try_into().expect("slice length checked"));
    let mixed = pk_a
        ^ pk_b.rotate_left(7)
        ^ msg_a.rotate_left(19)
        ^ msg_b.rotate_left(29)
        ^ sig_a.rotate_left(41)
        ^ sig_b.rotate_left(53)
        ^ sig_c.rotate_left(61);
    masked_cache_index(mixed, VERIFY_OK_EXACT_CACHE_SIZE)
}

thread_local! {
    static PUBLIC_KEY_PARSE_CACHE: RefCell<PublicKeyParseCache> = RefCell::new(PublicKeyParseCache::new());
    static VERIFY_OK_CACHE: RefCell<VerifyOkCache> = RefCell::new(VerifyOkCache::new());
    #[cfg(test)]
    static VERIFY_OK_CACHE_KEY_CALLS: RefCell<usize> = const { RefCell::new(0) };
    #[cfg(test)]
    static ED25519_SIGNATURE_PARSE_CALLS: RefCell<usize> = const { RefCell::new(0) };
    #[cfg(test)]
    static ED25519_UNCACHED_BATCH_VERIFY_CALLS: RefCell<usize> = const { RefCell::new(0) };
}

#[cfg(test)]
fn reset_public_key_parse_cache_for_tests() {
    PUBLIC_KEY_PARSE_CACHE.with(|cache| cache.borrow_mut().reset());
}

#[cfg(test)]
fn public_key_parse_cache_stats_for_tests() -> PublicKeyParseCacheStats {
    PUBLIC_KEY_PARSE_CACHE.with(|cache| cache.borrow().stats())
}

#[cfg(test)]
fn reset_verify_ok_cache_for_tests() {
    VERIFY_OK_CACHE.with(|cache| *cache.borrow_mut() = VerifyOkCache::new());
    VERIFY_OK_CACHE_KEY_CALLS.with(|calls| *calls.borrow_mut() = 0);
}

#[cfg(test)]
fn verify_ok_cache_key_calls_for_tests() -> usize {
    VERIFY_OK_CACHE_KEY_CALLS.with(|calls| *calls.borrow())
}

#[cfg(test)]
fn verify_ok_general_cache_allocated_for_tests() -> bool {
    VERIFY_OK_CACHE.with(|cache| cache.borrow().general_cache_allocated())
}

#[cfg(test)]
fn reset_batch_cache_counters_for_tests() {
    ED25519_SIGNATURE_PARSE_CALLS.with(|calls| *calls.borrow_mut() = 0);
    ED25519_UNCACHED_BATCH_VERIFY_CALLS.with(|calls| *calls.borrow_mut() = 0);
}

#[cfg(test)]
fn signature_parse_calls_for_tests() -> usize {
    ED25519_SIGNATURE_PARSE_CALLS.with(|calls| *calls.borrow())
}

#[cfg(test)]
fn uncached_batch_verify_calls_for_tests() -> usize {
    ED25519_UNCACHED_BATCH_VERIFY_CALLS.with(|calls| *calls.borrow())
}

fn verify_ok_cache_key(pk: &PublicKey, message: &[u8], signature: &[u8]) -> [u8; 32] {
    #[cfg(test)]
    VERIFY_OK_CACHE_KEY_CALLS.with(|calls| {
        let mut calls = calls.borrow_mut();
        *calls = (*calls).saturating_add(1);
    });

    let pk_bytes = pk.to_bytes();
    let mut h = <Blake2bVar as blake2::digest::VariableOutput>::new(32)
        .expect("blake2b init for signature verify cache");
    blake2::digest::Update::update(&mut h, b"iroha:ed25519:verify_ok_cache:v1");
    blake2::digest::Update::update(&mut h, &pk_bytes);
    blake2::digest::Update::update(&mut h, message);
    blake2::digest::Update::update(&mut h, signature);
    let mut out = [0u8; 32];
    blake2::digest::VariableOutput::finalize_variable(h, &mut out)
        .expect("blake2b output length must match");
    out
}

fn remember_verify_ok(pk: &PublicKey, message: &[u8], signature: &[u8]) {
    VERIFY_OK_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if !cache.insert_exact_32(pk, message, signature) {
            cache.insert(verify_ok_cache_key(pk, message, signature));
        }
    });
}

pub(crate) fn is_verify_ok_cached(pk: &PublicKey, message: &[u8], signature: &[u8]) -> bool {
    if signature.len() != ed25519_dalek::SIGNATURE_LENGTH {
        return false;
    }
    VERIFY_OK_CACHE.with(|cache| {
        let cache = cache.borrow();
        if message.len() == 32 {
            return cache.contains_exact_32(pk, message, signature);
        }
        cache.contains(&verify_ok_cache_key(pk, message, signature))
    })
}

fn parse_fixed_size<T, E, F, const SIZE: usize>(
    payload: &[u8],
    fixed_parser: F,
) -> Result<T, ParseError>
where
    F: FnOnce(&[u8; SIZE]) -> Result<T, E>,
    E: core::fmt::Display,
{
    let fixed_payload: [u8; SIZE] = payload.try_into().map_err(|_| {
        ParseError(format!(
            "the payload size is incorrect: expected {}, but got {}",
            SIZE,
            payload.len()
        ))
    })?;

    fixed_parser(&fixed_payload).map_err(|err| ParseError(err.to_string()))
}

fn ed25519_seed_from_material(seed: &[u8]) -> [u8; 32] {
    if seed.len() == 32 {
        let mut out = [0u8; 32];
        out.copy_from_slice(seed);
        return out;
    }

    let digest = Sha256::digest(seed);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

#[derive(Debug, Clone, Copy)]
pub struct Ed25519Sha512;

impl Ed25519Sha512 {
    pub fn keypair(option: KeyGenOption<PrivateKey>) -> (PublicKey, PrivateKey) {
        let signing_key = match option {
            #[cfg(feature = "rand")]
            KeyGenOption::Random => {
                let mut rng = os_rng();
                PrivateKey::generate(&mut rng)
            }
            KeyGenOption::UseSeed(mut seed) => {
                let seed_bytes = ed25519_seed_from_material(&seed);
                seed.zeroize();
                PrivateKey::from_bytes(&seed_bytes)
            }
            KeyGenOption::FromPrivateKey(ref s) => PrivateKey::clone(s),
        };
        (signing_key.verifying_key(), signing_key)
    }

    pub fn parse_public_key(payload: &[u8]) -> Result<PublicKey, ParseError> {
        let bytes: [u8; 32] = payload.try_into().map_err(|_| {
            ParseError(format!(
                "the payload size is incorrect: expected {}, but got {}",
                32,
                payload.len()
            ))
        })?;

        if let Some(result) = PUBLIC_KEY_PARSE_CACHE.with(|cache| cache.borrow_mut().get(&bytes)) {
            return result;
        }

        let result = Self::parse_public_key_uncached(&bytes);
        let outcome = match &result {
            Ok(key) => PublicKeyParseOutcome::valid(*key),
            Err(err) => PublicKeyParseOutcome::invalid(err.clone()),
        };
        PUBLIC_KEY_PARSE_CACHE.with(|cache| cache.borrow_mut().insert(bytes, outcome));
        result
    }

    fn parse_public_key_uncached(bytes: &[u8; 32]) -> Result<PublicKey, ParseError> {
        let compressed = CompressedEdwardsY(*bytes);
        let point = compressed
            .decompress()
            .ok_or_else(|| ParseError("invalid ed25519 public key encoding".to_string()))?;
        let canonical = point.compress();

        // Reject non-canonical encodings (ZIP-215 allows them, but our ABI requires canonical
        // byte representation to keep deterministic I105/in-memory forms in sync).
        if canonical.as_bytes() != bytes {
            return Err(ParseError(
                "non-canonical ed25519 public key encoding".to_string(),
            ));
        }

        let key = PublicKey::from(point);

        if key.is_weak() {
            return Err(ParseError(
                "ed25519 public key is small-order (weak); rejected".to_string(),
            ));
        }

        Ok(key)
    }

    pub fn parse_private_key(payload: &[u8]) -> Result<PrivateKey, ParseError> {
        match payload.len() {
            32 => parse_fixed_size(payload, |bytes| {
                Ok::<_, Infallible>(PrivateKey::from_bytes(bytes))
            }),
            64 => {
                let mut seed = [0u8; 32];
                seed.copy_from_slice(&payload[..32]);
                let mut public = [0u8; 32];
                public.copy_from_slice(&payload[32..]);
                let signing_key = PrivateKey::from_bytes(&seed);
                if signing_key.verifying_key().to_bytes() != public {
                    return Err(ParseError(
                        "ed25519 private key payload has mismatched public key".to_string(),
                    ));
                }
                seed.zeroize();
                Ok(signing_key)
            }
            len => Err(ParseError(format!(
                "the payload size is incorrect: expected 32 or 64, but got {len}"
            ))),
        }
    }

    pub fn sign(message: &[u8], sk: &PrivateKey) -> Vec<u8> {
        sk.sign(message).to_bytes().to_vec()
    }

    pub fn verify(message: &[u8], signature: &[u8], pk: &PublicKey) -> Result<(), Error> {
        if signature.len() == ed25519_dalek::SIGNATURE_LENGTH {
            if message.len() == 32 {
                if VERIFY_OK_CACHE
                    .with(|cache| cache.borrow().contains_exact_32(pk, message, signature))
                {
                    return Ok(());
                }
            } else {
                let key = verify_ok_cache_key(pk, message, signature);
                if VERIFY_OK_CACHE.with(|cache| cache.borrow().contains(&key)) {
                    return Ok(());
                }
            }
            // `Signature::try_from` only checks length for Ed25519; we already know it's correct.
            let s = Signature::try_from(signature).map_err(|e| ParseError(e.to_string()))?;
            pk.verify_strict(message, &s)
                .map_err(|_| Error::BadSignature)?;
            remember_verify_ok(pk, message, signature);
            return Ok(());
        }
        let s = Signature::try_from(signature).map_err(|e| ParseError(e.to_string()))?;
        pk.verify_strict(message, &s)
            .map_err(|_| Error::BadSignature)
    }

    /// Deterministic batch verification helper using already parsed public keys.
    ///
    /// Under `ecc-batch`, this calls dalek's transcript-derived deterministic batch verifier.
    /// Without `ecc-batch`, it verifies each tuple independently in input order.
    /// The `seed32` parameter is reserved for API compatibility and is ignored.
    pub fn verify_batch_preparsed_deterministic(
        messages: &[&[u8]],
        signatures: &[&[u8]],
        public_keys: &[PublicKey],
        seed32: [u8; 32],
    ) -> Result<(), Error> {
        if messages.is_empty()
            || !(messages.len() == signatures.len() && signatures.len() == public_keys.len())
        {
            return Err(Error::BadSignature);
        }
        let _ = seed32;

        let mut parsed_signatures = Vec::new();
        Self::parse_signatures_into(signatures, &mut parsed_signatures)?;
        Self::verify_batch_preparsed_signatures_deterministic(
            messages,
            signatures,
            &parsed_signatures,
            public_keys,
            seed32,
        )
    }

    pub(crate) fn parse_signatures_into(
        signatures: &[&[u8]],
        out: &mut Vec<Signature>,
    ) -> Result<(), Error> {
        out.clear();
        out.try_reserve(signatures.len())
            .map_err(|_| Error::BadSignature)?;
        for signature in signatures {
            out.push(Self::parse_signature(signature)?);
        }
        Ok(())
    }

    pub(crate) fn parse_signature(signature: &[u8]) -> Result<Signature, Error> {
        #[cfg(test)]
        ED25519_SIGNATURE_PARSE_CALLS.with(|calls| {
            let mut calls = calls.borrow_mut();
            *calls = (*calls).saturating_add(1);
        });
        let parsed = Signature::try_from(signature).map_err(|_| Error::BadSignature)?;
        validate_signature_r_for_strict_batch(signature)?;
        Ok(parsed)
    }

    pub(crate) fn verify_batch_preparsed_signatures_deterministic(
        messages: &[&[u8]],
        raw_signatures: &[&[u8]],
        parsed_signatures: &[Signature],
        public_keys: &[PublicKey],
        seed32: [u8; 32],
    ) -> Result<(), Error> {
        if messages.is_empty()
            || !(messages.len() == raw_signatures.len()
                && raw_signatures.len() == parsed_signatures.len()
                && parsed_signatures.len() == public_keys.len())
        {
            return Err(Error::BadSignature);
        }
        let _ = seed32;

        let first_cached = messages
            .iter()
            .zip(raw_signatures.iter())
            .zip(public_keys.iter())
            .position(|((message, signature), public_key)| {
                is_verify_ok_cached(public_key, message, signature)
            });

        if let Some(first_cached) = first_cached {
            let mut miss_messages = Vec::with_capacity(messages.len().saturating_sub(1));
            let mut miss_raw_signatures =
                Vec::with_capacity(raw_signatures.len().saturating_sub(1));
            let mut miss_parsed_signatures =
                Vec::with_capacity(parsed_signatures.len().saturating_sub(1));
            let mut miss_public_keys = Vec::with_capacity(public_keys.len().saturating_sub(1));

            for idx in 0..first_cached {
                miss_messages.push(messages[idx]);
                miss_raw_signatures.push(raw_signatures[idx]);
                miss_parsed_signatures.push(parsed_signatures[idx]);
                miss_public_keys.push(public_keys[idx]);
            }

            for idx in first_cached.saturating_add(1)..messages.len() {
                if is_verify_ok_cached(&public_keys[idx], messages[idx], raw_signatures[idx]) {
                    continue;
                }
                miss_messages.push(messages[idx]);
                miss_raw_signatures.push(raw_signatures[idx]);
                miss_parsed_signatures.push(parsed_signatures[idx]);
                miss_public_keys.push(public_keys[idx]);
            }

            if miss_messages.is_empty() {
                return Ok(());
            }

            Self::verify_batch_preparsed_signatures_uncached(
                &miss_messages,
                &miss_raw_signatures,
                &miss_parsed_signatures,
                &miss_public_keys,
            )?;
            return Ok(());
        }

        Self::verify_batch_preparsed_signatures_uncached(
            messages,
            raw_signatures,
            parsed_signatures,
            public_keys,
        )
    }

    pub(crate) fn verify_batch_preparsed_signatures_uncached(
        messages: &[&[u8]],
        raw_signatures: &[&[u8]],
        parsed_signatures: &[Signature],
        public_keys: &[PublicKey],
    ) -> Result<(), Error> {
        #[cfg(test)]
        ED25519_UNCACHED_BATCH_VERIFY_CALLS.with(|calls| {
            let mut calls = calls.borrow_mut();
            *calls = (*calls).saturating_add(1);
        });

        #[cfg(feature = "ecc-batch")]
        ed25519_dalek::verify_batch(messages, parsed_signatures, public_keys)
            .map_err(|_| Error::BadSignature)?;

        #[cfg(not(feature = "ecc-batch"))]
        for ((message, signature), public_key) in messages
            .iter()
            .zip(parsed_signatures.iter())
            .zip(public_keys.iter())
        {
            public_key
                .verify_strict(message, signature)
                .map_err(|_| Error::BadSignature)?;
        }

        for ((message, signature), public_key) in messages
            .iter()
            .zip(raw_signatures.iter())
            .zip(public_keys.iter())
        {
            remember_verify_ok(public_key, message, signature);
        }
        Ok(())
    }

    /// Deterministic batch verification helper.
    ///
    /// Parses public keys once, then delegates to [`Self::verify_batch_preparsed_deterministic`].
    /// Under `ecc-batch`, this uses dalek's true deterministic batch verifier.
    /// Without `ecc-batch`, this retains the ordered per-signature fallback.
    /// The `seed32` parameter is reserved for API compatibility and is ignored.
    /// Returns `Err(Error::BadSignature)` when input is empty or lengths mismatch.
    pub fn verify_batch_deterministic(
        messages: &[&[u8]],
        signatures: &[&[u8]],
        public_keys: &[&[u8]],
        seed32: [u8; 32],
    ) -> Result<(), Error> {
        if messages.is_empty()
            || !(messages.len() == signatures.len() && signatures.len() == public_keys.len())
        {
            return Err(Error::BadSignature);
        }
        let parsed_public_keys = public_keys
            .iter()
            .map(|public_key| Self::parse_public_key(public_key).map_err(|_| Error::BadSignature))
            .collect::<Result<Vec<_>, _>>()?;
        Self::verify_batch_preparsed_deterministic(
            messages,
            signatures,
            &parsed_public_keys,
            seed32,
        )
    }
}

fn validate_signature_r_for_strict_batch(signature: &[u8]) -> Result<(), Error> {
    let r_bytes: [u8; 32] = signature
        .get(..32)
        .ok_or(Error::BadSignature)?
        .try_into()
        .map_err(|_| Error::BadSignature)?;
    let r_compressed = CompressedEdwardsY(r_bytes);
    let r_point = r_compressed.decompress().ok_or(Error::BadSignature)?;
    if r_point.is_small_order() || r_point.compress().as_bytes() != &r_bytes {
        return Err(Error::BadSignature);
    }
    Ok(())
}

#[cfg(test)]
mod test {
    #[cfg(feature = "crypto-parity-tests")]
    use openssl::{
        pkey::{Id, PKey, Private, Public},
        sign::{Signer, Verifier as OpenSslVerifier},
    };
    #[cfg(feature = "ecc-batch")]
    use rand::{RngCore, SeedableRng, rngs::StdRng};

    use self::Ed25519Sha512;
    use super::*;
    use crate::{
        Algorithm, Error, KeyGenOption, PrivateKey as CryptoPrivateKey,
        PublicKey as CryptoPublicKey, secrecy::Secret, signature::ed25519,
    };
    use curve25519_dalek::{
        edwards::EdwardsPoint,
        scalar::Scalar,
        traits::{Identity, IsIdentity},
    };
    use ed25519_dalek::Verifier;
    use sha2::{Digest, Sha256, Sha512};

    const MESSAGE_1: &[u8] = b"This is a dummy message for use with tests";
    const SIGNATURE_1: &str = "451b5b8e8725321541954997781de51f4142e4a56bab68d24f6a6b92615de5eefb74134138315859a32c7cf5fe5a488bc545e2e08e5eedfd1fb10188d532d808";
    const PRIVATE_KEY: &str = "1c1179a560d092b90458fe6ab8291215a427fcd6b3927cb240701778ef552019";
    const PUBLIC_KEY: &str = "27c96646f2d4632d4fc241f84cbc427fbc3ecaa95becba55088d6c7b81fc5bbf";

    #[cfg(feature = "crypto-parity-tests")]
    fn openssl_public_key(pk: &ed25519::PublicKey) -> PKey<Public> {
        PKey::public_key_from_raw_bytes(pk.as_bytes(), Id::ED25519).expect("openssl public key")
    }

    #[cfg(feature = "crypto-parity-tests")]
    fn openssl_private_key(sk: &ed25519::PrivateKey) -> PKey<Private> {
        PKey::private_key_from_raw_bytes(&sk.to_bytes(), Id::ED25519).expect("openssl private key")
    }

    fn key_pair_factory() -> (ed25519::PublicKey, ed25519::PrivateKey) {
        Ed25519Sha512::keypair(KeyGenOption::FromPrivateKey(
            Ed25519Sha512::parse_private_key(&hex::decode(PRIVATE_KEY).unwrap()).unwrap(),
        ))
    }

    const ED25519_SMALL_ORDER_POINT: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];

    const ED25519_NON_CANONICAL_IDENTITY: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];
    const ED25519_INVALID_ENCODING: [u8; 32] = [0x02; 32];

    #[test]
    fn create_new_keys() {
        let (p, s) = Ed25519Sha512::keypair(KeyGenOption::Random);

        println!("{s:?}");
        println!("{p:?}");
    }

    #[test]
    fn ed25519_load_keys() {
        let (p1, s1) = key_pair_factory();

        assert_eq!(
            CryptoPrivateKey(Box::new(Secret::new(crate::PrivateKeyInner::Ed25519(s1)))),
            CryptoPrivateKey::from_hex(Algorithm::Ed25519, PRIVATE_KEY).unwrap()
        );
        assert_eq!(
            CryptoPublicKey::new(crate::PublicKeyFull::Ed25519(p1)),
            CryptoPublicKey::from_hex(Algorithm::Ed25519, PUBLIC_KEY).unwrap()
        );
    }

    #[test]
    fn ed25519_verify() {
        let (p, _) = key_pair_factory();

        Ed25519Sha512::verify(MESSAGE_1, hex::decode(SIGNATURE_1).unwrap().as_slice(), &p).unwrap();

        // Check if signatures produced here can be verified by OpenSSL.
        #[cfg(feature = "crypto-parity-tests")]
        {
            let signature = hex::decode(SIGNATURE_1).unwrap();
            let openssl_pk = openssl_public_key(&p);
            let mut verifier = OpenSslVerifier::new_without_digest(&openssl_pk).unwrap();
            assert!(verifier.verify_oneshot(&signature, MESSAGE_1).unwrap());
        }
    }

    #[test]
    fn ed25519_verify_ok_cache_separates_message_and_signature() {
        let (pk, sk) = Ed25519Sha512::keypair(KeyGenOption::Random);
        let msg1 = b"ed25519 verify-ok-cache msg1";
        let msg2 = b"ed25519 verify-ok-cache msg2";
        let sig1 = Ed25519Sha512::sign(msg1, &sk);
        let sig2 = Ed25519Sha512::sign(msg2, &sk);

        Ed25519Sha512::verify(msg1, &sig1, &pk).expect("valid signature 1");
        assert!(
            Ed25519Sha512::verify(msg1, &sig2, &pk).is_err(),
            "cache must not mix distinct signatures"
        );
        Ed25519Sha512::verify(msg2, &sig2, &pk).expect("valid signature 2");
        assert!(
            Ed25519Sha512::verify(msg2, &sig1, &pk).is_err(),
            "cache must not mix distinct messages"
        );

        // Exercise cached hit path.
        Ed25519Sha512::verify(msg1, &sig1, &pk).expect("cached signature 1");
    }

    #[test]
    fn ed25519_verify_ok_cache_skips_blake2_for_transaction_hash_messages() {
        reset_verify_ok_cache_for_tests();
        let (pk, sk) = Ed25519Sha512::keypair(KeyGenOption::UseSeed(vec![0x41; 32]));
        let message = [0x5A; 32];
        let signature = Ed25519Sha512::sign(&message, &sk);

        Ed25519Sha512::verify(&message, &signature, &pk).expect("valid signature");
        assert_eq!(verify_ok_cache_key_calls_for_tests(), 0);
        assert!(
            !verify_ok_general_cache_allocated_for_tests(),
            "32-byte transaction hashes should only use the exact verify cache"
        );

        Ed25519Sha512::verify(&message, &signature, &pk).expect("exact cache hit");
        assert_eq!(verify_ok_cache_key_calls_for_tests(), 0);
        assert!(
            !verify_ok_general_cache_allocated_for_tests(),
            "exact cache hits must not allocate the generic verify cache"
        );
    }

    #[test]
    fn ed25519_verify_ok_cache_keeps_general_message_lookup() {
        reset_verify_ok_cache_for_tests();
        let (pk, sk) = Ed25519Sha512::keypair(KeyGenOption::UseSeed(vec![0x42; 32]));
        let message = b"general ed25519 cache lookup";
        let signature = Ed25519Sha512::sign(message, &sk);

        Ed25519Sha512::verify(message, &signature, &pk).expect("valid signature");
        let after_insert = verify_ok_cache_key_calls_for_tests();
        assert!(after_insert > 0);
        assert!(
            verify_ok_general_cache_allocated_for_tests(),
            "non-32-byte messages should allocate the generic verify cache"
        );

        Ed25519Sha512::verify(message, &signature, &pk).expect("hash cache hit");
        assert!(verify_ok_cache_key_calls_for_tests() > after_insert);
    }

    #[test]
    fn ed25519_cache_indexes_stay_within_cache_masks() {
        let pk = [0xFF; 32];
        let message = [0xA5; 32];
        let signature = [0x5A; 64];

        assert!(public_key_parse_fast_index(&pk) < PUBLIC_KEY_PARSE_FAST_CACHE_SIZE);
        assert!(verify_ok_exact_index(&pk, &message, &signature) < VERIFY_OK_EXACT_CACHE_SIZE);
    }

    #[test]
    fn parse_public_key_uses_thread_local_cache_for_valid_keys() {
        reset_public_key_parse_cache_for_tests();
        let (pk, _) = Ed25519Sha512::keypair(KeyGenOption::Random);
        let bytes = pk.to_bytes();

        let first = Ed25519Sha512::parse_public_key(&bytes).expect("first parse succeeds");
        assert_eq!(first.to_bytes(), bytes);
        assert_eq!(
            public_key_parse_cache_stats_for_tests(),
            PublicKeyParseCacheStats {
                hits: 0,
                misses: 1,
                inserts: 1,
            }
        );

        let second = Ed25519Sha512::parse_public_key(&bytes).expect("cached parse succeeds");
        assert_eq!(second.to_bytes(), bytes);
        assert_eq!(
            public_key_parse_cache_stats_for_tests(),
            PublicKeyParseCacheStats {
                hits: 1,
                misses: 1,
                inserts: 1,
            }
        );
    }

    #[test]
    fn parse_public_key_cache_stores_non_canonical_rejections() {
        reset_public_key_parse_cache_for_tests();

        let first = Ed25519Sha512::parse_public_key(&ED25519_NON_CANONICAL_IDENTITY)
            .expect_err("non-canonical public key must be rejected");
        let second = Ed25519Sha512::parse_public_key(&ED25519_NON_CANONICAL_IDENTITY)
            .expect_err("cached non-canonical public key must be rejected");
        assert_eq!(first, second);
        assert!(
            second.0.contains("non-canonical"),
            "unexpected error: {second:?}"
        );

        assert_eq!(
            public_key_parse_cache_stats_for_tests(),
            PublicKeyParseCacheStats {
                hits: 1,
                misses: 1,
                inserts: 1,
            }
        );
    }

    #[test]
    fn parse_public_key_cache_stores_weak_key_rejections() {
        reset_public_key_parse_cache_for_tests();

        let first = Ed25519Sha512::parse_public_key(&ED25519_SMALL_ORDER_POINT)
            .expect_err("weak public key must be rejected");
        let second = Ed25519Sha512::parse_public_key(&ED25519_SMALL_ORDER_POINT)
            .expect_err("cached weak public key must be rejected");
        assert_eq!(first, second);
        assert!(
            second.0.contains("small-order"),
            "unexpected error: {second:?}"
        );

        assert_eq!(
            public_key_parse_cache_stats_for_tests(),
            PublicKeyParseCacheStats {
                hits: 1,
                misses: 1,
                inserts: 1,
            }
        );
    }

    #[test]
    fn parse_public_key_cache_stores_decompression_rejections() {
        reset_public_key_parse_cache_for_tests();

        let first = Ed25519Sha512::parse_public_key(&ED25519_INVALID_ENCODING)
            .expect_err("invalid public key encoding must be rejected");
        let second = Ed25519Sha512::parse_public_key(&ED25519_INVALID_ENCODING)
            .expect_err("cached invalid public key encoding must be rejected");
        assert_eq!(first, second);
        assert!(
            second.0.contains("invalid ed25519 public key encoding"),
            "unexpected error: {second:?}"
        );

        assert_eq!(
            public_key_parse_cache_stats_for_tests(),
            PublicKeyParseCacheStats {
                hits: 1,
                misses: 1,
                inserts: 1,
            }
        );
    }

    #[test]
    fn parse_public_key_cache_does_not_store_wrong_lengths() {
        reset_public_key_parse_cache_for_tests();

        for _ in 0..2 {
            let err = Ed25519Sha512::parse_public_key(&[])
                .expect_err("wrong-length public key must be rejected");
            assert!(
                err.0.contains("expected 32, but got 0"),
                "unexpected error: {err:?}"
            );
        }

        assert_eq!(
            public_key_parse_cache_stats_for_tests(),
            PublicKeyParseCacheStats::default()
        );
    }

    #[test]
    fn public_key_parse_cache_keeps_izanami_sized_working_set() {
        let mut cache = PublicKeyParseCache::new();

        for idx in 0..20_000u64 {
            let mut bytes = [0u8; 32];
            bytes[..8].copy_from_slice(&idx.to_le_bytes());
            cache.insert(
                bytes,
                PublicKeyParseOutcome::invalid(ParseError("cached rejection".into())),
            );
        }

        assert_eq!(cache.map.len(), 20_000);
        let mut first = [0u8; 32];
        first[..8].copy_from_slice(&0u64.to_le_bytes());
        assert!(cache.get(&first).is_some());
    }

    #[test]
    fn public_key_compact_to_full_ed25519_uses_parse_cache() {
        reset_public_key_parse_cache_for_tests();
        let (pk, _) = Ed25519Sha512::keypair(KeyGenOption::UseSeed(vec![0x6A; 32]));
        let public_key = CryptoPublicKey::new(crate::PublicKeyFull::Ed25519(pk));
        let compact = public_key.0.clone();

        let first = crate::PublicKeyFull::from(&compact);
        match first {
            crate::PublicKeyFull::Ed25519(parsed) => assert_eq!(parsed.to_bytes(), pk.to_bytes()),
            _ => panic!("compact Ed25519 key converted to a non-Ed25519 full key"),
        }
        assert_eq!(
            public_key_parse_cache_stats_for_tests(),
            PublicKeyParseCacheStats {
                hits: 0,
                misses: 1,
                inserts: 1,
            }
        );

        let second = crate::PublicKeyFull::from(&compact);
        match second {
            crate::PublicKeyFull::Ed25519(parsed) => assert_eq!(parsed.to_bytes(), pk.to_bytes()),
            _ => panic!("compact Ed25519 key converted to a non-Ed25519 full key"),
        }
        assert_eq!(
            public_key_parse_cache_stats_for_tests(),
            PublicKeyParseCacheStats {
                hits: 1,
                misses: 1,
                inserts: 1,
            }
        );
    }

    #[cfg(feature = "ecc-batch")]
    #[test]
    fn deterministic_batch_verification_respects_seed_and_order() {
        let mut rng = StdRng::seed_from_u64(0x0BAD_5EED);
        let mut triples: Vec<(Vec<u8>, Vec<u8>, Vec<u8>)> = Vec::new();

        for idx in 0..4u8 {
            let label = format!("batch-message-{idx}");
            let mut seed = [0u8; 32];
            rng.fill_bytes(&mut seed);
            let (pk, sk) = Ed25519Sha512::keypair(KeyGenOption::UseSeed(seed.to_vec()));
            let sig = Ed25519Sha512::sign(label.as_bytes(), &sk);
            triples.push((label.into_bytes(), sig, pk.to_bytes().to_vec()));
        }

        let msg_refs: Vec<&[u8]> = triples.iter().map(|(m, _, _)| m.as_slice()).collect();
        let sig_refs: Vec<&[u8]> = triples.iter().map(|(_, s, _)| s.as_slice()).collect();
        let pk_refs: Vec<&[u8]> = triples.iter().map(|(_, _, p)| p.as_slice()).collect();

        // Baseline passes for any deterministic seed.
        Ed25519Sha512::verify_batch_deterministic(&msg_refs, &sig_refs, &pk_refs, [0xA5; 32])
            .expect("baseline batch verification");

        // Order should not affect outcome because verification is per-signature.
        triples.reverse();
        let msgs_rev: Vec<&[u8]> = triples.iter().map(|(m, _, _)| m.as_slice()).collect();
        let sigs_rev: Vec<&[u8]> = triples.iter().map(|(_, s, _)| s.as_slice()).collect();
        let pks_rev: Vec<&[u8]> = triples.iter().map(|(_, _, p)| p.as_slice()).collect();
        Ed25519Sha512::verify_batch_deterministic(
            msgs_rev.as_slice(),
            sigs_rev.as_slice(),
            pks_rev.as_slice(),
            [0x5A; 32],
        )
        .expect("reordered batch verification");

        // Tampering any signature must fail deterministically for every seed.
        let mut tampered = triples.clone();
        tampered[1].1[0] ^= 0x55;

        let err = Ed25519Sha512::verify_batch_deterministic(
            tampered
                .iter()
                .map(|(m, _, _)| m.as_slice())
                .collect::<Vec<_>>()
                .as_slice(),
            tampered
                .iter()
                .map(|(_, s, _)| s.as_slice())
                .collect::<Vec<_>>()
                .as_slice(),
            tampered
                .iter()
                .map(|(_, _, p)| p.as_slice())
                .collect::<Vec<_>>()
                .as_slice(),
            [0x01; 32],
        );
        assert!(matches!(err, Err(Error::BadSignature)));
    }

    #[test]
    fn ed25519_sign() {
        let (p, s) = key_pair_factory();

        let sig = Ed25519Sha512::sign(MESSAGE_1, &s);
        Ed25519Sha512::verify(MESSAGE_1, &sig, &p).unwrap();

        assert_eq!(sig.len(), ed25519_dalek::SIGNATURE_LENGTH);
        assert_eq!(hex::encode(sig.as_slice()), SIGNATURE_1);

        // Check if OpenSSL signs the message and this module still can verify it.
        #[cfg(feature = "crypto-parity-tests")]
        {
            let openssl_sk = openssl_private_key(&s);
            let mut signer = Signer::new_without_digest(&openssl_sk).unwrap();
            let signature = signer.sign_oneshot_to_vec(MESSAGE_1).unwrap();
            Ed25519Sha512::verify(MESSAGE_1, &signature, &p).unwrap();
        }
    }

    #[test]
    fn invalid_parse_size_does_not_panic() {
        // passing an empty slice (or some other slice that is not appropriately sized) should not cause a panic
        // an error should be returned
        let err = Ed25519Sha512::parse_public_key(&[]).unwrap_err();
        assert_eq!(
            err,
            ParseError("the payload size is incorrect: expected 32, but got 0".to_string())
        );
        let err = Ed25519Sha512::parse_private_key(&[1, 2, 3]).unwrap_err();
        assert_eq!(
            err,
            ParseError("the payload size is incorrect: expected 32 or 64, but got 3".to_string())
        );
    }

    #[test]
    fn seeded_keypair_uses_seed_bytes() {
        let seed = [0x11; 32];
        let (pk, sk) = Ed25519Sha512::keypair(KeyGenOption::UseSeed(seed.to_vec()));
        let expected = PrivateKey::from_bytes(&seed);
        assert_eq!(sk.to_bytes(), expected.to_bytes());
        assert_eq!(pk.to_bytes(), expected.verifying_key().to_bytes());
    }

    #[test]
    fn seeded_keypair_hashes_non_32_seed() {
        let seed = b"iroha-ed25519-seed";
        let (pk, sk) = Ed25519Sha512::keypair(KeyGenOption::UseSeed(seed.to_vec()));
        let digest = Sha256::digest(seed);
        let mut derived = [0u8; 32];
        derived.copy_from_slice(&digest);
        let expected = PrivateKey::from_bytes(&derived);
        assert_eq!(sk.to_bytes(), expected.to_bytes());
        assert_eq!(pk.to_bytes(), expected.verifying_key().to_bytes());
    }

    #[test]
    fn parse_private_key_accepts_seed_or_keypair_bytes() {
        let seed = [0x42; 32];
        let signing_key = PrivateKey::from_bytes(&seed);
        let public = signing_key.verifying_key().to_bytes();

        let seed_parsed = Ed25519Sha512::parse_private_key(&seed).expect("seed parse");
        assert_eq!(seed_parsed.to_bytes(), signing_key.to_bytes());

        let mut keypair_bytes = [0u8; 64];
        keypair_bytes[..32].copy_from_slice(&seed);
        keypair_bytes[32..].copy_from_slice(&public);
        let keypair_parsed =
            Ed25519Sha512::parse_private_key(&keypair_bytes).expect("keypair parse");
        assert_eq!(keypair_parsed.to_bytes(), signing_key.to_bytes());
    }

    #[test]
    fn parse_private_key_rejects_mismatched_keypair_bytes() {
        let seed = [0x01; 32];
        let signing_key = PrivateKey::from_bytes(&seed);
        let mut keypair_bytes = [0u8; 64];
        keypair_bytes[..32].copy_from_slice(&seed);
        keypair_bytes[32..].copy_from_slice(&signing_key.verifying_key().to_bytes());
        keypair_bytes[63] ^= 0x01;
        let err = Ed25519Sha512::parse_private_key(&keypair_bytes).unwrap_err();
        assert!(
            err.0.contains("mismatched public key"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn parse_public_key_rejects_small_order() {
        let err = Ed25519Sha512::parse_public_key(&ED25519_SMALL_ORDER_POINT).unwrap_err();
        assert!(err.0.contains("small-order"), "unexpected error: {err:?}");
    }

    #[test]
    fn parse_public_key_rejects_non_canonical_encoding() {
        let err = Ed25519Sha512::parse_public_key(&ED25519_NON_CANONICAL_IDENTITY).unwrap_err();
        assert!(err.0.contains("non-canonical"), "unexpected error: {err:?}");
    }

    #[test]
    fn batch_verify_two_signatures_deterministic() {
        use crate::rng::os_rng;

        let mut rng1 = os_rng();
        let sk1 = ed25519::PrivateKey::generate(&mut rng1);
        let pk1 = sk1.verifying_key();
        let mut rng2 = os_rng();
        let sk2 = ed25519::PrivateKey::generate(&mut rng2);
        let pk2 = sk2.verifying_key();

        let m1 = b"msg1".as_ref();
        let m2 = b"msg2".as_ref();
        let s1 = Ed25519Sha512::sign(m1, &sk1);
        let s2 = Ed25519Sha512::sign(m2, &sk2);

        let msgs: [&[u8]; 2] = [m1, m2];
        let sigs: [&[u8]; 2] = [s1.as_slice(), s2.as_slice()];
        let pks_arr: [&[u8]; 2] = [pk1.as_bytes(), pk2.as_bytes()];
        let seed = [7u8; 32];

        Ed25519Sha512::verify_batch_deterministic(&msgs, &sigs, &pks_arr, seed)
            .expect("batch verify ok");

        // Order invariance: reverse input order; per-signature verification is order-independent
        let msgs_r: [&[u8]; 2] = [m2, m1];
        let sigs_r: [&[u8]; 2] = [s2.as_slice(), s1.as_slice()];
        let pks_r_arr: [&[u8]; 2] = [pk2.as_bytes(), pk1.as_bytes()];
        Ed25519Sha512::verify_batch_deterministic(&msgs_r, &sigs_r, &pks_r_arr, seed)
            .expect("batch verify ok rev");
    }

    fn ed25519_batch_fixture() -> Vec<(Vec<u8>, Vec<u8>, PublicKey)> {
        (0u8..5)
            .map(|idx| {
                let seed = [idx.saturating_add(1); 32];
                let (pk, sk) = Ed25519Sha512::keypair(KeyGenOption::UseSeed(seed.to_vec()));
                let message = format!("ed25519-batch-message-{idx}").into_bytes();
                let signature = Ed25519Sha512::sign(&message, &sk);
                (message, signature, pk)
            })
            .collect()
    }

    fn ed25519_hash_message_batch_fixture() -> Vec<(Vec<u8>, Vec<u8>, PublicKey)> {
        (0u8..5)
            .map(|idx| {
                let seed = [idx.saturating_add(11); 32];
                let (pk, sk) = Ed25519Sha512::keypair(KeyGenOption::UseSeed(seed.to_vec()));
                let message = vec![idx; 32];
                let signature = Ed25519Sha512::sign(&message, &sk);
                (message, signature, pk)
            })
            .collect()
    }

    #[test]
    fn ed25519_batch_preparsed_valid_and_reordered_inputs_pass() {
        let triples = ed25519_batch_fixture();
        let messages = triples
            .iter()
            .map(|(message, _, _)| message.as_slice())
            .collect::<Vec<_>>();
        let signatures = triples
            .iter()
            .map(|(_, signature, _)| signature.as_slice())
            .collect::<Vec<_>>();
        let public_keys = triples
            .iter()
            .map(|(_, _, public_key)| *public_key)
            .collect::<Vec<_>>();
        Ed25519Sha512::verify_batch_preparsed_deterministic(
            &messages,
            &signatures,
            &public_keys,
            [0x33; 32],
        )
        .expect("valid preparsed batch");

        let reordered = triples.into_iter().rev().collect::<Vec<_>>();
        let messages = reordered
            .iter()
            .map(|(message, _, _)| message.as_slice())
            .collect::<Vec<_>>();
        let signatures = reordered
            .iter()
            .map(|(_, signature, _)| signature.as_slice())
            .collect::<Vec<_>>();
        let public_keys = reordered
            .iter()
            .map(|(_, _, public_key)| *public_key)
            .collect::<Vec<_>>();
        Ed25519Sha512::verify_batch_preparsed_deterministic(
            &messages,
            &signatures,
            &public_keys,
            [0x44; 32],
        )
        .expect("reordered valid preparsed batch");
    }

    #[test]
    fn ed25519_batch_preparsed_invalid_signature_rejected() {
        let mut triples = ed25519_batch_fixture();
        triples[2].1[0] ^= 0x80;
        let messages = triples
            .iter()
            .map(|(message, _, _)| message.as_slice())
            .collect::<Vec<_>>();
        let signatures = triples
            .iter()
            .map(|(_, signature, _)| signature.as_slice())
            .collect::<Vec<_>>();
        let public_keys = triples
            .iter()
            .map(|(_, _, public_key)| *public_key)
            .collect::<Vec<_>>();

        let err = Ed25519Sha512::verify_batch_preparsed_deterministic(
            &messages,
            &signatures,
            &public_keys,
            [0x55; 32],
        )
        .expect_err("tampered signature must fail");
        assert_eq!(err, Error::BadSignature);
    }

    #[test]
    fn parse_signature_rejects_noncanonical_or_small_order_r() {
        let mut small_order = [0u8; ed25519_dalek::SIGNATURE_LENGTH];
        small_order[..32].copy_from_slice(&ED25519_SMALL_ORDER_POINT);
        assert_eq!(
            Ed25519Sha512::parse_signature(&small_order).expect_err("small-order R must fail"),
            Error::BadSignature
        );

        let mut noncanonical = [0u8; ed25519_dalek::SIGNATURE_LENGTH];
        noncanonical[..32].copy_from_slice(&ED25519_NON_CANONICAL_IDENTITY);
        assert_eq!(
            Ed25519Sha512::parse_signature(&noncanonical).expect_err("non-canonical R must fail"),
            Error::BadSignature
        );
    }

    #[test]
    fn ed25519_batch_rejects_small_order_r_before_batch_backend() {
        reset_batch_cache_counters_for_tests();
        let mut triples = ed25519_batch_fixture();
        triples[0].1[..32].copy_from_slice(&ED25519_SMALL_ORDER_POINT);
        let messages = triples
            .iter()
            .map(|(message, _, _)| message.as_slice())
            .collect::<Vec<_>>();
        let signatures = triples
            .iter()
            .map(|(_, signature, _)| signature.as_slice())
            .collect::<Vec<_>>();
        let public_keys = triples
            .iter()
            .map(|(_, _, public_key)| *public_key)
            .collect::<Vec<_>>();

        let err = Ed25519Sha512::verify_batch_preparsed_deterministic(
            &messages,
            &signatures,
            &public_keys,
            [0x56; 32],
        )
        .expect_err("small-order R must fail before batch verification");
        assert_eq!(err, Error::BadSignature);
        assert_eq!(
            uncached_batch_verify_calls_for_tests(),
            0,
            "strict R validation must run before the dalek batch backend"
        );
    }

    #[test]
    fn ed25519_batch_rejects_empty_and_mismatched_inputs() {
        let empty_messages: [&[u8]; 0] = [];
        let empty_signatures: [&[u8]; 0] = [];
        let empty_public_keys: [PublicKey; 0] = [];
        assert_eq!(
            Ed25519Sha512::verify_batch_preparsed_deterministic(
                &empty_messages,
                &empty_signatures,
                &empty_public_keys,
                [0; 32],
            )
            .expect_err("empty batch must fail"),
            Error::BadSignature
        );

        let triples = ed25519_batch_fixture();
        let messages = triples
            .iter()
            .map(|(message, _, _)| message.as_slice())
            .collect::<Vec<_>>();
        let signatures = triples
            .iter()
            .map(|(_, signature, _)| signature.as_slice())
            .collect::<Vec<_>>();
        let public_keys = triples
            .iter()
            .take(1)
            .map(|(_, _, public_key)| *public_key)
            .collect::<Vec<_>>();
        assert_eq!(
            Ed25519Sha512::verify_batch_preparsed_deterministic(
                &messages,
                &signatures,
                &public_keys,
                [0; 32],
            )
            .expect_err("mismatched batch must fail"),
            Error::BadSignature
        );
    }

    #[test]
    fn ed25519_batch_public_preparsed_api_matches_raw_api() {
        let triples = ed25519_batch_fixture();
        let messages = triples
            .iter()
            .map(|(message, _, _)| message.as_slice())
            .collect::<Vec<_>>();
        let signatures = triples
            .iter()
            .map(|(_, signature, _)| signature.as_slice())
            .collect::<Vec<_>>();
        let raw_public_keys = triples
            .iter()
            .map(|(_, _, public_key)| public_key.as_bytes().as_slice())
            .collect::<Vec<_>>();
        let parsed_public_keys = raw_public_keys
            .iter()
            .map(|public_key| crate::ed25519_parse_public_key(public_key).expect("parse key"))
            .collect::<Vec<_>>();

        crate::ed25519_verify_batch_deterministic(
            &messages,
            &signatures,
            &raw_public_keys,
            [0x66; 32],
        )
        .expect("raw batch API");
        crate::ed25519_verify_batch_preparsed_deterministic(
            &messages,
            &signatures,
            &parsed_public_keys,
            [0x66; 32],
        )
        .expect("preparsed batch API");

        let mut scratch = crate::Ed25519BatchScratch::default();
        crate::ed25519_verify_batch_preparsed_deterministic_with_scratch(
            &messages,
            &signatures,
            &parsed_public_keys,
            [0x66; 32],
            &mut scratch,
        )
        .expect("preparsed batch API with scratch");
    }

    #[test]
    fn ed25519_batch_all_cached_skips_signature_parse_and_verifier_setup() {
        reset_verify_ok_cache_for_tests();
        let triples = ed25519_hash_message_batch_fixture();
        for (message, signature, public_key) in &triples {
            Ed25519Sha512::verify(message, signature, public_key).expect("seed verify-ok cache");
        }
        reset_batch_cache_counters_for_tests();

        let messages = triples
            .iter()
            .map(|(message, _, _)| message.as_slice())
            .collect::<Vec<_>>();
        let signatures = triples
            .iter()
            .map(|(_, signature, _)| signature.as_slice())
            .collect::<Vec<_>>();
        let public_keys = triples
            .iter()
            .map(|(_, _, public_key)| {
                crate::ed25519_parse_public_key(public_key.as_bytes()).expect("parse key")
            })
            .collect::<Vec<_>>();

        let mut scratch = crate::Ed25519BatchScratch::default();
        crate::ed25519_verify_batch_preparsed_deterministic_with_scratch(
            &messages,
            &signatures,
            &public_keys,
            [0x71; 32],
            &mut scratch,
        )
        .expect("all cached batch verifies");

        assert_eq!(signature_parse_calls_for_tests(), 0);
        assert_eq!(uncached_batch_verify_calls_for_tests(), 0);
    }

    #[test]
    fn ed25519_batch_mixed_cached_and_uncached_reports_lowest_index() {
        reset_verify_ok_cache_for_tests();
        let mut triples = ed25519_hash_message_batch_fixture();
        for (message, signature, public_key) in [0usize, 2]
            .into_iter()
            .map(|idx| (&triples[idx].0, &triples[idx].1, &triples[idx].2))
        {
            Ed25519Sha512::verify(message, signature, public_key).expect("seed verify-ok cache");
        }
        triples[1].1[0] ^= 0x01;
        triples[3].1[0] ^= 0x01;
        reset_batch_cache_counters_for_tests();

        let messages = triples
            .iter()
            .map(|(message, _, _)| message.as_slice())
            .collect::<Vec<_>>();
        let signatures = triples
            .iter()
            .map(|(_, signature, _)| signature.as_slice())
            .collect::<Vec<_>>();
        let public_keys = triples
            .iter()
            .map(|(_, _, public_key)| {
                crate::ed25519_parse_public_key(public_key.as_bytes()).expect("parse key")
            })
            .collect::<Vec<_>>();

        let mut scratch = crate::Ed25519BatchScratch::default();
        crate::ed25519_verify_batch_preparsed_deterministic_with_scratch(
            &messages,
            &signatures,
            &public_keys,
            [0x72; 32],
            &mut scratch,
        )
        .expect_err("mixed cached/uncached batch must reject tampered signatures");
        assert!(
            signature_parse_calls_for_tests() < triples.len(),
            "cached hits should not be parsed again"
        );

        let (idx, _detail) = crate::ed25519_first_bad_preparsed_deterministic_with_scratch(
            &messages,
            &signatures,
            &public_keys,
            [0x72; 32],
            &mut scratch,
        )
        .expect("tampered tuple must be found");
        assert_eq!(idx, 1);
    }

    #[test]
    fn ed25519_first_bad_preparsed_reports_lowest_original_index_with_cache_hits() {
        let mut triples = ed25519_batch_fixture();
        for (message, signature, public_key) in triples.iter().take(2) {
            Ed25519Sha512::verify(message, signature, public_key).expect("seed verify-ok cache");
        }
        triples[3].1[0] ^= 0x80;

        let messages = triples
            .iter()
            .map(|(message, _, _)| message.as_slice())
            .collect::<Vec<_>>();
        let signatures = triples
            .iter()
            .map(|(_, signature, _)| signature.as_slice())
            .collect::<Vec<_>>();
        let public_keys = triples
            .iter()
            .map(|(_, _, public_key)| {
                crate::ed25519_parse_public_key(public_key.as_bytes()).expect("parse key")
            })
            .collect::<Vec<_>>();

        let mut scratch = crate::Ed25519BatchScratch::default();
        let (idx, _detail) = crate::ed25519_first_bad_preparsed_deterministic_with_scratch(
            &messages,
            &signatures,
            &public_keys,
            [0x77; 32],
            &mut scratch,
        )
        .expect("tampered tuple must be found");
        assert_eq!(idx, 3);
    }

    #[test]
    fn verify_rejects_low_order_public_key_signatures() {
        fn hash_mod_order(
            r: &EdwardsPoint,
            pk_bytes: &[u8; 32],
            msg: &[u8],
            order: usize,
        ) -> usize {
            let mut h = Sha512::new();
            h.update(r.compress().as_bytes());
            h.update(pk_bytes);
            h.update(msg);
            let k = Scalar::from_hash(h);
            (k.to_bytes()[0] as usize) % order
        }

        fn find_forged_signature(pk: &ed25519_dalek::VerifyingKey) -> (Vec<u8>, [u8; 64]) {
            let a_point = pk.to_edwards();
            let mut order = 1usize;
            let mut acc = a_point;
            while !acc.is_identity() {
                acc += a_point;
                order += 1;
                assert!(order <= 8, "torsion order exceeded expected bound");
            }

            let mut torsion_points = Vec::with_capacity(order);
            let mut acc = EdwardsPoint::identity();
            for _ in 0..order {
                torsion_points.push(acc);
                acc += a_point;
            }

            for counter in 0u32..512 {
                let msg = format!("iroha-low-order-{counter}").into_bytes();
                for (m, r_point) in torsion_points.iter().enumerate() {
                    let k_mod = hash_mod_order(r_point, pk.as_bytes(), &msg, order);
                    let expected_m = (order - k_mod) % order;
                    if m == expected_m {
                        let mut sig = [0u8; 64];
                        sig[..32].copy_from_slice(r_point.compress().as_bytes());
                        return (msg, sig);
                    }
                }
            }

            panic!("failed to forge low-order signature");
        }

        let pk = ed25519_dalek::VerifyingKey::from_bytes(&ED25519_SMALL_ORDER_POINT)
            .expect("low-order public key should parse");
        let (message, sig_bytes) = find_forged_signature(&pk);
        let signature = Signature::from_bytes(&sig_bytes);
        pk.verify(&message, &signature)
            .expect("non-strict verify accepts low-order key signature");
        let err = Ed25519Sha512::verify(&message, &sig_bytes, &pk);
        assert!(matches!(err, Err(Error::BadSignature)));
    }
}
