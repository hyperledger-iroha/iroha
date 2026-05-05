//! Compliance-suite validation for account address vectors.

use std::path::Path;

use hex::{FromHex, encode_upper};
use iroha_crypto::{Algorithm, PublicKey};
use iroha_data_model::account::{
    AccountAddress, AccountAddressError, AccountId, MultisigMember, MultisigPolicy,
};
use norito::json::{self, JsonDeserialize};

#[derive(Debug, JsonDeserialize)]
struct Root {
    format_version: u32,
    default_network_prefix: u16,
    cases: CaseSets,
}

#[derive(Debug, JsonDeserialize)]
struct CaseSets {
    positive: Vec<PositiveCase>,
    negative: Vec<NegativeCase>,
}

#[allow(dead_code)]
#[derive(Debug, JsonDeserialize)]
struct PositiveCase {
    case_id: String,
    category: String,
    note: Option<String>,
    input: PositiveInput,
    selector: Selector,
    controller: Controller,
    encodings: Encodings,
}

#[allow(dead_code)]
#[derive(Debug, JsonDeserialize)]
struct PositiveInput {
    raw_domain: Option<String>,
    normalized_domain: Option<String>,
    seed_byte: Option<u8>,
    registry_id: Option<u32>,
    equivalent_domain: Option<String>,
    member_keys_hex: Option<Vec<String>>,
    member_weights: Option<Vec<u16>>,
    threshold: Option<u16>,
}

#[allow(dead_code)]
#[derive(Debug, JsonDeserialize)]
struct Selector {
    kind: String,
    digest_hex: Option<String>,
    registry_id: Option<u32>,
    domain_equivalents: Option<Vec<String>>,
}

#[allow(dead_code)]
#[derive(Debug, JsonDeserialize)]
struct Controller {
    kind: String,
    curve: Option<String>,
    public_key_hex: Option<String>,
    version: Option<u8>,
    threshold: Option<u16>,
    members: Option<Vec<Member>>,
    ctap2_cbor_hex: Option<String>,
    digest_blake2b256_hex: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, JsonDeserialize)]
struct Member {
    curve: String,
    weight: u16,
    public_key_hex: String,
}

#[allow(dead_code)]
#[derive(Debug, JsonDeserialize)]
struct Encodings {
    canonical_hex: String,
    i105: I105Encoding,
}

#[allow(dead_code)]
#[derive(Debug, JsonDeserialize)]
struct I105Encoding {
    prefix: u16,
    string: String,
}

#[allow(dead_code)]
#[derive(Debug, JsonDeserialize)]
struct NegativeCase {
    case_id: String,
    format: String,
    input: String,
    note: Option<String>,
    expected_prefix: Option<u16>,
    expected_error: ExpectedError,
}

#[allow(dead_code)]
#[derive(Debug, JsonDeserialize)]
struct ExpectedError {
    kind: String,
    expected: Option<u16>,
    found: Option<u16>,
    char: Option<String>,
    policy_error: Option<String>,
}

fn decode_canonical(hex_value: &str) -> Vec<u8> {
    let body = hex_value
        .strip_prefix("0x")
        .unwrap_or(hex_value)
        .to_ascii_lowercase();
    Vec::from_hex(body.as_str()).expect("canonical hex decode")
}

fn canonical_bytes_from_address(address: &AccountAddress) -> Vec<u8> {
    let canonical_hex = address
        .canonical_hex()
        .expect("canonical address encoding should succeed");
    decode_canonical(&canonical_hex)
}

fn ed25519_public_key(hex_value: &str) -> PublicKey {
    PublicKey::from_hex(Algorithm::Ed25519, hex_value).expect("valid ed25519 public key payload")
}

#[test]
fn account_address_vectors_validate() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures/account/address_vectors.json");
    let data = std::fs::read_to_string(&fixture_path).expect("fixture must read");
    let root: Root = json::from_str(&data).expect("fixture must parse");
    assert_eq!(root.format_version, 1, "unexpected format version");

    for case in &root.cases.positive {
        validate_positive_case(case, root.default_network_prefix);
    }

    for case in &root.cases.negative {
        validate_negative_case(case, root.default_network_prefix);
    }
}

#[allow(clippy::too_many_lines)]
fn validate_positive_case(case: &PositiveCase, default_prefix: u16) {
    let canonical_bytes = decode_canonical(&case.encodings.canonical_hex);
    let canonical_address =
        AccountAddress::from_canonical_bytes(&canonical_bytes).expect("canonical decode");

    // I105 decoding (legacy fixture key: `i105`)
    let i105_addr = AccountAddress::from_i105_for_discriminant(
        &case.encodings.i105.string,
        Some(case.encodings.i105.prefix),
    )
    .expect("i105 decode");
    assert_eq!(
        i105_addr, canonical_address,
        "{} i105 payload mismatch",
        case.case_id
    );
    assert_eq!(
        canonical_bytes_from_address(&i105_addr),
        canonical_bytes,
        "{} i105 canonical mismatch",
        case.case_id
    );
    assert_eq!(
        case.encodings.i105.prefix, default_prefix,
        "{} i105 uses unexpected prefix",
        case.case_id
    );

    // Strict parser must reject canonical hex literals.
    let err = AccountAddress::parse_encoded(&case.encodings.canonical_hex, None)
        .expect_err("canonical hex must be rejected by strict parser");
    assert!(
        matches!(err, AccountAddressError::UnsupportedAddressFormat),
        "{} canonical hex should be unsupported",
        case.case_id
    );

    // Ensure canonical hex rendering matches fixture.
    let rendered_hex = canonical_address
        .canonical_hex()
        .expect("render canonical hex");
    assert_eq!(
        rendered_hex.to_ascii_lowercase(),
        case.encodings.canonical_hex.to_ascii_lowercase(),
        "{} canonical hex mismatch",
        case.case_id
    );

    match case.category.as_str() {
        "single" => validate_single_case(case, &canonical_address),
        "multisig" => validate_multisig_case(case, &canonical_address),
        other => panic!("unknown positive category {other}"),
    }

    assert_eq!(
        case.selector.kind, "default",
        "{} selector kind mismatch",
        case.case_id
    );
    assert!(
        case.selector.digest_hex.is_none(),
        "{} selector digest should be absent for selector-free canonical payloads",
        case.case_id
    );
    assert!(
        case.selector.registry_id.is_none(),
        "{} selector registry should be absent for selector-free canonical payloads",
        case.case_id
    );
    assert!(
        case.selector.domain_equivalents.is_none(),
        "{} selector equivalents should be absent for selector-free canonical payloads",
        case.case_id
    );
}

fn validate_single_case(case: &PositiveCase, address: &AccountAddress) {
    if let Some(_raw_domain) = case.input.raw_domain.as_deref() {
        let public_key_hex = case
            .controller
            .public_key_hex
            .as_ref()
            .expect("single controllers provide public key");
        let public_key = ed25519_public_key(public_key_hex);
        let account_id = AccountId::new(public_key.clone());
        let rebuilt =
            AccountAddress::from_account_id(&account_id).expect("rebuild single address succeeds");
        assert_eq!(
            rebuilt, *address,
            "{} single-key canonical mismatch",
            case.case_id
        );
    }
}

fn validate_multisig_case(case: &PositiveCase, address: &AccountAddress) {
    let members_hex = case
        .input
        .member_keys_hex
        .as_ref()
        .expect("multisig input must provide member keys");
    let weights = case
        .input
        .member_weights
        .as_ref()
        .expect("multisig input must provide weights");
    assert_eq!(
        members_hex.len(),
        weights.len(),
        "{} member/weight length mismatch",
        case.case_id
    );
    let mut members = Vec::with_capacity(members_hex.len());
    for (hex_key, &weight) in members_hex.iter().zip(weights.iter()) {
        let public_key = ed25519_public_key(hex_key);
        members
            .push(MultisigMember::new(public_key, weight).expect("multisig member must construct"));
    }
    let threshold = case
        .input
        .threshold
        .expect("multisig input must supply threshold");
    let policy = MultisigPolicy::new(threshold, members)
        .expect("multisig policy must construct from fixture");
    if let Some(version) = case.controller.version {
        assert_eq!(
            version,
            policy.version(),
            "{} controller version mismatch",
            case.case_id
        );
    }
    if let Some(ctrl_threshold) = case.controller.threshold {
        assert_eq!(
            ctrl_threshold,
            policy.threshold(),
            "{} controller threshold mismatch",
            case.case_id
        );
    }
    if let Some(expected_hex) = case.controller.ctap2_cbor_hex.as_deref() {
        let encoded = policy.encode_ctap2();
        let expected = format!("0x{}", encode_upper(&encoded)).to_ascii_uppercase();
        assert_eq!(
            expected_hex.to_ascii_uppercase(),
            expected,
            "{} CTAP2 CBOR hex mismatch",
            case.case_id
        );
    }
    if let Some(expected_digest) = case.controller.digest_blake2b256_hex.as_deref() {
        let digest = policy.digest_blake2b256();
        let expected = format!("0x{}", encode_upper(digest)).to_ascii_uppercase();
        assert_eq!(
            expected_digest.to_ascii_uppercase(),
            expected,
            "{} policy digest mismatch",
            case.case_id
        );
    }

    let account = AccountId::new_multisig(policy);
    let rebuilt =
        AccountAddress::from_account_id(&account).expect("multisig address reconstruction");
    assert_eq!(
        rebuilt, *address,
        "{} multisig canonical mismatch",
        case.case_id
    );
}

fn validate_negative_case(case: &NegativeCase, default_prefix: u16) {
    match case.format.as_str() {
        "i105" => {
            let expected_prefix = case.expected_prefix.unwrap_or(default_prefix);
            let err =
                AccountAddress::from_i105_for_discriminant(&case.input, Some(expected_prefix))
                    .expect_err("i105 case should fail");
            assert_error(&err, &case.expected_error, &case.case_id);
        }
        "canonical_hex" => {
            let err = AccountAddress::parse_encoded(&case.input, None)
                .expect_err("canonical case should fail");
            assert_error(&err, &case.expected_error, &case.case_id);
        }
        other => panic!("unknown negative format {other}"),
    }
}

fn assert_error(err: &AccountAddressError, expected: &ExpectedError, case_id: &str) {
    match expected.kind.as_str() {
        "ChecksumMismatch" => {
            assert!(
                matches!(err, AccountAddressError::ChecksumMismatch),
                "{case_id}: expected ChecksumMismatch, got {err}"
            );
        }
        "UnexpectedNetworkPrefix" => {
            if let AccountAddressError::UnexpectedNetworkPrefix {
                expected: exp,
                found,
            } = err
            {
                assert_eq!(
                    Some(*exp),
                    expected.expected,
                    "{case_id}: unexpected expected-prefix mismatch"
                );
                assert_eq!(
                    Some(*found),
                    expected.found,
                    "{case_id}: unexpected found-prefix mismatch"
                );
            } else {
                panic!("{case_id}: expected UnexpectedNetworkPrefix, got {err}");
            }
        }
        "InvalidCompressedChar" | "InvalidI105Char" => {
            if let AccountAddressError::InvalidI105Char(ch) = err {
                let expected_char = expected
                    .char
                    .as_deref()
                    .unwrap_or_default()
                    .chars()
                    .next()
                    .unwrap_or_default();
                assert_eq!(expected_char, *ch, "{case_id}: invalid i105 char mismatch");
            } else {
                panic!("{case_id}: expected InvalidI105Char, got {err}");
            }
        }
        "InvalidHexAddress" => {
            assert!(
                matches!(err, AccountAddressError::InvalidHexAddress),
                "{case_id}: expected InvalidHexAddress, got {err}"
            );
        }
        "UnsupportedAddressFormat" => {
            assert!(
                matches!(err, AccountAddressError::UnsupportedAddressFormat),
                "{case_id}: expected UnsupportedAddressFormat, got {err}"
            );
        }
        "InvalidLength" => {
            assert!(
                matches!(err, AccountAddressError::InvalidLength),
                "{case_id}: expected InvalidLength, got {err}"
            );
        }
        "UnexpectedTrailingBytes" => {
            assert!(
                matches!(err, AccountAddressError::UnexpectedTrailingBytes),
                "{case_id}: expected UnexpectedTrailingBytes, got {err}"
            );
        }
        "InvalidMultisigPolicy" => {
            if let AccountAddressError::InvalidMultisigPolicy(inner) = err {
                match expected.policy_error.as_deref() {
                    Some("ZeroThreshold") => assert!(
                        matches!(
                            inner,
                            iroha_data_model::account::MultisigPolicyError::ZeroThreshold
                        ),
                        "{case_id}: expected ZeroThreshold, got {inner}"
                    ),
                    Some(other) => panic!("{case_id}: unsupported policy error {other}"),
                    None => panic!("{case_id}: missing policy_error expectation"),
                }
            } else {
                panic!("{case_id}: expected InvalidMultisigPolicy, got {err}");
            }
        }
        other => panic!("{case_id}: unsupported expected error kind {other}"),
    }
}
