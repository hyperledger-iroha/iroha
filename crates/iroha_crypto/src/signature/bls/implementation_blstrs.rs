use core::marker::PhantomData;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, OnceLock},
    vec::Vec,
};

use blstrs::{G1Affine, G1Projective, G2Affine, G2Prepared, G2Projective};
use group::{Curve, Group as _, prime::PrimeCurveAffine};
use pairing::{MillerLoopResult as _, MultiMillerLoop};
use parking_lot::Mutex;
#[cfg(feature = "rand")]
use rand::rngs::OsRng;
use w3f_bls::SerializableToBytes as _;
use zeroize::Zeroize as _;

pub(super) const MESSAGE_CONTEXT: &[u8; 20] = b"for signing messages";

use crate::{Algorithm, Error, KeyGenOption, ParseError};

pub trait BlsConfiguration {
    const ALGORITHM: Algorithm;
    // true: Normal (pk in G1, sig in G2); false: Small (pk in G2, sig in G1)
    const NORMAL: bool;
}

// Public key wrapper stores compressed bytes; orientation depends on C::NORMAL
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicKey<C: BlsConfiguration> {
    bytes: Vec<u8>,
    _m: PhantomData<C>,
}
impl<C: BlsConfiguration> PublicKey<C> {
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
    pub fn keypair(mut option: KeyGenOption<SecretKey<C>>) -> (PublicKey<C>, SecretKey<C>) {
        let sk = match option {
            #[cfg(feature = "rand")]
            KeyGenOption::Random => {
                let bytes = if C::NORMAL {
                    w3f_bls::SecretKeyVT::<w3f_bls::ZBLS>::generate(OsRng).to_bytes()
                } else {
                    w3f_bls::SecretKeyVT::<w3f_bls::TinyBLS381>::generate(OsRng).to_bytes()
                };
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                SecretKey::from_bytes(arr)
            }
            KeyGenOption::UseSeed(ref mut seed) => {
                let bytes = if C::NORMAL {
                    w3f_bls::SecretKeyVT::<w3f_bls::ZBLS>::from_seed(seed).to_bytes()
                } else {
                    w3f_bls::SecretKeyVT::<w3f_bls::TinyBLS381>::from_seed(seed).to_bytes()
                };
                seed.zeroize();
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&bytes);
                SecretKey::from_bytes(arr)
            }
            KeyGenOption::FromPrivateKey(key) => key,
        };

        // Public key depends on orientation; derive via w3f to ensure stable encoding
        let pk_bytes = if C::NORMAL {
            let sk_w = w3f_bls::SecretKeyVT::<w3f_bls::ZBLS>::from_bytes(&sk.bytes)
                .expect("valid w3f secret from bytes");
            sk_w.into_public().to_bytes()
        } else {
            let sk_w = w3f_bls::SecretKeyVT::<w3f_bls::TinyBLS381>::from_bytes(&sk.bytes)
                .expect("valid w3f secret from bytes");
            sk_w.into_public().to_bytes()
        };
        (
            PublicKey {
                bytes: pk_bytes,
                _m: PhantomData,
            },
            sk,
        )
    }

    pub fn sign(message: &[u8], sk: &SecretKey<C>) -> Vec<u8> {
        // Produce signature with w3f to match canonical encoding exactly.
        let msg = w3f_bls::Message::new(MESSAGE_CONTEXT, message);
        if C::NORMAL {
            let sk_w = w3f_bls::SecretKeyVT::<w3f_bls::ZBLS>::from_bytes(&sk.bytes)
                .expect("valid w3f secret from bytes");
            sk_w.sign(&msg).to_bytes()
        } else {
            let sk_w = w3f_bls::SecretKeyVT::<w3f_bls::TinyBLS381>::from_bytes(&sk.bytes)
                .expect("valid w3f secret from bytes");
            sk_w.sign(&msg).to_bytes()
        }
    }

    pub fn verify(message: &[u8], signature: &[u8], pk: &PublicKey<C>) -> Result<(), Error> {
        // Verify with the same pairing-based check used by aggregate paths and
        // reject non-canonical or identity encodings up front.
        if C::NORMAL {
            let sig = to_g2(signature)
                .ok_or_else(|| ParseError("invalid BLS signature encoding".to_string()))?;
            let pk = to_g1_public_key(&pk.bytes)
                .ok_or_else(|| ParseError("invalid BLS public key encoding".to_string()))?;
            let h = hash_msg_to_g2(message);
            let terms: [(&G1Affine, &G2Prepared); 2] = [
                (&G1Affine::generator(), &G2Prepared::from(sig)),
                (&(-G1Projective::from(pk)).to_affine(), &G2Prepared::from(h)),
            ];
            let gt = blstrs::Bls12::multi_miller_loop(&terms).final_exponentiation();
            if gt.is_identity().into() {
                Ok(())
            } else {
                Err(Error::BadSignature)
            }
        } else {
            let sig = to_g1(signature)
                .ok_or_else(|| ParseError("invalid BLS signature encoding".to_string()))?;
            let pk = to_g2_prepared(&pk.bytes)
                .ok_or_else(|| ParseError("invalid BLS public key encoding".to_string()))?;
            let h = hash_msg_to_g1(message);
            let terms: [(&G1Affine, &G2Prepared); 2] = [
                (&sig, g2_prepared_generator().as_ref()),
                (&(-G1Projective::from(h)).to_affine(), pk.as_ref()),
            ];
            let gt = blstrs::Bls12::multi_miller_loop(&terms).final_exponentiation();
            if gt.is_identity().into() {
                Ok(())
            } else {
                Err(Error::BadSignature)
            }
        }
    }

    pub fn verify_aggregate_same_message(
        message: &[u8],
        signatures: &[&[u8]],
        public_keys: &[&[u8]],
    ) -> Result<(), Error> {
        if signatures.is_empty() || signatures.len() != public_keys.len() {
            return Err(Error::BadSignature);
        }
        let mut seen_pks: BTreeSet<Vec<u8>> = BTreeSet::new();
        if C::NORMAL {
            // Aggregate sigs in G2, PKs in G1
            let mut agg_sig: Option<G2Projective> = None;
            for s in signatures {
                let s = to_g2(s).ok_or(Error::BadSignature)?;
                agg_sig =
                    Some(agg_sig.unwrap_or_else(G2Projective::identity) + G2Projective::from(s));
            }
            let mut agg_pk: Option<G1Projective> = None;
            for pk in public_keys {
                let pk = to_g1_public_key(pk).ok_or(Error::BadSignature)?;
                if !seen_pks.insert(pk.to_compressed().to_vec()) {
                    return Err(Error::BadSignature);
                }
                agg_pk =
                    Some(agg_pk.unwrap_or_else(G1Projective::identity) + G1Projective::from(pk));
            }
            let sig = agg_sig.unwrap();
            if sig.is_identity().into() {
                return Err(Error::BadSignature);
            }
            let pk = agg_pk.unwrap();
            if pk.is_identity().into() {
                return Err(Error::BadSignature);
            }
            let sig = sig.to_affine();
            let pk = pk.to_affine();
            let h = hash_msg_to_g2(message);
            let terms: [(&G1Affine, &G2Prepared); 2] = [
                (&G1Affine::generator(), &G2Prepared::from(sig)),
                (&(-G1Projective::from(pk)).to_affine(), &G2Prepared::from(h)),
            ];
            let gt = blstrs::Bls12::multi_miller_loop(&terms).final_exponentiation();
            if gt.is_identity().into() {
                Ok(())
            } else {
                Err(Error::BadSignature)
            }
        } else {
            // Aggregate sigs in G1, PKs in G2
            let mut agg_sig: Option<G1Projective> = None;
            for s in signatures {
                let s = to_g1(s).ok_or(Error::BadSignature)?;
                agg_sig =
                    Some(agg_sig.unwrap_or_else(G1Projective::identity) + G1Projective::from(s));
            }
            let mut agg_pk: Option<G2Projective> = None;
            for pk in public_keys {
                let pk = to_g2_public_key(pk).ok_or(Error::BadSignature)?;
                if !seen_pks.insert(pk.to_compressed().to_vec()) {
                    return Err(Error::BadSignature);
                }
                agg_pk =
                    Some(agg_pk.unwrap_or_else(G2Projective::identity) + G2Projective::from(pk));
            }
            let sig = agg_sig.unwrap();
            if sig.is_identity().into() {
                return Err(Error::BadSignature);
            }
            let pk = agg_pk.unwrap();
            if pk.is_identity().into() {
                return Err(Error::BadSignature);
            }
            let sig = sig.to_affine();
            let pk = pk.to_affine();
            let h = hash_msg_to_g1(message);
            let prepared_pk = G2Prepared::from(pk);
            let terms: [(&G1Affine, &G2Prepared); 2] = [
                (&sig, g2_prepared_generator().as_ref()),
                (&(-G1Projective::from(h)).to_affine(), &prepared_pk),
            ];
            let gt = blstrs::Bls12::multi_miller_loop(&terms).final_exponentiation();
            if gt.is_identity().into() {
                Ok(())
            } else {
                Err(Error::BadSignature)
            }
        }
    }

    /// Aggregate a sequence of BLS signatures (same-message context) into a single signature.
    /// The caller is responsible for ensuring all signatures are valid and from the same suite.
    /// Rejects aggregates that cancel to the identity element.
    pub fn aggregate_signatures(signatures: &[&[u8]]) -> Result<Vec<u8>, Error> {
        if signatures.is_empty() {
            return Err(Error::BadSignature);
        }
        if C::NORMAL {
            let mut agg_sig = G2Projective::identity();
            for s in signatures {
                let sig = to_g2(s).ok_or(Error::BadSignature)?;
                agg_sig += G2Projective::from(sig);
            }
            if agg_sig.is_identity().into() {
                return Err(Error::BadSignature);
            }
            Ok(agg_sig.to_affine().to_compressed().to_vec())
        } else {
            let mut agg_sig = G1Projective::identity();
            for s in signatures {
                let sig = to_g1(s).ok_or(Error::BadSignature)?;
                agg_sig += G1Projective::from(sig);
            }
            if agg_sig.is_identity().into() {
                return Err(Error::BadSignature);
            }
            Ok(agg_sig.to_affine().to_compressed().to_vec())
        }
    }

    /// Verify a pre-aggregated signature for the same-message case.
    pub fn verify_preaggregated_same_message(
        message: &[u8],
        aggregated_signature: &[u8],
        public_keys: &[&[u8]],
    ) -> Result<(), Error> {
        if public_keys.is_empty() {
            return Err(Error::BadSignature);
        }
        let mut seen_pks: BTreeSet<Vec<u8>> = BTreeSet::new();
        if C::NORMAL {
            let sig = to_g2(aggregated_signature).ok_or(Error::BadSignature)?;
            let mut agg_pk = G1Projective::identity();
            for pk in public_keys {
                let pk = to_g1_public_key(pk).ok_or(Error::BadSignature)?;
                if !seen_pks.insert(pk.to_compressed().to_vec()) {
                    return Err(Error::BadSignature);
                }
                agg_pk += G1Projective::from(pk);
            }
            let pk = agg_pk.to_affine();
            let h = hash_msg_to_g2(message);
            let terms: [(&G1Affine, &G2Prepared); 2] = [
                (&G1Affine::generator(), &G2Prepared::from(sig)),
                (&(-G1Projective::from(pk)).to_affine(), &G2Prepared::from(h)),
            ];
            let gt = blstrs::Bls12::multi_miller_loop(&terms).final_exponentiation();
            if gt.is_identity().into() {
                Ok(())
            } else {
                Err(Error::BadSignature)
            }
        } else {
            let sig = to_g1(aggregated_signature).ok_or(Error::BadSignature)?;
            let mut agg_pk = G2Projective::identity();
            for pk in public_keys {
                let pk = to_g2_public_key(pk).ok_or(Error::BadSignature)?;
                if !seen_pks.insert(pk.to_compressed().to_vec()) {
                    return Err(Error::BadSignature);
                }
                agg_pk += G2Projective::from(pk);
            }
            let pk = agg_pk.to_affine();
            let h = hash_msg_to_g1(message);
            let prepared_pk = G2Prepared::from(pk);
            let terms: [(&G1Affine, &G2Prepared); 2] = [
                (&sig, g2_prepared_generator().as_ref()),
                (&(-G1Projective::from(h)).to_affine(), &prepared_pk),
            ];
            let gt = blstrs::Bls12::multi_miller_loop(&terms).final_exponentiation();
            if gt.is_identity().into() {
                Ok(())
            } else {
                Err(Error::BadSignature)
            }
        }
    }

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
        {
            use std::collections::BTreeSet;
            let mut seen = BTreeSet::new();
            for &msg in messages {
                if !seen.insert(msg) {
                    return Err(Error::BadSignature);
                }
            }
        }

        if C::NORMAL {
            let mut pairs: Vec<(G1Affine, G2Prepared)> = Vec::with_capacity(messages.len() * 2);
            for ((m, s_bytes), pk_bytes) in messages
                .iter()
                .zip(signatures.iter())
                .zip(public_keys.iter())
            {
                let sig = to_g2(s_bytes).ok_or(Error::BadSignature)?;
                let pk = to_g1_public_key(pk_bytes).ok_or(Error::BadSignature)?;
                let h = hash_msg_to_g2(m);
                pairs.push((G1Affine::generator(), G2Prepared::from(sig)));
                pairs.push(((-G1Projective::from(pk)).to_affine(), G2Prepared::from(h)));
            }
            let terms: Vec<(&G1Affine, &G2Prepared)> = pairs.iter().map(|(p, q)| (p, q)).collect();
            let gt = blstrs::Bls12::multi_miller_loop(&terms).final_exponentiation();
            if gt.is_identity().into() {
                Ok(())
            } else {
                Err(Error::BadSignature)
            }
        } else {
            let generator = g2_prepared_generator();
            let mut pairs: Vec<(G1Affine, Arc<G2Prepared>)> =
                Vec::with_capacity(messages.len() * 2);
            for ((m, s_bytes), pk_bytes) in messages
                .iter()
                .zip(signatures.iter())
                .zip(public_keys.iter())
            {
                let sig = to_g1(s_bytes).ok_or(Error::BadSignature)?;
                let pk = to_g2_prepared(pk_bytes).ok_or(Error::BadSignature)?;
                let h = hash_msg_to_g1(m);
                pairs.push((sig, Arc::clone(generator)));
                pairs.push(((-G1Projective::from(h)).to_affine(), pk));
            }
            let terms: Vec<(&G1Affine, &G2Prepared)> =
                pairs.iter().map(|(p, q)| (p, q.as_ref())).collect();
            let gt = blstrs::Bls12::multi_miller_loop(&terms).final_exponentiation();
            if gt.is_identity().into() {
                Ok(())
            } else {
                Err(Error::BadSignature)
            }
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

const PUBKEY_CACHE_MAX: usize = 4096;

fn g1_pubkey_cache() -> &'static Mutex<BTreeMap<Vec<u8>, G1Affine>> {
    static CACHE: OnceLock<Mutex<BTreeMap<Vec<u8>, G1Affine>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn g2_pubkey_cache() -> &'static Mutex<BTreeMap<Vec<u8>, G2Affine>> {
    static CACHE: OnceLock<Mutex<BTreeMap<Vec<u8>, G2Affine>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn g2_prepared_cache() -> &'static Mutex<BTreeMap<Vec<u8>, Arc<G2Prepared>>> {
    static CACHE: OnceLock<Mutex<BTreeMap<Vec<u8>, Arc<G2Prepared>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn g2_prepared_generator() -> &'static Arc<G2Prepared> {
    static GENERATOR: OnceLock<Arc<G2Prepared>> = OnceLock::new();
    GENERATOR.get_or_init(|| Arc::new(G2Prepared::from(G2Affine::generator())))
}

fn to_g1_public_key(bytes: &[u8]) -> Option<G1Affine> {
    if let Some(point) = g1_pubkey_cache().lock().get(bytes).cloned() {
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
    if let Some(point) = g2_pubkey_cache().lock().get(bytes).cloned() {
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
    let ct = G1Affine::from_compressed(&arr);
    if !ct.is_some().into() {
        return None;
    }
    let point = ct.unwrap();
    if point.is_identity().into() {
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
    let ct = G2Affine::from_compressed(&arr);
    if !ct.is_some().into() {
        return None;
    }
    let point = ct.unwrap();
    if point.is_identity().into() {
        return None;
    }
    if point.to_compressed() != arr {
        return None;
    }
    Some(point)
}

fn hash_msg_to_g2(msg: &[u8]) -> G2Affine {
    // Concatenation variant: MESSAGE_CONTEXT || msg with standard RO ciphersuite
    const DST: &[u8] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_";
    let mut buf = Vec::with_capacity(MESSAGE_CONTEXT.len() + msg.len());
    buf.extend_from_slice(MESSAGE_CONTEXT);
    buf.extend_from_slice(msg);
    G2Projective::hash_to_curve(&buf, DST, &[]).to_affine()
}
fn hash_msg_to_g1(msg: &[u8]) -> G1Affine {
    const DST: &[u8] = b"BLS_SIG_BLS12381G1_XMD:SHA-256_SSWU_RO_";
    let mut buf = Vec::with_capacity(MESSAGE_CONTEXT.len() + msg.len());
    buf.extend_from_slice(MESSAGE_CONTEXT);
    buf.extend_from_slice(msg);
    G1Projective::hash_to_curve(&buf, DST, &[]).to_affine()
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
        let (pk, sk) = BlsImpl::<CNormal>::keypair(KeyGenOption::UseSeed(vec![7; 10]));
        let sig = BlsImpl::<CNormal>::sign(b"abc", &sk);
        assert!(BlsImpl::<CNormal>::verify(b"abc", &sig, &pk).is_ok());
    }

    #[test]
    fn smoke_small() {
        let (pk, sk) = BlsImpl::<CSmall>::keypair(KeyGenOption::UseSeed(vec![9; 16]));
        let sig = BlsImpl::<CSmall>::sign(b"xyz", &sk);
        assert!(BlsImpl::<CSmall>::verify(b"xyz", &sig, &pk).is_ok());
    }

    #[test]
    fn public_key_cache_roundtrip_normal() {
        let (pk, _sk) = BlsImpl::<CNormal>::keypair(KeyGenOption::UseSeed(vec![1; 8]));
        let bytes = pk.to_bytes();
        let parsed = to_g1_public_key(&bytes).expect("valid public key");
        let cached = to_g1_public_key(&bytes).expect("cached public key");
        assert_eq!(parsed.to_compressed(), cached.to_compressed());
    }

    #[test]
    fn public_key_cache_roundtrip_small() {
        let (pk, _sk) = BlsImpl::<CSmall>::keypair(KeyGenOption::UseSeed(vec![2; 8]));
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
        let (pk, _sk) = BlsImpl::<CSmall>::keypair(KeyGenOption::UseSeed(vec![3; 8]));
        let bytes = pk.to_bytes();
        let prepared = to_g2_prepared(&bytes).expect("valid prepared key");
        let cached = to_g2_prepared(&bytes).expect("cached prepared key");
        assert!(Arc::ptr_eq(&prepared, &cached));
    }
}
