//! Compliance tests for confidential key-derivation fixtures.

use std::path::Path;

use base64::{Engine, engine::general_purpose::STANDARD};
use hex::FromHex;
use iroha_crypto::{ConfidentialKeyError, derive_keyset, derive_keyset_from_slice};
use norito::json::{self, JsonDeserialize};

#[derive(Debug, JsonDeserialize)]
struct Fixture {
    format_version: u32,
    cases: Cases,
}

#[derive(Debug, JsonDeserialize)]
struct Cases {
    positive: Vec<PositiveCase>,
    negative: Vec<NegativeCase>,
}

#[derive(Debug, JsonDeserialize)]
struct PositiveCase {
    case_id: String,
    seed_hex: String,
    seed_base64: String,
    derived: Derived,
}

#[derive(Debug, JsonDeserialize)]
struct Derived {
    nullifier_key_hex: String,
    nullifier_key_base64: String,
    incoming_view_key_hex: String,
    incoming_view_key_base64: String,
    outgoing_view_key_hex: String,
    outgoing_view_key_base64: String,
    full_view_key_hex: String,
    full_view_key_base64: String,
}

#[derive(Debug, JsonDeserialize)]
struct NegativeCase {
    case_id: String,
    seed_hex: String,
    expected_error: ExpectedError,
}

#[derive(Debug, JsonDeserialize)]
struct ExpectedError {
    kind: String,
    length_bytes: Option<usize>,
}

#[test]
fn confidential_keyset_vectors_validate() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures/confidential/keyset_derivation_v1.json");
    let data = std::fs::read_to_string(&fixture_path).expect("fixture file must load");
    let root: Fixture = json::from_str(&data).expect("fixture must parse");
    assert_eq!(root.format_version, 1, "unsupported fixture version");

    for case in root.cases.positive {
        assert_positive_case(&case);
    }

    for case in root.cases.negative {
        assert_negative_case(&case);
    }
}

fn assert_positive_case(case: &PositiveCase) {
    let seed = decode_hex_array::<32>(&case.seed_hex);
    let seed_from_b64 = decode_base64(&case.seed_base64);
    assert_eq!(
        seed.as_slice(),
        seed_from_b64.as_slice(),
        "{}: seed hex/base64 mismatch",
        case.case_id
    );

    let keyset = derive_keyset(seed);
    assert_eq!(
        hex::encode(keyset.nullifier_key()),
        case.derived.nullifier_key_hex.to_ascii_lowercase(),
        "{}: nullifier key mismatch",
        case.case_id
    );
    assert_eq!(
        hex::encode(keyset.incoming_view_key()),
        case.derived.incoming_view_key_hex.to_ascii_lowercase(),
        "{}: incoming view key mismatch",
        case.case_id
    );
    assert_eq!(
        hex::encode(keyset.outgoing_view_key()),
        case.derived.outgoing_view_key_hex.to_ascii_lowercase(),
        "{}: outgoing view key mismatch",
        case.case_id
    );
    assert_eq!(
        hex::encode(keyset.full_view_key()),
        case.derived.full_view_key_hex.to_ascii_lowercase(),
        "{}: full view key mismatch",
        case.case_id
    );

    assert_eq!(
        keyset.nullifier_key(),
        decode_base64(&case.derived.nullifier_key_base64).as_slice(),
        "{}: nullifier key base64 mismatch",
        case.case_id
    );
    assert_eq!(
        keyset.incoming_view_key(),
        decode_base64(&case.derived.incoming_view_key_base64).as_slice(),
        "{}: incoming view key base64 mismatch",
        case.case_id
    );
    assert_eq!(
        keyset.outgoing_view_key(),
        decode_base64(&case.derived.outgoing_view_key_base64).as_slice(),
        "{}: outgoing view key base64 mismatch",
        case.case_id
    );
    assert_eq!(
        keyset.full_view_key(),
        decode_base64(&case.derived.full_view_key_base64).as_slice(),
        "{}: full view key base64 mismatch",
        case.case_id
    );
}

fn assert_negative_case(case: &NegativeCase) {
    let seed_bytes = decode_hex_vec(&case.seed_hex);
    let err = derive_keyset_from_slice(&seed_bytes)
        .expect_err("expected derivation failure for negative case");
    match (&case.expected_error.kind[..], err) {
        ("invalid_length", ConfidentialKeyError::InvalidSpendKeyLength(actual_len)) => {
            if let Some(expected_len) = case.expected_error.length_bytes {
                assert_eq!(
                    actual_len, expected_len,
                    "{}: length mismatch",
                    case.case_id
                );
            }
        }
        (other, unexpected) => panic!("{other}: unexpected error {unexpected:?}"),
    }
}

fn decode_hex_array<const N: usize>(hex_value: &str) -> [u8; N] {
    let bytes = decode_hex_vec(hex_value);
    let len = bytes.len();
    bytes
        .try_into()
        .unwrap_or_else(|_| panic!("expected {N} bytes, got {len}"))
}

fn decode_hex_vec(hex_value: &str) -> Vec<u8> {
    Vec::from_hex(hex_value).expect("hex payload must decode")
}

fn decode_base64(value: &str) -> Vec<u8> {
    let mut out = vec![0u8; base64::decoded_len_estimate(value.len())];
    let written = STANDARD
        .decode_slice(value.as_bytes(), &mut out)
        .expect("base64 must decode");
    out.truncate(written);
    out
}
