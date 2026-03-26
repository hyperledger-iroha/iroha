#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Validate Torii's account address handling against the shared compliance vectors.

use std::{path::Path, str::FromStr};

use hex::FromHex;
use iroha_data_model::{
    account::{AccountAddress, AccountAddressError, AccountId, MultisigMember, MultisigPolicy},
    domain::DomainId,
    name::Name,
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

#[derive(Debug, JsonDeserialize)]
struct PositiveCase {
    case_id: String,
    category: String,
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
}

#[allow(dead_code)]
#[derive(Debug, JsonDeserialize)]
struct Member {
    curve: String,
    weight: u16,
    public_key_hex: String,
}

#[derive(Debug, JsonDeserialize)]
struct Encodings {
    canonical_hex: String,
    i105: I105Encoding,
    i105_default: String,
}

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

#[derive(Debug, JsonDeserialize)]
struct ExpectedError {
    kind: String,
    expected: Option<u16>,
    found: Option<u16>,
    char: Option<String>,
    policy_error: Option<String>,
}

fn domain(label: &str) -> DomainId {
    DomainId::new(Name::from_str(label).expect("valid domain label"))
}

fn decode_canonical(hex_value: &str) -> Vec<u8> {
    let body = hex_value
        .strip_prefix("0x")
        .unwrap_or(hex_value)
        .to_ascii_lowercase();
    Vec::from_hex(body.as_str()).expect("canonical hex decode")
}

fn canonical_bytes(address: &AccountAddress) -> Vec<u8> {
    let canonical = address
        .canonical_hex()
        .expect("canonical encoding must succeed");
    decode_canonical(&canonical)
}

fn ed25519_public_key(hex_value: &str) -> iroha_crypto::PublicKey {
    iroha_crypto::PublicKey::from_hex(iroha_crypto::Algorithm::Ed25519, hex_value)
        .expect("valid ed25519 payload")
}

#[test]
fn torii_account_address_vectors_pass() {
    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures/account/address_vectors.json");
    let fixture = std::fs::read_to_string(&fixture_path).expect("fixture read");
    let root: Root = json::from_str(&fixture).expect("fixture parse");
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
    let canonical_payload = decode_canonical(&case.encodings.canonical_hex);
    let canonical =
        AccountAddress::from_canonical_bytes(&canonical_payload).expect("canonical payload decode");

    // Strict parser rejects canonical-hex literals.
    let err = AccountAddress::parse_encoded(&case.encodings.canonical_hex, None)
        .expect_err("canonical hex should be rejected");
    assert!(
        matches!(err, AccountAddressError::UnsupportedAddressFormat),
        "{}: canonical hex must be unsupported",
        case.case_id
    );

    // I105 parsing + re-encoding
    let parsed_i105 = AccountAddress::parse_encoded(
        &case.encodings.i105.string,
        Some(case.encodings.i105.prefix),
    )
    .expect("parse_encoded i105");
    assert_eq!(
        canonical_bytes(&parsed_i105),
        canonical_payload,
        "{}: parse_encoded i105 canonical mismatch",
        case.case_id
    );

    let i105 = AccountAddress::from_i105_for_discriminant(
        &case.encodings.i105.string,
        Some(case.encodings.i105.prefix),
    )
    .expect("I105 decode");
    assert_eq!(
        canonical_bytes(&i105),
        canonical_payload,
        "{}: I105 canonical mismatch",
        case.case_id
    );
    assert_eq!(
        i105.to_i105_for_discriminant(default_prefix)
            .expect("I105 re-encode"),
        case.encodings.i105.string,
        "{}: I105 re-encode mismatch",
        case.case_id
    );

    // Compressed parse_encoded coverage
    let parsed_compressed = AccountAddress::parse_encoded(&case.encodings.i105_default, None)
        .expect("parse_encoded i105_default");
    assert_eq!(
        canonical_bytes(&parsed_compressed),
        canonical_payload,
        "{}: parse_encoded i105_default canonical mismatch",
        case.case_id
    );

    let decoded = AccountAddress::from_i105(&case.encodings.i105_default)
        .unwrap_or_else(|err| panic!("i105_default decode failed: {err}"));
    assert_eq!(
        canonical_bytes(&decoded),
        canonical_payload,
        "{}: i105_default canonical mismatch",
        case.case_id
    );

    match case.category.as_str() {
        "single" => validate_single_case(case, &canonical),
        "multisig" => validate_multisig_case(case, &canonical),
        other => panic!("{}: unsupported positive category {other}", case.case_id),
    }
}

fn validate_negative_case(case: &NegativeCase, default_prefix: u16) {
    match case.format.as_str() {
        "i105" => {
            let expected_prefix = case.expected_prefix.unwrap_or(default_prefix);
            let err =
                AccountAddress::from_i105_for_discriminant(&case.input, Some(expected_prefix))
                    .expect_err("I105 negative should fail");
            assert_error(&err, &case.expected_error, &case.case_id);
        }
        "i105_default" => {
            let err = AccountAddress::from_i105(&case.input)
                .expect_err("i105_default negative should fail");
            assert_error(&err, &case.expected_error, &case.case_id);
        }
        "canonical_hex" => {
            let err = AccountAddress::parse_encoded(&case.input, None)
                .expect_err("canonical negative should fail");
            assert_error(&err, &case.expected_error, &case.case_id);
        }
        other => panic!("{}: unsupported negative format {other}", case.case_id),
    }
}

fn assert_error(err: &AccountAddressError, expected: &ExpectedError, case_id: &str) {
    match expected.kind.as_str() {
        "ChecksumMismatch" => assert!(
            matches!(err, AccountAddressError::ChecksumMismatch),
            "{case_id}: expected ChecksumMismatch, got {err}"
        ),
        "UnexpectedNetworkPrefix" => {
            if let AccountAddressError::UnexpectedNetworkPrefix {
                expected: exp,
                found,
            } = err
            {
                assert_eq!(
                    Some(*exp),
                    expected.expected,
                    "{case_id}: expected-prefix mismatch"
                );
                assert_eq!(
                    Some(*found),
                    expected.found,
                    "{case_id}: found-prefix mismatch"
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
                assert_eq!(expected_char, *ch, "{case_id}: invalid char mismatch");
            } else {
                panic!("{case_id}: expected InvalidI105Char, got {err}");
            }
        }
        "InvalidHexAddress" => assert!(
            matches!(err, AccountAddressError::InvalidHexAddress),
            "{case_id}: expected InvalidHexAddress, got {err}"
        ),
        "UnsupportedAddressFormat" => assert!(
            matches!(err, AccountAddressError::UnsupportedAddressFormat),
            "{case_id}: expected UnsupportedAddressFormat, got {err}"
        ),
        "InvalidLength" => assert!(
            matches!(err, AccountAddressError::InvalidLength),
            "{case_id}: expected InvalidLength, got {err}"
        ),
        "UnexpectedTrailingBytes" => assert!(
            matches!(err, AccountAddressError::UnexpectedTrailingBytes),
            "{case_id}: expected UnexpectedTrailingBytes, got {err}"
        ),
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

fn validate_single_case(case: &PositiveCase, address: &AccountAddress) {
    if let Some(public_key_hex) = case.controller.public_key_hex.as_deref() {
        let account = AccountId::new(ed25519_public_key(public_key_hex));
        let rebuilt =
            AccountAddress::from_account_id(&account).expect("single address reconstruction");
        assert_eq!(
            rebuilt, *address,
            "{} single canonical mismatch",
            case.case_id
        );
    }

    if case.selector.kind == "global" {
        let equivalents = case
            .selector
            .domain_equivalents
            .as_ref()
            .expect("global selector requires equivalents");
        if let Some(equivalent) = case.input.equivalent_domain.as_deref() {
            assert!(
                equivalents.iter().any(|domain| domain == equivalent),
                "{} global selector missing equivalent mapping",
                case.case_id
            );
        }
    }
}

fn validate_multisig_case(case: &PositiveCase, address: &AccountAddress) {
    let members_hex = case
        .input
        .member_keys_hex
        .as_ref()
        .expect("multisig case requires member keys");
    let weights = case
        .input
        .member_weights
        .as_ref()
        .expect("multisig case requires member weights");
    assert_eq!(
        members_hex.len(),
        weights.len(),
        "{} member/weight mismatch",
        case.case_id
    );

    let mut members = Vec::with_capacity(members_hex.len());
    for (hex_key, &weight) in members_hex.iter().zip(weights.iter()) {
        members.push(
            MultisigMember::new(ed25519_public_key(hex_key), weight)
                .expect("multisig member construct"),
        );
    }

    let threshold = case
        .input
        .threshold
        .expect("multisig case requires threshold");
    let policy = MultisigPolicy::new(threshold, members).expect("multisig policy should construct");
    let domain_label = case
        .input
        .normalized_domain
        .as_deref()
        .expect("multisig case requires normalized domain");
    let _ = domain(domain_label);
    let account = AccountId::new_multisig(policy);
    let rebuilt =
        AccountAddress::from_account_id(&account).expect("multisig address reconstruction");
    assert_eq!(
        rebuilt, *address,
        "{} multisig canonical mismatch",
        case.case_id
    );
}
