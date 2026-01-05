#![doc = "Cross-SDK SM2 fixture validation."]
#![cfg(feature = "sm")]

use std::{fs, path::PathBuf};

use hex::FromHex;
use iroha_crypto::{
    Algorithm, PublicKey,
    sm::{Sm2PrivateKey, Sm2PublicKey, Sm2Signature, encode_sm2_public_key_payload},
};
use norito::json::{self, JsonDeserialize};
use sm3::{Digest, Sm3};

#[derive(Debug, JsonDeserialize)]
struct FixtureRoot {
    algorithm: String,
    distid: String,
    seed_hex: String,
    message_hex: String,
    private_key_hex: String,
    public_key_sec1_hex: String,
    public_key_multihash: String,
    public_key_prefixed: String,
    za: String,
    signature: String,
    r: String,
    s: String,
    vectors: Option<Vec<FixtureCase>>,
}

#[derive(Debug, JsonDeserialize)]
struct FixtureCase {
    case_id: String,
    note: Option<String>,
    distid: String,
    message_hex: String,
    public_key_sec1_hex: String,
    public_key_multihash: Option<String>,
    public_key_prefixed: Option<String>,
    signature: String,
    r: Option<String>,
    s: Option<String>,
    za: Option<String>,
    private_key_hex: Option<String>,
    seed_hex: Option<String>,
    signature_der_hex: Option<String>,
    hash_e_hex: Option<String>,
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("sm")
        .join("sm2_fixture.json")
}

fn hex_to_vec(value: &str) -> Vec<u8> {
    if value.is_empty() {
        Vec::new()
    } else {
        Vec::from_hex(value).unwrap_or_else(|err| panic!("invalid hex '{value}': {err}"))
    }
}

fn hex_to_array<const N: usize>(value: &str) -> [u8; N] {
    let bytes = hex_to_vec(value);
    let len = bytes.len();
    bytes
        .try_into()
        .unwrap_or_else(|_| panic!("expected {N} bytes but received {len}"))
}

fn assert_root_alignment(root: &FixtureRoot, default: &FixtureCase) {
    let default_seed = default
        .seed_hex
        .as_deref()
        .expect("default SM2 vector provides seed_hex");
    assert_eq!(
        root.seed_hex.as_str(),
        default_seed,
        "root seed must mirror default vector"
    );
    assert_eq!(
        root.message_hex.as_str(),
        default.message_hex.as_str(),
        "root message must mirror default vector"
    );
    let default_private = default
        .private_key_hex
        .as_deref()
        .expect("default SM2 vector provides private_key_hex");
    assert_eq!(
        root.private_key_hex.as_str(),
        default_private,
        "root private key must mirror default vector"
    );
    assert_eq!(
        root.public_key_sec1_hex.as_str(),
        default.public_key_sec1_hex.as_str(),
        "root SEC1 public key must mirror default vector"
    );
    let default_multihash = default
        .public_key_multihash
        .as_deref()
        .expect("default SM2 vector provides public_key_multihash");
    assert_eq!(
        root.public_key_multihash.as_str(),
        default_multihash,
        "root multihash must mirror default vector"
    );
    let default_prefixed = default
        .public_key_prefixed
        .as_deref()
        .expect("default SM2 vector provides public_key_prefixed");
    assert_eq!(
        root.public_key_prefixed.as_str(),
        default_prefixed,
        "root prefixed public key must mirror default vector"
    );
    let default_za = default
        .za
        .as_deref()
        .expect("default SM2 vector provides ZA digest");
    assert_eq!(
        root.za.as_str(),
        default_za,
        "root ZA must mirror default vector"
    );
    assert_eq!(
        root.signature.as_str(),
        default.signature.as_str(),
        "root signature must mirror default vector"
    );
    let default_r = default
        .r
        .as_deref()
        .expect("default SM2 vector provides r component");
    assert_eq!(
        root.r.as_str(),
        default_r,
        "root r component must mirror default vector"
    );
    let default_s = default
        .s
        .as_deref()
        .expect("default SM2 vector provides s component");
    assert_eq!(
        root.s.as_str(),
        default_s,
        "root s component must mirror default vector"
    );
}

fn verify_public_encodings(case_id: &str, public_sec1: &[u8], case: &FixtureCase) {
    if let Some(multihash_expected) = &case.public_key_multihash {
        let payload = encode_sm2_public_key_payload(&case.distid, public_sec1)
            .unwrap_or_else(|err| panic!("case {case_id}: invalid SM2 payload: {err}"));
        let derived = PublicKey::from_bytes(Algorithm::Sm2, &payload)
            .unwrap_or_else(|err| panic!("case {case_id}: cannot derive multihash: {err}"));
        assert_eq!(
            derived.to_string(),
            multihash_expected.as_str(),
            "case {case_id}: public key multihash mismatch"
        );
        if let Some(prefixed_expected) = &case.public_key_prefixed {
            assert_eq!(
                derived.to_prefixed_string(),
                prefixed_expected.as_str(),
                "case {case_id}: prefixed public key mismatch"
            );
        }
    }
}

fn verify_signature_artifacts(
    case_id: &str,
    case: &FixtureCase,
    message: &[u8],
    public_sec1: &[u8],
    public: &Sm2PublicKey,
) {
    let signature_bytes = hex_to_array::<{ Sm2Signature::LENGTH }>(&case.signature);
    let signature = Sm2Signature::from_bytes(&signature_bytes)
        .unwrap_or_else(|err| panic!("case {case_id}: invalid SM2 signature bytes: {err}"));

    public
        .verify(message, &signature)
        .unwrap_or_else(|err| panic!("case {case_id}: signature verification failed: {err}"));

    if let Some(za_expected) = case.za.as_deref() {
        let za = public
            .compute_z(&case.distid)
            .unwrap_or_else(|err| panic!("case {case_id}: compute ZA failed: {err}"));
        assert_eq!(
            hex::encode_upper(za),
            za_expected,
            "case {case_id}: ZA mismatch"
        );
    }

    if let Some(r_hex) = case.r.as_deref() {
        assert_eq!(
            hex::encode_upper(&signature_bytes[..Sm2Signature::LENGTH / 2]),
            r_hex,
            "case {case_id}: r component mismatch"
        );
    }
    if let Some(s_hex) = case.s.as_deref() {
        assert_eq!(
            hex::encode_upper(&signature_bytes[Sm2Signature::LENGTH / 2..]),
            s_hex,
            "case {case_id}: s component mismatch"
        );
    }

    if let Some(der_hex) = case.signature_der_hex.as_deref() {
        let der = hex_to_vec(der_hex);
        let parsed = Sm2Signature::from_der(&der)
            .unwrap_or_else(|err| panic!("case {case_id}: DER parse failed: {err}"));
        assert_eq!(
            parsed.to_bytes(),
            signature_bytes,
            "case {case_id}: DER vs r∥s payload mismatch"
        );
    }

    if let Some(seed_hex) = case.seed_hex.as_deref() {
        let seed = hex_to_vec(seed_hex);
        let private = Sm2PrivateKey::from_seed(&case.distid, &seed)
            .unwrap_or_else(|err| panic!("case {case_id}: seed derivation failed: {err}"));
        let signed = private.sign(message);
        assert_eq!(
            signed.to_bytes(),
            signature_bytes,
            "case {case_id}: deterministic seed signing mismatch"
        );
        assert_eq!(
            private.public_key().to_sec1_bytes(false).as_slice(),
            public_sec1,
            "case {case_id}: derived public key mismatch"
        );
    }

    if let Some(private_hex) = case.private_key_hex.as_deref() {
        let private_bytes = hex_to_array::<32>(private_hex);
        let private = Sm2PrivateKey::from_bytes(&case.distid, &private_bytes)
            .unwrap_or_else(|err| panic!("case {case_id}: private key decode failed: {err}"));
        let signed = private.sign(message);
        assert_eq!(
            signed.to_bytes(),
            signature_bytes,
            "case {case_id}: private-key signing mismatch"
        );
        assert_eq!(
            private.public_key().to_sec1_bytes(false).as_slice(),
            public_sec1,
            "case {case_id}: reconstructed public key mismatch"
        );
    }

    if let Some(expected_hash_e) = case.hash_e_hex.as_deref() {
        let za = public
            .compute_z(&case.distid)
            .unwrap_or_else(|err| panic!("case {case_id}: compute ZA failed: {err}"));
        let mut hasher = Sm3::new();
        hasher.update(za);
        hasher.update(message);
        let digest = hasher.finalize();
        assert_eq!(
            hex::encode_upper(digest),
            expected_hash_e,
            "case {case_id}: SM2 digest e mismatch"
        );
    }
}

fn verify_fixture_case(case: &FixtureCase) {
    let case_id = &case.case_id;
    if let Some(note) = case.note.as_ref() {
        assert!(
            !note.trim().is_empty(),
            "case {case_id}: note must not be empty when present"
        );
    }

    let message = hex_to_vec(&case.message_hex);
    let public_sec1 = hex_to_vec(&case.public_key_sec1_hex);
    let public = Sm2PublicKey::from_sec1_bytes(&case.distid, &public_sec1)
        .unwrap_or_else(|err| panic!("case {case_id}: invalid SEC1 public key: {err}"));

    verify_public_encodings(case_id, &public_sec1, case);
    verify_signature_artifacts(case_id, case, &message, &public_sec1, &public);
}

#[test]
fn sm2_vectors_cover_cross_sdk_fixtures() {
    let payload = fs::read_to_string(fixture_path()).expect("fixture present");
    let mut root: FixtureRoot = json::from_str(&payload).expect("valid SM2 fixture JSON");
    assert_eq!(root.algorithm, "sm2", "fixture algorithm mismatch");

    let vectors = root.vectors.take().unwrap_or_default();
    assert!(
        vectors.len() >= 2,
        "expected at least two cross-SDK SM2 vectors"
    );
    let default_vector = vectors
        .iter()
        .find(|case| case.case_id == "sm2-fixture-default-v1")
        .expect("default fixture vector missing");
    assert_eq!(
        root.distid, default_vector.distid,
        "root distid must mirror default vector"
    );
    assert_eq!(
        root.signature, default_vector.signature,
        "root signature must mirror default vector"
    );
    assert_root_alignment(&root, default_vector);

    for case in &vectors {
        verify_fixture_case(case);
    }
}
