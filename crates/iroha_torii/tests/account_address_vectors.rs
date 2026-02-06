#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Validate Torii's account address handling against the shared compliance vectors.

use std::{path::Path, str::FromStr};

use hex::FromHex;
use iroha_data_model::{
    account::{
        AccountAddress, AccountAddressError, AccountAddressFormat, AccountId, MultisigMember,
        MultisigPolicy,
    },
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
    ih58: Ih58Encoding,
    compressed: String,
    compressed_fullwidth: String,
}

#[derive(Debug, JsonDeserialize)]
struct Ih58Encoding {
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

    // Sanity-check canonical round-trip via Torii parsing helpers.
    let (any_addr, any_format) = AccountAddress::parse_any(&case.encodings.canonical_hex, None)
        .expect("parse_any canonical");
    assert_eq!(
        any_format,
        AccountAddressFormat::CanonicalHex,
        "{}: canonical parse_any reported unexpected format",
        case.case_id
    );
    assert_eq!(
        canonical_bytes(&any_addr),
        canonical_payload,
        "{}: parse_any canonical mismatch",
        case.case_id
    );

    // IH58 parsing + re-encoding
    let (parsed_ih58, format_ih58) = AccountAddress::parse_any(
        &case.encodings.ih58.string,
        Some(case.encodings.ih58.prefix),
    )
    .expect("parse_any ih58");
    match format_ih58 {
        AccountAddressFormat::IH58 { network_prefix } => {
            assert_eq!(
                network_prefix, case.encodings.ih58.prefix,
                "{}: parse_any IH58 prefix mismatch",
                case.case_id
            );
        }
        other => panic!(
            "{}: parse_any IH58 reported unexpected format {other:?}",
            case.case_id
        ),
    }
    assert_eq!(
        canonical_bytes(&parsed_ih58),
        canonical_payload,
        "{}: parse_any IH58 canonical mismatch",
        case.case_id
    );

    let ih58 = AccountAddress::from_ih58(
        &case.encodings.ih58.string,
        Some(case.encodings.ih58.prefix),
    )
    .expect("IH58 decode");
    assert_eq!(
        canonical_bytes(&ih58),
        canonical_payload,
        "{}: IH58 canonical mismatch",
        case.case_id
    );
    assert_eq!(
        ih58.to_ih58(default_prefix).expect("IH58 re-encode"),
        case.encodings.ih58.string,
        "{}: IH58 re-encode mismatch",
        case.case_id
    );

    // Compressed parse_any coverage
    let (parsed_compressed, format_compressed) =
        AccountAddress::parse_any(&case.encodings.compressed, None).expect("parse_any compressed");
    assert_eq!(
        format_compressed,
        AccountAddressFormat::Compressed,
        "{}: parse_any compressed reported unexpected format",
        case.case_id
    );
    assert_eq!(
        canonical_bytes(&parsed_compressed),
        canonical_payload,
        "{}: parse_any compressed canonical mismatch",
        case.case_id
    );

    // Compressed decoding (half-/full-width)
    for (encoding, label) in [
        (&case.encodings.compressed, "compressed-half"),
        (&case.encodings.compressed_fullwidth, "compressed-full"),
    ] {
        let decoded = AccountAddress::from_compressed_sora(encoding)
            .unwrap_or_else(|err| panic!("{label} decode failed: {err}"));
        assert_eq!(
            canonical_bytes(&decoded),
            canonical_payload,
            "{}: {label} canonical mismatch",
            case.case_id
        );
    }

    match case.category.as_str() {
        "single" => validate_single_case(case, &canonical),
        "multisig" => validate_multisig_case(case, &canonical),
        other => panic!("{}: unsupported positive category {other}", case.case_id),
    }
}

fn validate_negative_case(case: &NegativeCase, default_prefix: u16) {
    match case.format.as_str() {
        "ih58" => {
            let expected_prefix = case.expected_prefix.unwrap_or(default_prefix);
            let err = AccountAddress::from_ih58(&case.input, Some(expected_prefix))
                .expect_err("IH58 negative should fail");
            assert_error(&err, &case.expected_error, &case.case_id);
        }
        "compressed" => {
            let err = AccountAddress::from_compressed_sora(&case.input)
                .expect_err("compressed negative should fail");
            assert_error(&err, &case.expected_error, &case.case_id);
        }
        "canonical_hex" => {
            let err = AccountAddress::parse_any(&case.input, None)
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
        "MissingCompressedSentinel" => assert!(
            matches!(err, AccountAddressError::MissingCompressedSentinel),
            "{case_id}: expected MissingCompressedSentinel, got {err}"
        ),
        "InvalidCompressedChar" => {
            if let AccountAddressError::InvalidCompressedChar(ch) = err {
                let expected_char = expected
                    .char
                    .as_deref()
                    .unwrap_or_default()
                    .chars()
                    .next()
                    .unwrap_or_default();
                assert_eq!(expected_char, *ch, "{case_id}: invalid char mismatch");
            } else {
                panic!("{case_id}: expected InvalidCompressedChar, got {err}");
            }
        }
        "InvalidHexAddress" => assert!(
            matches!(err, AccountAddressError::InvalidHexAddress),
            "{case_id}: expected InvalidHexAddress, got {err}"
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
    if let (Some(domain_label), Some(public_key_hex)) = (
        case.input.normalized_domain.as_deref(),
        case.controller.public_key_hex.as_deref(),
    ) {
        let account = AccountId::new(domain(domain_label), ed25519_public_key(public_key_hex));
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
    let account = AccountId::new_multisig(domain(domain_label), policy);
    let rebuilt =
        AccountAddress::from_account_id(&account).expect("multisig address reconstruction");
    assert_eq!(
        rebuilt, *address,
        "{} multisig canonical mismatch",
        case.case_id
    );
}
