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

use std::{cell::RefCell, collections::HashSet, format, string::ToString as _, vec::Vec};

const VERIFY_OK_CACHE_LIMIT: usize = 4096;

struct VerifyOkCache {
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
            // Simple bounded cache: clear rather than paying LRU bookkeeping cost.
            self.map.clear();
        }
        self.map.insert(key);
    }
}

thread_local! {
    static VERIFY_OK_CACHE: RefCell<VerifyOkCache> = RefCell::new(VerifyOkCache::new());
}

fn verify_ok_cache_key(pk: &PublicKey, message: &[u8], signature: &[u8]) -> [u8; 32] {
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
        parse_fixed_size(payload, |bytes| {
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

            // Reject non-canonical encodings (ZIP-215 allows them, but our ABI requires canonical
            // byte representation to keep deterministic I105/in-memory forms in sync).
            if key.is_weak() {
                return Err(ParseError(
                    "ed25519 public key is small-order (weak); rejected".to_string(),
                ));
            }

            Ok(key)
        })
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
            let key = verify_ok_cache_key(pk, message, signature);
            if VERIFY_OK_CACHE.with(|cache| cache.borrow().contains(&key)) {
                return Ok(());
            }
            // `Signature::try_from` only checks length for Ed25519; we already know it's correct.
            let s = Signature::try_from(signature).map_err(|e| ParseError(e.to_string()))?;
            pk.verify_strict(message, &s)
                .map_err(|_| Error::BadSignature)?;
            VERIFY_OK_CACHE.with(|cache| cache.borrow_mut().insert(key));
            return Ok(());
        }
        let s = Signature::try_from(signature).map_err(|e| ParseError(e.to_string()))?;
        pk.verify_strict(message, &s)
            .map_err(|_| Error::BadSignature)
    }

    /// Deterministic batch verification helper.
    ///
    /// Verifies each (message, signature, `public_key`) triple independently in order.
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
        let _ = seed32;
        for ((m, s), pk_bytes) in messages
            .iter()
            .zip(signatures.iter())
            .zip(public_keys.iter())
        {
            let pk = match Self::parse_public_key(pk_bytes) {
                Ok(v) => v,
                Err(_) => return Err(Error::BadSignature),
            };
            // Reuse single-verify to keep semantics identical.
            Self::verify(m, s, &pk)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
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

    fn openssl_public_key(pk: &ed25519::PublicKey) -> PKey<Public> {
        PKey::public_key_from_raw_bytes(pk.as_bytes(), Id::ED25519).expect("openssl public key")
    }

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
        let signature = hex::decode(SIGNATURE_1).unwrap();
        let openssl_pk = openssl_public_key(&p);
        let mut verifier = OpenSslVerifier::new_without_digest(&openssl_pk).unwrap();
        assert!(verifier.verify_oneshot(&signature, MESSAGE_1).unwrap());
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
        let openssl_sk = openssl_private_key(&s);
        let mut signer = Signer::new_without_digest(&openssl_sk).unwrap();
        let signature = signer.sign_oneshot_to_vec(MESSAGE_1).unwrap();
        Ed25519Sha512::verify(MESSAGE_1, &signature, &p).unwrap();
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
