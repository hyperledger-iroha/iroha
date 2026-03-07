//! ADDR-2 compliance vector generator shared by the example binary and CLI tooling.

use std::str::FromStr;

use hex::encode_upper;
use iroha_crypto::{Algorithm, KeyPair, PublicKey};
use norito::json::{Map, Value};

use crate::{
    account::{
        AccountAddress, AccountAddressError, AccountId, MultisigMember, MultisigPolicy,
        MultisigPolicyError,
    },
    domain::DomainId,
    name::Name,
};

macro_rules! json_obj {
    ({ $( $key:literal : $value:expr ),* $(,)? }) => {{
        let mut map = Map::new();
        $( map.insert($key.to_string(), Value::from($value)); )*
        Value::Object(map)
    }};
}

const NETWORK_PREFIX: u16 = 753;

struct PositiveEncodings {
    canonical_hex: String,
    canonical_bytes: Vec<u8>,
    ih58: String,
    compressed: String,
    compressed_fullwidth: String,
}

struct SingleCase {
    value: Value,
    address: AccountAddress,
    encodings: PositiveEncodings,
}

struct MultisigCase {
    value: Value,
    encodings: PositiveEncodings,
}

struct MultisigFixture {
    case_id: &'static str,
    note: &'static str,
    domain: &'static str,
    members: &'static [(u8, u16)],
    threshold: u16,
}

const MULTISIG_FIXTURES: &[MultisigFixture] = &[
    MultisigFixture {
        case_id: "addr-multisig-council-threshold3",
        note: "Council domain multisig with three members (weights 2,1,1) requiring weight ≥3.",
        domain: "council",
        members: &[(0x21, 2), (0x36, 1), (0x4B, 1)],
        threshold: 3,
    },
    MultisigFixture {
        case_id: "addr-multisig-wonderland-threshold2",
        note: "Two-member multisig policy with weights 1 and 2 requiring weight ≥2 on wonderland.",
        domain: "wonderland",
        members: &[(0x10, 1), (0x11, 2)],
        threshold: 2,
    },
    MultisigFixture {
        case_id: "addr-multisig-default-quorum3",
        note: "Implicit-default domain multisig with four members (weight 1 each) requiring weight ≥3.",
        domain: "default",
        members: &[(0xA0, 1), (0xA1, 1), (0xA2, 1), (0xA3, 1)],
        threshold: 3,
    },
];

fn ed25519_pk_with(seed_byte: u8) -> PublicKey {
    let seed = vec![seed_byte; 32];
    let (public_key, _) = KeyPair::from_seed(seed, Algorithm::Ed25519).into_parts();
    public_key
}

fn domain(label: &str) -> DomainId {
    DomainId::new(Name::from_str(label).expect("valid domain name"))
}

fn canonical_hex(address: &AccountAddress) -> String {
    address
        .canonical_hex()
        .expect("canonical encoding must succeed")
}

fn canonical_bytes(hex_value: &str) -> Vec<u8> {
    let body = hex_value.strip_prefix("0x").unwrap_or(hex_value);
    hex::decode(body).expect("canonical hex should decode")
}

fn selector_value() -> Value {
    // Canonical IH58 payloads are globally scoped and no longer embed domain selectors.
    json_obj!({ "kind": "default" })
}

fn controller_single_value(public_key: &PublicKey) -> Value {
    let (algorithm, payload) = public_key.to_bytes();
    assert_eq!(algorithm, Algorithm::Ed25519, "expected ed25519 key");
    json_obj!({
        "kind": "single",
        "curve": "ed25519",
        "public_key_hex": encode_upper(payload),
        "public_key_multihash": public_key.to_string(),
        "public_key_prefixed": public_key.to_prefixed_string(),
    })
}

fn controller_multisig_value(policy: &MultisigPolicy) -> Value {
    let members: Vec<Value> = policy
        .members()
        .iter()
        .map(|member| {
            let (algorithm, payload) = member.public_key().to_bytes();
            json_obj!({
                "curve": format!("{algorithm:?}").to_lowercase(),
                "weight": member.weight(),
                "public_key_hex": encode_upper(payload),
                "public_key_multihash": member.public_key().to_string(),
                "public_key_prefixed": member.public_key().to_prefixed_string(),
            })
        })
        .collect();
    let ctap2 = policy.encode_ctap2();
    let digest = policy.digest_blake2b256();
    json_obj!({
        "kind": "multisig",
        "version": policy.version(),
        "threshold": policy.threshold(),
        "members": Value::Array(members),
        "ctap2_cbor_hex": format!("0x{}", encode_upper(&ctap2)),
        "digest_blake2b256_hex": format!("0x{}", encode_upper(digest)),
    })
}

fn encodings(address: &AccountAddress) -> PositiveEncodings {
    let canonical_hex = canonical_hex(address);
    let canonical_bytes = canonical_bytes(&canonical_hex);
    let ih58 = address
        .to_ih58(NETWORK_PREFIX)
        .expect("IH58 encoding must succeed");
    let compressed = address
        .to_compressed_sora()
        .expect("compressed encoding must succeed");
    let compressed_fullwidth = address
        .to_compressed_sora_fullwidth()
        .expect("fullwidth compressed encoding must succeed");

    PositiveEncodings {
        canonical_hex,
        canonical_bytes,
        ih58,
        compressed,
        compressed_fullwidth,
    }
}

fn build_single_case(case_id: &str, seed: u8, raw_domain: &str, note: &str) -> SingleCase {
    let public_key = ed25519_pk_with(seed);
    let account = AccountId::new(domain(raw_domain), public_key.clone());
    let address = AccountAddress::from_account_id(&account).expect("address encoding");
    let encodings = encodings(&address);
    let selector = selector_value();

    let value = json_obj!({
        "case_id": case_id,
        "category": "single",
        "note": note,
        "input": json_obj!({
            "raw_domain": raw_domain,
            "normalized_domain": account.domain().name().as_ref(),
            "seed_byte": seed,
        }),
        "selector": selector,
        "controller": controller_single_value(&public_key),
        "encodings": json_obj!({
            "canonical_hex": encodings.canonical_hex.clone(),
            "ih58": json_obj!({
                "prefix": NETWORK_PREFIX,
                "string": encodings.ih58.clone(),
            }),
            "compressed": encodings.compressed.clone(),
            "compressed_fullwidth": encodings.compressed_fullwidth.clone(),
        })
    });

    SingleCase {
        value,
        address,
        encodings,
    }
}

fn build_multisig_cases() -> Vec<MultisigCase> {
    MULTISIG_FIXTURES
        .iter()
        .map(|fixture| {
            let domain = domain(fixture.domain);
            let members = fixture
                .members
                .iter()
                .map(|(seed, weight)| {
                    MultisigMember::new(ed25519_pk_with(*seed), *weight)
                        .expect("multisig member must construct")
                })
                .collect::<Vec<_>>();
            let policy = MultisigPolicy::new(fixture.threshold, members).expect("policy");
            let account = AccountId::new_multisig(domain, policy.clone());
            let address =
                AccountAddress::from_account_id(&account).expect("multisig encoding succeeds");
            let encodings = encodings(&address);
            let selector = selector_value();

            let member_keys_hex: Vec<String> = policy
                .members()
                .iter()
                .map(|member| {
                    let (_, payload) = member.public_key().to_bytes();
                    encode_upper(payload)
                })
                .collect();
            let member_weights: Vec<u16> = policy
                .members()
                .iter()
                .map(MultisigMember::weight)
                .collect();

            let value = json_obj!({
                "case_id": fixture.case_id,
                "category": "multisig",
                "note": fixture.note,
                "input": json_obj!({
                    "raw_domain": fixture.domain,
                    "normalized_domain": account.domain().name().as_ref(),
                    "member_keys_hex": Value::Array(
                        member_keys_hex
                            .iter()
                            .cloned()
                            .map(Value::from)
                            .collect()
                    ),
                    "member_weights": Value::Array(
                        member_weights
                            .iter()
                            .copied()
                            .map(Value::from)
                            .collect()
                    ),
                    "threshold": fixture.threshold,
                }),
                "selector": selector,
                "controller": controller_multisig_value(&policy),
                "encodings": json_obj!({
                    "canonical_hex": encodings.canonical_hex.clone(),
                    "ih58": json_obj!({
                        "prefix": NETWORK_PREFIX,
                        "string": encodings.ih58.clone(),
                    }),
                    "compressed": encodings.compressed.clone(),
                    "compressed_fullwidth": encodings.compressed_fullwidth.clone(),
                })
            });

            MultisigCase { value, encodings }
        })
        .collect()
}

fn error_to_json(err: &AccountAddressError) -> Value {
    match err {
        AccountAddressError::ChecksumMismatch => {
            json_obj!({ "kind": "ChecksumMismatch" })
        }
        AccountAddressError::UnexpectedNetworkPrefix { expected, found } => json_obj!({
            "kind": "UnexpectedNetworkPrefix",
            "expected": *expected,
            "found": *found,
        }),
        AccountAddressError::MissingCompressedSentinel => {
            json_obj!({ "kind": "MissingCompressedSentinel" })
        }
        AccountAddressError::InvalidCompressedChar(ch) => json_obj!({
            "kind": "InvalidCompressedChar",
            "char": ch.to_string(),
        }),
        AccountAddressError::InvalidHexAddress => {
            json_obj!({ "kind": "InvalidHexAddress" })
        }
        AccountAddressError::InvalidLength => {
            json_obj!({ "kind": "InvalidLength" })
        }
        AccountAddressError::UnexpectedTrailingBytes => {
            json_obj!({ "kind": "UnexpectedTrailingBytes" })
        }
        AccountAddressError::InvalidMultisigPolicy(inner) => json_obj!({
            "kind": "InvalidMultisigPolicy",
            "policy_error": policy_error_to_string(*inner),
        }),
        other => panic!("unhandled error variant in generator: {other}"),
    }
}

fn policy_error_to_string(err: MultisigPolicyError) -> &'static str {
    match err {
        MultisigPolicyError::EmptyMembers => "EmptyMembers",
        MultisigPolicyError::ZeroThreshold => "ZeroThreshold",
        MultisigPolicyError::MemberWeightZero => "MemberWeightZero",
        MultisigPolicyError::DuplicateMember => "DuplicateMember",
        MultisigPolicyError::ThresholdExceedsTotal { .. } => "ThresholdExceedsTotal",
        MultisigPolicyError::UnsupportedVersion(_) => "UnsupportedVersion",
        MultisigPolicyError::UnsupportedCurve(_) => "UnsupportedCurve",
    }
}

fn mutate_last_char(input: &str, replacement: char) -> String {
    let mut chars: Vec<char> = input.chars().collect();
    let last = chars
        .last_mut()
        .expect("address strings are non-empty for compliance vectors");
    if *last == replacement {
        *last = '0';
    } else {
        *last = replacement;
    }
    chars.into_iter().collect()
}

fn replace_nth_char(input: &str, index: usize, replacement: char) -> String {
    let mut chars: Vec<char> = input.chars().collect();
    if let Some(slot) = chars.get_mut(index) {
        *slot = replacement;
    }
    chars.into_iter().collect()
}

fn canonical_with_trailing_zero(encodings: &PositiveEncodings) -> String {
    format!("{}00", encodings.canonical_hex)
}

fn canonical_invalid_hex(encodings: &PositiveEncodings) -> String {
    let mut chars: Vec<char> = encodings.canonical_hex.chars().collect();
    let last = chars
        .last_mut()
        .expect("canonical hex must contain at least one digit");
    *last = 'G';
    chars.into_iter().collect()
}

/// Build the canonical ADDR-2 JSON bundle consumed by SDK/tests.
#[allow(clippy::too_many_lines)]
pub fn compliance_vectors_json() -> Value {
    let single_default = build_single_case(
        "addr-single-default-ed25519",
        0x00,
        "default",
        "Implicit default-domain address using deterministic Ed25519 key derived from seed byte 0x00.",
    );
    let single_treasury = build_single_case(
        "addr-single-treasury-ed25519",
        0x01,
        "treasury",
        "Non-default domain address (treasury) using deterministic Ed25519 key derived from seed byte 0x01.",
    );
    let multisig_cases = build_multisig_cases();

    let mut positive_cases = vec![single_default.value.clone(), single_treasury.value.clone()];
    positive_cases.extend(multisig_cases.iter().map(|case| case.value.clone()));

    let ih58_checksum = mutate_last_char(&single_default.encodings.ih58, 'z');
    let err_checksum = AccountAddress::from_ih58(&ih58_checksum, Some(NETWORK_PREFIX)).unwrap_err();

    let ih58_wrong_prefix = single_default
        .address
        .to_ih58(NETWORK_PREFIX + 1)
        .expect("IH58 encoding must succeed");
    let err_prefix =
        AccountAddress::from_ih58(&ih58_wrong_prefix, Some(NETWORK_PREFIX)).unwrap_err();

    let compressed_missing = single_default
        .encodings
        .compressed
        .strip_prefix("sora")
        .unwrap()
        .to_string();
    let err_missing = AccountAddress::from_compressed_sora(&compressed_missing).unwrap_err();

    let compressed_bad_char = replace_nth_char(&single_default.encodings.compressed, 6, 'A');
    let err_bad_char = AccountAddress::from_compressed_sora(&compressed_bad_char).unwrap_err();

    let compressed_bad_checksum = mutate_last_char(&single_default.encodings.compressed, 'ﾇ');
    let err_bad_checksum =
        AccountAddress::from_compressed_sora(&compressed_bad_checksum).unwrap_err();

    let canonical_invalid = canonical_invalid_hex(&single_default.encodings);
    let err_invalid_hex = AccountAddress::parse_encoded(&canonical_invalid, None).unwrap_err();

    let canonical_trailing = canonical_with_trailing_zero(&single_default.encodings);
    let err_trailing = AccountAddress::parse_encoded(&canonical_trailing, None).unwrap_err();

    let primary_multisig = multisig_cases
        .first()
        .expect("multisig fixtures must include at least one case");
    let mut multisig_truncated = primary_multisig.encodings.canonical_bytes.clone();
    multisig_truncated.truncate(multisig_truncated.len().saturating_sub(4));
    let multisig_truncated_hex = format!("0x{}", encode_upper(&multisig_truncated));
    let err_multisig_truncated =
        AccountAddress::parse_encoded(&multisig_truncated_hex, None).unwrap_err();

    let negative_cases = vec![
        json_obj!({
            "case_id": "ih58-checksum-mismatch",
            "format": "ih58",
            "note": "Checksum tampering on IH58 alias for the default-domain vector.",
            "input": ih58_checksum,
            "expected_prefix": NETWORK_PREFIX,
            "expected_error": error_to_json(&err_checksum),
        }),
        json_obj!({
            "case_id": "ih58-prefix-mismatch",
            "format": "ih58",
            "note": "Encoded with prefix NETWORK_PREFIX + 1 while validators expect NETWORK_PREFIX.",
            "input": ih58_wrong_prefix,
            "expected_prefix": NETWORK_PREFIX,
            "expected_error": error_to_json(&err_prefix),
        }),
        json_obj!({
            "case_id": "compressed-missing-sentinel",
            "format": "compressed",
            "note": "Missing required sora sentinel prefix.",
            "input": compressed_missing,
            "expected_error": error_to_json(&err_missing),
        }),
        json_obj!({
            "case_id": "compressed-invalid-character",
            "format": "compressed",
            "note": "Introduces ASCII 'A' outside the Sora compressed alphabet.",
            "input": compressed_bad_char,
            "expected_error": error_to_json(&err_bad_char),
        }),
        json_obj!({
            "case_id": "compressed-checksum-mismatch",
            "format": "compressed",
            "note": "Checksum nibble replaced to trigger Bech32m verification failure.",
            "input": compressed_bad_checksum,
            "expected_error": error_to_json(&err_bad_checksum),
        }),
        json_obj!({
            "case_id": "canonical-invalid-hex",
            "format": "canonical_hex",
            "note": "Invalid hex digit injected into canonical representation.",
            "input": canonical_invalid,
            "expected_error": error_to_json(&err_invalid_hex),
        }),
        json_obj!({
            "case_id": "canonical-trailing-bytes",
            "format": "canonical_hex",
            "note": "Extra zero byte appended past the canonical payload.",
            "input": canonical_trailing,
            "expected_error": error_to_json(&err_trailing),
        }),
        json_obj!({
            "case_id": "canonical-multisig-truncated",
            "format": "canonical_hex",
            "note": "Canonical multisig payload truncated after removing final member bytes.",
            "input": multisig_truncated_hex,
            "expected_error": error_to_json(&err_multisig_truncated),
        }),
    ];

    json_obj!({
        "format_version": 1,
        "default_network_prefix": NETWORK_PREFIX,
        "cases": json_obj!({
            "positive": Value::Array(positive_cases),
            "negative": Value::Array(negative_cases),
        })
    })
}
