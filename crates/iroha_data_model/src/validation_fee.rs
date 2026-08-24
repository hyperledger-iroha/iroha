//! Validation-fee policy data shared by validators and clients.
#[cfg(feature = "json")]
use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use crate::{
    Level, NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    isi::{InstructionBox, Log},
    name::Name,
    parameter::{CustomParameter, CustomParameterId},
    parliament_types::{GovernanceCertificateId, GovernanceCertificateV1, ProposalContentId},
    smart_contract::ContractAddress,
};
use iroha_crypto::Hash;
use iroha_primitives::{
    json::Json,
    numeric::{Numeric, Quantity},
};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use std::collections::BTreeSet;
/// Schema version for the initial validation-fee policy.
pub const VALIDATION_FEE_POLICY_SCHEMA_VERSION: u16 = 1;
/// Decimal scale required for the initial policy fee asset.
pub const VALIDATION_FEE_DS_SCALE: u8 = 2;
/// Canonical fee amount required by the initial validation-fee policy (0.10 DS).
pub const VALIDATION_FEE_INITIAL_AMOUNT: &str = "0.10";
/// Minimum delay from Parliament enactment to policy activation.
pub const VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS: u64 = 120_960;
/// Exact DS batch consumed by every validation-fee payout lifecycle tick.
pub const VALIDATION_FEE_PAYOUT_BATCH_DS: &str = "10";
/// Exact inclusive minimum XOR output accepted by the payout lifecycle.
pub const VALIDATION_FEE_PAYOUT_MIN_XOR: &str = "4";
/// Exact inclusive maximum XOR output accepted by the payout lifecycle.
pub const VALIDATION_FEE_PAYOUT_MAX_XOR: &str = "100";
/// Exact share assigned to each of the four payout recipients.
pub const VALIDATION_FEE_PAYOUT_RECIPIENT_SHARE: &str = "0.25";
/// Only release exemption class implemented by validator admission.
pub const VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS: &str = "TREASURY_PAYOUT";
/// Number of recipients required by the atomic treasury-payout plan.
pub const VALIDATION_FEE_TREASURY_PAYOUT_RECIPIENT_COUNT: usize = 4;
/// Domain separator for policy hashing.
pub const VALIDATION_FEE_POLICY_HASH_DOMAIN: &[u8] = b"iroha.validation_fee.policy.parliament.v1";
/// Domain separator for an exact Parliament-approved payout lifecycle.
pub const VALIDATION_FEE_PAYOUT_LIFECYCLE_SEAL_DOMAIN: &[u8] =
    b"iroha.validation_fee.payout_lifecycle.seal.v1";
/// Domain separator for the canonical full-registry snapshot hash.
pub const VALIDATION_FEE_REGISTRY_SNAPSHOT_HASH_DOMAIN: &[u8] =
    b"iroha.validation_fee.registry.snapshot.v1";
/// Current validation-fee synthetic witness format.
pub const VALIDATION_FEE_POLICY_SNAPSHOT_VERSION_V1: u16 = 1;
/// Exact sparse-tree depth of the execution-witness proof.
pub const VALIDATION_FEE_POLICY_WITNESS_SIBLINGS_V1: usize = 256;
/// Fixed synthetic execution-witness key committed by every block.
pub const VALIDATION_FEE_POLICY_WITNESS_KEY_V1: &[u8] =
    b"\xd4iroha:validation-fee:policy-registry:v1";
/// Retired custom-parameter identifier for the pre-release governance keyset.
pub const RETIRED_VALIDATION_FEE_GOVERNANCE_KEYSET_PARAMETER_ID: &str =
    "iroha:validation_fee_governance_keyset_v1";
/// Retired custom-parameter identifier for the pre-release active-policy copy.
pub const RETIRED_VALIDATION_FEE_POLICY_PARAMETER_ID: &str = "iroha:validation_fee_policy_v1";
/// Transaction metadata key that binds a signed transaction to a policy version.
pub const VALIDATION_FEE_POLICY_VERSION_METADATA_KEY: &str = "validation_fee_policy_version";
/// Transaction metadata key that binds a signed transaction to a policy hash.
pub const VALIDATION_FEE_POLICY_HASH_METADATA_KEY: &str = "validation_fee_policy_hash";
/// Transaction metadata key that identifies the aggregate validation-fee instruction.
pub const VALIDATION_FEE_INSTRUCTION_INDEX_METADATA_KEY: &str = "validation_fee_instruction_index";
/// Transaction metadata key that identifies the aggregate validation-fee batch entry, when used.
pub const VALIDATION_FEE_TRANSFER_ENTRY_INDEX_METADATA_KEY: &str =
    "validation_fee_transfer_entry_index";
/// Reserved prefix for the canonical marker carried inside fee-bearing multisig proposals.
pub const VALIDATION_FEE_MULTISIG_MARKER_PREFIX: &str = "iroha:validation_fee:multisig:v1:";
const VALIDATION_FEE_MULTISIG_MARKER_RESERVED_PREFIX: &str = "iroha:validation_fee:multisig:";
/// Return whether a custom parameter identifier belongs to the consensus-owned
/// validation-fee governance surface.
///
/// These parameters are enacted atomically by SORA Parliament and must never be writable through
/// the generic `SetParameter` instruction, including at genesis.
#[must_use]
pub fn is_reserved_validation_fee_parameter_id(id: &CustomParameterId) -> bool {
    id == &ValidationFeePolicyRegistryV1::parameter_id()
        || id.to_string() == RETIRED_VALIDATION_FEE_GOVERNANCE_KEYSET_PARAMETER_ID
        || id.to_string() == RETIRED_VALIDATION_FEE_POLICY_PARAMETER_ID
}
/// Transaction-bound fee designation carried inside a multisig proposal's instruction list.
///
/// The marker is encoded as a canonical `TRACE` [`Log`] instruction. Because it is part of the
/// proposal instruction list, both the proposal hash and every approval bind the active policy and
/// exact fee coordinate, including an optional batch-entry coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ValidationFeeMultisigMarkerV1 {
    /// Active Parliament-enacted validation-fee policy version.
    pub policy_version: u64,
    /// Active Parliament-enacted validation-fee policy hash.
    pub policy_hash: [u8; 32],
    /// Fee transfer instruction index within this proposal execution context.
    pub instruction_index: u64,
    /// Fee batch-entry index, when the fee is an entry in `TransferAssetBatch`.
    pub transfer_entry_index: Option<u64>,
}
impl ValidationFeeMultisigMarkerV1 {
    /// Construct a canonical multisig validation-fee marker.
    pub const fn new(
        policy_version: u64,
        policy_hash: [u8; 32],
        instruction_index: u64,
        transfer_entry_index: Option<u64>,
    ) -> Self {
        Self {
            policy_version,
            policy_hash,
            instruction_index,
            transfer_entry_index,
        }
    }
    /// Encode this marker as the canonical no-asset-effect instruction.
    pub fn into_instruction(self) -> InstructionBox {
        let entry = self
            .transfer_entry_index
            .map_or_else(|| "-".to_owned(), |index| index.to_string());
        Log::new(
            Level::TRACE,
            format!(
                "{VALIDATION_FEE_MULTISIG_MARKER_PREFIX}{}:{}:{}:{entry}",
                self.policy_version,
                hex::encode(self.policy_hash),
                self.instruction_index,
            ),
        )
        .into()
    }
    /// Parse a canonical marker instruction.
    ///
    /// Returns `Ok(None)` for ordinary instructions and logs outside the reserved marker namespace.
    /// Any instruction claiming the reserved namespace must be canonical or parsing fails closed.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationFeeMultisigMarkerError`] when an instruction claims
    /// the reserved marker namespace but is not its exact canonical encoding.
    pub fn parse_instruction(
        instruction: &InstructionBox,
    ) -> Result<Option<Self>, ValidationFeeMultisigMarkerError> {
        let Some(log) = instruction.as_any().downcast_ref::<Log>() else {
            return Ok(None);
        };
        if !log
            .msg
            .starts_with(VALIDATION_FEE_MULTISIG_MARKER_RESERVED_PREFIX)
        {
            return Ok(None);
        }
        if log.level != Level::TRACE {
            return Err(ValidationFeeMultisigMarkerError::WrongLogLevel);
        }
        let Some(payload) = log.msg.strip_prefix(VALIDATION_FEE_MULTISIG_MARKER_PREFIX) else {
            return Err(ValidationFeeMultisigMarkerError::Malformed);
        };
        let mut fields = payload.split(':');
        let policy_version = fields
            .next()
            .and_then(parse_canonical_marker_u64)
            .filter(|version| *version > 0)
            .ok_or(ValidationFeeMultisigMarkerError::Malformed)?;
        let policy_hash_hex = fields
            .next()
            .ok_or(ValidationFeeMultisigMarkerError::Malformed)?;
        if policy_hash_hex.len() != 64
            || !policy_hash_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ValidationFeeMultisigMarkerError::Malformed);
        }
        let policy_hash: [u8; 32] = hex::decode(policy_hash_hex)
            .map_err(|_| ValidationFeeMultisigMarkerError::Malformed)?
            .try_into()
            .map_err(|_| ValidationFeeMultisigMarkerError::Malformed)?;
        let instruction_index = fields
            .next()
            .and_then(parse_canonical_marker_u64)
            .ok_or(ValidationFeeMultisigMarkerError::Malformed)?;
        let entry = fields
            .next()
            .ok_or(ValidationFeeMultisigMarkerError::Malformed)?;
        if fields.next().is_some() {
            return Err(ValidationFeeMultisigMarkerError::Malformed);
        }
        let transfer_entry_index = if entry == "-" {
            None
        } else {
            Some(
                parse_canonical_marker_u64(entry)
                    .ok_or(ValidationFeeMultisigMarkerError::Malformed)?,
            )
        };
        Ok(Some(Self {
            policy_version,
            policy_hash,
            instruction_index,
            transfer_entry_index,
        }))
    }
}
fn parse_canonical_marker_u64(value: &str) -> Option<u64> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return None;
    }
    value.parse().ok()
}
/// Error returned when an instruction claims the reserved multisig marker namespace but is not
/// canonical.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationFeeMultisigMarkerError {
    /// The reserved marker used a log level other than `TRACE`.
    WrongLogLevel,
    /// The marker payload was not in the canonical versioned representation.
    Malformed,
}
impl core::fmt::Display for ValidationFeeMultisigMarkerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::WrongLogLevel => write!(
                f,
                "validation-fee multisig marker must use the TRACE log level"
            ),
            Self::Malformed => write!(f, "validation-fee multisig marker is malformed"),
        }
    }
}
impl std::error::Error for ValidationFeeMultisigMarkerError {}
/// Error returned when a validation-fee policy registry is malformed.
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
    /// A policy hash could not be computed.
    PolicyHashEncoding,
    /// A policy payload violates the validation-fee policy invariants.
    InvalidPolicyInvariant {
        /// Version of the invalid policy.
        policy_version: u64,
    },
    /// An entry's stored hash does not match its complete policy.
    PolicyHashMismatch {
        /// Version of the malformed entry.
        policy_version: u64,
    },
    /// A successor was scheduled before its predecessor.
    EffectiveHeightRollback {
        /// Version of the malformed entry.
        policy_version: u64,
    },
    /// A successor changed the immutable exact network binding.
    NetworkIdentityChanged {
        /// Version of the malformed entry.
        policy_version: u64,
    },
    /// Typed Parliament authorization evidence is malformed.
    InvalidParliamentAuthorization {
        /// Version of the malformed entry.
        policy_version: u64,
    },
    /// A payout lifecycle reference is missing, unexpected, or malformed.
    InvalidPayoutLifecycleReference {
        /// Version of the malformed entry.
        policy_version: u64,
    },
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
            Self::PolicyHashEncoding => {
                write!(
                    f,
                    "validation-fee registry policy hash could not be encoded"
                )
            }
            Self::InvalidPolicyInvariant { policy_version } => write!(
                f,
                "validation-fee registry policy version {policy_version} violates policy invariants"
            ),
            Self::PolicyHashMismatch { policy_version } => write!(
                f,
                "validation-fee registry policy hash mismatch at version {policy_version}"
            ),
            Self::EffectiveHeightRollback { policy_version } => write!(
                f,
                "validation-fee policy effective height moves backwards at version {policy_version}"
            ),
            Self::NetworkIdentityChanged { policy_version } => write!(
                f,
                "validation-fee policy changes the immutable network identity at version {policy_version}"
            ),
            Self::InvalidParliamentAuthorization { policy_version } => write!(
                f,
                "validation-fee policy version {policy_version} has invalid Parliament authorization evidence"
            ),
            Self::InvalidPayoutLifecycleReference { policy_version } => write!(
                f,
                "validation-fee policy version {policy_version} has an invalid payout lifecycle reference"
            ),
        }
    }
}
impl std::error::Error for ValidationFeePolicyRegistryError {}
/// Validation-fee charging mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(
    tag = "charging_mode",
    content = "value",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum ValidationFeeChargingMode {
    /// Disable validation-fee charging through the governed policy chain.
    Disabled,
    /// Charge once per qualifying fee-asset transfer instruction or batch entry.
    PerQualifyingTransferInstruction,
}
/// Canonical Parliament certificate authorization for one enacted validation-fee proposal.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct ValidationFeeParliamentAuthorizationV1 {
    /// Canonical transaction authority bound into the exact proposal preimage.
    pub proposal_operator: AccountId,
    /// Fingerprint of the exact stored proposal preimage.
    pub proposal_fingerprint: [u8; 32],
    /// Canonical content identifier of the complete retained Parliament certificate.
    pub governance_certificate_id: GovernanceCertificateId,
    /// Complete private-ballot Parliament certificate retained for independent validation.
    pub governance_certificate: GovernanceCertificateV1,
    /// Height at which the certified proposal was appended to its governed registry.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::u64_string"))]
    pub enacted_at_height: u64,
}
impl ValidationFeeParliamentAuthorizationV1 {
    /// Return a stable invariant violation, if any.
    #[must_use]
    pub fn invariant_error(&self) -> Option<&'static str> {
        if self.proposal_fingerprint == [0; 32] {
            return Some(
                "validation-fee Parliament proposal fingerprint must be a non-zero native identifier",
            );
        }
        if self.governance_certificate.validate().is_err() {
            return Some("validation-fee Parliament certificate is structurally invalid");
        }
        if self.governance_certificate_id.as_bytes() == &[0; 32]
            || self.governance_certificate_id
                != GovernanceCertificateId::derive_v1(&self.governance_certificate)
        {
            return Some(
                "validation-fee Parliament certificate identifier is not the canonical certificate hash",
            );
        }
        if self.governance_certificate.proposal_content_id
            != ProposalContentId::new(self.proposal_fingerprint)
        {
            return Some(
                "validation-fee Parliament certificate targets a different proposal fingerprint",
            );
        }
        if self.enacted_at_height != self.governance_certificate.enact_at_height {
            return Some(
                "validation-fee enactment height must equal the Parliament certificate due height",
            );
        }
        None
    }
}
/// Exact enacted payout-lifecycle proposal referenced by a validation-fee policy.
///
/// First-release registries retain this reference append-only. There is no
/// physical lifecycle-retirement state or caller-supplied reference count.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ValidationFeePayoutLifecycleReferenceV1 {
    /// Non-zero lifecycle seal bound into the proposal fingerprint.
    pub lifecycle_seal: [u8; 32],
    /// Full typed Parliament certificate authorization for the lifecycle proposal.
    pub parliament_authorization: ValidationFeeParliamentAuthorizationV1,
}
impl ValidationFeePayoutLifecycleReferenceV1 {
    /// Return a stable invariant violation, if any.
    #[must_use]
    pub fn invariant_error(&self) -> Option<&'static str> {
        if self.lifecycle_seal == [0; 32] {
            return Some("validation-fee payout lifecycle seal must be non-zero");
        }
        if self.parliament_authorization.invariant_error().is_some() {
            return Some(
                "validation-fee payout lifecycle Parliament authorization evidence is invalid",
            );
        }
        None
    }
}
/// One entry in the registered validation-fee policy hash chain.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ValidationFeePolicyRegistryEntryV1 {
    /// Complete governed policy, retained so scheduled policies do not hide
    /// the policy that is effective at the current height.
    pub policy: ValidationFeePolicyV1,
    /// Domain-separated policy hash.
    pub policy_hash: [u8; 32],
    /// Typed, independently checkable Parliament certificate authorization.
    pub parliament_authorization: ValidationFeeParliamentAuthorizationV1,
    /// Exact enacted payout lifecycle required by a policy carrying a payout binding.
    pub payout_lifecycle: Option<ValidationFeePayoutLifecycleReferenceV1>,
}
impl ValidationFeePolicyRegistryEntryV1 {
    /// Build a registry entry from one enacted Parliament proposal.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the policy cannot be hashed.
    pub fn from_enactment(
        policy: ValidationFeePolicyV1,
        parliament_authorization: ValidationFeeParliamentAuthorizationV1,
        payout_lifecycle: Option<ValidationFeePayoutLifecycleReferenceV1>,
    ) -> Result<Self, norito::Error> {
        let policy_hash = policy.policy_hash()?;
        Ok(Self {
            policy,
            policy_hash,
            parliament_authorization,
            payout_lifecycle,
        })
    }
}
/// On-ledger validation-fee policy registry used to reject rollback and
/// skipped-version policy changes while retaining scheduled policy history.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct ValidationFeePolicyRegistryV1 {
    /// Registered policy chain in ascending, contiguous version order.
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
    /// Validate the complete contiguous policy chain and authenticate every retained Parliament
    /// proposal fingerprint.
    ///
    /// The validation-fee module reproduces the frozen V1 proposal preimages locally and validates
    /// each complete canonical Parliament certificate through the always-compiled type layer.
    ///
    /// # Errors
    ///
    /// Returns an error when the registry is empty, non-monotonic, broken, unauthenticated, or
    /// contains a policy whose stored hash differs from its payload.
    pub fn validate(&self) -> Result<(), ValidationFeePolicyRegistryError> {
        let mut entries = self.registered_policies.iter();
        let Some(first) = entries.next() else {
            return Err(ValidationFeePolicyRegistryError::EmptyRegistry);
        };
        if first.policy.policy_version != 1 {
            return Err(ValidationFeePolicyRegistryError::UnexpectedPolicyVersion {
                expected: 1,
                found: first.policy.policy_version,
            });
        }
        if first.policy.policy_invariant_error().is_some() {
            return Err(ValidationFeePolicyRegistryError::InvalidPolicyInvariant {
                policy_version: first.policy.policy_version,
            });
        }
        if first.policy.previous_policy_hash.is_some() {
            return Err(ValidationFeePolicyRegistryError::BrokenPreviousPolicyHash {
                policy_version: first.policy.policy_version,
            });
        }
        validate_registry_entry_authorization(first)?;
        let first_hash = first
            .policy
            .policy_hash()
            .map_err(|_| ValidationFeePolicyRegistryError::PolicyHashEncoding)?;
        if first_hash != first.policy_hash {
            return Err(ValidationFeePolicyRegistryError::PolicyHashMismatch {
                policy_version: first.policy.policy_version,
            });
        }
        let mut seen_hashes = BTreeSet::from([first.policy_hash]);
        let mut expected_version = 2u64;
        let mut previous_hash = first.policy_hash;
        let mut previous_effective_height = first.policy.effective_from_height;
        let network_id = first.policy.network_id;
        for entry in entries {
            if entry.policy.policy_version != expected_version {
                return Err(ValidationFeePolicyRegistryError::UnexpectedPolicyVersion {
                    expected: expected_version,
                    found: entry.policy.policy_version,
                });
            }
            if entry.policy.policy_invariant_error().is_some() {
                return Err(ValidationFeePolicyRegistryError::InvalidPolicyInvariant {
                    policy_version: entry.policy.policy_version,
                });
            }
            if !seen_hashes.insert(entry.policy_hash) {
                return Err(ValidationFeePolicyRegistryError::DuplicatePolicyHash {
                    policy_version: entry.policy.policy_version,
                });
            }
            if entry.policy.previous_policy_hash != Some(previous_hash) {
                return Err(ValidationFeePolicyRegistryError::BrokenPreviousPolicyHash {
                    policy_version: entry.policy.policy_version,
                });
            }
            if entry.policy.network_id != network_id {
                return Err(ValidationFeePolicyRegistryError::NetworkIdentityChanged {
                    policy_version: entry.policy.policy_version,
                });
            }
            validate_registry_entry_authorization(entry)?;
            let policy_hash = entry
                .policy
                .policy_hash()
                .map_err(|_| ValidationFeePolicyRegistryError::PolicyHashEncoding)?;
            if policy_hash != entry.policy_hash {
                return Err(ValidationFeePolicyRegistryError::PolicyHashMismatch {
                    policy_version: entry.policy.policy_version,
                });
            }
            if entry.policy.effective_from_height < previous_effective_height {
                return Err(ValidationFeePolicyRegistryError::EffectiveHeightRollback {
                    policy_version: entry.policy.policy_version,
                });
            }
            expected_version = expected_version.checked_add(1).ok_or(
                ValidationFeePolicyRegistryError::UnexpectedPolicyVersion {
                    expected: u64::MAX,
                    found: entry.policy.policy_version,
                },
            )?;
            previous_hash = entry.policy_hash;
            previous_effective_height = entry.policy.effective_from_height;
        }
        Ok(())
    }
    /// Return the latest enacted entry, including a policy scheduled for a future height.
    #[must_use]
    pub fn head(&self) -> Option<&ValidationFeePolicyRegistryEntryV1> {
        self.registered_policies.last()
    }
    /// Return the highest-version policy whose effective height has arrived.
    ///
    /// An expired higher version never falls back to an older version.
    #[must_use]
    pub fn scheduled_entry_at_height(
        &self,
        height: u64,
    ) -> Option<&ValidationFeePolicyRegistryEntryV1> {
        self.registered_policies
            .iter()
            .rev()
            .find(|entry| entry.policy.effective_from_height <= height)
    }
    /// Return the effective policy entry at `height`.
    ///
    /// This returns `None` before the first policy is effective or after the
    /// selected highest-version policy expires.
    #[must_use]
    pub fn effective_entry_at_height(
        &self,
        height: u64,
    ) -> Option<&ValidationFeePolicyRegistryEntryV1> {
        self.scheduled_entry_at_height(height)
            .filter(|entry| entry.policy.is_active_at_height(height))
    }
    /// Hash the canonical complete registry for a finality-bound snapshot.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error when the registry cannot be serialized.
    pub fn snapshot_hash(&self) -> Result<[u8; 32], norito::Error> {
        let encoded = norito::encode_canonical(self)?;
        let mut preimage = Vec::with_capacity(
            VALIDATION_FEE_REGISTRY_SNAPSHOT_HASH_DOMAIN.len() + 1 + encoded.len(),
        );
        preimage.extend_from_slice(VALIDATION_FEE_REGISTRY_SNAPSHOT_HASH_DOMAIN);
        preimage.push(0);
        preimage.extend_from_slice(&encoded);
        Ok(*Hash::new(preimage).as_ref())
    }
}
/// Valid registry facts bound into each block's synthetic witness write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct ValidationFeePolicySnapshotAvailableV1 {
    /// Hash of the canonical complete registry.
    pub registry_hash: [u8; 32],
    /// Latest enacted policy hash, including a future scheduled successor.
    pub head_policy_hash: [u8; 32],
    /// Highest-version policy whose effective height has arrived.
    pub scheduled_policy_hash: Option<[u8; 32]>,
    /// Scheduled policy hash when its validity window is active.
    pub effective_policy_hash: Option<[u8; 32]>,
}
/// Registry availability committed by a validation-fee synthetic witness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    norito(
        tag = "status",
        content = "value",
        rename_all = "SCREAMING_SNAKE_CASE",
        deny_unknown_fields
    ),
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub enum ValidationFeePolicySnapshotStatusV1 {
    /// Parliament has not enacted the first policy.
    Unconfigured,
    /// A malformed protected registry was observed; the hash identifies the failure.
    Invalid(Hash),
    /// A validated full registry and its height-dependent selection.
    Available(ValidationFeePolicySnapshotAvailableV1),
}
/// Canonical validation-fee registry commitment written into every block witness.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct ValidationFeePolicySnapshotCommitmentV1 {
    /// Snapshot format version.
    pub version: u16,
    /// Block height whose post-execution registry state was evaluated.
    pub evaluated_height: u64,
    /// Validated registry state and selected policy hashes.
    pub status: ValidationFeePolicySnapshotStatusV1,
}
impl ValidationFeePolicySnapshotCommitmentV1 {
    /// Derive a deterministic commitment from the protected registry state.
    #[must_use]
    pub fn from_registry(
        evaluated_height: u64,
        registry: Option<&ValidationFeePolicyRegistryV1>,
    ) -> Self {
        let Some(registry) = registry else {
            return Self {
                version: VALIDATION_FEE_POLICY_SNAPSHOT_VERSION_V1,
                evaluated_height,
                status: ValidationFeePolicySnapshotStatusV1::Unconfigured,
            };
        };
        let available = registry.validate().and_then(|()| {
            let registry_hash = registry
                .snapshot_hash()
                .map_err(|_| ValidationFeePolicyRegistryError::PolicyHashEncoding)?;
            let head = registry
                .head()
                .ok_or(ValidationFeePolicyRegistryError::EmptyRegistry)?;
            Ok(ValidationFeePolicySnapshotAvailableV1 {
                registry_hash,
                head_policy_hash: head.policy_hash,
                scheduled_policy_hash: registry
                    .scheduled_entry_at_height(evaluated_height)
                    .map(|entry| entry.policy_hash),
                effective_policy_hash: registry
                    .effective_entry_at_height(evaluated_height)
                    .map(|entry| entry.policy_hash),
            })
        });
        let status = match available {
            Ok(available) => ValidationFeePolicySnapshotStatusV1::Available(available),
            Err(error) => {
                ValidationFeePolicySnapshotStatusV1::Invalid(Hash::new(error.to_string()))
            }
        };
        Self {
            version: VALIDATION_FEE_POLICY_SNAPSHOT_VERSION_V1,
            evaluated_height,
            status,
        }
    }
    /// Derive the commitment directly from the protected custom parameter.
    #[must_use]
    pub fn from_custom_parameter_state(
        evaluated_height: u64,
        custom: Option<&CustomParameter>,
    ) -> Self {
        let Some(custom) = custom else {
            return Self::from_registry(evaluated_height, None);
        };
        let Some(registry) = ValidationFeePolicyRegistryV1::from_custom_parameter(custom) else {
            let invalid_hash = norito::encode_canonical(custom)
                .map_or_else(|_| Hash::new(custom.id().to_string()), Hash::new);
            return Self {
                version: VALIDATION_FEE_POLICY_SNAPSHOT_VERSION_V1,
                evaluated_height,
                status: ValidationFeePolicySnapshotStatusV1::Invalid(invalid_hash),
            };
        };
        Self::from_registry(evaluated_height, Some(&registry))
    }
}
/// Sparse-SMT proof that the validation-fee snapshot is an ordinary write.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct ValidationFeePolicyWitnessProofV1 {
    /// Fixed raw execution-witness key.
    pub key: Vec<u8>,
    /// Exact canonical encoded snapshot commitment.
    pub value: Vec<u8>,
    /// Exactly 256 siblings from leaf level to the ordinary-write root.
    pub siblings: Vec<Hash>,
}
impl ValidationFeePolicyWitnessProofV1 {
    /// Verify the fixed synthetic write against an ordinary-write SMT root.
    #[must_use]
    pub fn verify(&self, expected_ordinary_writes_root: Hash) -> bool {
        if self.key != VALIDATION_FEE_POLICY_WITNESS_KEY_V1
            || self.siblings.len() != VALIDATION_FEE_POLICY_WITNESS_SIBLINGS_V1
        {
            return false;
        }
        let Ok(commitment) =
            norito::decode_canonical::<ValidationFeePolicySnapshotCommitmentV1>(&self.value)
        else {
            return false;
        };
        if commitment.version != VALIDATION_FEE_POLICY_SNAPSHOT_VERSION_V1 {
            return false;
        }
        let path = Hash::new(&self.key);
        let value_hash = Hash::new(&self.value);
        let mut leaf_preimage = Vec::with_capacity(1 + 2 * Hash::LENGTH);
        leaf_preimage.push(0);
        leaf_preimage.extend_from_slice(path.as_ref());
        leaf_preimage.extend_from_slice(value_hash.as_ref());
        let mut current = Hash::new(leaf_preimage);
        for (level, sibling) in self.siblings.iter().copied().enumerate() {
            let path_bit = 255_usize.saturating_sub(level);
            let byte = path.as_ref()[path_bit / 8];
            let right = byte & (1_u8 << (path_bit % 8)) != 0;
            current = if right {
                validation_fee_ordinary_smt_node_hash(sibling, current)
            } else {
                validation_fee_ordinary_smt_node_hash(current, sibling)
            };
        }
        current == expected_ordinary_writes_root
    }
    /// Decode and return the exact canonical snapshot commitment.
    ///
    /// # Errors
    ///
    /// Returns an error when the stored value is not a valid canonical Norito
    /// encoding of [`ValidationFeePolicySnapshotCommitmentV1`].
    pub fn commitment(&self) -> Result<ValidationFeePolicySnapshotCommitmentV1, String> {
        norito::decode_canonical(&self.value).map_err(|error| {
            if matches!(&error, norito::Error::NonCanonicalEncoding) {
                "validation-fee snapshot commitment is non-canonical".into()
            } else {
                format!("validation-fee snapshot commitment is invalid: {error}")
            }
        })
    }
}
fn validation_fee_ordinary_smt_node_hash(left: Hash, right: Hash) -> Hash {
    let mut preimage = Vec::with_capacity(1 + 2 * Hash::LENGTH);
    preimage.push(1);
    preimage.extend_from_slice(left.as_ref());
    preimage.extend_from_slice(right.as_ref());
    Hash::new(preimage)
}
// Validation-fee registries are needed by lightweight data-model consumers
// that do not compile governance instructions or events, but their certificate
// checks must still reproduce the exact governance proposal fingerprint.
// Explicit V1 discriminants bind this private preimage to the matching
// `ProposalKind` wire tags. Each semantic proposal kind also has a distinct
// hash domain, so a future discriminant mistake cannot create a cross-kind
// fingerprint collision. Parity tests below verify the complete encoded bytes
// and fingerprints against the always-compiled Parliament type layer.
#[derive(Encode)]
enum ValidationFeePolicyProposalFingerprintEnvelopeV1 {
    #[codec(index = 3)]
    ValidationFeePolicy(ValidationFeePolicyFingerprintPayloadV1),
}
#[derive(Encode)]
enum ValidationFeePayoutLifecycleProposalFingerprintEnvelopeV1 {
    #[codec(index = 4)]
    ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleFingerprintPayloadV1),
}
#[derive(Encode)]
struct ValidationFeePolicyFingerprintPayloadV1 {
    proposal_operator: AccountId,
    policy: ValidationFeePolicyV1,
    payout_lifecycle_proposal_id: Option<[u8; 32]>,
}
#[derive(Encode)]
struct ValidationFeePayoutLifecycleFingerprintPayloadV1 {
    proposal_operator: AccountId,
    payout_binding: ValidationFeeTreasuryPayoutBindingV1,
}
fn validation_fee_policy_proposal_fingerprint(
    proposal_operator: &AccountId,
    policy: &ValidationFeePolicyV1,
    payout_lifecycle_proposal_id: Option<[u8; 32]>,
) -> [u8; 32] {
    crate::governance_fingerprint::fingerprint(
        crate::governance_fingerprint::VALIDATION_FEE_POLICY_V1,
        &ValidationFeePolicyProposalFingerprintEnvelopeV1::ValidationFeePolicy(
            ValidationFeePolicyFingerprintPayloadV1 {
                proposal_operator: proposal_operator.clone(),
                policy: policy.clone(),
                payout_lifecycle_proposal_id,
            },
        ),
    )
}
fn validation_fee_payout_lifecycle_proposal_fingerprint(
    proposal_operator: &AccountId,
    payout_binding: &ValidationFeeTreasuryPayoutBindingV1,
) -> [u8; 32] {
    crate::governance_fingerprint::fingerprint(
        crate::governance_fingerprint::VALIDATION_FEE_PAYOUT_LIFECYCLE_V1,
        &ValidationFeePayoutLifecycleProposalFingerprintEnvelopeV1::ValidationFeePayoutLifecycle(
            ValidationFeePayoutLifecycleFingerprintPayloadV1 {
                proposal_operator: proposal_operator.clone(),
                payout_binding: payout_binding.clone(),
            },
        ),
    )
}
fn validate_registry_entry_authorization(
    entry: &ValidationFeePolicyRegistryEntryV1,
) -> Result<(), ValidationFeePolicyRegistryError> {
    let policy_version = entry.policy.policy_version;
    if entry.parliament_authorization.invariant_error().is_some() {
        return Err(
            ValidationFeePolicyRegistryError::InvalidParliamentAuthorization { policy_version },
        );
    }
    let payout_lifecycle_proposal_id = match (
        entry.policy.treasury_payout_binding.as_ref(),
        entry.payout_lifecycle.as_ref(),
    ) {
        (Some(binding), Some(reference))
            if reference.invariant_error().is_none()
                && reference.parliament_authorization.enacted_at_height
                    <= entry.parliament_authorization.enacted_at_height
                && binding.lifecycle_seal().ok() == Some(reference.lifecycle_seal) =>
        {
            let fingerprint = validation_fee_payout_lifecycle_proposal_fingerprint(
                &reference.parliament_authorization.proposal_operator,
                binding,
            );
            if reference.parliament_authorization.proposal_fingerprint != fingerprint {
                return Err(
                    ValidationFeePolicyRegistryError::InvalidPayoutLifecycleReference {
                        policy_version,
                    },
                );
            }
            Some(fingerprint)
        }
        (None, None) => None,
        _ => {
            return Err(
                ValidationFeePolicyRegistryError::InvalidPayoutLifecycleReference {
                    policy_version,
                },
            );
        }
    };
    if entry
        .parliament_authorization
        .enacted_at_height
        .checked_add(VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS)
        != Some(entry.policy.effective_from_height)
    {
        return Err(
            ValidationFeePolicyRegistryError::InvalidParliamentAuthorization { policy_version },
        );
    }
    let fingerprint = validation_fee_policy_proposal_fingerprint(
        &entry.parliament_authorization.proposal_operator,
        &entry.policy,
        payout_lifecycle_proposal_id,
    );
    if entry.parliament_authorization.proposal_fingerprint != fingerprint {
        return Err(
            ValidationFeePolicyRegistryError::InvalidParliamentAuthorization { policy_version },
        );
    }
    Ok(())
}
/// One exact recipient and share in the atomic treasury-payout effect plan.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[norito(deny_unknown_fields)]
pub struct ValidationFeeTreasuryPayoutRecipientV1 {
    /// Validator account receiving XOR.
    pub account_id: AccountId,
    /// Exact positive dimensionless share. All four shares must sum to one.
    pub share: Numeric,
}
/// Parliament-enacted binding for the only opaque treasury payout admitted by the policy.
///
/// The binding names one immutable contract image and entrypoint plus the complete
/// six-transfer effect plan. It is part of policy hashing, authorization, registry
/// validation, and Norito/JSON serialization.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct ValidationFeeTreasuryPayoutBindingV1 {
    /// Immutable deployed pool contract address.
    pub contract_address: ContractAddress,
    /// SHA-256 hash of the exact deployed contract artifact bytes.
    pub code_hash: [u8; 32],
    /// Only public entrypoint allowed to consume reserved validation-fee credit.
    pub entrypoint: Name,
    /// Immutable non-signable contract subject and policy treasury.
    pub treasury_account_id: AccountId,
    /// Exact policy DS fee asset used by the pool quote leg.
    pub ds_asset_id: AssetDefinitionId,
    /// Exact XOR asset returned by the pool base leg.
    pub xor_asset_id: AssetDefinitionId,
    /// Exact pool vault receiving DS and sourcing XOR.
    pub pool_vault_account_id: AccountId,
    /// Exact DS amount consumed per successful payout tick.
    pub batch_ds: Quantity,
    /// Inclusive minimum XOR output accepted from the pool.
    pub min_xor_out: Quantity,
    /// Inclusive maximum XOR output accepted from the pool.
    pub max_xor_out: Quantity,
    /// Exactly four ordered validator recipients and deterministic shares.
    pub recipients: Vec<ValidationFeeTreasuryPayoutRecipientV1>,
}
impl ValidationFeeTreasuryPayoutBindingV1 {
    /// Compute the release seal for this exact payout binding.
    ///
    /// The seal is consensus-derived rather than caller supplied, so a lifecycle
    /// proposal cannot authorize one binding while publishing an unrelated label.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the binding cannot be encoded.
    pub fn lifecycle_seal(&self) -> Result<[u8; 32], norito::Error> {
        let encoded = norito::encode_canonical(self)?;
        let mut preimage = Vec::with_capacity(
            VALIDATION_FEE_PAYOUT_LIFECYCLE_SEAL_DOMAIN.len() + 1 + encoded.len(),
        );
        preimage.extend_from_slice(VALIDATION_FEE_PAYOUT_LIFECYCLE_SEAL_DOMAIN);
        preimage.push(0);
        preimage.extend_from_slice(&encoded);
        Ok(*Hash::new(preimage).as_ref())
    }
    /// Return a stable invariant violation, if any.
    #[must_use]
    pub fn invariant_error(&self) -> Option<&'static str> {
        if self.code_hash == [0; 32] {
            return Some("validation-fee treasury payout code hash must be non-zero");
        }
        if self.entrypoint.as_ref() != "autonomous_validation_fee_tick" {
            return Some(
                "validation-fee treasury payout entrypoint must be autonomous_validation_fee_tick",
            );
        }
        if self.treasury_account_id == self.pool_vault_account_id {
            return Some("validation-fee treasury payout treasury and pool vault must differ");
        }
        if self.ds_asset_id == self.xor_asset_id {
            return Some("validation-fee treasury payout DS and XOR assets must differ");
        }
        if self.batch_ds != validation_fee_payout_batch_ds() {
            return Some("validation-fee treasury payout batch must be exactly 10 DS");
        }
        if self.min_xor_out != validation_fee_payout_min_xor()
            || self.max_xor_out != validation_fee_payout_max_xor()
        {
            return Some("validation-fee treasury payout XOR output bounds must be exactly 4..100");
        }
        if self.recipients.len() != VALIDATION_FEE_TREASURY_PAYOUT_RECIPIENT_COUNT {
            return Some("validation-fee treasury payout must bind exactly four recipients");
        }
        let mut accounts = BTreeSet::new();
        let mut share_sum = Numeric::zero();
        for recipient in &self.recipients {
            if recipient.account_id == self.treasury_account_id
                || recipient.account_id == self.pool_vault_account_id
                || !accounts.insert(recipient.account_id.clone())
            {
                return Some(
                    "validation-fee treasury payout recipients must be unique and differ from treasury and vault",
                );
            }
            if recipient.share != validation_fee_payout_recipient_share() {
                return Some(
                    "validation-fee treasury payout recipients must each receive exactly 25%",
                );
            }
            let Ok(next) = share_sum.try_decimal_add(&recipient.share) else {
                return Some(
                    "validation-fee treasury payout share sum is outside the numeric domain",
                );
            };
            share_sum = next;
        }
        if share_sum != Numeric::one() {
            return Some("validation-fee treasury payout shares must sum exactly to one");
        }
        None
    }
}
/// Exact-network validation-fee policy.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct ValidationFeePolicyV1 {
    /// Policy schema version.
    pub schema_version: u16,
    /// Exact genesis-derived network identity bound into the policy.
    pub network_id: NetworkId,
    /// Monotonic policy version.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::u64_string"))]
    pub policy_version: u64,
    /// Previous policy hash for policy-chain validation.
    pub previous_policy_hash: Option<[u8; 32]>,
    /// Concrete fee-asset definition charged by this policy.
    pub ds_asset_id: AssetDefinitionId,
    /// Required decimal precision of the charged fee asset.
    pub ds_scale: u8,
    /// Exact non-negative fee charged for each qualifying transfer.
    pub fee: Quantity,
    /// Concrete validator treasury account.
    pub treasury_account_id: AccountId,
    /// Charging mode.
    pub charging_mode: ValidationFeeChargingMode,
    /// First height at which the policy is active.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::u64_string"))]
    pub effective_from_height: u64,
    /// Optional last active height.
    #[cfg_attr(
        feature = "json",
        norito(json = "crate::json_helpers::u64_string::option")
    )]
    pub expires_after_height: Option<u64>,
    /// Explicit exemption classes recognized by this policy.
    pub exemption_classes: Vec<String>,
    /// Exact typed contract and six-transfer plan for `TREASURY_PAYOUT`.
    pub treasury_payout_binding: Option<ValidationFeeTreasuryPayoutBindingV1>,
}
impl ValidationFeePolicyV1 {
    /// Return true when this policy is active at the provided height.
    #[must_use]
    pub fn is_active_at_height(&self, height: u64) -> bool {
        height >= self.effective_from_height
            && self
                .expires_after_height
                .is_none_or(|expires_after_height| height < expires_after_height)
    }
    /// Deterministic domain-separated policy hash.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error if the policy cannot be serialized.
    pub fn policy_hash(&self) -> Result<[u8; 32], norito::Error> {
        let bytes = norito::encode_canonical(self)?;
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
        if self.network_id.as_bytes() == &[0; 32] {
            return Some("validation-fee policy network id must be non-zero");
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
        if self.ds_scale != VALIDATION_FEE_DS_SCALE {
            return Some("validation-fee policy asset scale must be 2");
        }
        match self.charging_mode {
            ValidationFeeChargingMode::Disabled if !self.fee.is_zero() => {
                return Some("disabled validation-fee policy amount must be zero");
            }
            ValidationFeeChargingMode::Disabled
                if !self.exemption_classes.is_empty() || self.treasury_payout_binding.is_some() =>
            {
                return Some(
                    "disabled validation-fee policy cannot carry exemptions or a treasury payout binding",
                );
            }
            ValidationFeeChargingMode::PerQualifyingTransferInstruction
                if self.fee != initial_validation_fee_amount() =>
            {
                return Some("enabled validation-fee policy amount must be exactly 0.10 DS");
            }
            ValidationFeeChargingMode::Disabled
            | ValidationFeeChargingMode::PerQualifyingTransferInstruction => {}
        }
        let mut exemption_classes = BTreeSet::new();
        for class in &self.exemption_classes {
            if class != VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS
                || !exemption_classes.insert(class)
            {
                return Some(
                    "validation-fee policy exemption classes must be unique approved release classes: TREASURY_PAYOUT",
                );
            }
        }
        let treasury_payout_enabled = self
            .exemption_classes
            .iter()
            .any(|class| class == VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS);
        match (
            treasury_payout_enabled,
            self.treasury_payout_binding.as_ref(),
        ) {
            (false, None) => {}
            (true, Some(binding)) => {
                if let Some(reason) = binding.invariant_error() {
                    return Some(reason);
                }
                if binding.treasury_account_id != self.treasury_account_id
                    || binding.contract_address.subject_id() != self.treasury_account_id
                {
                    return Some(
                        "validation-fee treasury payout contract subject must equal the policy treasury",
                    );
                }
                if binding.ds_asset_id != self.ds_asset_id {
                    return Some(
                        "validation-fee treasury payout DS asset must equal the policy fee asset",
                    );
                }
                if binding.batch_ds.scale() > u32::from(self.ds_scale) {
                    return Some(
                        "validation-fee treasury payout DS batch exceeds the policy asset scale",
                    );
                }
            }
            (true, None) => {
                return Some(
                    "validation-fee TREASURY_PAYOUT exemption requires an exact typed binding",
                );
            }
            (false, Some(_)) => {
                return Some(
                    "validation-fee treasury payout binding requires the TREASURY_PAYOUT exemption",
                );
            }
        }
        if self
            .expires_after_height
            .is_some_and(|expires_after_height| expires_after_height <= self.effective_from_height)
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
}
/// Return the canonical initial validation-fee amount.
#[must_use]
pub fn initial_validation_fee_amount() -> Quantity {
    VALIDATION_FEE_INITIAL_AMOUNT
        .parse()
        .expect("hard-coded validation-fee amount is canonical")
}
/// Return the exact DS payout batch amount.
#[must_use]
pub fn validation_fee_payout_batch_ds() -> Quantity {
    VALIDATION_FEE_PAYOUT_BATCH_DS
        .parse()
        .expect("hard-coded validation-fee payout batch is canonical")
}
/// Return the exact minimum XOR output.
#[must_use]
pub fn validation_fee_payout_min_xor() -> Quantity {
    VALIDATION_FEE_PAYOUT_MIN_XOR
        .parse()
        .expect("hard-coded validation-fee minimum XOR output is canonical")
}
/// Return the exact maximum XOR output.
#[must_use]
pub fn validation_fee_payout_max_xor() -> Quantity {
    VALIDATION_FEE_PAYOUT_MAX_XOR
        .parse()
        .expect("hard-coded validation-fee maximum XOR output is canonical")
}
/// Return the exact share assigned to each payout recipient.
#[must_use]
pub fn validation_fee_payout_recipient_share() -> Numeric {
    VALIDATION_FEE_PAYOUT_RECIPIENT_SHARE
        .parse()
        .expect("hard-coded validation-fee payout recipient share is canonical")
}
#[cfg(test)]
mod parliament_tests {
    use super::*;
    use crate::parliament_types::{
        BallotAttemptId, BeaconPulseId, BeaconSessionId, BodyElectionAttemptId, BodyInstanceId,
        GovernanceAttemptId, GovernanceCertificateV1, GovernanceExpectedHeadPresentV1,
        GovernanceExpectedHeadV1, ParliamentAggregateOutcomeV1, ParliamentAggregateTallyV1,
        ParliamentBallotCertificateBindingV1, ParliamentBody, ParliamentBodyCertificateBindingV1,
        ProposalContentId, ProposalKind, RiskTierV1, SortitionRequestV1, TleKeySessionId,
        TleSessionId, ValidationFeePayoutLifecycleProposal, ValidationFeePolicyProposal,
        parliament_ballot_result_root_v1,
    };
    use crate::{domain::DomainId, name::Name};
    use iroha_crypto::{Algorithm, KeyPair};
    use std::str::FromStr as _;
    const TEST_AUTHORIZATION_STRIDE: u64 = 10_000;

    fn account(seed: u8) -> AccountId {
        let key_pair =
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("key pair");
        AccountId::new(key_pair.public_key().clone())
    }
    fn fee_asset() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("fees", "validation").expect("domain id"),
            Name::from_str("fee").expect("asset name"),
        )
    }
    fn xor_asset() -> AssetDefinitionId {
        AssetDefinitionId::derive_from_components(
            DomainId::try_new("xor", "validation").expect("domain id"),
            Name::from_str("xor").expect("asset name"),
        )
    }
    fn payout_binding() -> ValidationFeeTreasuryPayoutBindingV1 {
        let contract_address: ContractAddress =
            "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
                .parse()
                .expect("contract address");
        ValidationFeeTreasuryPayoutBindingV1 {
            treasury_account_id: contract_address.subject_id(),
            contract_address,
            code_hash: [0x11; 32],
            entrypoint: Name::from_str("autonomous_validation_fee_tick").expect("entrypoint"),
            ds_asset_id: fee_asset(),
            xor_asset_id: xor_asset(),
            pool_vault_account_id: account(2),
            batch_ds: validation_fee_payout_batch_ds(),
            min_xor_out: validation_fee_payout_min_xor(),
            max_xor_out: validation_fee_payout_max_xor(),
            recipients: (3..=6)
                .map(|seed| ValidationFeeTreasuryPayoutRecipientV1 {
                    account_id: account(seed),
                    share: validation_fee_payout_recipient_share(),
                })
                .collect(),
        }
    }
    fn proposal_operator() -> AccountId {
        account(7)
    }
    fn authorization(
        proposal_fingerprint: [u8; 32],
        marker: u8,
    ) -> ValidationFeeParliamentAuthorizationV1 {
        let root = |offset: u8| [marker.wrapping_add(offset); 32];
        let base = u64::from(marker)
            .checked_mul(TEST_AUTHORIZATION_STRIDE)
            .expect("test authorization base height");
        let proposal_content_id = ProposalContentId::new(proposal_fingerprint);
        let governance_attempt_sequence = 0;
        let governance_attempt_id =
            GovernanceAttemptId::derive_v1(proposal_content_id, governance_attempt_sequence);
        let election_attempt_sequence = 0;
        let election_attempt_id = BodyElectionAttemptId::derive_v1(
            governance_attempt_id,
            ParliamentBody::PolicyJury,
            election_attempt_sequence,
        );
        let beacon_session_id = BeaconSessionId::new(root(2));
        let sortition_request = SortitionRequestV1::try_new_canonical(
            governance_attempt_id,
            election_attempt_id,
            ParliamentBody::PolicyJury,
            root(1),
            500,
            500,
            base + 1,
            base + 2,
            beacon_session_id,
            None,
        )
        .expect("canonical validation-fee Policy Jury request");
        let roster_root = root(4);
        let body_instance_id = BodyInstanceId::derive_v1(election_attempt_id, roster_root);
        let ballot_attempt_sequence = 0;
        let ballot_attempt_id =
            BallotAttemptId::derive_v1(body_instance_id, ballot_attempt_sequence);
        let release_beacon_session_id = BeaconSessionId::new(root(7));
        let tle_key_session_id = TleKeySessionId::new(root(8));
        let release_height = base + 7;
        let tle_session_id = TleSessionId::derive_v1(
            ballot_attempt_id,
            tle_key_session_id,
            release_beacon_session_id,
            release_height,
        );
        let opening_root = root(16);
        let tally = ParliamentAggregateTallyV1 {
            original_seats: 500,
            accepted_ballots: 334,
            aye: 200,
            nay: 100,
            abstain: 34,
        };
        let outcome = ParliamentAggregateOutcomeV1::Approved;
        let result_height = base + 8;
        let result_root = parliament_ballot_result_root_v1(
            governance_attempt_id,
            body_instance_id,
            ballot_attempt_id,
            opening_root,
            tally,
            outcome,
            result_height,
        );
        let certified_at_height = base + 9;
        let enact_at_height = base + 10;
        let governance_certificate = GovernanceCertificateV1 {
            proposal_content_id,
            governance_attempt_id,
            governance_attempt_sequence,
            risk_tier: RiskTierV1::Standard,
            body_bindings: vec![ParliamentBodyCertificateBindingV1 {
                body_instance_id,
                election_attempt_id,
                election_attempt_sequence,
                sortition_request_id: sortition_request.id,
                sortition_request,
                body: ParliamentBody::PolicyJury,
                original_seats: tally.original_seats,
                beacon_session_id,
                beacon_pulse_id: BeaconPulseId::new(root(3)),
                roster_root,
                assignment_root: root(5),
                result_root,
                result_height,
                public_finding: None,
                ballot: Some(ParliamentBallotCertificateBindingV1 {
                    ballot_attempt_id,
                    ballot_attempt_sequence,
                    tle_session_id,
                    tle_key_session_id,
                    registration_root: root(9),
                    dropout_root: root(10),
                    survivor_root: root(11),
                    corpus_root: root(12),
                    no_recovery_root: root(13),
                    timed_commitment_root: root(14),
                    release_beacon_session_id,
                    registered_at_height: base + 3,
                    registration_close_height: base + 4,
                    survivor_freeze_height: base + 5,
                    commitment_close_height: base + 6,
                    registration_closed_at_height: base + 4,
                    survivors_frozen_at_height: base + 5,
                    commitment_closed_at_height: base + 6,
                    max_ballot_retries: 3,
                    max_corpus_entries: 1_000,
                    release_height,
                    opening_deadline_height: result_height,
                    release_pulse_id: BeaconPulseId::new(root(15)),
                    opening_height: release_height,
                    opening_root,
                    tally,
                    outcome,
                }),
            }],
            policy_version: 1,
            effect_preimage_hash: root(19),
            expected_head: GovernanceExpectedHeadV1::Present(GovernanceExpectedHeadPresentV1 {
                subject_id: root(17),
                version: 1,
                head_root: root(18),
            }),
            certified_at_height,
            enact_at_height,
        };
        let governance_certificate_id = GovernanceCertificateId::derive_v1(&governance_certificate);
        ValidationFeeParliamentAuthorizationV1 {
            proposal_operator: proposal_operator(),
            proposal_fingerprint,
            governance_certificate_id,
            governance_certificate,
            enacted_at_height: enact_at_height,
        }
    }
    fn policy_effective_height(version: u64) -> u64 {
        version
            .checked_mul(TEST_AUTHORIZATION_STRIDE)
            .and_then(|base| base.checked_add(7))
            .and_then(|enacted| enacted.checked_add(VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS))
            .expect("test policy effective height")
    }
    fn policy(version: u64, previous_policy_hash: Option<[u8; 32]>) -> ValidationFeePolicyV1 {
        ValidationFeePolicyV1 {
            schema_version: VALIDATION_FEE_POLICY_SCHEMA_VERSION,
            network_id: NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
                Hash::prehashed([7; 32]),
            )),
            policy_version: version,
            previous_policy_hash,
            ds_asset_id: fee_asset(),
            ds_scale: VALIDATION_FEE_DS_SCALE,
            fee: initial_validation_fee_amount(),
            treasury_account_id: account(1),
            charging_mode: ValidationFeeChargingMode::PerQualifyingTransferInstruction,
            effective_from_height: policy_effective_height(version),
            expires_after_height: None,
            exemption_classes: Vec::new(),
            treasury_payout_binding: None,
        }
    }
    fn entry(policy: ValidationFeePolicyV1, marker: u8) -> ValidationFeePolicyRegistryEntryV1 {
        let proposal_id =
            validation_fee_policy_proposal_fingerprint(&proposal_operator(), &policy, None);
        ValidationFeePolicyRegistryEntryV1::from_enactment(
            policy,
            authorization(proposal_id, marker),
            None,
        )
        .expect("policy hash")
    }
    struct PayoutLifecyclePolicyFixture {
        policy: ValidationFeePolicyV1,
        lifecycle_seal: [u8; 32],
    }
    fn payout_lifecycle_policy_fixture() -> PayoutLifecyclePolicyFixture {
        let binding = payout_binding();
        let lifecycle_seal = binding.lifecycle_seal().expect("lifecycle seal");
        let mut policy = policy(1, None);
        policy.effective_from_height = authorization([1; 32], 10)
            .enacted_at_height
            .checked_add(VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS)
            .expect("payout policy effective height");
        policy.exemption_classes = vec![VALIDATION_FEE_TREASURY_PAYOUT_EXEMPTION_CLASS.to_owned()];
        policy.treasury_account_id = binding.treasury_account_id.clone();
        policy.treasury_payout_binding = Some(binding);
        PayoutLifecyclePolicyFixture {
            policy,
            lifecycle_seal,
        }
    }
    fn assert_invalid_payout_lifecycle_reference(registry: &ValidationFeePolicyRegistryV1) {
        assert!(matches!(
            registry.validate(),
            Err(
                ValidationFeePolicyRegistryError::InvalidPayoutLifecycleReference {
                    policy_version: 1
                }
            )
        ));
    }
    fn assert_invalid_policy_authorization(registry: &ValidationFeePolicyRegistryV1) {
        assert!(matches!(
            registry.validate(),
            Err(
                ValidationFeePolicyRegistryError::InvalidParliamentAuthorization {
                    policy_version: 1
                }
            )
        ));
    }
    #[test]
    fn validation_fee_identity_hashes_ignore_and_restore_ambient_flags() {
        let policy = policy(1, None);
        let binding = payout_binding();
        let registry = ValidationFeePolicyRegistryV1 {
            registered_policies: vec![entry(policy.clone(), 1)],
        };
        let baseline = (
            policy.policy_hash().expect("policy hash"),
            binding.lifecycle_seal().expect("lifecycle seal"),
            registry.snapshot_hash().expect("registry snapshot hash"),
        );
        let canonical_policy =
            norito::encode_canonical(&policy).expect("encode canonical validation-fee policy");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate_policy = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&policy).expect("encode alternate-layout validation-fee policy")
        };
        assert_ne!(
            alternate_policy, canonical_policy,
            "fixture must exercise a distinct advertised Norito layout"
        );
        let ambient = {
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            let before =
                norito::to_bytes(&policy).expect("encode policy under caller ambient flags");
            let observed = (
                policy.policy_hash().expect("ambient policy hash"),
                binding.lifecycle_seal().expect("ambient lifecycle seal"),
                registry.snapshot_hash().expect("ambient registry hash"),
            );
            let after =
                norito::to_bytes(&policy).expect("re-encode policy under caller ambient flags");
            assert_eq!(
                before, after,
                "canonical identity helpers must restore the caller's ambient layout"
            );
            observed
        };
        assert_eq!(ambient, baseline);
    }
    #[test]
    fn decimal_string_validation_fee_u64_fields_keep_the_full_domain() {
        let mut policy = policy(1, None);
        policy.policy_version = u64::MAX;
        policy.effective_from_height = u64::MAX;
        policy.expires_after_height = Some(u64::MAX);
        let proposal = ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
            proposal_operator: proposal_operator(),
            policy,
            payout_lifecycle_proposal_id: None,
        });
        assert_eq!(
            proposal.first_release_exact_json_u64_invariant_error(),
            None,
            "decimal-string fields do not lose precision in JavaScript JSON runtimes"
        );
    }
    #[test]
    fn validation_fee_policy_roundtrips_canonical_network_id_wire() {
        let policy = policy(1, None);
        let canonical =
            norito::encode_canonical(&policy).expect("encode canonical validation-fee policy");
        let decoded: ValidationFeePolicyV1 =
            norito::decode_canonical(&canonical).expect("decode canonical validation-fee policy");
        assert_eq!(decoded, policy);
        assert_eq!(
            norito::encode_canonical(&decoded).expect("re-encode validation-fee policy"),
            canonical,
            "the mandatory typed NetworkId must have one canonical Norito representation"
        );
        let json = norito::json::to_json(&policy).expect("encode validation-fee policy JSON");
        assert!(json.contains("\"network_id\""));
        assert!(!json.contains("\"chain_id\""));
        assert!(!json.contains("\"genesis_hash\""));
        let decoded_json: ValidationFeePolicyV1 =
            norito::json::from_json(&json).expect("decode validation-fee policy JSON");
        assert_eq!(decoded_json, policy);
        let mut legacy = norito::json::to_value(&policy).expect("encode legacy mutation source");
        let legacy = legacy
            .as_object_mut()
            .expect("validation-fee policy JSON object");
        legacy.remove("network_id");
        legacy.insert("chain_id".into(), norito::json::Value::from("same-label"));
        legacy.insert(
            "genesis_hash".into(),
            norito::json::Value::from(hex::encode([7; 32])),
        );
        let legacy = norito::json::to_json(&legacy).expect("encode legacy identity fields");
        assert!(
            norito::json::from_json::<ValidationFeePolicyV1>(&legacy).is_err(),
            "the first-release decoder must not accept dual ChainId + genesis_hash identity"
        );
    }
    #[test]
    fn validation_fee_policy_json_requires_explicit_optional_and_list_fields() {
        let policy = policy(1, None);
        for field in [
            "previous_policy_hash",
            "expires_after_height",
            "exemption_classes",
        ] {
            let mut value =
                norito::json::to_value(&policy).expect("encode validation-fee policy JSON value");
            value
                .as_object_mut()
                .expect("validation-fee policy JSON object")
                .remove(field);
            let value =
                norito::json::to_json(&value).expect("encode incomplete validation-fee policy");
            assert!(
                norito::json::from_json::<ValidationFeePolicyV1>(&value).is_err(),
                "missing `{field}` must be rejected"
            );
        }
    }
    #[test]
    fn validation_fee_nested_json_types_reject_unknown_fields() {
        let mut recipient = norito::json::to_value(
            payout_binding()
                .recipients
                .first()
                .expect("payout recipient fixture"),
        )
        .expect("encode payout recipient JSON value");
        recipient
            .as_object_mut()
            .expect("payout recipient JSON object")
            .insert("legacy_weight".into(), norito::json::Value::from("0.25"));
        let recipient =
            norito::json::to_json(&recipient).expect("encode payout recipient unknown field");
        assert!(
            norito::json::from_json::<ValidationFeeTreasuryPayoutRecipientV1>(&recipient).is_err()
        );

        let mut charging_mode =
            norito::json::to_value(&ValidationFeeChargingMode::PerQualifyingTransferInstruction)
                .expect("encode charging-mode JSON value");
        charging_mode
            .as_object_mut()
            .expect("charging-mode JSON object")
            .insert(
                "legacy_mode".into(),
                norito::json::Value::from("PER_TRANSFER"),
            );
        let charging_mode =
            norito::json::to_json(&charging_mode).expect("encode charging-mode unknown field");
        assert!(norito::json::from_json::<ValidationFeeChargingMode>(&charging_mode).is_err());
    }
    #[test]
    fn payout_binding_roundtrips_canonical_ds_fields() {
        let binding = payout_binding();
        let canonical =
            norito::encode_canonical(&binding).expect("encode canonical payout binding");
        let decoded: ValidationFeeTreasuryPayoutBindingV1 =
            norito::decode_canonical(&canonical).expect("decode canonical payout binding");
        assert_eq!(decoded, binding);
        assert_eq!(
            norito::encode_canonical(&decoded).expect("re-encode canonical payout binding"),
            canonical,
            "the payout binding must have one canonical Norito representation"
        );

        let json = norito::json::to_json(&binding).expect("encode payout binding JSON");
        assert!(json.contains(r#""ds_asset_id""#));
        assert!(json.contains(r#""batch_ds""#));
        assert!(!json.contains(r#""sbd_asset_id""#));
        assert!(!json.contains(r#""batch_sbd""#));
        let decoded_json: ValidationFeeTreasuryPayoutBindingV1 =
            norito::json::from_json(&json).expect("decode payout binding JSON");
        assert_eq!(decoded_json, binding);

        let mut retired = norito::json::to_value(&binding)
            .expect("encode retired payout binding mutation source");
        let retired = retired.as_object_mut().expect("payout binding JSON object");
        let ds_asset_id = retired.get("ds_asset_id").expect("DS asset field").clone();
        retired.insert("sbd_asset_id".into(), ds_asset_id);
        let retired = norito::json::to_json(&retired).expect("encode retired payout binding JSON");
        assert!(
            norito::json::from_json::<ValidationFeeTreasuryPayoutBindingV1>(&retired).is_err(),
            "the first-release decoder must not preserve the retired SBD field name"
        );
    }
    #[test]
    fn lightweight_validation_fee_fingerprints_match_governance_proposal_bytes() {
        let payout_binding = payout_binding();
        let proposal_operator = proposal_operator();
        let lifecycle_governance =
            ProposalKind::ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleProposal {
                proposal_operator: proposal_operator.clone(),
                payout_binding: payout_binding.clone(),
            });
        let lifecycle_lightweight =
            ValidationFeePayoutLifecycleProposalFingerprintEnvelopeV1::ValidationFeePayoutLifecycle(
                ValidationFeePayoutLifecycleFingerprintPayloadV1 {
                    proposal_operator: proposal_operator.clone(),
                    payout_binding: payout_binding.clone(),
                },
            );
        assert_eq!(
            lifecycle_lightweight.encode(),
            lifecycle_governance.encode()
        );
        assert_eq!(
            validation_fee_payout_lifecycle_proposal_fingerprint(
                &proposal_operator,
                &payout_binding,
            ),
            lifecycle_governance.fingerprint()
        );
        let lifecycle_id = lifecycle_governance.fingerprint();
        let mut governed_policy = policy(1, None);
        governed_policy.treasury_payout_binding = Some(payout_binding);
        for payout_lifecycle_proposal_id in [None, Some(lifecycle_id)] {
            let policy_governance =
                ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
                    proposal_operator: proposal_operator.clone(),
                    policy: governed_policy.clone(),
                    payout_lifecycle_proposal_id,
                });
            let policy_lightweight =
                ValidationFeePolicyProposalFingerprintEnvelopeV1::ValidationFeePolicy(
                    ValidationFeePolicyFingerprintPayloadV1 {
                        proposal_operator: proposal_operator.clone(),
                        policy: governed_policy.clone(),
                        payout_lifecycle_proposal_id,
                    },
                );
            assert_eq!(policy_lightweight.encode(), policy_governance.encode());
            assert_eq!(
                validation_fee_policy_proposal_fingerprint(
                    &proposal_operator,
                    &governed_policy,
                    payout_lifecycle_proposal_id,
                ),
                policy_governance.fingerprint()
            );
        }
    }
    #[test]
    fn lightweight_validation_fee_preimages_use_frozen_v1_tags() {
        let policy = ValidationFeePolicyProposalFingerprintEnvelopeV1::ValidationFeePolicy(
            ValidationFeePolicyFingerprintPayloadV1 {
                proposal_operator: proposal_operator(),
                policy: policy(1, None),
                payout_lifecycle_proposal_id: None,
            },
        );
        let lifecycle =
            ValidationFeePayoutLifecycleProposalFingerprintEnvelopeV1::ValidationFeePayoutLifecycle(
                ValidationFeePayoutLifecycleFingerprintPayloadV1 {
                    proposal_operator: proposal_operator(),
                    payout_binding: payout_binding(),
                },
            );
        assert_eq!(
            policy.encode().get(..4),
            Some(3_u32.to_le_bytes().as_slice())
        );
        assert_eq!(
            lifecycle.encode().get(..4),
            Some(4_u32.to_le_bytes().as_slice())
        );
    }
    #[test]
    fn registry_retains_history_and_selects_scheduled_policy() {
        let first = policy(1, None);
        let first_entry = entry(first, 1);
        let second = policy(2, Some(first_entry.policy_hash));
        let first_effective_height = first_entry.policy.effective_from_height;
        let second_effective_height = second.effective_from_height;
        let registry = ValidationFeePolicyRegistryV1 {
            registered_policies: vec![first_entry, entry(second, 2)],
        };
        registry.validate().expect("valid policy chain");
        assert!(
            registry
                .scheduled_entry_at_height(first_effective_height - 1)
                .is_none()
        );
        assert_eq!(
            registry
                .effective_entry_at_height(first_effective_height)
                .expect("first policy")
                .policy
                .policy_version,
            1
        );
        assert_eq!(
            registry
                .effective_entry_at_height(second_effective_height)
                .expect("successor policy")
                .policy
                .policy_version,
            2
        );
    }
    #[test]
    fn registry_rejects_successor_from_another_exact_network() {
        let first = policy(1, None);
        let first_entry = entry(first, 1);
        let second = policy(2, Some(first_entry.policy_hash));
        let mut registry = ValidationFeePolicyRegistryV1 {
            registered_policies: vec![first_entry, entry(second, 2)],
        };
        registry.registered_policies[1].policy.network_id = NetworkId::from_genesis_hash(
            iroha_crypto::HashOf::from_untyped_unchecked(Hash::prehashed([9; 32])),
        );
        assert!(matches!(
            registry.validate(),
            Err(ValidationFeePolicyRegistryError::NetworkIdentityChanged { policy_version: 2 })
        ));
    }
    #[test]
    fn registry_rejects_stale_predecessor() {
        let first = entry(policy(1, None), 1);
        let second = policy(2, Some([9; 32]));
        let registry = ValidationFeePolicyRegistryV1 {
            registered_policies: vec![first, entry(second, 2)],
        };
        assert!(matches!(
            registry.validate(),
            Err(ValidationFeePolicyRegistryError::BrokenPreviousPolicyHash { policy_version: 2 })
        ));
    }
    #[test]
    fn disabled_policy_is_explicit_and_zero_fee() {
        let mut disabled = policy(1, None);
        disabled.charging_mode = ValidationFeeChargingMode::Disabled;
        disabled.fee = Quantity::zero();
        assert_eq!(disabled.policy_invariant_error(), None);
        disabled.fee = initial_validation_fee_amount();
        assert_eq!(
            disabled.policy_invariant_error(),
            Some("disabled validation-fee policy amount must be zero")
        );
    }
    #[test]
    fn enabled_policy_fee_is_exactly_ten_cents() {
        let mut enabled = policy(1, None);
        assert_eq!(enabled.policy_invariant_error(), None);
        for malformed_fee in ["0.09", "0.11", "10"] {
            enabled.fee = malformed_fee.parse().expect("quantity");
            assert_eq!(
                enabled.policy_invariant_error(),
                Some("enabled validation-fee policy amount must be exactly 0.10 DS")
            );
        }
    }
    #[test]
    fn retired_and_live_parameter_ids_are_reserved() {
        for raw in [
            RETIRED_VALIDATION_FEE_GOVERNANCE_KEYSET_PARAMETER_ID,
            RETIRED_VALIDATION_FEE_POLICY_PARAMETER_ID,
            ValidationFeePolicyRegistryV1::PARAMETER_ID_STR,
        ] {
            let id: CustomParameterId = raw.parse().expect("parameter id");
            assert!(is_reserved_validation_fee_parameter_id(&id));
        }
    }
    #[test]
    fn payout_binding_accepts_only_the_release_constants() {
        let binding = payout_binding();
        assert_eq!(binding.invariant_error(), None);
        for malformed in [
            {
                let mut value = binding.clone();
                value.batch_ds = "9.99".parse().expect("quantity");
                value
            },
            {
                let mut value = binding.clone();
                value.batch_ds = "10.01".parse().expect("quantity");
                value
            },
            {
                let mut value = binding.clone();
                value.min_xor_out = "3".parse().expect("quantity");
                value
            },
            {
                let mut value = binding.clone();
                value.max_xor_out = "101".parse().expect("quantity");
                value
            },
            {
                let mut value = binding.clone();
                value.recipients[0].share = "0.24".parse().expect("numeric");
                value
            },
            {
                let mut value = binding.clone();
                value.recipients[1].account_id = value.recipients[0].account_id.clone();
                value
            },
            {
                let mut value = binding.clone();
                value.recipients.pop();
                value
            },
        ] {
            assert!(
                malformed.invariant_error().is_some(),
                "malformed payout binding must be rejected"
            );
        }
    }
    #[test]
    fn lifecycle_seal_and_fingerprint_bind_exact_binding_and_operator() {
        let binding = payout_binding();
        let proposal_operator = proposal_operator();
        let seal = binding.lifecycle_seal().expect("lifecycle seal");
        assert_ne!(seal, [0; 32]);
        let proposal_fingerprint =
            validation_fee_payout_lifecycle_proposal_fingerprint(&proposal_operator, &binding);
        let mut changed_binding = binding.clone();
        changed_binding.code_hash[0] ^= 1;
        let changed_seal = changed_binding
            .lifecycle_seal()
            .expect("changed lifecycle seal");
        let changed_fingerprint = validation_fee_payout_lifecycle_proposal_fingerprint(
            &proposal_operator,
            &changed_binding,
        );
        assert_ne!(seal, changed_seal);
        assert_ne!(proposal_fingerprint, changed_fingerprint);
        assert_ne!(
            proposal_fingerprint,
            validation_fee_payout_lifecycle_proposal_fingerprint(&account(8), &binding),
            "proposal operator must be part of the exact typed preimage"
        );
    }
    #[test]
    fn payout_policy_requires_matching_enacted_lifecycle_reference() {
        let PayoutLifecyclePolicyFixture {
            policy: payout_policy,
            lifecycle_seal: seal,
        } = payout_lifecycle_policy_fixture();
        let missing = ValidationFeePolicyRegistryV1 {
            registered_policies: vec![
                ValidationFeePolicyRegistryEntryV1::from_enactment(
                    payout_policy.clone(),
                    authorization([0x10; 32], 10),
                    None,
                )
                .expect("registry entry"),
            ],
        };
        assert_invalid_payout_lifecycle_reference(&missing);
        let mut bad_seal = seal;
        bad_seal[0] ^= 1;
        let mismatched = ValidationFeePolicyRegistryV1 {
            registered_policies: vec![
                ValidationFeePolicyRegistryEntryV1::from_enactment(
                    payout_policy.clone(),
                    authorization([0x10; 32], 10),
                    Some(ValidationFeePayoutLifecycleReferenceV1 {
                        lifecycle_seal: bad_seal,
                        parliament_authorization: authorization([0x11; 32], 11),
                    }),
                )
                .expect("registry entry"),
            ],
        };
        assert_invalid_payout_lifecycle_reference(&mismatched);
        let binding = payout_policy
            .treasury_payout_binding
            .as_ref()
            .expect("payout binding");
        let lifecycle_id =
            validation_fee_payout_lifecycle_proposal_fingerprint(&proposal_operator(), binding);
        let policy_id = validation_fee_policy_proposal_fingerprint(
            &proposal_operator(),
            &payout_policy,
            Some(lifecycle_id),
        );
        let valid = ValidationFeePolicyRegistryV1 {
            registered_policies: vec![
                ValidationFeePolicyRegistryEntryV1::from_enactment(
                    payout_policy,
                    authorization(policy_id, 10),
                    Some(ValidationFeePayoutLifecycleReferenceV1 {
                        lifecycle_seal: seal,
                        parliament_authorization: authorization(lifecycle_id, 9),
                    }),
                )
                .expect("registry entry"),
            ],
        };
        valid.validate().expect("exact typed proposal fingerprints");
        let mut rebound_operator = valid.clone();
        rebound_operator.registered_policies[0]
            .parliament_authorization
            .proposal_operator = account(8);
        assert_invalid_policy_authorization(&rebound_operator);
        let mut rebound_lifecycle = valid.clone();
        let lifecycle_authorization = &mut rebound_lifecycle.registered_policies[0]
            .payout_lifecycle
            .as_mut()
            .expect("lifecycle")
            .parliament_authorization;
        lifecycle_authorization.proposal_fingerprint = [0xA1; 32];
        assert_invalid_payout_lifecycle_reference(&rebound_lifecycle);
        let mut rebound_policy = valid;
        let policy_authorization =
            &mut rebound_policy.registered_policies[0].parliament_authorization;
        policy_authorization.proposal_fingerprint = [0xA2; 32];
        assert_invalid_policy_authorization(&rebound_policy);
    }
    #[test]
    fn parliament_authorization_requires_exact_certificate_identity_and_due_height() {
        let valid = authorization([0x12; 32], 12);
        assert_eq!(valid.invariant_error(), None);
        assert_eq!(
            valid.governance_certificate.proposal_content_id,
            ProposalContentId::new(valid.proposal_fingerprint)
        );
        let encoded = norito::to_bytes(&valid).expect("encode certificate authorization");
        let decoded = norito::decode_from_bytes::<ValidationFeeParliamentAuthorizationV1>(&encoded)
            .expect("decode certificate authorization");
        assert_eq!(decoded, valid);
        assert_eq!(decoded.invariant_error(), None);
        let mut wrong_certificate_id = valid.clone();
        wrong_certificate_id.governance_certificate_id = GovernanceCertificateId::new([0xAA; 32]);
        assert_eq!(
            wrong_certificate_id.invariant_error(),
            Some(
                "validation-fee Parliament certificate identifier is not the canonical certificate hash"
            )
        );
        let mut wrong_proposal = authorization([0x13; 32], 13);
        wrong_proposal.proposal_fingerprint = valid.proposal_fingerprint;
        assert_eq!(
            wrong_proposal.invariant_error(),
            Some("validation-fee Parliament certificate targets a different proposal fingerprint")
        );
        let mut wrong_due_height = valid.clone();
        wrong_due_height.enacted_at_height = wrong_due_height.enacted_at_height.saturating_add(1);
        assert_eq!(
            wrong_due_height.invariant_error(),
            Some(
                "validation-fee enactment height must equal the Parliament certificate due height"
            )
        );
        let mut invalid_certificate = valid;
        invalid_certificate.governance_certificate.body_bindings[0].result_root = [0; 32];
        assert_eq!(
            invalid_certificate.invariant_error(),
            Some("validation-fee Parliament certificate is structurally invalid")
        );
    }
}
#[cfg(test)]
mod snapshot_tests {
    use super::*;
    fn witness_root(proof: &ValidationFeePolicyWitnessProofV1) -> Hash {
        let path = Hash::new(&proof.key);
        let value_hash = Hash::new(&proof.value);
        let mut leaf_preimage = Vec::with_capacity(1 + 2 * Hash::LENGTH);
        leaf_preimage.push(0);
        leaf_preimage.extend_from_slice(path.as_ref());
        leaf_preimage.extend_from_slice(value_hash.as_ref());
        let mut current = Hash::new(leaf_preimage);
        for (level, sibling) in proof.siblings.iter().copied().enumerate() {
            let path_bit = 255_usize.saturating_sub(level);
            let byte = path.as_ref()[path_bit / 8];
            let right = byte & (1_u8 << (path_bit % 8)) != 0;
            current = if right {
                validation_fee_ordinary_smt_node_hash(sibling, current)
            } else {
                validation_fee_ordinary_smt_node_hash(current, sibling)
            };
        }
        current
    }
    #[test]
    fn witness_commitment_rejects_alternate_norito_layout() {
        let commitment = ValidationFeePolicySnapshotCommitmentV1::from_registry(17, None);
        let canonical = norito::encode_canonical(&commitment).expect("encode canonical commitment");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&commitment).expect("encode alternate-layout commitment")
        };
        assert_ne!(
            alternate, canonical,
            "fixture must exercise a distinct advertised Norito layout"
        );
        norito::decode_from_bytes::<ValidationFeePolicySnapshotCommitmentV1>(&alternate)
            .expect("ordinary Norito accepts the advertised alternate layout");
        let canonical_proof = ValidationFeePolicyWitnessProofV1 {
            key: VALIDATION_FEE_POLICY_WITNESS_KEY_V1.to_vec(),
            value: canonical,
            siblings: vec![
                Hash::new(b"validation-fee witness sibling");
                VALIDATION_FEE_POLICY_WITNESS_SIBLINGS_V1
            ],
        };
        assert_eq!(
            canonical_proof.commitment().expect("canonical commitment"),
            commitment
        );
        assert!(canonical_proof.verify(witness_root(&canonical_proof)));
        let alternate_proof = ValidationFeePolicyWitnessProofV1 {
            value: alternate,
            ..canonical_proof
        };
        assert_eq!(
            alternate_proof
                .commitment()
                .expect_err("alternate-layout commitment must fail"),
            "validation-fee snapshot commitment is non-canonical"
        );
        assert!(!alternate_proof.verify(witness_root(&alternate_proof)));
    }
    #[test]
    fn snapshot_identity_encoding_ignores_and_restores_ambient_flags() {
        let commitment = ValidationFeePolicySnapshotCommitmentV1::from_registry(17, None);
        let canonical = norito::encode_canonical(&commitment).expect("encode canonical commitment");
        let malformed_parameter = CustomParameter::new(
            ValidationFeePolicyRegistryV1::parameter_id(),
            Json::new("not a validation-fee registry"),
        );
        let baseline_invalid = ValidationFeePolicySnapshotCommitmentV1::from_custom_parameter_state(
            19,
            Some(&malformed_parameter),
        );
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let (ambient_canonical, ambient_invalid) = {
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            let before = norito::to_bytes(&commitment)
                .expect("encode commitment under caller ambient flags");
            let encoded = norito::encode_canonical(&commitment)
                .expect("canonicalize commitment under caller ambient flags");
            let invalid = ValidationFeePolicySnapshotCommitmentV1::from_custom_parameter_state(
                19,
                Some(&malformed_parameter),
            );
            let after = norito::to_bytes(&commitment)
                .expect("re-encode commitment under caller ambient flags");
            assert_eq!(
                before, after,
                "canonical helpers must restore the caller's ambient layout"
            );
            (encoded, invalid)
        };
        assert_eq!(ambient_canonical, canonical);
        assert_eq!(ambient_invalid, baseline_invalid);
    }
    #[test]
    fn snapshot_constructors_fail_closed_for_absent_and_malformed_registry() {
        let unconfigured = ValidationFeePolicySnapshotCommitmentV1::from_registry(17, None);
        assert_eq!(
            unconfigured.version,
            VALIDATION_FEE_POLICY_SNAPSHOT_VERSION_V1
        );
        assert_eq!(unconfigured.evaluated_height, 17);
        assert!(matches!(
            unconfigured.status,
            ValidationFeePolicySnapshotStatusV1::Unconfigured
        ));
        let empty_registry = ValidationFeePolicyRegistryV1 {
            registered_policies: Vec::new(),
        };
        let invalid_registry =
            ValidationFeePolicySnapshotCommitmentV1::from_registry(18, Some(&empty_registry));
        assert!(matches!(
            invalid_registry.status,
            ValidationFeePolicySnapshotStatusV1::Invalid(_)
        ));
        let malformed_parameter = CustomParameter::new(
            ValidationFeePolicyRegistryV1::parameter_id(),
            Json::new("not a validation-fee registry"),
        );
        let first = ValidationFeePolicySnapshotCommitmentV1::from_custom_parameter_state(
            19,
            Some(&malformed_parameter),
        );
        let second = ValidationFeePolicySnapshotCommitmentV1::from_custom_parameter_state(
            19,
            Some(&malformed_parameter),
        );
        assert_eq!(
            first, second,
            "malformed-state commitment must be deterministic"
        );
        assert!(matches!(
            first.status,
            ValidationFeePolicySnapshotStatusV1::Invalid(_)
        ));
    }
}
