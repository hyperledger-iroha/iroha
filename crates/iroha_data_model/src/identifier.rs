//! Hidden-function-backed identifier policy and claim types.

use std::{fmt, str::FromStr, string::String, vec::Vec};

use iroha_crypto::{Hash, PublicKey, SignatureOf};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    account::{AccountId, OpaqueAccountId},
    name::Name,
    nexus::UniversalAccountId,
    ram_lfe::{RamLfeExecutionReceiptPayload, RamLfeProgramId, RamLfeReceiptAttestation},
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

    use iroha_crypto::{
        KeyPair, PublicKey, RamLfeBackend, RamLfeVerificationMode, Signature, SignatureOf,
    };

    use super::*;

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
        let account_signatory = KeyPair::random().public_key().clone();
        let payload = IdentifierResolutionReceiptPayload {
            policy_id: IdentifierPolicyId::from_str("email#retail").expect("valid policy"),
            execution: RamLfeExecutionReceiptPayload {
                program_id: RamLfeProgramId::from_str("email_retail").expect("valid program id"),
                program_digest: Hash::new(b"program"),
                backend: RamLfeBackend::BfvProgrammedSha3_256V1,
                verification_mode: RamLfeVerificationMode::Signed,
                output_hash: Hash::new(b"output"),
                associated_data_hash: Hash::new(b"associated-data"),
                executed_at_ms: 1_777_777_777_000,
                expires_at_ms: Some(1_777_777_877_000),
            },
            opaque_id: OpaqueAccountId::from_hash(Hash::new(b"opaque")),
            receipt_hash: Hash::new(b"receipt"),
            uaid: UniversalAccountId::from_hash(Hash::new(b"uaid")),
            account_id: AccountId::new(account_signatory),
        };
        let signer = KeyPair::random();
        let signature = SignatureOf::new(signer.private_key(), &payload);
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
    fn live_identifier_resolution_receipt_signature_fixture_verifies() {
        let payload = IdentifierResolutionReceiptPayload {
            policy_id: IdentifierPolicyId::from_str("email#retail").expect("valid policy"),
            execution: RamLfeExecutionReceiptPayload {
                program_id: RamLfeProgramId::from_str("email_retail").expect("valid program id"),
                program_digest: Hash::from_str(
                    "fe36ceb3996d101200b895fd2a377cce4426426a473da9fe08b2dbd2bd8b9375",
                )
                .expect("valid hash"),
                backend: RamLfeBackend::BfvProgrammedSha3_256V1,
                verification_mode: RamLfeVerificationMode::Signed,
                output_hash: Hash::from_str(
                    "72dcdee1435552e943d5e2e1c978d3f728c6a1ce7e6870b50c63568d4876eea5",
                )
                .expect("valid hash"),
                associated_data_hash: Hash::from_str(
                    "35b8bc8a30685e7cc5679b6e6a45675539548f5a24326bbee1d8c20e55918f55",
                )
                .expect("valid hash"),
                executed_at_ms: 1_776_812_470_694,
                expires_at_ms: Some(1_776_812_500_694),
            },
            opaque_id: OpaqueAccountId::from_str(
                "opaque:fd14cb369e853352d4b9c578745627d154471ce5fd3462c4db542c104766e983",
            )
            .expect("valid opaque id"),
            receipt_hash: Hash::from_str(
                "51bbe55b70e09d4c2bb75d9c31b2cde46a7bdd5414134f6786255c679a68ac53",
            )
            .expect("valid hash"),
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
        };
        let receipt = IdentifierResolutionReceipt {
            payload,
            attestation: RamLfeReceiptAttestation::Signed(
                Signature::from_hex(
                    "4B26BF33F721C551C13F102D4D7F483CB8DD8A13FD6BF4ED26C845E2B69D5D0124B8CFA05493772F6748A42408EEE4542C470B284AB87F686B423F9DF87C8D00",
                )
                .expect("valid signature"),
            ),
        };
        let resolver_key = PublicKey::from_str(
            "ed01200376E59E9078B647F55003896B59758B7BE99908535EC24BAF80A6D52C8B3EB8",
        )
        .expect("valid resolver key");

        println!(
            "live receipt payload hex {}",
            hex::encode_upper(receipt.payload_bytes())
        );
        receipt
            .verify(&resolver_key)
            .expect("live receipt signature should verify");
    }
}
