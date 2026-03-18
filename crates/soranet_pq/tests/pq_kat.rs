//! Known-answer regression tests for the `SoraNet` PQ helpers.
#![allow(clippy::unwrap_used)]

use norito::json::{self, Map, Value};
use soranet_pq::{MlDsaSuite, MlKemSuite, decapsulate_mlkem, sign_mldsa, verify_mldsa};

#[test]
fn mlkem_vectors_decapsulate_to_expected_shared_secret() {
    let root = load_vectors();
    let entries = root
        .get("mlkem")
        .and_then(Value::as_array)
        .expect("mlkem vectors array");
    for vector in entries {
        let obj = vector.as_object().expect("mlkem vector object");
        let suite = obj
            .get("suite")
            .and_then(Value::as_str)
            .map(parse_mlkem_suite)
            .expect("suite label");
        let secret = decode_hex_field(obj, "secret_key");
        let ciphertext = decode_hex_field(obj, "ciphertext");
        let expected_secret = decode_hex_field(obj, "shared_secret");
        let shared = decapsulate_mlkem(suite, &secret, &ciphertext)
            .expect("decapsulation succeeds for fixture");
        assert_eq!(shared.as_bytes(), expected_secret.as_slice());
    }
}

#[test]
fn mldsa_vectors_verify_and_resign() {
    let root = load_vectors();
    let entries = root
        .get("mldsa")
        .and_then(Value::as_array)
        .expect("mldsa vectors array");
    for vector in entries {
        let obj = vector.as_object().expect("mldsa vector object");
        let suite = obj
            .get("suite")
            .and_then(Value::as_str)
            .map(parse_mldsa_suite)
            .expect("suite label");
        let public_key = decode_hex_field(obj, "public_key");
        let secret_key = decode_hex_field(obj, "secret_key");
        let message = decode_hex_field(obj, "message");
        let signature = decode_hex_field(obj, "signature");

        verify_mldsa(suite, &public_key, &message, &signature).expect("fixture signature verifies");

        let regenerated =
            sign_mldsa(suite, &secret_key, &message).expect("fixture signing succeeds");
        assert_eq!(regenerated.as_bytes(), signature.as_slice());
    }
}

fn load_vectors() -> Value {
    let bytes = include_bytes!("fixtures/pq_vectors.json");
    json::from_slice(bytes).expect("decode pq vectors JSON")
}

fn decode_hex_field(obj: &Map, key: &str) -> Vec<u8> {
    let value = obj
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing {key}"));
    decode_hex(value)
}

fn parse_mlkem_suite(label: &str) -> MlKemSuite {
    match label {
        "mlkem512" => MlKemSuite::MlKem512,
        "mlkem768" => MlKemSuite::MlKem768,
        "mlkem1024" => MlKemSuite::MlKem1024,
        other => panic!("unknown ML-KEM suite {other}"),
    }
}

fn parse_mldsa_suite(label: &str) -> MlDsaSuite {
    match label {
        "mldsa44" => MlDsaSuite::MlDsa44,
        "mldsa65" => MlDsaSuite::MlDsa65,
        "mldsa87" => MlDsaSuite::MlDsa87,
        other => panic!("unknown ML-DSA suite {other}"),
    }
}

fn decode_hex(input: &str) -> Vec<u8> {
    assert!(
        input.len().is_multiple_of(2),
        "hex input must have even length"
    );
    let mut bytes = Vec::with_capacity(input.len() / 2);
    let raw = input.as_bytes();
    for chunk in raw.chunks(2) {
        let hi = decode_nibble(chunk[0]);
        let lo = decode_nibble(chunk[1]);
        bytes.push((hi << 4) | lo);
    }
    bytes
}

fn decode_nibble(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        other => panic!("invalid hex nibble {other:#x}"),
    }
}
