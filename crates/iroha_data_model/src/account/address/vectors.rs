//! Deterministic address test vector generator for ADDR-2.

use core::fmt;
use std::{convert::TryInto, str::FromStr};

use hex;
use iroha_crypto::{Algorithm, KeyPair, PublicKey};
use norito::{
    json,
    json::{JsonSerialize, Value},
};

use super::{
    AccountAddressError::*, CONTROLLER_MULTISIG_TAG, CONTROLLER_SINGLE_KEY_TAG, DomainSelector,
    compute_local_digest, default_domain_guard, default_domain_name,
};
use crate::{
    account::{AccountAddress, AccountAddressError, AccountId, MultisigMember, MultisigPolicy},
    domain::DomainId,
    name::Name,
};

/// Default I105 prefix used for deterministic vectors.
pub const DEFAULT_VECTOR_NETWORK_PREFIX: u16 = 0x1234;

const I105_BASE_U8: u8 = 105;
const I105_CHECKSUM_LEN: usize = 6;

fn i105_to_digits(payload: &str) -> Result<Vec<u8>, AccountAddressError> {
    let discriminant = super::i105_discriminant_from_sentinel(payload)
        .ok_or(AccountAddressError::MissingI105Sentinel)?;
    let payload = payload
        .strip_prefix(&super::i105_sentinel_for_discriminant(discriminant))
        .or_else(|| {
            super::ascii_str_to_fullwidth(&super::i105_sentinel_for_discriminant(discriminant))
                .and_then(|fullwidth| payload.strip_prefix(&fullwidth))
        })
        .ok_or(AccountAddressError::MissingI105Sentinel)?;
    super::i105_payload_digits(payload)
}

fn digits_to_i105_literal(digits: &[u8]) -> String {
    let mut out = String::with_capacity(digits.len() * 2);
    for digit in digits {
        out.push_str(i105_digit_symbol(*digit));
    }
    out
}

fn i105_digit_symbol(digit: u8) -> &'static str {
    super::i105_digit_symbol(digit, false).expect("digit must be in range")
}

const VECTOR_SINGLE_DOMAINS: [(&str, u8); 12] = [
    ("default", 0x00),
    ("treasury", 0x01),
    ("wonderland", 0x02),
    ("iroha", 0x03),
    ("alpha", 0x04),
    ("omega", 0x05),
    ("governance", 0x06),
    ("validators", 0x07),
    ("explorer", 0x08),
    ("soranet", 0x09),
    ("kitsune", 0x0A),
    ("da", 0x0B),
];

struct MultisigFixtureSpec {
    domain: &'static str,
    members: &'static [(u8, u16)],
    threshold: u16,
}

const MULTISIG_FIXTURES: &[MultisigFixtureSpec] = &[
    MultisigFixtureSpec {
        domain: "council",
        members: &[(0x21, 2), (0x36, 1), (0x4B, 1)],
        threshold: 3,
    },
    MultisigFixtureSpec {
        domain: "wonderland",
        members: &[(0x10, 1), (0x11, 2)],
        threshold: 2,
    },
    MultisigFixtureSpec {
        domain: "default",
        members: &[(0xA0, 1), (0xA1, 1), (0xA2, 1), (0xA3, 1)],
        threshold: 3,
    },
];

fn json_value<T>(value: &T) -> Value
where
    T: JsonSerialize + ?Sized,
{
    json::to_value(value).expect("serialize JSON value")
}

fn json_object(pairs: Vec<(&str, Value)>) -> Value {
    let mut map = json::Map::new();
    for (key, value) in pairs {
        map.insert(key.to_owned(), value);
    }
    Value::Object(map)
}

/// Full bundle of deterministic vectors.
#[derive(Clone, Debug)]
pub struct AddressVectorBundle {
    /// Default human-readable label applied when deriving account identifiers.
    pub default_domain_label: String,
    /// Network prefix encoded in I105-addressed test vectors.
    pub network_prefix: u16,
    /// Deterministic fixtures covering single-key address encodings.
    pub single_key: Vec<SingleKeyVector>,
    /// Deterministic fixtures covering multisignature address encodings.
    pub multisig: Vec<MultisigVector>,
    /// Negative fixtures describing expected decoder failures.
    pub errors: Vec<ErrorVector>,
}

impl AddressVectorBundle {
    /// Serialise the bundle into a Norito JSON value.
    #[must_use]
    pub fn to_json_value(&self) -> Value {
        let single = self
            .single_key
            .iter()
            .map(|vector| vector.to_json_value(self.network_prefix))
            .collect::<Vec<Value>>();
        let multisig = self
            .multisig
            .iter()
            .map(|vector| vector.to_json_value(self.network_prefix))
            .collect::<Vec<Value>>();
        let errors = self
            .errors
            .iter()
            .map(ErrorVector::to_json_value)
            .collect::<Vec<Value>>();

        let network_prefix_hex = format_u16_hex(self.network_prefix);
        let metadata = json_object(vec![
            (
                "default_domain_label",
                json_value(&self.default_domain_label),
            ),
            ("network_prefix", json_value(&network_prefix_hex)),
            (
                "formats",
                Value::Array(
                    ["i105", "canonical_hex"]
                        .into_iter()
                        .map(json_value)
                        .collect(),
                ),
            ),
        ]);

        json_object(vec![
            ("metadata", metadata),
            ("single_key", Value::Array(single)),
            ("multisig", Value::Array(multisig)),
            ("errors", Value::Array(errors)),
        ])
    }
}

/// Deterministic vectors for single-key controllers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SingleKeyVector {
    /// Domain label used when deriving the account identifier.
    pub domain_label: &'static str,
    /// Seed byte applied to deterministically derive the key pair.
    pub seed_byte: u8,
    /// Canonical account identifier for the controller.
    pub account_id: String,
    /// Canonical uppercase hexadecimal encoding of the controller address.
    pub canonical_hex: String,
    /// I105-encoded controller address string.
    pub i105: String,
    /// Domain selector input data required to reproduce the controller address.
    pub domain_selector: DomainSelectorVector,
    /// Curve identifier used by the controller's public key.
    pub controller_curve_id: u8,
    /// Algorithm identifier for the controller's public key.
    pub controller_algorithm: String,
    /// Hexadecimal encoding of the controller public key.
    pub public_key_hex: String,
}

impl SingleKeyVector {
    fn to_json_value(&self, network_prefix: u16) -> Value {
        let network_prefix_hex = format_u16_hex(network_prefix);
        let controller_curve_hex = format_u8_hex(self.controller_curve_id);
        let seed_byte_hex = format_u8_hex(self.seed_byte);
        let i105 = json_object(vec![
            ("value", json_value(&self.i105)),
            ("network_prefix", json_value(&network_prefix_hex)),
        ]);
        let controller = json_object(vec![
            ("kind", json_value("single")),
            ("curve_id", json_value(&controller_curve_hex)),
            ("algorithm", json_value(&self.controller_algorithm)),
            ("public_key_hex", json_value(&self.public_key_hex)),
        ]);

        json_object(vec![
            ("domain", json_value(self.domain_label)),
            ("seed_byte", json_value(&seed_byte_hex)),
            ("account_id", json_value(&self.account_id)),
            ("canonical_hex", json_value(&self.canonical_hex)),
            ("i105", i105),
            ("domain_selector", self.domain_selector.to_json_value()),
            ("controller", controller),
        ])
    }
}

/// Deterministic vectors for multisignature controllers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultisigVector {
    /// Domain label used when deriving account identifiers.
    pub domain_label: &'static str,
    /// Canonical account identifier associated with the multisig controller.
    pub account_id: String,
    /// Canonical uppercase hexadecimal encoding of the account address.
    pub canonical_hex: String,
    /// I105-encoded multisig address string.
    pub i105: String,
    /// Domain selector inputs that reproduce the canonical account.
    pub domain_selector: DomainSelectorVector,
    /// Multisig version number embedded in the controller payload.
    pub version: u8,
    /// Minimum total weight required to authorise transactions.
    pub threshold: u16,
    /// Sum of member weights participating in the multisig.
    pub total_weight: u32,
    /// Member descriptors contributing to the multisig controller.
    pub members: Vec<MultisigMemberVector>,
    /// Hexadecimal encoding of the CTAP2 CBOR payload used for policy hashing.
    pub policy_cbor_hex: String,
    /// Hexadecimal encoding of the policy digest (Blake2b-256, personalised).
    pub policy_digest_hex: String,
}

impl MultisigVector {
    fn to_json_value(&self, network_prefix: u16) -> Value {
        let network_prefix_hex = format_u16_hex(network_prefix);
        let members = self
            .members
            .iter()
            .map(MultisigMemberVector::to_json_value)
            .collect::<Vec<Value>>();
        let i105 = json_object(vec![
            ("value", json_value(&self.i105)),
            ("network_prefix", json_value(&network_prefix_hex)),
        ]);
        let version = self.version;
        let threshold = self.threshold;
        let total_weight = self.total_weight;
        let controller = json_object(vec![
            ("kind", json_value("multisig")),
            ("version", json_value(&version)),
            ("threshold", json_value(&threshold)),
            ("total_weight", json_value(&total_weight)),
            ("members", Value::Array(members)),
            ("ctap2_cbor_hex", json_value(&self.policy_cbor_hex)),
            ("digest_blake2b256_hex", json_value(&self.policy_digest_hex)),
        ]);

        json_object(vec![
            ("domain", json_value(self.domain_label)),
            ("account_id", json_value(&self.account_id)),
            ("canonical_hex", json_value(&self.canonical_hex)),
            ("i105", i105),
            ("domain_selector", self.domain_selector.to_json_value()),
            ("controller", controller),
        ])
    }
}

/// Metadata for a multisignature member entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultisigMemberVector {
    /// Stable ordering of the member within the controller list.
    pub index: usize,
    /// Curve identifier used by the member's public key.
    pub curve_id: u8,
    /// Weight contributed by this member toward threshold satisfaction.
    pub weight: u16,
    /// Hexadecimal encoding of the member public key.
    pub public_key_hex: String,
    /// Length of the public key material in bytes.
    pub key_length: usize,
}

impl MultisigMemberVector {
    fn to_json_value(&self) -> Value {
        let curve_hex = format_u8_hex(self.curve_id);
        json_object(vec![
            ("index", json_value(&self.index)),
            ("curve_id", json_value(&curve_hex)),
            ("weight", json_value(&self.weight)),
            ("key_length", json_value(&self.key_length)),
            ("public_key_hex", json_value(&self.public_key_hex)),
        ])
    }
}

/// Domain selector metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomainSelectorVector {
    /// Use the implicit domain identifier derived from `default_domain_label`.
    ImplicitDefault,
    /// Embed a canonical digest string directly in the selector.
    LocalDigest {
        /// Hexadecimal representation of the selector digest.
        digest_hex: String,
    },
    /// Reference a global registry entry by identifier.
    Global {
        /// Numeric registry identifier describing the selector mapping.
        registry_id: u32,
    },
    /// Carry an opaque selector tag for forward-compatibility tests.
    Unknown {
        /// Raw selector tag value supplied by the vector.
        tag: u8,
    },
}

impl DomainSelectorVector {
    fn to_json_value(&self) -> Value {
        match self {
            Self::ImplicitDefault => json_object(vec![("kind", json_value("implicit_default"))]),
            Self::LocalDigest { digest_hex } => json_object(vec![
                ("kind", json_value("local_digest")),
                ("digest_hex", json_value(digest_hex)),
            ]),
            Self::Global { registry_id } => json_object(vec![
                ("kind", json_value("global_registry")),
                ("registry_id", json_value(registry_id)),
            ]),
            Self::Unknown { tag } => json_object(vec![
                ("kind", json_value("unknown")),
                ("tag", json_value(tag)),
            ]),
        }
    }
}

/// Negative vector harness capturing failure expectations.
#[derive(Clone, Debug)]
pub struct ErrorVector {
    /// Short label describing the failure scenario.
    pub label: &'static str,
    /// Decoder under test (e.g., address format or parser).
    pub decoder: &'static str,
    /// Input string expected to fail decoding.
    pub input: String,
    /// Error variant the decoder is expected to emit.
    pub error_variant: &'static str,
    /// Stable error code emitted by the decoder.
    pub error_code: &'static str,
    /// Human-readable error message emitted by the decoder.
    pub message: String,
    /// Optional structured details associated with the error.
    pub details: Option<Value>,
}

impl ErrorVector {
    fn to_json_value(&self) -> Value {
        let message = &self.message;
        let error_body = self.details.as_ref().map_or_else(
            || {
                json_object(vec![
                    ("variant", json_value(self.error_variant)),
                    ("code", json_value(self.error_code)),
                    ("message", json_value(message)),
                ])
            },
            |details| {
                json_object(vec![
                    ("variant", json_value(self.error_variant)),
                    ("code", json_value(self.error_code)),
                    ("message", json_value(message)),
                    ("details", details.clone()),
                ])
            },
        );
        json_object(vec![
            ("label", json_value(self.label)),
            ("decoder", json_value(self.decoder)),
            ("input", json_value(&self.input)),
            ("error", error_body),
        ])
    }
}

/// Produce the deterministic ADDR-2 vector bundle.
#[must_use]
pub fn build_vector_bundle() -> AddressVectorBundle {
    let _guard = default_domain_guard(Some("default"));

    let default_domain_label = default_domain_name().as_ref().to_owned();

    let single_key = build_single_key_vectors(DEFAULT_VECTOR_NETWORK_PREFIX);
    let multisig = build_multisig_vectors(DEFAULT_VECTOR_NETWORK_PREFIX);
    let errors = build_error_vectors(DEFAULT_VECTOR_NETWORK_PREFIX);

    AddressVectorBundle {
        default_domain_label,
        network_prefix: DEFAULT_VECTOR_NETWORK_PREFIX,
        single_key,
        multisig,
        errors,
    }
}

/// Convenience helper returning the bundle encoded as JSON.
#[must_use]
pub fn address_vectors_json() -> Value {
    build_vector_bundle().to_json_value()
}

fn build_single_key_vectors(network_prefix: u16) -> Vec<SingleKeyVector> {
    VECTOR_SINGLE_DOMAINS
        .iter()
        .map(|(label, seed)| {
            let public_key = ed25519_pk_with(*seed);
            let account = AccountId::new(public_key);
            let address = AccountAddress::from_account_id(&account)
                .expect("single-key account should encode into AccountAddress");
            build_single_vector(label, *seed, &account, &address, network_prefix)
        })
        .collect()
}

fn build_single_vector(
    label: &'static str,
    seed: u8,
    account: &AccountId,
    address: &AccountAddress,
    network_prefix: u16,
) -> SingleKeyVector {
    let canonical_hex = address
        .canonical_hex()
        .expect("canonical hex must encode for deterministic vectors");
    let i105 = address
        .to_i105_for_discriminant(network_prefix)
        .expect("I105 encoding must succeed");
    let canonical_bytes = address
        .canonical_bytes()
        .expect("canonical bytes must be obtainable");
    let view = canonical_view(&canonical_bytes);
    debug_assert_eq!(
        view.controller_tag, CONTROLLER_SINGLE_KEY_TAG,
        "single-key vectors expect single controller tag"
    );
    let single_payload = decode_single_controller_payload(view.controller_payload);
    let domain_selector = canonical_selector_metadata();

    let (algorithm, key_bytes) = account
        .controller()
        .single_signatory()
        .expect("single-key account must have signatory")
        .to_bytes();

    let public_key_hex = format_hex_prefixed(key_bytes);

    SingleKeyVector {
        domain_label: label,
        seed_byte: seed,
        account_id: account.to_string(),
        canonical_hex,
        i105,
        domain_selector,
        controller_curve_id: single_payload.curve_id,
        controller_algorithm: algorithm.to_string(),
        public_key_hex,
    }
}

fn build_multisig_vectors(network_prefix: u16) -> Vec<MultisigVector> {
    MULTISIG_FIXTURES
        .iter()
        .map(|spec| {
            let members = spec
                .members
                .iter()
                .map(|(seed, weight)| {
                    MultisigMember::new(ed25519_pk_with(*seed), *weight)
                        .expect("multisig member configuration must be valid")
                })
                .collect::<Vec<_>>();
            let policy = MultisigPolicy::new(spec.threshold, members).expect("valid policy");
            let account = AccountId::new_multisig(policy.clone());
            let address = AccountAddress::from_account_id(&account)
                .expect("multisig account should encode into AccountAddress");
            let canonical_bytes = address
                .canonical_bytes()
                .expect("canonical bytes must be obtainable");
            let view = canonical_view(&canonical_bytes);
            debug_assert_eq!(
                view.controller_tag, CONTROLLER_MULTISIG_TAG,
                "multisig vector expects multisig controller tag"
            );
            let controller_payload = decode_multisig_payload(view.controller_payload);
            let members = controller_payload
                .members
                .iter()
                .enumerate()
                .map(|(index, member)| MultisigMemberVector {
                    index,
                    curve_id: member.curve_id,
                    weight: member.weight,
                    public_key_hex: format_hex_prefixed(member.key_bytes),
                    key_length: member.key_bytes.len(),
                })
                .collect::<Vec<_>>();

            let policy_cbor = policy.encode_ctap2();
            let policy_digest = policy.digest_blake2b256();

            MultisigVector {
                domain_label: spec.domain,
                account_id: account.to_string(),
                canonical_hex: address
                    .canonical_hex()
                    .expect("canonical hex must encode for multisig vector"),
                i105: address
                    .to_i105_for_discriminant(network_prefix)
                    .expect("I105 encoding must succeed for multisig vector"),
                domain_selector: canonical_selector_metadata(),
                version: controller_payload.version,
                threshold: controller_payload.threshold,
                total_weight: policy.total_weight(),
                members,
                policy_cbor_hex: format_hex_prefixed(policy_cbor),
                policy_digest_hex: format_hex_prefixed(policy_digest),
            }
        })
        .collect()
}

fn build_error_vectors(network_prefix: u16) -> Vec<ErrorVector> {
    ErrorHarness::new(network_prefix).build_all()
}

struct ErrorHarness {
    network_prefix: u16,
    address: AccountAddress,
    i105: String,
    canonical_hex: String,
}

impl ErrorHarness {
    fn new(network_prefix: u16) -> Self {
        let account = AccountId::new(ed25519_pk_with(0x2A));
        let address = AccountAddress::from_account_id(&account)
            .expect("single-key account should encode into AccountAddress");
        let i105 = address
            .to_i105_for_discriminant(network_prefix)
            .expect("I105 encoding must succeed");
        let canonical_hex = address
            .canonical_hex()
            .expect("canonical hex must encode for error harness");
        Self {
            network_prefix,
            address,
            i105,
            canonical_hex,
        }
    }

    fn build_all(&self) -> Vec<ErrorVector> {
        vec![
            self.i105_invalid_char(),
            self.i105_checksum_mismatch(),
            Self::i105_too_short(),
            self.i105_unexpected_discriminant(),
            Self::canonical_invalid_hex(),
            Self::unsupported_alias_literal(),
            self.domain_mismatch(),
        ]
    }

    fn i105_invalid_char(&self) -> ErrorVector {
        let mut invalid_char = self.i105.clone();
        invalid_char.replace_range(0..=0, "!");
        let err =
            AccountAddress::from_i105(&invalid_char).expect_err("invalid character must fail");

        ErrorVector {
            label: "i105_invalid_char",
            decoder: "i105",
            input: invalid_char,
            error_variant: variant_name(&err),
            error_code: err.code_str(),
            message: err.to_string(),
            details: Some(json_object(vec![("invalid_char", json_value("!"))])),
        }
    }

    fn i105_checksum_mismatch(&self) -> ErrorVector {
        let mut digits = i105_to_digits(&self.i105).expect("valid i105 digits");
        let tamper_index = digits
            .len()
            .saturating_sub(I105_CHECKSUM_LEN)
            .saturating_sub(1);
        digits[tamper_index] = (digits[tamper_index] + 1) % I105_BASE_U8;
        let tampered = digits_to_i105_literal(&digits);
        let err = AccountAddress::from_i105(&tampered).expect_err("checksum mismatch must fail");

        ErrorVector {
            label: "i105_checksum_mismatch",
            decoder: "i105",
            input: tampered,
            error_variant: variant_name(&err),
            error_code: err.code_str(),
            message: err.to_string(),
            details: None,
        }
    }

    fn i105_too_short() -> ErrorVector {
        let too_short = String::new();
        let err = AccountAddress::from_i105(&too_short).expect_err("too short i105 form must fail");

        ErrorVector {
            label: "i105_too_short",
            decoder: "i105",
            input: too_short,
            error_variant: variant_name(&err),
            error_code: err.code_str(),
            message: err.to_string(),
            details: None,
        }
    }

    fn i105_unexpected_discriminant(&self) -> ErrorVector {
        let err = AccountAddress::from_i105_for_discriminant(
            &self.i105,
            Some(self.network_prefix.wrapping_add(1)),
        )
        .expect_err("unexpected network prefix must fail");
        let AccountAddressError::UnexpectedNetworkPrefix { expected, found } = err else {
            panic!("unexpected error variant from I105 discriminant guard");
        };
        let expected_hex = format_u16_hex(expected);
        let found_hex = format_u16_hex(found);

        ErrorVector {
            label: "i105_unexpected_discriminant",
            decoder: "i105",
            input: self.i105.clone(),
            error_variant: variant_name(&err),
            error_code: err.code_str(),
            message: err.to_string(),
            details: Some(json_object(vec![
                ("expected", json_value(&expected_hex)),
                ("found", json_value(&found_hex)),
            ])),
        }
    }

    fn canonical_invalid_hex() -> ErrorVector {
        let invalid_hex = "0xnothex";
        let err = AccountAddress::parse_encoded(invalid_hex, None)
            .expect_err("invalid canonical_hex strict decode must fail");

        ErrorVector {
            label: "canonical_invalid_hex",
            decoder: "canonical_hex",
            input: invalid_hex.to_owned(),
            error_variant: variant_name(&err),
            error_code: err.code_str(),
            message: err.to_string(),
            details: None,
        }
    }

    fn unsupported_alias_literal() -> ErrorVector {
        let alias_literal = "alice@hbl.dataspace";
        let err = AccountAddress::parse_encoded(alias_literal, None)
            .expect_err("alias literal must fail");

        ErrorVector {
            label: "unsupported_alias_literal",
            decoder: "auto_detect",
            input: alias_literal.to_owned(),
            error_variant: variant_name(&err),
            error_code: err.code_str(),
            message: err.to_string(),
            details: None,
        }
    }

    fn domain_mismatch(&self) -> ErrorVector {
        let mut mismatched = self.address.clone();
        mismatched.domain = DomainSelector::Local12(compute_local_digest("wonderland"));
        let other_domain = domain_id("treasury");
        let err = mismatched
            .ensure_domain_matches(&other_domain)
            .expect_err("domain mismatch must fail");
        let other_domain_label = other_domain.to_string();

        ErrorVector {
            label: "domain_mismatch",
            decoder: "domain_check",
            input: self.canonical_hex.clone(),
            error_variant: variant_name(&err),
            error_code: err.code_str(),
            message: err.to_string(),
            details: Some(json_object(vec![(
                "expected_domain",
                json_value(&other_domain_label),
            )])),
        }
    }
}

#[derive(Clone, Debug)]
struct SingleControllerPayload {
    curve_id: u8,
}

#[derive(Clone, Debug)]
struct MultisigControllerPayload<'a> {
    version: u8,
    threshold: u16,
    members: Vec<MultisigMemberPayload<'a>>,
}

#[derive(Clone, Debug)]
struct MultisigMemberPayload<'a> {
    curve_id: u8,
    weight: u16,
    key_bytes: &'a [u8],
}

#[derive(Clone, Copy)]
struct CanonicalView<'a> {
    controller_tag: u8,
    controller_payload: &'a [u8],
}

fn canonical_view(bytes: &[u8]) -> CanonicalView<'_> {
    debug_assert!(
        !bytes.is_empty(),
        "canonical bytes must contain address header"
    );
    let controller_tag = *bytes
        .get(1)
        .expect("canonical payload must contain controller tag");
    let controller_payload = bytes
        .get(2..)
        .expect("controller payload slice must be present");
    CanonicalView {
        controller_tag,
        controller_payload,
    }
}

fn canonical_selector_metadata() -> DomainSelectorVector {
    DomainSelectorVector::ImplicitDefault
}

fn decode_single_controller_payload(payload: &[u8]) -> SingleControllerPayload {
    let curve_id = payload
        .first()
        .copied()
        .expect("single controller payload must include curve id");
    SingleControllerPayload { curve_id }
}

fn decode_multisig_payload(payload: &[u8]) -> MultisigControllerPayload<'_> {
    let version = payload
        .first()
        .copied()
        .expect("multisig payload must include version");
    let threshold = u16::from_be_bytes(
        payload
            .get(1..3)
            .expect("multisig payload must include threshold")
            .try_into()
            .expect("threshold slice must be 2 bytes"),
    );
    let member_count = payload
        .get(3)
        .copied()
        .expect("multisig payload must include member count") as usize;
    let mut cursor = 4;
    let mut members = Vec::with_capacity(member_count);
    for _ in 0..member_count {
        let curve_id = payload
            .get(cursor)
            .copied()
            .expect("member payload must include curve id");
        cursor += 1;
        let weight = u16::from_be_bytes(
            payload
                .get(cursor..cursor + 2)
                .expect("member payload must include weight")
                .try_into()
                .expect("weight slice must be 2 bytes"),
        );
        cursor += 2;
        let key_len = u16::from_be_bytes(
            payload
                .get(cursor..cursor + 2)
                .expect("member payload must include key length")
                .try_into()
                .expect("key length slice must be 2 bytes"),
        ) as usize;
        cursor += 2;
        let key_bytes = payload
            .get(cursor..cursor + key_len)
            .expect("member payload must include key bytes");
        cursor += key_len;
        members.push(MultisigMemberPayload {
            curve_id,
            weight,
            key_bytes,
        });
    }
    MultisigControllerPayload {
        version,
        threshold,
        members,
    }
}

fn domain_id(label: &str) -> DomainId {
    DomainId::new(
        Name::from_str(label).unwrap_or_else(|_| panic!("invalid domain label `{label}`")),
    )
}

fn ed25519_pk_with(byte: u8) -> PublicKey {
    let seed = vec![byte; 32];
    let (public_key, _) = KeyPair::from_seed(seed, Algorithm::Ed25519).into_parts();
    public_key
}

fn variant_name(error: &AccountAddressError) -> &'static str {
    match error {
        UnsupportedAlgorithm(_) => "UnsupportedAlgorithm",
        KeyPayloadTooLong(_) => "KeyPayloadTooLong",
        InvalidHeaderVersion(_) => "InvalidHeaderVersion",
        InvalidNormVersion(_) => "InvalidNormVersion",
        InvalidLength => "InvalidLength",
        ChecksumMismatch => "ChecksumMismatch",
        InvalidHexAddress => "InvalidHexAddress",
        DomainMismatch => "DomainMismatch",
        InvalidDomainLabel(_) => "InvalidDomainLabel",
        UnexpectedNetworkPrefix { .. } => "UnexpectedNetworkPrefix",
        UnknownAddressClass(_) => "UnknownAddressClass",
        UnexpectedExtensionFlag => "UnexpectedExtensionFlag",
        UnknownControllerTag(_) => "UnknownControllerTag",
        InvalidPublicKey => "InvalidPublicKey",
        UnknownCurve(_) => "UnknownCurve",
        UnexpectedTrailingBytes => "UnexpectedTrailingBytes",
        MissingI105Sentinel => "MissingI105Sentinel",
        I105TooShort => "I105TooShort",
        InvalidI105Char(_) => "InvalidI105Char",
        InvalidI105Base => "InvalidI105Base",
        InvalidI105Digit(_) => "InvalidI105Digit",
        UnsupportedAddressFormat => "UnsupportedAddressFormat",
        MultisigMemberOverflow(_) => "MultisigMemberOverflow",
        InvalidMultisigPolicy(_) => "InvalidMultisigPolicy",
    }
}

fn format_u8_hex(value: u8) -> String {
    format!("0x{value:02x}")
}

fn format_u16_hex(value: u16) -> String {
    format!("0x{value:04x}")
}

fn format_hex_prefixed(bytes: impl AsRef<[u8]>) -> String {
    format!("0x{}", hex::encode(bytes.as_ref()))
}

impl fmt::Display for AddressVectorBundle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let json = json::to_json_pretty(&self.to_json_value())
            .expect("vector bundle must serialize to JSON");
        write!(f, "{json}")
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::account::address::AccountAddress;

    #[test]
    fn json_value_serialises_borrowed_inputs() {
        let owned = String::from("owned");
        let owned_value = json_value(&owned);
        assert_eq!(
            owned_value,
            json::to_value(&owned).expect("serialize owned string")
        );

        let literal_value = json_value("literal");
        assert_eq!(
            literal_value,
            json::to_value("literal").expect("serialize literal str")
        );
        assert_eq!(literal_value, Value::String("literal".to_owned()));

        let number = 7_u32;
        let number_value = json_value(&number);
        assert_eq!(
            number_value,
            json::to_value(&number).expect("serialize integer")
        );

        assert_eq!(owned, "owned");
    }

    #[test]
    fn default_single_vector_matches_fixture() {
        let _guard = default_domain_guard(Some("default"));
        let bundle = build_vector_bundle();
        assert_eq!(bundle.single_key.len(), VECTOR_SINGLE_DOMAINS.len());

        let default_vector = bundle
            .single_key
            .iter()
            .find(|vector| vector.domain_label == "default")
            .expect("default domain vector must be present");

        assert_eq!(
            default_vector.canonical_hex,
            "0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29"
        );
        assert!(default_vector.i105.starts_with("n4660"));
        assert!(matches!(
            default_vector.domain_selector,
            DomainSelectorVector::ImplicitDefault
        ));
    }

    #[test]
    fn error_vectors_cover_expected_variants() {
        let bundle = build_vector_bundle();
        let variants = bundle
            .errors
            .iter()
            .map(|vector| vector.error_variant)
            .collect::<Vec<_>>();
        let codes = bundle
            .errors
            .iter()
            .map(|vector| vector.error_code)
            .collect::<Vec<_>>();

        assert!(variants.contains(&"InvalidI105Char"));
        assert!(variants.contains(&"ChecksumMismatch"));
        assert!(variants.contains(&"I105TooShort"));
        assert!(variants.contains(&"UnexpectedNetworkPrefix"));
        assert!(variants.contains(&"UnsupportedAddressFormat"));
        assert!(variants.contains(&"DomainMismatch"));
        assert!(codes.contains(&"ERR_INVALID_I105_CHAR"));
        assert!(codes.contains(&"ERR_CHECKSUM_MISMATCH"));
        assert!(codes.contains(&"ERR_I105_TOO_SHORT"));
        assert!(codes.contains(&"ERR_UNEXPECTED_NETWORK_PREFIX"));
        assert!(codes.contains(&"ERR_UNSUPPORTED_ADDRESS_FORMAT"));
        assert!(codes.contains(&"ERR_DOMAIN_MISMATCH"));
    }

    #[test]
    fn error_vector_shapes_match_expected_decoders() {
        let bundle = build_vector_bundle();
        let actual = bundle
            .errors
            .iter()
            .map(|vector| (vector.label, vector.decoder))
            .collect::<Vec<_>>();
        let expected = vec![
            ("i105_invalid_char", "i105"),
            ("i105_checksum_mismatch", "i105"),
            ("i105_too_short", "i105"),
            ("i105_unexpected_discriminant", "i105"),
            ("canonical_invalid_hex", "canonical_hex"),
            ("unsupported_alias_literal", "auto_detect"),
            ("domain_mismatch", "domain_check"),
        ];
        assert_eq!(actual, expected);
    }

    proptest! {
        #[test]
        fn i105_roundtrip(seed in any::<u8>(), domain_index in 0usize..VECTOR_SINGLE_DOMAINS.len()) {
            let _guard = default_domain_guard(Some("default"));
            let label = VECTOR_SINGLE_DOMAINS[domain_index].0;
            let _domain = domain_id(label);
            let account = AccountId::new(ed25519_pk_with(seed));
            let address = AccountAddress::from_account_id(&account)
                .expect("account must encode into AccountAddress");
            let literal = address.to_i105().expect("i105 encoding succeeds");
            let decoded = AccountAddress::parse_encoded(&literal, None).expect("parse i105 value succeeds");
            prop_assert_eq!(
                decoded.canonical_bytes().expect("decoded canonical bytes"),
                address.canonical_bytes().expect("source canonical bytes")
            );
        }
    }
}
