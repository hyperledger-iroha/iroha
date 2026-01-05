//! Property-based fuzz tests for SM2 key generation and signatures.
#![cfg(all(feature = "sm", feature = "sm_proptest"))]

#[path = "sm2_negative_vector_fixture.rs"]
mod negative_fixture;

use std::sync::{Arc, OnceLock};

use hex::decode as hex_decode;
use iroha_crypto::{
    Algorithm, Error, KeyPair, Signature, Sm2PrivateKey, Sm2PublicKey, Sm2Signature,
};
use negative_fixture::{NegativeVector, apply_mutation, load_negative_vectors};
use norito::json::Value;
use proptest::{collection::vec, prelude::*, proptest};
use sm2::dsa::{Signature as Sm2RawSignature, signature::hazmat::PrehashVerifier};
use sm3::{Digest, Sm3};

fn sm2_keypair() -> KeyPair {
    KeyPair::random_with_algorithm(Algorithm::Sm2)
}

#[derive(Clone, Debug)]
struct WycheproofCase {
    distid: String,
    public_sec1: Vec<u8>,
    message: Vec<u8>,
    signature_der: Vec<u8>,
    expect_valid: bool,
}

static WYCHEPROOF_CASES: OnceLock<Arc<Vec<WycheproofCase>>> = OnceLock::new();
static WYCHEPROOF_VALID_CASES: OnceLock<Arc<Vec<WycheproofCase>>> = OnceLock::new();
static NEGATIVE_VECTORS: OnceLock<Arc<Vec<NegativeVector>>> = OnceLock::new();

fn decode_hex(value: &str) -> Vec<u8> {
    if value.is_empty() {
        Vec::new()
    } else {
        hex_decode(value).unwrap_or_else(|err| panic!("invalid hex '{value}': {err}"))
    }
}

fn wycheproof_case_strategy() -> impl Strategy<Value = WycheproofCase> {
    let cases = load_wycheproof_cases();
    prop::sample::select((*cases).clone())
}

fn wycheproof_valid_case_strategy() -> impl Strategy<Value = WycheproofCase> {
    let cases = load_valid_wycheproof_cases();
    prop::sample::select((*cases).clone())
}

fn negative_vector_strategy() -> impl Strategy<Value = NegativeVector> {
    let vectors = load_negative_vectors_arc();
    prop::sample::select((*vectors).clone())
}

fn load_wycheproof_cases() -> Arc<Vec<WycheproofCase>> {
    WYCHEPROOF_CASES
        .get_or_init(|| {
            let raw = include_str!("fixtures/wycheproof_sm2.json");
            let value: Value =
                norito::json::from_str(raw).expect("parse Wycheproof SM2 fixture JSON");
            let groups = value["testGroups"]
                .as_array()
                .expect("Wycheproof SM2 testGroups array");

            let mut cases = Vec::new();
            for group in groups {
                let distid = group["distid"]
                    .as_str()
                    .unwrap_or("1234567812345678")
                    .to_owned();
                let public_hex = group["key"]["uncompressed"]
                    .as_str()
                    .expect("Wycheproof SM2 key missing");
                let public_sec1 = decode_hex(public_hex);

                let tests = group["tests"]
                    .as_array()
                    .expect("Wycheproof SM2 tests array missing");
                for test in tests {
                    let msg_hex = test["msg"].as_str().expect("Wycheproof SM2 msg missing");
                    let message = decode_hex(msg_hex);
                    let sig_hex = test["sig"]
                        .as_str()
                        .expect("Wycheproof SM2 signature missing");
                    let signature_der = decode_hex(sig_hex);
                    let expect_valid = matches!(
                        test["result"].as_str().map(str::to_ascii_lowercase),
                        Some(ref result) if result == "valid"
                    );

                    cases.push(WycheproofCase {
                        distid: distid.clone(),
                        public_sec1: public_sec1.clone(),
                        message,
                        signature_der,
                        expect_valid,
                    });
                }
            }

            Arc::new(cases)
        })
        .clone()
}

fn load_valid_wycheproof_cases() -> Arc<Vec<WycheproofCase>> {
    WYCHEPROOF_VALID_CASES
        .get_or_init(|| {
            let all = load_wycheproof_cases();
            Arc::new(
                all.iter()
                    .filter(|case| case.expect_valid)
                    .cloned()
                    .collect(),
            )
        })
        .clone()
}

fn load_negative_vectors_arc() -> Arc<Vec<NegativeVector>> {
    NEGATIVE_VECTORS
        .get_or_init(|| Arc::new(load_negative_vectors()))
        .clone()
}

proptest! {
    #[test]
    fn sm2_invalid_rs_are_rejected(rs in any::<[u8; Sm2Signature::LENGTH]>()) {
        let keypair = sm2_keypair();
        let signature = Signature::from_bytes(&rs);
        prop_assert!(matches!(
            signature.verify(keypair.public_key(), b"fuzz"),
            Err(Error::Parse(_) | Error::BadSignature)
        ));
    }

    #[test]
    fn sm2_wrong_distid_is_rejected(distid in any::<[u8; 16]>()) {
        let secret = [0xAA; 32];
        let private = Sm2PrivateKey::new(Sm2PublicKey::DEFAULT_DISTID, secret).expect("key");
        let message = b"sm2 fuzz";
        let signature = private.sign(message);
        let pk_bytes = private.public_key().to_sec1_bytes(false);
        let distid_suffix = u128::from_be_bytes(distid);
        let alt_distid = format!("ALT-{distid_suffix:032X}");
        let altered = Sm2PublicKey::from_sec1_bytes(&alt_distid, &pk_bytes)
            .expect("distid alteration should yield valid key point");
        prop_assert!(matches!(
            altered.verify(message, &signature),
            Err(Error::BadSignature)
        ));
    }

    #[test]
    fn sm2_valid_signature_roundtrip(seed in any::<[u8; 32]>()) {
        let sk = Sm2PrivateKey::from_seed(Sm2PublicKey::DEFAULT_DISTID, &seed).expect("seeded key");
        let message = b"sm2 roundtrip";
        let signature = sk.sign(message);
        let pk = sk.public_key();
        prop_assert!(pk.verify(message, &signature).is_ok());
    }

    #[test]
    fn sm2_compute_z_matches_signing_key(
        seed in any::<[u8; 32]>(),
        distid_entropy in any::<[u8; 8]>(),
        message in vec(any::<u8>(), 0..64),
    ) {
        let distid = format!(
            "device:{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            distid_entropy[0],
            distid_entropy[1],
            distid_entropy[2],
            distid_entropy[3],
            distid_entropy[4],
            distid_entropy[5],
            distid_entropy[6],
            distid_entropy[7]
        );
        let private = match Sm2PrivateKey::from_seed(&distid, &seed) {
            Ok(private) => private,
            Err(_) => return Ok(()),
        };
        let public = private.public_key();
        let za = public
            .compute_z(&distid)
            .expect("compute ZA for generated key");

        let msg: &[u8] = if message.is_empty() {
            &[0u8][..]
        } else {
            message.as_slice()
        };
        let signature = private.sign(msg);

        let mut hasher = Sm3::new();
        hasher.update(za);
        hasher.update(msg);
        let digest = hasher.finalize();

        let raw = Sm2RawSignature::from_bytes(&signature.as_bytes())
            .expect("signature converts to raw SM2 form");
        public
            .as_inner()
            .verify_prehash(digest.as_slice(), &raw)
            .expect("prehash verification must succeed");
    }

    #[test]
    fn sm2_upstream_negative_vectors_fail(vector in negative_vector_strategy()) {
        let private = Sm2PrivateKey::from_seed(
            Sm2PublicKey::DEFAULT_DISTID,
            b"sm2-negative-vectors",
        )
        .expect("deterministic key");
        let outcome = apply_mutation(&vector, &private);
        if outcome.public_parse_failed {
            prop_assert!(true);
            return Ok(());
        }
        if outcome.expect_signature_parse_error {
            if let Ok(bytes) = outcome.signature_bytes.clone().try_into() {
                prop_assert!(
                    Sm2Signature::from_bytes(&bytes).is_err(),
                    "Negative vector `{}` should fail to parse",
                    vector.label
                );
            } else {
                prop_assert!(outcome.signature_bytes.len() != Sm2Signature::LENGTH);
            }
            return Ok(());
        }

        let sig_bytes: [u8; Sm2Signature::LENGTH] = outcome
            .signature_bytes
            .clone()
            .try_into()
            .expect("mutation should retain 64-byte signature");
        let signature = Sm2Signature::from_bytes(&sig_bytes)
            .expect("mutation marked for verification should parse");
        let verify_result = outcome
            .public_key
            .verify(&outcome.verify_message, &signature);
        prop_assert!(
            verify_result.is_err(),
            "Negative vector `{}` unexpectedly verified",
            vector.label
        );
    }

    #[test]
    fn sm2_wycheproof_cases_hold(case in wycheproof_case_strategy()) {
        let public = match Sm2PublicKey::from_sec1_bytes(&case.distid, &case.public_sec1) {
            Ok(public) => public,
            Err(_) => return Ok(()),
        };
        match Sm2Signature::from_der(&case.signature_der) {
            Ok(signature) => {
                let verify = public.verify(&case.message, &signature);
                if case.expect_valid {
                    prop_assert!(verify.is_ok());
                } else {
                    prop_assert!(verify.is_err());
                }
            }
            Err(_) => {
                prop_assert!(!case.expect_valid);
            }
        }
    }

    #[test]
    fn sm2_wycheproof_detects_random_tampering(
        input in wycheproof_valid_case_strategy().prop_flat_map(|case| {
            (
                Just(case),
                prop_oneof![Just(true), Just(false)],
                any::<usize>(),
                any::<u8>(),
            )
        })
    ) {
        let (case, tamper_message, flip_idx, flip_mask) = input;
        let WycheproofCase { distid, public_sec1, message, signature_der, .. } = case;
        let public = match Sm2PublicKey::from_sec1_bytes(&distid, &public_sec1) {
            Ok(public) => public,
            Err(_) => return Ok(()),
        };

        let signature = Sm2Signature::from_der(&signature_der).expect("valid Wycheproof signature");

        // Decide whether to tamper with the message or the signature; fall back to signature tampering
        // when the message is empty (so we always exercise a mutation).
        let mut tampered_message = message.clone();
        let mut tampered_signature = signature_der.clone();

        if tamper_message && !tampered_message.is_empty() {
            let idx = flip_idx % tampered_message.len();
            let mask = if flip_mask == 0 { 0x80 } else { flip_mask };
            tampered_message[idx] ^= mask;
            let verify = public.verify(&tampered_message, &signature);
            prop_assert!(verify.is_err());
        } else {
            let idx = flip_idx % tampered_signature.len();
            let mask = if flip_mask == 0 { 0x01 } else { flip_mask };
            tampered_signature[idx] ^= mask;
            if let Ok(tampered) = Sm2Signature::from_der(&tampered_signature) {
                let verify = public.verify(&message, &tampered);
                prop_assert!(verify.is_err());
            }
        }
    }

    #[test]
    fn sm2_truncated_signature_is_rejected(case in wycheproof_valid_case_strategy()) {
        let public = match Sm2PublicKey::from_sec1_bytes(&case.distid, &case.public_sec1) {
            Ok(public) => public,
            Err(_) => return Ok(()),
        };
        let mut truncated = case.signature_der.clone();
        if truncated.len() < 2 {
            return Ok(());
        }
        truncated.truncate(truncated.len() - 2);
        match Sm2Signature::from_der(&truncated) {
            Ok(sig) => {
                let verify = public.verify(&case.message, &sig);
                prop_assert!(verify.is_err());
            }
            Err(_) => prop_assert!(true),
        }
    }

    #[test]
    fn sm2_bitflip_signature_is_rejected(case in wycheproof_valid_case_strategy()) {
        let public = match Sm2PublicKey::from_sec1_bytes(&case.distid, &case.public_sec1) {
            Ok(public) => public,
            Err(_) => return Ok(()),
        };
        let mut tampered = case.signature_der.clone();
        if tampered.is_empty() {
            return Ok(());
        }
        let idx = tampered.len() - 1;
        tampered[idx] ^= if tampered[idx] == 0 { 0x01 } else { 0x80 };
        match Sm2Signature::from_der(&tampered) {
            Ok(sig) => {
                let verify = public.verify(&case.message, &sig);
                prop_assert!(verify.is_err());
            }
            Err(_) => prop_assert!(true),
        }
    }
}
