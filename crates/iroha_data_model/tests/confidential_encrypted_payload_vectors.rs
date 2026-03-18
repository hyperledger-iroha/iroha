//! Compliance tests for confidential encrypted payload fixtures.

use std::path::Path;

use hex::FromHex;
use iroha_data_model::confidential::ConfidentialEncryptedPayload;
use norito::{
    codec::{decode_adaptive, encode_adaptive},
    json::{self, JsonDeserialize},
};

#[derive(Debug, JsonDeserialize)]
struct Fixture {
    format_version: u32,
    cases: CaseSets,
}

#[derive(Debug, JsonDeserialize)]
struct CaseSets {
    positive: Vec<PositiveCase>,
    negative: Vec<NegativeCase>,
}

#[derive(Debug, JsonDeserialize)]
struct PositiveCase {
    case_id: String,
    version: u8,
    ephemeral_public_key_hex: String,
    nonce_hex: String,
    ciphertext_hex: String,
    serialized_hex: String,
}

#[derive(Debug, JsonDeserialize)]
struct NegativeCase {
    case_id: String,
    serialized_hex: String,
    expected_error: ExpectedError,
}

#[derive(Debug, JsonDeserialize)]
struct ExpectedError {
    mode: String,
    kind: String,
    version: Option<u8>,
}

#[test]
fn encrypted_payload_vectors_validate() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures/confidential/encrypted_payload_v1.json");
    let data = std::fs::read_to_string(&fixture_path).expect("fixture file must load");
    let root: Fixture = json::from_str(&data).expect("fixture must parse");
    assert_eq!(root.format_version, 1, "unsupported fixture version");

    for case in &root.cases.positive {
        assert_positive_case(case);
    }

    for case in &root.cases.negative {
        assert_negative_case(case);
    }
}

fn assert_positive_case(case: &PositiveCase) {
    let ephemeral = decode_hex_array::<32>(&case.ephemeral_public_key_hex);
    let nonce = decode_hex_array::<24>(&case.nonce_hex);
    let ciphertext = decode_hex_vec(&case.ciphertext_hex);

    let payload = ConfidentialEncryptedPayload::new(ephemeral, nonce, ciphertext.clone());
    assert_eq!(
        payload.version(),
        case.version,
        "{}: unexpected payload version",
        case.case_id
    );
    assert_eq!(
        payload.ciphertext(),
        ciphertext.as_slice(),
        "{}: ciphertext mismatch",
        case.case_id
    );

    let encoded = encode_adaptive(&payload);
    let actual_hex = hex::encode(&encoded);
    assert_eq!(
        actual_hex,
        case.serialized_hex.to_ascii_lowercase(),
        "{}: serialized bytes mismatch",
        case.case_id
    );

    let decoded: ConfidentialEncryptedPayload =
        decode_adaptive(&encoded).expect("positive decode should succeed");
    assert_eq!(
        decoded, payload,
        "{}: round-trip payload mismatch",
        case.case_id
    );
}

fn assert_negative_case(case: &NegativeCase) {
    let bytes = decode_hex_vec(&case.serialized_hex);
    match case.expected_error.mode.as_str() {
        "unsupported_version" => {
            let decoded: ConfidentialEncryptedPayload =
                decode_adaptive(&bytes).expect("unsupported version still decodes");
            assert_eq!(
                decoded.version(),
                case.expected_error
                    .version
                    .expect("fixture must include version"),
                "{}: decoded version mismatch",
                case.case_id
            );
            assert!(
                !decoded.is_supported(),
                "{}: payload unexpectedly marked as supported",
                case.case_id
            );
        }
        "decode_error" => {
            let err = decode_adaptive::<ConfidentialEncryptedPayload>(&bytes)
                .expect_err("decode must fail for malformed payloads");
            assert!(
                err.to_string()
                    .to_ascii_lowercase()
                    .contains(&case.expected_error.kind.to_ascii_lowercase()),
                "{}: unexpected error '{err}'",
                case.case_id
            );
        }
        other => panic!("{other}: unsupported negative-case mode"),
    }
}

fn decode_hex_vec(hex_value: &str) -> Vec<u8> {
    Vec::from_hex(hex_value).expect("hex payload must decode")
}

fn decode_hex_array<const N: usize>(hex_value: &str) -> [u8; N] {
    let bytes = decode_hex_vec(hex_value);
    let len = bytes.len();
    bytes
        .try_into()
        .unwrap_or_else(|_| panic!("expected {N} bytes, got {len}"))
}
