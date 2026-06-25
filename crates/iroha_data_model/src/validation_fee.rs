//! Validation-fee policy data shared by validators and clients.

use std::collections::{BTreeMap, BTreeSet};

use iroha_crypto::Hash;
use iroha_crypto::{PublicKey, SignatureOf};
use iroha_primitives::{json::Json, numeric::Numeric};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    account::AccountId,
    asset::AssetDefinitionId,
    parameter::{CustomParameter, CustomParameterId},
};

/// Schema version for the initial validation-fee policy.
pub const VALIDATION_FEE_POLICY_SCHEMA_VERSION: u16 = 1;
/// Decimal scale required for the initial SBD validation-fee policy.
pub const VALIDATION_FEE_SBD_SCALE: u8 = 2;
/// Fee amount required by the initial SBD validation-fee policy, in minor units.
pub const VALIDATION_FEE_INITIAL_MINOR_UNITS: u64 = 10;
/// Domain separator for policy hashing.
pub const VALIDATION_FEE_POLICY_HASH_DOMAIN: &[u8] = b"iroha.validation_fee.policy.v1";
/// Domain separator carried in signed validation-fee policy payloads.
pub const VALIDATION_FEE_POLICY_SIGNATURE_DOMAIN: &str = "iroha.validation_fee.policy.signature.v1";
/// Transaction metadata key that binds a signed transaction to a policy version.
pub const VALIDATION_FEE_POLICY_VERSION_METADATA_KEY: &str = "validation_fee_policy_version";
/// Transaction metadata key that binds a signed transaction to a policy hash.
pub const VALIDATION_FEE_POLICY_HASH_METADATA_KEY: &str = "validation_fee_policy_hash";
/// Transaction metadata key that identifies the aggregate validation-fee instruction.
pub const VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY: &str = "validation_fee_instruction_index";
/// Transaction metadata key that identifies the aggregate validation-fee batch entry, when used.
pub const VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY: &str =
    "validation_fee_transfer_entry_index";

/// Error returned when signed validation-fee policy verification fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationFeePolicySignatureError {
    /// The referenced governance keyset is malformed.
    InvalidGovernanceKeyset(&'static str),
    /// The signed policy references a different keyset id.
    GovernanceKeysetMismatch {
        /// Keyset id expected from the policy.
        expected: String,
        /// Keyset id found in the supplied keyset parameter.
        found: String,
    },
    /// No signatures were supplied.
    NoSignatures,
    /// A signature was produced by a key outside the active keyset.
    UnknownSigner,
    /// A signer appeared more than once.
    DuplicateSigner,
    /// A signature failed cryptographic verification.
    InvalidSignature,
    /// Valid signatures did not meet the active keyset threshold.
    InsufficientThreshold {
        /// Weight collected from valid signatures.
        collected: u32,
        /// Weight required by the governance keyset.
        required: u32,
    },
}

impl core::fmt::Display for ValidationFeePolicySignatureError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidGovernanceKeyset(reason) => {
                write!(f, "validation-fee governance keyset is invalid: {reason}")
            }
            Self::GovernanceKeysetMismatch { expected, found } => write!(
                f,
                "validation-fee governance keyset mismatch: expected {expected}, found {found}"
            ),
            Self::NoSignatures => write!(f, "validation-fee policy has no signatures"),
            Self::UnknownSigner => write!(
                f,
                "validation-fee policy signature was produced by an unknown signer"
            ),
            Self::DuplicateSigner => write!(
                f,
                "validation-fee policy signature set contains a duplicate signer"
            ),
            Self::InvalidSignature => write!(f, "validation-fee policy signature is invalid"),
            Self::InsufficientThreshold {
                collected,
                required,
            } => write!(
                f,
                "validation-fee policy signatures do not meet threshold: collected {collected}, required {required}"
            ),
        }
    }
}

impl std::error::Error for ValidationFeePolicySignatureError {}

/// Error returned when a validation-fee policy registry is malformed or
/// does not match the active signed policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationFeePolicyRegistryError {
    /// No registered policy entries were supplied.
    EmptyRegistry,
    /// A policy entry did not continue the monotonic version chain.
    UnexpectedPolicyVersion {
        /// Version expected at this position in the chain.
        expected: u64,
        /// Version found in the registry entry.
        found: u64,
    },
    /// The same policy hash appeared more than once.
    DuplicatePolicyHash {
        /// Version of the duplicate policy entry.
        policy_version: u64,
    },
    /// A registry entry does not point at the immediately previous policy hash.
    BrokenPreviousPolicyHash {
        /// Version of the broken policy entry.
        policy_version: u64,
    },
    /// The registry active version does not match the latest registered entry
    /// or the supplied active signed policy.
    ActiveVersionMismatch {
        /// Version expected by the registry or policy.
        expected: u64,
        /// Version found in the registry or policy.
        found: u64,
    },
    /// The registry active hash does not match the latest registered entry or
    /// the supplied active signed policy.
    ActiveHashMismatch,
    /// The active policy carries a different previous hash than its registry
    /// entry.
    ActivePreviousHashMismatch,
    /// The active policy hash could not be computed.
    PolicyHashEncoding,
}

impl core::fmt::Display for ValidationFeePolicyRegistryError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyRegistry => write!(f, "validation-fee policy registry is empty"),
            Self::UnexpectedPolicyVersion { expected, found } => write!(
                f,
                "validation-fee policy registry version chain is not monotonic: expected {expected}, found {found}"
            ),
            Self::DuplicatePolicyHash { policy_version } => write!(
                f,
                "validation-fee policy registry contains duplicate hash at version {policy_version}"
            ),
            Self::BrokenPreviousPolicyHash { policy_version } => write!(
                f,
                "validation-fee policy registry previous hash is broken at version {policy_version}"
            ),
            Self::ActiveVersionMismatch { expected, found } => write!(
                f,
                "validation-fee policy registry active version mismatch: expected {expected}, found {found}"
            ),
            Self::ActiveHashMismatch => {
                write!(f, "validation-fee policy registry active hash mismatch")
            }
            Self::ActivePreviousHashMismatch => write!(
                f,
                "validation-fee policy registry active previous hash mismatch"
            ),
            Self::PolicyHashEncoding => {
                write!(f, "validation-fee active policy hash could not be encoded")
            }
        }
    }
}

impl std::error::Error for ValidationFeePolicyRegistryError {}

/// Validation-fee charging mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(
    tag = "charging_mode",
    content = "value",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum ValidationFeeChargingMode {
    /// Charge once per qualifying fee-asset transfer instruction or batch entry.
    PerQualifyingTransferInstruction,
}

/// One public key and its threshold weight in a validation-fee governance keyset.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidationFeeGovernanceKeyV1 {
    /// Governance signer public key.
    pub public_key: PublicKey,
    /// Weight contributed by this signer.
    pub weight: u16,
}

/// Active governance keyset used to verify signed validation-fee policies.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidationFeeGovernanceKeysetV1 {
    /// Stable keyset identifier referenced by policies.
    pub keyset_id: String,
    /// Required aggregate signer weight.
    pub threshold: u16,
    /// Signer keys and weights.
    pub keys: Vec<ValidationFeeGovernanceKeyV1>,
}

impl ValidationFeeGovernanceKeysetV1 {
    /// Identifier of the chain-level custom parameter carrying the active keyset.
    pub const PARAMETER_ID_STR: &'static str = "iroha:validation_fee_governance_keyset_v1";

    /// Construct the custom-parameter identifier for this keyset.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        Self::PARAMETER_ID_STR
            .parse()
            .expect("valid validation-fee governance keyset parameter identifier")
    }

    /// Convert this keyset into an on-ledger custom parameter.
    #[must_use]
    pub fn into_custom_parameter(self) -> CustomParameter {
        CustomParameter::new(Self::parameter_id(), Json::new(self))
    }

    /// Decode a validation-fee keyset from a custom parameter.
    #[must_use]
    pub fn from_custom_parameter(custom: &CustomParameter) -> Option<Self> {
        if custom.id() != &Self::parameter_id() {
            return None;
        }
        custom.payload().try_into_any_norito::<Self>().ok()
    }

    /// Return governance keyset invariant violations, if any.
    #[must_use]
    pub fn invariant_error(&self) -> Option<&'static str> {
        if self.keyset_id.trim().is_empty() {
            return Some("validation-fee governance keyset id must be non-empty");
        }
        if self.threshold == 0 {
            return Some("validation-fee governance keyset threshold must be positive");
        }
        if self.keys.is_empty() {
            return Some("validation-fee governance keyset must contain at least one key");
        }

        let mut seen = BTreeSet::new();
        let mut total_weight: u32 = 0;
        for key in &self.keys {
            if key.weight == 0 {
                return Some("validation-fee governance key weight must be positive");
            }
            if !seen.insert(key.public_key.clone()) {
                return Some("validation-fee governance keyset contains duplicate public keys");
            }
            total_weight = total_weight.saturating_add(u32::from(key.weight));
        }
        if total_weight < u32::from(self.threshold) {
            return Some("validation-fee governance keyset threshold exceeds total key weight");
        }
        None
    }

    fn key_weights(&self) -> BTreeMap<PublicKey, u32> {
        self.keys
            .iter()
            .map(|key| (key.public_key.clone(), u32::from(key.weight)))
            .collect()
    }
}

/// One entry in the registered validation-fee policy hash chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidationFeePolicyRegistryEntryV1 {
    /// Monotonic policy version.
    pub policy_version: u64,
    /// Domain-separated policy hash.
    pub policy_hash: [u8; 32],
    /// Previous policy hash for chain validation.
    #[norito(default)]
    pub previous_policy_hash: Option<[u8; 32]>,
}

impl ValidationFeePolicyRegistryEntryV1 {
    /// Build a registry entry from a policy.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the policy cannot be hashed.
    pub fn from_policy(policy: &ValidationFeePolicyV1) -> Result<Self, norito::Error> {
        Ok(Self {
            policy_version: policy.policy_version,
            policy_hash: policy.policy_hash()?,
            previous_policy_hash: policy.previous_policy_hash,
        })
    }
}

/// On-ledger validation-fee policy registry used to reject rollback and
/// skipped-version policy changes.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidationFeePolicyRegistryV1 {
    /// Hash of the active signed validation-fee policy.
    pub active_policy_hash: [u8; 32],
    /// Version of the active signed validation-fee policy.
    pub active_policy_version: u64,
    /// Registered policy hash chain through the active policy.
    pub registered_policies: Vec<ValidationFeePolicyRegistryEntryV1>,
}

impl ValidationFeePolicyRegistryV1 {
    /// Identifier of the chain-level custom parameter carrying the policy registry.
    pub const PARAMETER_ID_STR: &'static str = "iroha:validation_fee_policy_registry_v1";

    /// Construct the custom-parameter identifier for this registry.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        Self::PARAMETER_ID_STR
            .parse()
            .expect("valid validation-fee policy registry parameter identifier")
    }

    /// Convert this registry into an on-ledger custom parameter.
    #[must_use]
    pub fn into_custom_parameter(self) -> CustomParameter {
        CustomParameter::new(Self::parameter_id(), Json::new(self))
    }

    /// Decode a validation-fee policy registry from a custom parameter.
    #[must_use]
    pub fn from_custom_parameter(custom: &CustomParameter) -> Option<Self> {
        if custom.id() != &Self::parameter_id() {
            return None;
        }
        custom.payload().try_into_any_norito::<Self>().ok()
    }

    /// Validate that the registry is a contiguous chain ending in `policy`.
    ///
    /// # Errors
    ///
    /// Returns an error when the registry is empty, non-monotonic, broken, or
    /// does not identify `policy` as the active policy.
    pub fn validate_active_policy(
        &self,
        policy: &ValidationFeePolicyV1,
    ) -> Result<(), ValidationFeePolicyRegistryError> {
        let mut entries = self.registered_policies.iter();
        let Some(first) = entries.next() else {
            return Err(ValidationFeePolicyRegistryError::EmptyRegistry);
        };
        if first.policy_version != 1 {
            return Err(ValidationFeePolicyRegistryError::UnexpectedPolicyVersion {
                expected: 1,
                found: first.policy_version,
            });
        }
        if first.previous_policy_hash.is_some() {
            return Err(ValidationFeePolicyRegistryError::BrokenPreviousPolicyHash {
                policy_version: first.policy_version,
            });
        }

        let mut seen_hashes = BTreeSet::from([first.policy_hash]);
        let mut expected_version = 2u64;
        let mut previous_hash = first.policy_hash;
        let mut latest = first;

        for entry in entries {
            if entry.policy_version != expected_version {
                return Err(ValidationFeePolicyRegistryError::UnexpectedPolicyVersion {
                    expected: expected_version,
                    found: entry.policy_version,
                });
            }
            if !seen_hashes.insert(entry.policy_hash) {
                return Err(ValidationFeePolicyRegistryError::DuplicatePolicyHash {
                    policy_version: entry.policy_version,
                });
            }
            if entry.previous_policy_hash != Some(previous_hash) {
                return Err(ValidationFeePolicyRegistryError::BrokenPreviousPolicyHash {
                    policy_version: entry.policy_version,
                });
            }
            expected_version = expected_version.saturating_add(1);
            previous_hash = entry.policy_hash;
            latest = entry;
        }

        if self.active_policy_version != latest.policy_version {
            return Err(ValidationFeePolicyRegistryError::ActiveVersionMismatch {
                expected: latest.policy_version,
                found: self.active_policy_version,
            });
        }
        if self.active_policy_hash != latest.policy_hash {
            return Err(ValidationFeePolicyRegistryError::ActiveHashMismatch);
        }
        if self.active_policy_version != policy.policy_version {
            return Err(ValidationFeePolicyRegistryError::ActiveVersionMismatch {
                expected: self.active_policy_version,
                found: policy.policy_version,
            });
        }
        let policy_hash = policy
            .policy_hash()
            .map_err(|_| ValidationFeePolicyRegistryError::PolicyHashEncoding)?;
        if self.active_policy_hash != policy_hash {
            return Err(ValidationFeePolicyRegistryError::ActiveHashMismatch);
        }
        if latest.previous_policy_hash != policy.previous_policy_hash {
            return Err(ValidationFeePolicyRegistryError::ActivePreviousHashMismatch);
        }

        Ok(())
    }
}

/// Chain-level validation-fee policy.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidationFeePolicyV1 {
    /// Policy schema version.
    pub schema_version: u16,
    /// Network identifier bound into the policy.
    pub network_id: String,
    /// Genesis hash bound into the policy.
    pub genesis_hash: [u8; 32],
    /// Monotonic policy version.
    pub policy_version: u64,
    /// Previous policy hash for policy-chain validation.
    #[norito(default)]
    pub previous_policy_hash: Option<[u8; 32]>,
    /// Asset definition charged by this policy.
    pub fee_asset_definition_id: AssetDefinitionId,
    /// Decimal scale used to interpret fee asset minor units.
    pub fee_asset_scale: u8,
    /// Fee amount in fee asset minor units.
    pub fee_minor_units: u64,
    /// Concrete validator treasury account.
    pub treasury_account_id: AccountId,
    /// Charging mode.
    pub charging_mode: ValidationFeeChargingMode,
    /// First height at which the policy is active.
    pub effective_from_height: u64,
    /// Optional last active height.
    #[norito(default)]
    pub expires_after_height: Option<u64>,
    /// Governance keyset identifier that authorized registration.
    pub governance_keyset_id: String,
    /// Explicit exemption classes recognized by this policy.
    #[norito(default)]
    pub exemption_classes: Vec<String>,
}

impl ValidationFeePolicyV1 {
    /// Identifier of the chain-level custom parameter carrying the active validation-fee policy.
    pub const PARAMETER_ID_STR: &'static str = "iroha:validation_fee_policy_v1";

    /// Construct the custom-parameter identifier for this policy.
    #[must_use]
    pub fn parameter_id() -> CustomParameterId {
        Self::PARAMETER_ID_STR
            .parse()
            .expect("valid validation-fee policy parameter identifier")
    }

    /// Convert this policy into an on-ledger custom parameter.
    #[must_use]
    pub fn into_custom_parameter(self) -> CustomParameter {
        CustomParameter::new(Self::parameter_id(), Json::new(self))
    }

    /// Decode a validation-fee policy from a custom parameter.
    #[must_use]
    pub fn from_custom_parameter(custom: &CustomParameter) -> Option<Self> {
        if custom.id() != &Self::parameter_id() {
            return None;
        }
        custom.payload().try_into_any_norito::<Self>().ok()
    }

    /// Return true when this policy is active at the provided height.
    #[must_use]
    pub fn is_active_at_height(&self, height: u64) -> bool {
        height >= self.effective_from_height
            && self
                .expires_after_height
                .is_none_or(|expires_after_height| height <= expires_after_height)
    }

    /// Deterministic domain-separated policy hash.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the policy cannot be serialized.
    pub fn policy_hash(&self) -> Result<[u8; 32], norito::Error> {
        let bytes = norito::to_bytes(self)?;
        let mut payload =
            Vec::with_capacity(VALIDATION_FEE_POLICY_HASH_DOMAIN.len() + 1 + bytes.len());
        payload.extend_from_slice(VALIDATION_FEE_POLICY_HASH_DOMAIN);
        payload.push(0);
        payload.extend_from_slice(&bytes);
        Ok(*Hash::new(payload.as_slice()).as_ref())
    }

    /// Return policy invariant violations, if any.
    #[must_use]
    pub fn policy_invariant_error(&self) -> Option<&'static str> {
        if self.schema_version != VALIDATION_FEE_POLICY_SCHEMA_VERSION {
            return Some("unsupported validation-fee policy schema version");
        }
        if self.policy_version == 0 {
            return Some("validation-fee policy version must be positive");
        }
        if self.policy_version == 1 && self.previous_policy_hash.is_some() {
            return Some("initial validation-fee policy must not carry a previous policy hash");
        }
        if self.policy_version > 1 && self.previous_policy_hash.is_none() {
            return Some("non-initial validation-fee policy must carry a previous policy hash");
        }
        if self.fee_asset_scale != VALIDATION_FEE_SBD_SCALE {
            return Some("validation-fee policy asset scale must be 2");
        }
        if self.fee_minor_units != VALIDATION_FEE_INITIAL_MINOR_UNITS {
            return Some("validation-fee policy amount must be 10 minor units");
        }
        if self.governance_keyset_id.trim().is_empty() {
            return Some("validation-fee policy governance keyset id must be non-empty");
        }
        if !matches!(
            self.charging_mode,
            ValidationFeeChargingMode::PerQualifyingTransferInstruction
        ) {
            return Some("unsupported validation-fee charging mode");
        }
        if self
            .expires_after_height
            .is_some_and(|expires_after_height| expires_after_height < self.effective_from_height)
        {
            return Some("validation-fee policy validity window is invalid");
        }
        None
    }

    /// Return initial policy invariant violations, if any.
    #[must_use]
    pub fn initial_policy_invariant_error(&self) -> Option<&'static str> {
        self.policy_invariant_error()
    }

    /// Fee amount as a ledger [`Numeric`].
    #[must_use]
    pub fn fee_amount_numeric(&self) -> Numeric {
        Numeric::new(self.fee_minor_units, u32::from(self.fee_asset_scale))
    }

    /// Domain-separated payload that governance keys sign.
    #[must_use]
    pub fn signing_payload(&self) -> ValidationFeePolicySigningPayloadV1 {
        ValidationFeePolicySigningPayloadV1 {
            domain: VALIDATION_FEE_POLICY_SIGNATURE_DOMAIN.to_string(),
            policy: self.clone(),
        }
    }
}

/// Domain-separated payload signed by governance keys.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidationFeePolicySigningPayloadV1 {
    /// Domain separator preventing cross-protocol replay.
    pub domain: String,
    /// Policy being authorized.
    pub policy: ValidationFeePolicyV1,
}

/// One governance signature over a validation-fee policy.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidationFeePolicySignatureV1 {
    /// Governance signer public key.
    pub public_key: PublicKey,
    /// Signature over the domain-separated policy payload.
    pub signature: SignatureOf<ValidationFeePolicySigningPayloadV1>,
}

/// Signed validation-fee policy registered on-ledger.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct SignedValidationFeePolicyV1 {
    /// Policy payload.
    pub policy: ValidationFeePolicyV1,
    /// Governance signatures over the policy payload.
    pub signatures: Vec<ValidationFeePolicySignatureV1>,
}

impl SignedValidationFeePolicyV1 {
    /// Convert this signed policy into the active on-ledger custom parameter.
    #[must_use]
    pub fn into_custom_parameter(self) -> CustomParameter {
        CustomParameter::new(ValidationFeePolicyV1::parameter_id(), Json::new(self))
    }

    /// Decode a signed validation-fee policy from a custom parameter.
    #[must_use]
    pub fn from_custom_parameter(custom: &CustomParameter) -> Option<Self> {
        if custom.id() != &ValidationFeePolicyV1::parameter_id() {
            return None;
        }
        custom.payload().try_into_any_norito::<Self>().ok()
    }

    /// Verify signatures against the supplied active governance keyset.
    ///
    /// # Errors
    ///
    /// Returns an error when the keyset is malformed, mismatched, or the
    /// signature bundle does not satisfy threshold.
    pub fn verify_against_keyset(
        &self,
        keyset: &ValidationFeeGovernanceKeysetV1,
    ) -> Result<(), ValidationFeePolicySignatureError> {
        if let Some(reason) = keyset.invariant_error() {
            return Err(ValidationFeePolicySignatureError::InvalidGovernanceKeyset(
                reason,
            ));
        }
        if self.policy.governance_keyset_id != keyset.keyset_id {
            return Err(
                ValidationFeePolicySignatureError::GovernanceKeysetMismatch {
                    expected: self.policy.governance_keyset_id.clone(),
                    found: keyset.keyset_id.clone(),
                },
            );
        }
        if self.signatures.is_empty() {
            return Err(ValidationFeePolicySignatureError::NoSignatures);
        }

        let payload = self.policy.signing_payload();
        let weights = keyset.key_weights();
        let mut seen = BTreeSet::new();
        let mut collected = 0u32;

        for signature in &self.signatures {
            let Some(weight) = weights.get(&signature.public_key) else {
                return Err(ValidationFeePolicySignatureError::UnknownSigner);
            };
            if !seen.insert(signature.public_key.clone()) {
                return Err(ValidationFeePolicySignatureError::DuplicateSigner);
            }
            signature
                .signature
                .verify(&signature.public_key, &payload)
                .map_err(|_| ValidationFeePolicySignatureError::InvalidSignature)?;
            collected = collected.saturating_add(*weight);
        }

        let required = u32::from(keyset.threshold);
        if collected < required {
            return Err(ValidationFeePolicySignatureError::InsufficientThreshold {
                collected,
                required,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr as _;

    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;
    use crate::{domain::DomainId, name::Name};

    const TEST_VALIDATION_FEE_ASSET_SCALE: u8 = VALIDATION_FEE_SBD_SCALE;
    const TEST_VALIDATION_FEE_MINOR_UNITS: u64 = VALIDATION_FEE_INITIAL_MINOR_UNITS;

    fn account(seed: u8) -> AccountId {
        let key_pair =
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("key pair");
        AccountId::new(key_pair.public_key().clone())
    }

    fn key_pair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("key pair")
    }

    fn fee_asset() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("fees", "paynet").expect("domain id"),
            Name::from_str("fee_token").expect("asset name"),
        )
    }

    fn policy() -> ValidationFeePolicyV1 {
        ValidationFeePolicyV1 {
            schema_version: VALIDATION_FEE_POLICY_SCHEMA_VERSION,
            network_id: "generic-testnet".to_string(),
            genesis_hash: [7; 32],
            policy_version: 1,
            previous_policy_hash: None,
            fee_asset_definition_id: fee_asset(),
            fee_asset_scale: TEST_VALIDATION_FEE_ASSET_SCALE,
            fee_minor_units: TEST_VALIDATION_FEE_MINOR_UNITS,
            treasury_account_id: account(1),
            charging_mode: ValidationFeeChargingMode::PerQualifyingTransferInstruction,
            effective_from_height: 10,
            expires_after_height: Some(100),
            governance_keyset_id: "validation-fee-governance-v1".to_string(),
            exemption_classes: Vec::new(),
        }
    }

    fn keyset(keys: &[&KeyPair], threshold: u16) -> ValidationFeeGovernanceKeysetV1 {
        ValidationFeeGovernanceKeysetV1 {
            keyset_id: "validation-fee-governance-v1".to_string(),
            threshold,
            keys: keys
                .iter()
                .map(|key_pair| ValidationFeeGovernanceKeyV1 {
                    public_key: key_pair.public_key().clone(),
                    weight: 1,
                })
                .collect(),
        }
    }

    fn signature(
        policy: &ValidationFeePolicyV1,
        key_pair: &KeyPair,
    ) -> ValidationFeePolicySignatureV1 {
        ValidationFeePolicySignatureV1 {
            public_key: key_pair.public_key().clone(),
            signature: SignatureOf::try_new(key_pair.private_key(), &policy.signing_payload())
                .expect("policy signature"),
        }
    }

    fn signed_policy(
        policy: ValidationFeePolicyV1,
        key_pairs: &[&KeyPair],
    ) -> SignedValidationFeePolicyV1 {
        SignedValidationFeePolicyV1 {
            signatures: key_pairs
                .iter()
                .map(|key_pair| signature(&policy, key_pair))
                .collect(),
            policy,
        }
    }

    fn successor_policy(previous: &ValidationFeePolicyV1) -> ValidationFeePolicyV1 {
        let mut policy = previous.clone();
        policy.policy_version += 1;
        policy.previous_policy_hash = Some(previous.policy_hash().expect("previous policy hash"));
        policy.effective_from_height += 100;
        policy.expires_after_height = Some(policy.effective_from_height + 100);
        policy
    }

    fn policy_registry(policies: &[ValidationFeePolicyV1]) -> ValidationFeePolicyRegistryV1 {
        let registered_policies = policies
            .iter()
            .map(|policy| {
                ValidationFeePolicyRegistryEntryV1::from_policy(policy).expect("registry entry")
            })
            .collect::<Vec<_>>();
        let active = registered_policies.last().expect("at least one policy");
        ValidationFeePolicyRegistryV1 {
            active_policy_hash: active.policy_hash,
            active_policy_version: active.policy_version,
            registered_policies,
        }
    }

    #[test]
    fn policy_custom_parameter_roundtrips() {
        let expected = policy();
        let custom = expected.clone().into_custom_parameter();

        assert_eq!(custom.id, ValidationFeePolicyV1::parameter_id());
        assert_eq!(
            ValidationFeePolicyV1::from_custom_parameter(&custom),
            Some(expected)
        );
    }

    #[test]
    fn signed_policy_custom_parameter_roundtrips() {
        let key_pair = key_pair(21);
        let expected = signed_policy(policy(), &[&key_pair]);
        let custom = expected.clone().into_custom_parameter();

        assert_eq!(custom.id, ValidationFeePolicyV1::parameter_id());
        assert_eq!(
            SignedValidationFeePolicyV1::from_custom_parameter(&custom),
            Some(expected)
        );
    }

    #[test]
    fn governance_keyset_custom_parameter_roundtrips() {
        let key_pair = key_pair(21);
        let expected = keyset(&[&key_pair], 1);
        let custom = expected.clone().into_custom_parameter();

        assert_eq!(custom.id, ValidationFeeGovernanceKeysetV1::parameter_id());
        assert_eq!(
            ValidationFeeGovernanceKeysetV1::from_custom_parameter(&custom),
            Some(expected)
        );
    }

    #[test]
    fn policy_registry_custom_parameter_roundtrips() {
        let first = policy();
        let second = successor_policy(&first);
        let expected = policy_registry(&[first, second]);
        let custom = expected.clone().into_custom_parameter();

        assert_eq!(custom.id, ValidationFeePolicyRegistryV1::parameter_id());
        assert_eq!(
            ValidationFeePolicyRegistryV1::from_custom_parameter(&custom),
            Some(expected)
        );
    }

    #[test]
    fn policy_registry_validates_active_policy_chain() {
        let first = policy();
        let second = successor_policy(&first);
        let registry = policy_registry(&[first, second.clone()]);

        registry
            .validate_active_policy(&second)
            .expect("contiguous active chain validates");
    }

    #[test]
    fn policy_registry_rejects_skipped_version() {
        let first = policy();
        let second = successor_policy(&first);
        let mut registry = policy_registry(&[first, second.clone()]);
        registry.registered_policies[1].policy_version = 3;

        assert_eq!(
            registry.validate_active_policy(&second),
            Err(ValidationFeePolicyRegistryError::UnexpectedPolicyVersion {
                expected: 2,
                found: 3,
            })
        );
    }

    #[test]
    fn policy_registry_rejects_broken_previous_hash() {
        let first = policy();
        let second = successor_policy(&first);
        let mut registry = policy_registry(&[first, second.clone()]);
        registry.registered_policies[1].previous_policy_hash = Some([9; 32]);

        assert_eq!(
            registry.validate_active_policy(&second),
            Err(ValidationFeePolicyRegistryError::BrokenPreviousPolicyHash { policy_version: 2 })
        );
    }

    #[test]
    fn policy_registry_rejects_rollback_active_policy() {
        let first = policy();
        let second = successor_policy(&first);
        let registry = policy_registry(std::slice::from_ref(&first));

        assert_eq!(
            registry.validate_active_policy(&second),
            Err(ValidationFeePolicyRegistryError::ActiveVersionMismatch {
                expected: 1,
                found: 2,
            })
        );
    }

    #[test]
    fn signed_policy_verifies_against_threshold_keyset() {
        let first = key_pair(21);
        let second = key_pair(22);
        let third = key_pair(23);
        let keyset = keyset(&[&first, &second, &third], 2);
        let signed = signed_policy(policy(), &[&first, &second]);

        signed
            .verify_against_keyset(&keyset)
            .expect("threshold signatures verify");
    }

    #[test]
    fn signed_policy_rejects_invalid_signature() {
        let first = key_pair(21);
        let second = key_pair(22);
        let wrong = key_pair(23);
        let keyset = keyset(&[&first, &second], 2);
        let policy = policy();
        let mut signed = signed_policy(policy.clone(), &[&first]);
        signed.signatures.push(ValidationFeePolicySignatureV1 {
            public_key: second.public_key().clone(),
            signature: SignatureOf::try_new(wrong.private_key(), &policy.signing_payload())
                .expect("wrong signature"),
        });

        assert_eq!(
            signed.verify_against_keyset(&keyset),
            Err(ValidationFeePolicySignatureError::InvalidSignature)
        );
    }

    #[test]
    fn signed_policy_rejects_insufficient_threshold() {
        let first = key_pair(21);
        let second = key_pair(22);
        let keyset = keyset(&[&first, &second], 2);
        let signed = signed_policy(policy(), &[&first]);

        assert_eq!(
            signed.verify_against_keyset(&keyset),
            Err(ValidationFeePolicySignatureError::InsufficientThreshold {
                collected: 1,
                required: 2,
            })
        );
    }

    #[test]
    fn signed_policy_rejects_wrong_keyset() {
        let first = key_pair(21);
        let mut keyset = keyset(&[&first], 1);
        keyset.keyset_id = "other-governance".to_string();
        let signed = signed_policy(policy(), &[&first]);

        assert_eq!(
            signed.verify_against_keyset(&keyset),
            Err(
                ValidationFeePolicySignatureError::GovernanceKeysetMismatch {
                    expected: "validation-fee-governance-v1".to_string(),
                    found: "other-governance".to_string(),
                }
            )
        );
    }

    #[test]
    fn signed_policy_rejects_unknown_and_duplicate_signers() {
        let first = key_pair(21);
        let unknown = key_pair(22);
        let keyset = keyset(&[&first], 1);
        let policy = policy();
        let unknown_signed = signed_policy(policy.clone(), &[&unknown]);
        assert_eq!(
            unknown_signed.verify_against_keyset(&keyset),
            Err(ValidationFeePolicySignatureError::UnknownSigner)
        );

        let duplicate_signed = signed_policy(policy, &[&first, &first]);
        assert_eq!(
            duplicate_signed.verify_against_keyset(&keyset),
            Err(ValidationFeePolicySignatureError::DuplicateSigner)
        );
    }

    #[test]
    fn policy_hash_is_domain_separated_from_raw_norito() {
        let policy = policy();
        let policy_hash = policy.policy_hash().expect("policy hash");
        let raw_hash = Hash::new(norito::to_bytes(&policy).expect("policy bytes").as_slice());

        assert_ne!(policy_hash.as_slice(), raw_hash.as_ref());
    }

    #[test]
    fn active_height_observes_effective_and_expiry_bounds() {
        let policy = policy();

        assert!(!policy.is_active_at_height(9));
        assert!(policy.is_active_at_height(10));
        assert!(policy.is_active_at_height(100));
        assert!(!policy.is_active_at_height(101));
    }

    #[test]
    fn policy_invariants_reject_wrong_fee_amount() {
        let mut policy = policy();
        policy.fee_minor_units = VALIDATION_FEE_INITIAL_MINOR_UNITS + 1;

        assert_eq!(
            policy.policy_invariant_error(),
            Some("validation-fee policy amount must be 10 minor units")
        );
    }

    #[test]
    fn policy_invariants_reject_wrong_fee_scale() {
        let mut policy = policy();
        policy.fee_asset_scale = VALIDATION_FEE_SBD_SCALE + 1;

        assert_eq!(
            policy.policy_invariant_error(),
            Some("validation-fee policy asset scale must be 2")
        );
    }

    #[test]
    fn policy_invariants_reject_zero_version() {
        let mut policy = policy();
        policy.policy_version = 0;

        assert_eq!(
            policy.policy_invariant_error(),
            Some("validation-fee policy version must be positive")
        );
    }

    #[test]
    fn policy_invariants_reject_previous_hash_on_initial_policy() {
        let mut policy = policy();
        policy.previous_policy_hash = Some([9; 32]);

        assert_eq!(
            policy.policy_invariant_error(),
            Some("initial validation-fee policy must not carry a previous policy hash")
        );
    }

    #[test]
    fn policy_invariants_reject_missing_previous_hash_on_successor_policy() {
        let mut policy = policy();
        policy.policy_version = 2;

        assert_eq!(
            policy.policy_invariant_error(),
            Some("non-initial validation-fee policy must carry a previous policy hash")
        );
    }

    #[test]
    fn policy_invariants_reject_empty_governance_keyset() {
        let mut policy = policy();
        policy.governance_keyset_id.clear();

        assert_eq!(
            policy.policy_invariant_error(),
            Some("validation-fee policy governance keyset id must be non-empty")
        );
    }
}
