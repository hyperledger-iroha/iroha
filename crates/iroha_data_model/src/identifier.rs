//! Hidden-function-backed identifier policy and claim types.

use std::{fmt, str::FromStr, string::String, vec::Vec};

use iroha_crypto::{Hash, PublicKey, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    account::{AccountId, OpaqueAccountId},
    name::Name,
    nexus::UniversalAccountId,
    ram_lfe::{
        RamLfeExecutionReceiptPayload, RamLfeOutputOpening, RamLfeProgramId,
        RamLfeReceiptAttestation,
    },
};

/// Error returned while parsing [`IdentifierPolicyId`] literals.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum IdentifierPolicyIdParseError {
    /// The policy literal must use `kind#rule`.
    #[error("identifier policy literal must use `kind#rule`")]
    InvalidFormat,
    /// One of the policy components is invalid.
    #[error("{0}")]
    InvalidName(String),
}

/// Error returned while canonicalizing a raw identifier input.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum IdentifierNormalizationError {
    /// The raw input is empty after trimming.
    #[error("identifier input must not be empty")]
    Empty,
    /// The selected normalization mode rejected the input.
    #[error("{0}")]
    InvalidFormat(String),
}

/// Canonicalization strategy applied before an identifier enters policy derivation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(tag = "normalization", content = "value", rename_all = "snake_case")]
pub enum IdentifierNormalization {
    /// Trim outer whitespace and otherwise preserve the original bytes.
    Exact,
    /// Trim outer whitespace and lowercase the result.
    LowercaseTrimmed,
    /// Canonicalize a phone-like input into `+<digits>`.
    PhoneE164,
    /// Trim and lowercase a simple email address.
    EmailAddress,
    /// Remove spaces/`-`, uppercase ASCII letters, preserve digits.
    AccountNumber,
}

impl IdentifierNormalization {
    /// Canonicalize an external identifier string according to this mode.
    ///
    /// # Errors
    ///
    /// Returns [`IdentifierNormalizationError`] when the trimmed input is empty or when the
    /// selected normalization mode rejects the supplied format.
    pub fn normalize(self, raw: &str) -> Result<String, IdentifierNormalizationError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Err(IdentifierNormalizationError::Empty);
        }
        match self {
            Self::Exact => Ok(trimmed.to_owned()),
            Self::LowercaseTrimmed => Ok(trimmed.to_ascii_lowercase()),
            Self::PhoneE164 => normalize_phone_e164(trimmed),
            Self::EmailAddress => normalize_email_address(trimmed),
            Self::AccountNumber => normalize_account_number(trimmed),
        }
    }
}

/// Canonical identifier policy namespace key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct IdentifierPolicyId {
    /// Identifier kind such as `phone`, `email`, or `account_number`.
    pub kind: Name,
    /// Business-rule namespace within the identifier kind.
    pub business_rule: Name,
}

impl IdentifierPolicyId {
    /// Construct a new policy identifier.
    #[must_use]
    pub const fn new(kind: Name, business_rule: Name) -> Self {
        Self {
            kind,
            business_rule,
        }
    }
}

impl fmt::Display for IdentifierPolicyId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}#{}", self.kind, self.business_rule)
    }
}

impl FromStr for IdentifierPolicyId {
    type Err = IdentifierPolicyIdParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();
        let (kind, business_rule) = trimmed
            .split_once('#')
            .ok_or(IdentifierPolicyIdParseError::InvalidFormat)?;
        let kind = Name::from_str(kind)
            .map_err(|err| IdentifierPolicyIdParseError::InvalidName(err.to_string()))?;
        let business_rule = Name::from_str(business_rule)
            .map_err(|err| IdentifierPolicyIdParseError::InvalidName(err.to_string()))?;
        Ok(Self::new(kind, business_rule))
    }
}

/// Public metadata for a globally unique hidden-function identifier namespace.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct IdentifierPolicy {
    /// Policy namespace identifier.
    pub id: IdentifierPolicyId,
    /// Account that registered and controls this policy.
    pub owner: AccountId,
    /// Canonicalization mode applied before hidden-function derivation.
    pub normalization: IdentifierNormalization,
    /// Referenced generic RAM-LFE program policy.
    pub program_id: RamLfeProgramId,
    /// Whether the policy is active for new claims and resolutions.
    pub active: bool,
    /// Optional human-readable note.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub note: Option<String>,
}

impl IdentifierPolicy {
    /// Construct a new inactive identifier policy.
    #[must_use]
    pub fn new(
        id: IdentifierPolicyId,
        owner: AccountId,
        normalization: IdentifierNormalization,
        program_id: RamLfeProgramId,
    ) -> Self {
        Self {
            id,
            owner,
            normalization,
            program_id,
            active: false,
            note: None,
        }
    }

    /// Attach an optional note.
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

/// Persisted claim binding an opaque identifier to a UAID under one policy.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct IdentifierClaimRecord {
    /// Claimed identifier namespace.
    pub policy_id: IdentifierPolicyId,
    /// Bound opaque identifier.
    pub opaque_id: OpaqueAccountId,
    /// Hidden-function receipt hash that produced the opaque identifier.
    pub receipt_hash: Hash,
    /// UAID that owns the identifier claim.
    pub uaid: UniversalAccountId,
    /// Canonical account currently bound to the UAID.
    pub account_id: AccountId,
    /// Verification timestamp in milliseconds since Unix epoch.
    pub verified_at_ms: u64,
    /// Optional expiry timestamp.
    #[norito(skip_serializing_if = "Option::is_none")]
    #[norito(default)]
    pub expires_at_ms: Option<u64>,
}

/// Receipt emitted by identifier resolution services.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct IdentifierResolutionReceipt {
    /// Canonical payload covered by the attestation.
    pub payload: IdentifierResolutionReceiptPayload,
    /// Explicit receipt attestation.
    pub attestation: RamLfeReceiptAttestation,
}

/// Canonical payload covered by an identifier-resolution receipt signature.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct IdentifierResolutionReceiptPayload {
    /// Policy namespace used for the resolution.
    pub policy_id: IdentifierPolicyId,
    /// Generic RAM-LFE execution receipt payload.
    pub execution: RamLfeExecutionReceiptPayload,
    /// Externally verified opening for the RAM-LFE encrypted output.
    pub opening: RamLfeOutputOpening,
    /// Opaque identifier derived by the hidden-function resolver.
    pub opaque_id: OpaqueAccountId,
    /// Hidden-function receipt hash covering the evaluation transcript.
    pub receipt_hash: Hash,
    /// UAID reached by the opaque identifier.
    pub uaid: UniversalAccountId,
    /// Canonical account currently bound to the UAID.
    pub account_id: AccountId,
}

impl IdentifierResolutionReceipt {
    /// Return the canonical signed payload view of this receipt.
    #[must_use]
    pub fn payload(&self) -> IdentifierResolutionReceiptPayload {
        self.payload.clone()
    }

    /// Encode the canonical signed payload bytes used by resolver signatures.
    #[must_use]
    pub fn payload_bytes(&self) -> Vec<u8> {
        self.payload.encode()
    }

    /// Resolution timestamp in milliseconds since Unix epoch.
    #[must_use]
    pub const fn resolved_at_ms(&self) -> u64 {
        self.payload.execution.executed_at_ms
    }

    /// Optional expiry timestamp for the receipt.
    #[must_use]
    pub const fn expires_at_ms(&self) -> Option<u64> {
        self.payload.execution.expires_at_ms
    }

    /// Verify the receipt signature against the provided public key.
    ///
    /// # Errors
    /// Returns the underlying signature verification error when the signature is invalid.
    pub fn verify(&self, public_key: &PublicKey) -> Result<(), iroha_crypto::Error> {
        SignatureOf::<IdentifierResolutionReceiptPayload>::from_signature(
            self.attestation.signature().cloned().ok_or_else(|| {
                iroha_crypto::Error::Other("identifier receipt is missing a signature".to_owned())
            })?,
        )
        .verify(public_key, &self.payload)
    }
}

/// Prelude exports for identifier policy consumers.
pub mod prelude {
    pub use super::{
        IdentifierClaimRecord, IdentifierNormalization, IdentifierNormalizationError,
        IdentifierPolicy, IdentifierPolicyId, IdentifierPolicyIdParseError,
        IdentifierResolutionReceipt, IdentifierResolutionReceiptPayload,
    };
}

fn normalize_phone_e164(raw: &str) -> Result<String, IdentifierNormalizationError> {
    let compact: String = raw
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '\t' | '\n' | '\r' | '-' | '(' | ')' | '.'))
        .collect();
    let without_prefix = compact
        .strip_prefix('+')
        .or_else(|| compact.strip_prefix("00"))
        .unwrap_or(compact.as_str());
    if without_prefix.is_empty() || !without_prefix.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(IdentifierNormalizationError::InvalidFormat(
            "phone normalization expects digits with optional leading `+` or `00`".to_owned(),
        ));
    }
    Ok(format!("+{without_prefix}"))
}

fn normalize_email_address(raw: &str) -> Result<String, IdentifierNormalizationError> {
    let lowered = raw.trim().to_ascii_lowercase();
    let mut parts = lowered.split('@');
    let local = parts.next().unwrap_or_default();
    let domain = parts.next().unwrap_or_default();
    if local.is_empty() || domain.is_empty() || parts.next().is_some() {
        return Err(IdentifierNormalizationError::InvalidFormat(
            "email normalization expects exactly one `@` with non-empty local and domain parts"
                .to_owned(),
        ));
    }
    Ok(lowered)
}

fn normalize_account_number(raw: &str) -> Result<String, IdentifierNormalizationError> {
    let normalized: String = raw
        .chars()
        .filter(|ch| !matches!(ch, ' ' | '\t' | '\n' | '\r' | '-'))
        .map(|ch| ch.to_ascii_uppercase())
        .collect();
    if normalized.is_empty()
        || !normalized
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '/' | '.'))
    {
        return Err(IdentifierNormalizationError::InvalidFormat(
            "account-number normalization expects ASCII alphanumeric input".to_owned(),
        ));
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use iroha_crypto::{
        Algorithm, KeyPair, PublicKey, RamLfeBackend, RamLfeVerificationMode, Signature,
        SignatureOf,
    };
    use sha2::{Digest as _, Sha256};

    use super::*;

    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("generate checked identifier fixture keypair")
    }

    fn checked_seed_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked identifier fixture keypair")
    }

    fn checked_signature<T: norito::codec::Encode>(
        signer: &KeyPair,
        payload: &T,
    ) -> SignatureOf<T> {
        SignatureOf::try_new(signer.private_key(), payload)
            .expect("sign checked identifier fixture payload")
    }

    #[test]
    fn identifier_policy_id_roundtrip() {
        let id: IdentifierPolicyId = "phone#retail".parse().expect("valid policy id");
        assert_eq!(id.to_string(), "phone#retail");
    }

    #[test]
    fn phone_normalization_strips_formatting() {
        let normalized = IdentifierNormalization::PhoneE164
            .normalize(" +1 (555) 123-4567 ")
            .expect("phone should normalize");
        assert_eq!(normalized, "+15551234567");
    }

    #[test]
    fn email_normalization_lowercases_and_trims() {
        let normalized = IdentifierNormalization::EmailAddress
            .normalize(" Alice.Example@Example.COM ")
            .expect("email should normalize");
        assert_eq!(normalized, "alice.example@example.com");
    }

    #[test]
    fn receipt_payload_bytes_match_signed_encode_bytes() {
        let account_signatory = checked_random_keypair().public_key().clone();
        let opening_payload = crate::ram_lfe::RamLfeOutputOpeningPayload {
            program_id: RamLfeProgramId::from_str("email_retail").expect("valid program id"),
            input_ciphertext_hash: Hash::new(b"input-ciphertext"),
            output_ciphertext_hash: Hash::new(b"output-ciphertext"),
            parameter_digest: Hash::new(b"parameters"),
            evaluation_key_digest: Hash::new(b"evaluation-keys"),
            opened_output_hash: Hash::new(b"opened-output"),
            opened_at_ms: 1_777_777_777_001,
            expires_at_ms: Some(1_777_777_877_000),
        };
        let opening_signer = checked_random_keypair();
        let opening_signature = checked_signature(&opening_signer, &opening_payload);
        opening_signature
            .verify(opening_signer.public_key(), &opening_payload)
            .expect("checked identifier output-opening fixture signature verifies");
        let opening = crate::ram_lfe::RamLfeOutputOpening {
            signature: opening_signature.into(),
            payload: opening_payload,
        };
        let payload = IdentifierResolutionReceiptPayload {
            policy_id: IdentifierPolicyId::from_str("email#retail").expect("valid policy"),
            execution: RamLfeExecutionReceiptPayload {
                program_id: RamLfeProgramId::from_str("email_retail").expect("valid program id"),
                program_digest: Hash::new(b"program"),
                backend: RamLfeBackend::BfvProgrammedSha3_256V1,
                verification_mode: RamLfeVerificationMode::Signed,
                input_ciphertext_hash: Hash::new(b"input-ciphertext"),
                output_ciphertext_hash: Hash::new(b"output-ciphertext"),
                parameter_digest: Hash::new(b"parameters"),
                evaluation_key_digest: Hash::new(b"evaluation-keys"),
                output_hash: Hash::new(b"output"),
                associated_data_hash: Hash::new(b"associated-data"),
                executed_at_ms: 1_777_777_777_000,
                expires_at_ms: Some(1_777_777_877_000),
            },
            opening,
            opaque_id: OpaqueAccountId::from_hash(Hash::new(b"opaque")),
            receipt_hash: Hash::new(b"receipt"),
            uaid: UniversalAccountId::from_hash(Hash::new(b"uaid")),
            account_id: AccountId::new(account_signatory),
        };
        let signer = checked_random_keypair();
        let signature = checked_signature(&signer, &payload);
        let receipt = IdentifierResolutionReceipt {
            payload: payload.clone(),
            attestation: RamLfeReceiptAttestation::Signed(iroha_crypto::Signature::from_bytes(
                signature.payload(),
            )),
        };

        assert_eq!(receipt.payload_bytes(), payload.encode());
        signature
            .verify(signer.public_key(), &payload)
            .expect("signature should verify against bare encode bytes");
    }

    #[test]
    fn live_identifier_resolution_receipt_payload_fixture_matches_current_encoding() {
        let receipt = live_identifier_resolution_receipt_fixture();
        let resolver_key = PublicKey::from_str(
            "ed01200376E59E9078B647F55003896B59758B7BE99908535EC24BAF80A6D52C8B3EB8",
        )
        .expect("valid resolver key");

        assert!(!hex::encode_upper(receipt.payload_bytes()).is_empty());
        assert!(receipt.verify(&resolver_key).is_err());
    }

    #[test]
    fn identifier_resolution_receipt_matches_shared_fixture_vectors() {
        let fixture = shared_identifier_receipt_fixture();
        assert_eq!(
            fixture_str(&fixture, "vector_set"),
            "identifier-receipt-attestation-v1"
        );
        let receipt_object = fixture_object(&fixture, "receipt");
        let receipt = receipt_from_fixture(receipt_object);
        let resolver_key = public_key_literal(fixture_str(&fixture, "resolver_public_key"));

        assert_eq!(
            fixture_str(&fixture, "canonical_payload_sha256"),
            sha256_hex(&receipt.payload_bytes())
        );
        receipt
            .verify(&resolver_key)
            .expect("shared identifier receipt signature must verify");

        for vector in fixture_array(&fixture, "attestation_vectors") {
            let name = fixture_str(vector, "name");
            let attestation = attestation_from_fixture(fixture_object(vector, "attestation"));
            let encoded = attestation.encode();
            assert_eq!(
                fixture_u64(vector, "expected_attestation_bytes"),
                u64::try_from(encoded.len()).expect("attestation length fits u64"),
                "{name} byte length"
            );
            assert_eq!(
                fixture_str(vector, "expected_attestation_sha256"),
                sha256_hex(&encoded),
                "{name} digest"
            );
            if matches!(attestation, RamLfeReceiptAttestation::Proof(_)) {
                let proof_receipt = IdentifierResolutionReceipt {
                    payload: receipt.payload.clone(),
                    attestation,
                };
                proof_receipt
                    .verify(&resolver_key)
                    .expect_err("proof-only attestations must not verify as signatures");
            }
        }

        for negative in fixture_array(&fixture, "negative_cases") {
            let name = fixture_str(negative, "name");
            let mutation = fixture_str(negative, "mutation");
            let mut mutated = receipt.clone();
            let mut key = resolver_key.clone();
            match mutation {
                "receipt.payload.execution.output_ciphertext_hash" => {
                    mutated.payload.execution.output_ciphertext_hash =
                        hash_hex(fixture_str(negative, "value"));
                }
                "policy.resolver_public_key" => {
                    let raw = fixture_str(negative, "value");
                    if raw.trim() != raw {
                        assert!(
                            fixture_str(negative, "expected_error_contains").contains("whitespace"),
                            "{name} must document whitespace rejection"
                        );
                        continue;
                    }
                    key = public_key_literal(raw);
                }
                "policy.policy_id" => {
                    let raw = fixture_str(negative, "value");
                    if raw.trim() != raw {
                        assert!(
                            fixture_str(negative, "expected_error_contains").contains("whitespace"),
                            "{name} must document whitespace rejection"
                        );
                        continue;
                    }
                    mutated.payload.policy_id =
                        IdentifierPolicyId::from_str(raw).expect("valid policy id mutation");
                }
                "receipt.attestation.signature" => {
                    mutated.attestation = RamLfeReceiptAttestation::Signed(
                        Signature::from_hex(fixture_str(negative, "value"))
                            .unwrap_or_else(|_| Signature::from_bytes(&[0x42; 64])),
                    );
                }
                "receipt.attestation" => {
                    mutated.attestation =
                        attestation_from_fixture(fixture_object(negative, "value"));
                }
                other => panic!("unhandled shared fixture mutation `{other}`"),
            }
            assert!(mutated.verify(&key).is_err(), "{name} must reject");
        }
    }

    #[test]
    fn identifier_resolution_receipt_rejects_tampered_payload_after_signing() {
        let mut payload = live_identifier_resolution_payload_fixture();
        let signer = checked_random_keypair();
        let signature = checked_signature(&signer, &payload);
        let mut receipt = IdentifierResolutionReceipt {
            payload: payload.clone(),
            attestation: RamLfeReceiptAttestation::Signed(Signature::from_bytes(
                signature.payload(),
            )),
        };

        payload.opening.payload.opened_output_hash = Hash::new(b"tampered-opened-output");
        receipt.payload = payload;

        receipt
            .verify(signer.public_key())
            .expect_err("tampering nested output-opening payload must invalidate receipt");
    }

    #[test]
    fn identifier_resolution_receipt_rejects_mutation_of_security_bindings() {
        macro_rules! assert_rejected {
            ($label:literal, |$payload:ident| $body:block) => {{
                let signer = checked_random_keypair();
                let payload = live_identifier_resolution_payload_fixture();
                let signature = checked_signature(&signer, &payload);
                let mut receipt = IdentifierResolutionReceipt {
                    payload,
                    attestation: RamLfeReceiptAttestation::Signed(Signature::from_bytes(
                        signature.payload(),
                    )),
                };
                let $payload = &mut receipt.payload;
                $body
                assert!(
                    receipt.verify(signer.public_key()).is_err(),
                    "mutating {} must invalidate identifier receipt signature",
                    $label
                );
            }};
        }

        assert_rejected!("policy_id", |payload| {
            payload.policy_id = IdentifierPolicyId::from_str("phone#retail").expect("valid policy");
        });
        assert_rejected!("execution.program_id", |payload| {
            payload.execution.program_id =
                RamLfeProgramId::from_str("phone_retail").expect("valid program");
        });
        assert_rejected!("execution.backend", |payload| {
            payload.execution.backend = RamLfeBackend::BfvAffineSha3_256V1;
        });
        assert_rejected!("execution.input_ciphertext_hash", |payload| {
            payload.execution.input_ciphertext_hash = Hash::new(b"tampered-input");
        });
        assert_rejected!("execution.output_ciphertext_hash", |payload| {
            payload.execution.output_ciphertext_hash = Hash::new(b"tampered-output");
        });
        assert_rejected!("execution.parameter_digest", |payload| {
            payload.execution.parameter_digest = Hash::new(b"tampered-parameters");
        });
        assert_rejected!("execution.evaluation_key_digest", |payload| {
            payload.execution.evaluation_key_digest = Hash::new(b"tampered-eval-keys");
        });
        assert_rejected!("opening.payload.input_ciphertext_hash", |payload| {
            payload.opening.payload.input_ciphertext_hash = Hash::new(b"tampered-opening-input");
        });
        assert_rejected!("opening.payload.output_ciphertext_hash", |payload| {
            payload.opening.payload.output_ciphertext_hash = Hash::new(b"tampered-opening-output");
        });
        assert_rejected!("opening.signature", |payload| {
            payload.opening.signature = Signature::from_bytes(&[0x42; 64]);
        });
        assert_rejected!("opaque_id", |payload| {
            payload.opaque_id = OpaqueAccountId::from_hash(Hash::new(b"tampered-opaque"));
        });
        assert_rejected!("receipt_hash", |payload| {
            payload.receipt_hash = Hash::new(b"tampered-receipt");
        });
        assert_rejected!("uaid", |payload| {
            payload.uaid = UniversalAccountId::from_hash(Hash::new(b"tampered-uaid"));
        });
        assert_rejected!("account_id", |payload| {
            payload.account_id = AccountId::new(checked_random_keypair().public_key().clone());
        });
    }

    #[test]
    fn identifier_resolution_receipt_rejects_wrong_resolver_key() {
        let payload = live_identifier_resolution_payload_fixture();
        let signer = checked_random_keypair();
        let wrong_signer = checked_random_keypair();
        let receipt = IdentifierResolutionReceipt {
            payload: payload.clone(),
            attestation: RamLfeReceiptAttestation::Signed(Signature::from_bytes(
                checked_signature(&signer, &payload).payload(),
            )),
        };

        receipt
            .verify(wrong_signer.public_key())
            .expect_err("identifier receipt signatures must reject unrelated resolver keys");
    }

    #[test]
    fn identifier_resolution_receipt_rejects_proof_attestation_for_signature_verify() {
        let receipt = IdentifierResolutionReceipt {
            payload: live_identifier_resolution_payload_fixture(),
            attestation: RamLfeReceiptAttestation::Proof(crate::proof::ProofBox::new(
                "halo2/ipa".into(),
                vec![1, 2, 3],
            )),
        };
        let resolver_key = checked_random_keypair();

        receipt
            .verify(resolver_key.public_key())
            .expect_err("signature verification must reject proof-only attestations");
    }

    fn live_identifier_resolution_receipt_fixture() -> IdentifierResolutionReceipt {
        IdentifierResolutionReceipt {
            payload: live_identifier_resolution_payload_fixture(),
            attestation: RamLfeReceiptAttestation::Signed(
                Signature::from_hex(
                    "4B26BF33F721C551C13F102D4D7F483CB8DD8A13FD6BF4ED26C845E2B69D5D0124B8CFA05493772F6748A42408EEE4542C470B284AB87F686B423F9DF87C8D00",
                )
                .expect("valid signature"),
            ),
        }
    }

    fn live_identifier_resolution_payload_fixture() -> IdentifierResolutionReceiptPayload {
        IdentifierResolutionReceiptPayload {
            policy_id: IdentifierPolicyId::from_str("email#retail").expect("valid policy"),
            execution: live_identifier_execution_fixture(),
            opening: live_identifier_output_opening_fixture(),
            opaque_id: OpaqueAccountId::from_str(
                "opaque:fd14cb369e853352d4b9c578745627d154471ce5fd3462c4db542c104766e983",
            )
            .expect("valid opaque id"),
            receipt_hash: hash_hex(
                "51bbe55b70e09d4c2bb75d9c31b2cde46a7bdd5414134f6786255c679a68ac53",
            ),
            uaid: UniversalAccountId::from_str(
                "uaid:471b620a99c608af1c7a47199f27b3368ae0ea889a497dd774b52a8287a58393",
            )
            .expect("valid uaid"),
            account_id: AccountId::parse_encoded(
                "sorauﾛ1NiGｸﾛﾋRuﾎQtﾐpヱﾈｻHﾍﾐ3RZﾕYdvbｺhcｽG8A8ｿRﾗeP1E463",
            )
            .expect("valid i105 account")
            .account_id()
            .clone(),
        }
    }

    fn live_identifier_execution_fixture() -> RamLfeExecutionReceiptPayload {
        RamLfeExecutionReceiptPayload {
            program_id: RamLfeProgramId::from_str("email_retail").expect("valid program id"),
            program_digest: hash_hex(
                "fe36ceb3996d101200b895fd2a377cce4426426a473da9fe08b2dbd2bd8b9375",
            ),
            backend: RamLfeBackend::BfvProgrammedSha3_256V1,
            verification_mode: RamLfeVerificationMode::Signed,
            input_ciphertext_hash: fixture_input_ciphertext_hash(),
            output_ciphertext_hash: fixture_output_ciphertext_hash(),
            parameter_digest: fixture_parameter_digest(),
            evaluation_key_digest: fixture_evaluation_key_digest(),
            output_hash: hash_hex(
                "72dcdee1435552e943d5e2e1c978d3f728c6a1ce7e6870b50c63568d4876eea5",
            ),
            associated_data_hash: hash_hex(
                "35b8bc8a30685e7cc5679b6e6a45675539548f5a24326bbee1d8c20e55918f55",
            ),
            executed_at_ms: 1_776_812_470_694,
            expires_at_ms: Some(1_776_812_500_694),
        }
    }

    fn live_identifier_output_opening_fixture() -> crate::ram_lfe::RamLfeOutputOpening {
        let payload = crate::ram_lfe::RamLfeOutputOpeningPayload {
            program_id: RamLfeProgramId::from_str("email_retail").expect("valid program id"),
            input_ciphertext_hash: fixture_input_ciphertext_hash(),
            output_ciphertext_hash: fixture_output_ciphertext_hash(),
            parameter_digest: fixture_parameter_digest(),
            evaluation_key_digest: fixture_evaluation_key_digest(),
            opened_output_hash: hash_hex(
                "5555555555555555555555555555555555555555555555555555555555555555",
            ),
            opened_at_ms: 1_776_812_470_695,
            expires_at_ms: Some(1_776_812_500_694),
        };
        let signer = checked_seed_keypair(0x51);
        let signature = checked_signature(&signer, &payload);
        signature
            .verify(signer.public_key(), &payload)
            .expect("checked live identifier output-opening fixture signature verifies");
        crate::ram_lfe::RamLfeOutputOpening {
            signature: signature.into(),
            payload,
        }
    }

    fn fixture_input_ciphertext_hash() -> Hash {
        hash_hex("1111111111111111111111111111111111111111111111111111111111111111")
    }

    fn fixture_output_ciphertext_hash() -> Hash {
        hash_hex("2222222222222222222222222222222222222222222222222222222222222223")
    }

    fn fixture_parameter_digest() -> Hash {
        hash_hex("3333333333333333333333333333333333333333333333333333333333333333")
    }

    fn fixture_evaluation_key_digest() -> Hash {
        hash_hex("4444444444444444444444444444444444444444444444444444444444444445")
    }

    fn hash_hex(value: &str) -> Hash {
        Hash::from_str(value).expect("valid hash")
    }

    fn shared_identifier_receipt_fixture() -> norito::json::Value {
        let fixture_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/soracloud/identifier_receipt_vectors_v1.json");
        let fixture = std::fs::read_to_string(&fixture_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", fixture_path.display()));
        norito::json::from_str(&fixture)
            .unwrap_or_else(|err| panic!("failed to parse {}: {err}", fixture_path.display()))
    }

    fn fixture_get<'a>(value: &'a norito::json::Value, field: &str) -> &'a norito::json::Value {
        value
            .get(field)
            .unwrap_or_else(|| panic!("fixture field `{field}` is missing"))
    }

    fn fixture_object<'a>(value: &'a norito::json::Value, field: &str) -> &'a norito::json::Value {
        let item = fixture_get(value, field);
        item.as_object()
            .unwrap_or_else(|| panic!("fixture field `{field}` must be an object"));
        item
    }

    fn fixture_array<'a>(value: &'a norito::json::Value, field: &str) -> &'a [norito::json::Value] {
        fixture_get(value, field)
            .as_array()
            .unwrap_or_else(|| panic!("fixture field `{field}` must be an array"))
    }

    fn fixture_str<'a>(value: &'a norito::json::Value, field: &str) -> &'a str {
        fixture_get(value, field)
            .as_str()
            .unwrap_or_else(|| panic!("fixture field `{field}` must be a string"))
    }

    fn fixture_u64(value: &norito::json::Value, field: &str) -> u64 {
        fixture_get(value, field)
            .as_u64()
            .unwrap_or_else(|| panic!("fixture field `{field}` must be an unsigned integer"))
    }

    fn fixture_optional_u64(value: &norito::json::Value, field: &str) -> Option<u64> {
        fixture_get(value, field).as_u64()
    }

    fn receipt_from_fixture(receipt: &norito::json::Value) -> IdentifierResolutionReceipt {
        IdentifierResolutionReceipt {
            payload: payload_from_fixture(fixture_object(receipt, "payload")),
            attestation: attestation_from_fixture(fixture_object(receipt, "attestation")),
        }
    }

    fn payload_from_fixture(payload: &norito::json::Value) -> IdentifierResolutionReceiptPayload {
        let opening = fixture_object(payload, "opening");
        IdentifierResolutionReceiptPayload {
            policy_id: IdentifierPolicyId::from_str(fixture_str(payload, "policy_id"))
                .expect("valid policy id"),
            execution: execution_from_fixture(fixture_object(payload, "execution")),
            opening: RamLfeOutputOpening {
                payload: opening_payload_from_fixture(fixture_object(opening, "payload")),
                signature: Signature::from_hex(fixture_str(opening, "signature"))
                    .expect("valid opening signature hex"),
            },
            opaque_id: OpaqueAccountId::from_str(fixture_str(payload, "opaque_id"))
                .expect("valid opaque id"),
            receipt_hash: hash_hex(fixture_str(payload, "receipt_hash")),
            uaid: UniversalAccountId::from_str(fixture_str(payload, "uaid")).expect("valid uaid"),
            account_id: AccountId::parse_encoded(fixture_str(payload, "account_id"))
                .expect("valid account id")
                .into_account_id(),
        }
    }

    fn execution_from_fixture(execution: &norito::json::Value) -> RamLfeExecutionReceiptPayload {
        RamLfeExecutionReceiptPayload {
            program_id: RamLfeProgramId::from_str(fixture_str(execution, "program_id"))
                .expect("valid program id"),
            program_digest: hash_hex(fixture_str(execution, "program_digest")),
            backend: ram_lfe_backend(fixture_str(execution, "backend")),
            verification_mode: verification_mode(fixture_str(execution, "verification_mode")),
            input_ciphertext_hash: hash_hex(fixture_str(execution, "input_ciphertext_hash")),
            output_ciphertext_hash: hash_hex(fixture_str(execution, "output_ciphertext_hash")),
            parameter_digest: hash_hex(fixture_str(execution, "parameter_digest")),
            evaluation_key_digest: hash_hex(fixture_str(execution, "evaluation_key_digest")),
            output_hash: hash_hex(fixture_str(execution, "output_hash")),
            associated_data_hash: hash_hex(fixture_str(execution, "associated_data_hash")),
            executed_at_ms: fixture_u64(execution, "executed_at_ms"),
            expires_at_ms: fixture_optional_u64(execution, "expires_at_ms"),
        }
    }

    fn opening_payload_from_fixture(
        payload: &norito::json::Value,
    ) -> crate::ram_lfe::RamLfeOutputOpeningPayload {
        crate::ram_lfe::RamLfeOutputOpeningPayload {
            program_id: RamLfeProgramId::from_str(fixture_str(payload, "program_id"))
                .expect("valid program id"),
            input_ciphertext_hash: hash_hex(fixture_str(payload, "input_ciphertext_hash")),
            output_ciphertext_hash: hash_hex(fixture_str(payload, "output_ciphertext_hash")),
            parameter_digest: hash_hex(fixture_str(payload, "parameter_digest")),
            evaluation_key_digest: hash_hex(fixture_str(payload, "evaluation_key_digest")),
            opened_output_hash: hash_hex(fixture_str(payload, "opened_output_hash")),
            opened_at_ms: fixture_u64(payload, "opened_at_ms"),
            expires_at_ms: fixture_optional_u64(payload, "expires_at_ms"),
        }
    }

    fn attestation_from_fixture(attestation: &norito::json::Value) -> RamLfeReceiptAttestation {
        match fixture_str(attestation, "kind") {
            "signed" => RamLfeReceiptAttestation::Signed(
                Signature::from_hex(fixture_str(attestation, "signature"))
                    .expect("valid receipt signature hex"),
            ),
            "proof" => RamLfeReceiptAttestation::Proof(crate::proof::ProofBox::new(
                fixture_str(attestation, "proof_backend").into(),
                BASE64_STANDARD
                    .decode(fixture_str(attestation, "proof_b64"))
                    .expect("valid proof base64"),
            )),
            other => panic!("unsupported attestation kind `{other}`"),
        }
    }

    fn ram_lfe_backend(raw: &str) -> RamLfeBackend {
        match raw {
            "hkdf-sha3-512-prf-v1" => RamLfeBackend::HkdfSha3_512PrfV1,
            "bfv-affine-sha3-256-v1" => RamLfeBackend::BfvAffineSha3_256V1,
            "bfv-programmed-sha3-256-v1" => RamLfeBackend::BfvProgrammedSha3_256V1,
            other => panic!("unsupported RAM-LFE backend `{other}`"),
        }
    }

    fn verification_mode(raw: &str) -> RamLfeVerificationMode {
        match raw {
            "signed" => RamLfeVerificationMode::Signed,
            "proof" => RamLfeVerificationMode::Proof,
            other => panic!("unsupported verification mode `{other}`"),
        }
    }

    fn public_key_literal(raw: &str) -> PublicKey {
        assert_eq!(
            raw.trim(),
            raw,
            "public key literal must not contain surrounding whitespace"
        );
        let literal = raw.strip_prefix("ed25519:").unwrap_or(raw);
        PublicKey::from_str(literal).expect("valid public key literal")
    }

    fn sha256_hex(bytes: &[u8]) -> String {
        hex::encode_upper(Sha256::digest(bytes))
    }
}
