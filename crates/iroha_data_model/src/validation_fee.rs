//! Validation-fee policy data shared by validators and clients.

use std::collections::BTreeSet;

use iroha_crypto::{
    Hash,
    blake2::{
        Blake2bVar,
        digest::{Update, VariableOutput},
    },
};
use iroha_primitives::{
    json::Json,
    numeric::{Numeric, Quantity},
};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    ChainId, Level,
    account::AccountId,
    asset::AssetDefinitionId,
    isi::{InstructionBox, Log},
    name::Name,
    parameter::{CustomParameter, CustomParameterId},
    smart_contract::ContractAddress,
};

/// Schema version for the initial validation-fee policy.
pub const VALIDATION_FEE_POLICY_SCHEMA_VERSION: u16 = 1;
/// Decimal scale required for the initial policy fee asset.
pub const VALIDATION_FEE_DS_SCALE: u8 = 2;
/// Canonical fee amount required by the initial validation-fee policy (0.10 SBD).
pub const VALIDATION_FEE_INITIAL_AMOUNT: &str = "0.10";
/// Minimum delay from Parliament enactment to policy activation.
pub const VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS: u64 = 120_960;
/// Exact SBD batch consumed by every validation-fee payout lifecycle tick.
pub const VALIDATION_FEE_PAYOUT_BATCH_SBD: &str = "10";
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
/// Maximum number of citizens admitted to a first-release validation-fee PLAIN roster.
pub const VALIDATION_FEE_PLAIN_MAX_MEMBERS_V1: u64 = 256;
/// Domain separator for a canonical validation-fee PLAIN electorate snapshot root.
pub const VALIDATION_FEE_PLAIN_ELECTORATE_SNAPSHOT_ROOT_DOMAIN_V1: &[u8] =
    b"iroha.validation_fee.plain_electorate.snapshot.v1";
/// Domain separator for policy hashing.
pub const VALIDATION_FEE_POLICY_HASH_DOMAIN: &[u8] = b"iroha.validation_fee.policy.parliament.v1";
/// Canonical domain separator shared by all V1 governance proposal fingerprints.
///
/// This remains available without the `governance` feature because validation-fee
/// registry authorization must reproduce the exact fingerprint of the governing
/// proposal in lightweight data-model builds.
pub(crate) const GOVERNANCE_PROPOSAL_FINGERPRINT_DOMAIN_V1: &[u8] = b"gov:proposal:v1";
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
/// These parameters are enacted atomically by SORA Parliament and must never be
/// writable through the generic `SetParameter` instruction, including at
/// genesis.
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
    /// A successor changed the immutable chain or genesis binding.
    ChainIdentityChanged {
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
            Self::ChainIdentityChanged { policy_version } => write!(
                f,
                "validation-fee policy changes the immutable chain identity at version {policy_version}"
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
    /// Disable validation-fee charging through the governed policy chain.
    Disabled,
    /// Charge once per qualifying fee-asset transfer instruction or batch entry.
    PerQualifyingTransferInstruction,
}

/// Voting mode retained with validation-fee referendum finalization evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(
    tag = "voting_mode",
    content = "value",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum ValidationFeeGovernanceVotingModeV1 {
    /// Zero-knowledge referendum tally.
    Zk,
    /// Plain referendum tally.
    Plain,
}

/// Closed first-release eligibility rule for validation-fee PLAIN referenda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(
    feature = "json",
    norito(
        tag = "rule",
        content = "value",
        rename_all = "snake_case",
        deny_unknown_fields
    )
)]
pub enum ValidationFeePlainElectorateEligibilityRuleV1 {
    /// The proposal operator is eligible at or before the roster gate; every
    /// other citizen must join strictly after that gate.
    #[codec(index = 0)]
    ProposalOperatorAtOrBeforeGateOthersAfterGate,
}

/// Exact PLAIN electorate contract committed by a validation-fee proposal.
///
/// These fields are part of the proposal fingerprint and remain retained with
/// enacted registry entries. Validators must therefore verify historical
/// authorization from this immutable payload rather than mutable live config.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct ValidationFeePlainElectorateRulesV1 {
    /// Asset definition whose locked balance supplies PLAIN ballot weight.
    pub voting_asset_id: AssetDefinitionId,
    /// Proposal-bound escrow account that holds every PLAIN ballot lock.
    pub bond_escrow_account: AccountId,
    /// Proposal-bound account that receives any governance lock slash.
    pub slash_receiver_account: AccountId,
    /// Exact amount locked by every eligible ballot.
    pub ballot_amount: Quantity,
    /// Exact inclusive ballot duration in blocks.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub ballot_duration_blocks: u64,
    /// Exact citizenship bond required for electorate membership.
    pub citizenship_amount: Quantity,
    /// Maximum number of citizens frozen into the eligible roster.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub max_members: u64,
    /// Number of locked blocks per additional PLAIN conviction step.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub conviction_step_blocks: u64,
    /// Maximum PLAIN conviction multiplier.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub max_conviction: u64,
    /// Minimum final turnout required for approval.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u128_string"))]
    pub min_turnout: u128,
    /// Approval-fraction numerator.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub approval_threshold_numerator: u64,
    /// Approval-fraction denominator.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub approval_threshold_denominator: u64,
    /// Closed proposal-time citizen eligibility rule.
    pub eligibility_rule: ValidationFeePlainElectorateEligibilityRuleV1,
}

impl ValidationFeePlainElectorateRulesV1 {
    /// Return a stable invariant violation, if any.
    #[must_use]
    pub fn invariant_error(&self) -> Option<&'static str> {
        if self.ballot_amount.is_zero() || self.ballot_amount.scale() != 0 {
            return Some("validation-fee PLAIN ballot amount must be a positive exact integer");
        }
        if self.ballot_duration_blocks == 0 {
            return Some("validation-fee PLAIN ballot duration must be positive");
        }
        if self.citizenship_amount.is_zero() || self.citizenship_amount.scale() != 0 {
            return Some(
                "validation-fee PLAIN citizenship amount must be a positive exact integer",
            );
        }
        if self.max_members == 0 || self.max_members > VALIDATION_FEE_PLAIN_MAX_MEMBERS_V1 {
            return Some(
                "validation-fee PLAIN electorate member cap must be within the first-release maximum",
            );
        }
        if self.conviction_step_blocks == 0 || self.max_conviction == 0 {
            return Some("validation-fee PLAIN conviction step and maximum must both be positive");
        }
        if self.min_turnout == 0 {
            return Some("validation-fee PLAIN minimum turnout must be positive");
        }
        if self.approval_threshold_numerator == 0
            || self.approval_threshold_denominator == 0
            || self.approval_threshold_numerator > self.approval_threshold_denominator
        {
            return Some(
                "validation-fee PLAIN approval threshold must be a non-zero fraction no greater than one",
            );
        }
        None
    }
}

/// One citizen frozen into a validation-fee PLAIN electorate snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct ValidationFeePlainElectorateMemberV1 {
    /// Canonical citizen account.
    pub account_id: AccountId,
    /// Height at which the uninterrupted citizenship bond began.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub bonded_height: u64,
    /// Exact citizenship amount observed at the snapshot boundary.
    pub bonded_amount: Quantity,
}

#[derive(Encode)]
struct ValidationFeePlainElectorateSnapshotRootPayloadV1 {
    proposal_id: [u8; 32],
    proposal_operator: AccountId,
    captured_at_height: u64,
    approval_gate_height: u64,
    member_count: u64,
    members: Vec<ValidationFeePlainElectorateMemberV1>,
}

/// Canonical citizen roster frozen immediately before the referendum's inclusive start block.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
pub struct ValidationFeePlainElectorateSnapshotV1 {
    /// Native proposal identifier whose retained rules govern this roster.
    pub proposal_id: [u8; 32],
    /// Proposal operator receiving the closed first-release eligibility exception.
    pub proposal_operator: AccountId,
    /// Exact referendum start height at whose boundary the roster was frozen.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub captured_at_height: u64,
    /// First height at which all seven Parliament bodies held approval quorum.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub approval_gate_height: u64,
    /// Exact number of canonically ordered members.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub member_count: u64,
    /// Canonically ordered, duplicate-free electorate.
    pub members: Vec<ValidationFeePlainElectorateMemberV1>,
    /// Domain-separated commitment to the complete snapshot payload.
    pub roster_root: [u8; 32],
}

impl ValidationFeePlainElectorateSnapshotV1 {
    /// Construct a snapshot from an already canonical member vector.
    ///
    /// # Errors
    ///
    /// Returns a stable invariant reason when the vector is empty, oversized,
    /// unordered, duplicated, ineligible by height, or cannot be encoded.
    pub fn from_canonical_members(
        proposal_id: [u8; 32],
        proposal_operator: AccountId,
        captured_at_height: u64,
        approval_gate_height: u64,
        members: Vec<ValidationFeePlainElectorateMemberV1>,
    ) -> Result<Self, &'static str> {
        let member_count = u64::try_from(members.len())
            .map_err(|_| "validation-fee PLAIN electorate member count overflows u64")?;
        let mut snapshot = Self {
            proposal_id,
            proposal_operator,
            captured_at_height,
            approval_gate_height,
            member_count,
            members,
            roster_root: [0; 32],
        };
        snapshot.roster_root = snapshot.checked_roster_root().map_err(
            |_| "validation-fee PLAIN electorate snapshot cannot be canonically encoded",
        )?;
        if let Some(reason) = snapshot.invariant_error() {
            return Err(reason);
        }
        Ok(snapshot)
    }

    /// Recompute the domain-separated root of the complete snapshot payload.
    ///
    /// # Errors
    ///
    /// Returns a Norito encoding error when the typed payload cannot be serialized.
    pub fn checked_roster_root(&self) -> Result<[u8; 32], norito::Error> {
        let encoded =
            norito::encode_canonical(&ValidationFeePlainElectorateSnapshotRootPayloadV1 {
                proposal_id: self.proposal_id,
                proposal_operator: self.proposal_operator.clone(),
                captured_at_height: self.captured_at_height,
                approval_gate_height: self.approval_gate_height,
                member_count: self.member_count,
                members: self.members.clone(),
            })?;
        let mut preimage = Vec::with_capacity(
            VALIDATION_FEE_PLAIN_ELECTORATE_SNAPSHOT_ROOT_DOMAIN_V1.len() + 1 + encoded.len(),
        );
        preimage.extend_from_slice(VALIDATION_FEE_PLAIN_ELECTORATE_SNAPSHOT_ROOT_DOMAIN_V1);
        preimage.push(0);
        preimage.extend_from_slice(&encoded);
        Ok(*Hash::new(preimage).as_ref())
    }

    /// Return a stable intrinsic invariant violation, if any.
    #[must_use]
    pub fn invariant_error(&self) -> Option<&'static str> {
        if self.proposal_id == [0; 32] {
            return Some("validation-fee PLAIN electorate proposal id must be non-zero");
        }
        if self.captured_at_height == 0 || self.approval_gate_height >= self.captured_at_height {
            return Some(
                "validation-fee PLAIN electorate gate must precede its positive capture height",
            );
        }
        if self.member_count == 0
            || self.member_count > VALIDATION_FEE_PLAIN_MAX_MEMBERS_V1
            || usize::try_from(self.member_count).ok() != Some(self.members.len())
        {
            return Some(
                "validation-fee PLAIN electorate count must exactly match a non-empty bounded roster",
            );
        }
        let mut previous: Option<&AccountId> = None;
        for member in &self.members {
            if previous.is_some_and(|account_id| account_id >= &member.account_id) {
                return Some(
                    "validation-fee PLAIN electorate members must be strictly canonically ordered",
                );
            }
            if member.bonded_amount.is_zero() || member.bonded_amount.scale() != 0 {
                return Some(
                    "validation-fee PLAIN electorate member bond must be a positive exact integer",
                );
            }
            let eligible_height = if member.account_id == self.proposal_operator {
                member.bonded_height <= self.approval_gate_height
            } else {
                member.bonded_height > self.approval_gate_height
                    && member.bonded_height < self.captured_at_height
            };
            if !eligible_height {
                return Some(
                    "validation-fee PLAIN electorate member joined outside the frozen eligibility interval",
                );
            }
            previous = Some(&member.account_id);
        }
        if self.roster_root == [0; 32] || self.checked_roster_root().ok() != Some(self.roster_root)
        {
            return Some("validation-fee PLAIN electorate snapshot root is invalid");
        }
        None
    }

    /// Return a stable proposal/rules binding violation, if any.
    #[must_use]
    pub fn context_error(
        &self,
        proposal_id: [u8; 32],
        proposal_operator: &AccountId,
        rules: &ValidationFeePlainElectorateRulesV1,
    ) -> Option<&'static str> {
        if let Some(reason) = self.invariant_error() {
            return Some(reason);
        }
        if self.proposal_id != proposal_id || &self.proposal_operator != proposal_operator {
            return Some(
                "validation-fee PLAIN electorate snapshot targets a different proposal or operator",
            );
        }
        if self.member_count > rules.max_members {
            return Some(
                "validation-fee PLAIN electorate snapshot exceeds the proposal-bound member cap",
            );
        }
        if self
            .members
            .iter()
            .any(|member| member.bonded_amount < rules.citizenship_amount)
        {
            return Some(
                "validation-fee PLAIN electorate member is below the proposal-bound citizenship amount",
            );
        }
        if self.members.iter().any(|member| {
            member.account_id == rules.bond_escrow_account
                || member.account_id == rules.slash_receiver_account
        }) {
            return Some(
                "validation-fee PLAIN custody accounts cannot belong to the voting electorate",
            );
        }
        None
    }

    /// Return whether the canonical snapshot contains `account_id`.
    #[must_use]
    pub fn contains(&self, account_id: &AccountId) -> bool {
        self.members
            .binary_search_by(|member| member.account_id.cmp(account_id))
            .is_ok()
    }
}

/// Exact inclusive referendum window authorized for a validation-fee proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidationFeeGovernanceWindowV1 {
    /// First height in the authorized window.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub lower: u64,
    /// Last height in the authorized window.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub upper: u64,
}

/// Typed deterministic referendum result retained in the validation-fee registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidationFeeFinalizationEvidenceV1 {
    /// Referendum identifier, equal to the native proposal identifier.
    pub referendum_id: [u8; 32],
    /// Height at which the result was finalized.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub finalized_at_height: u64,
    /// Voting mode whose tally was finalized.
    pub mode: ValidationFeeGovernanceVotingModeV1,
    /// Final approve weight.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u128_string"))]
    pub approve: u128,
    /// Final reject weight.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u128_string"))]
    pub reject: u128,
    /// Final abstain weight.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u128_string"))]
    pub abstain: u128,
    /// Minimum turnout applied to this result.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u128_string"))]
    pub min_turnout: u128,
    /// Approval-threshold numerator applied to this result.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub approval_threshold_numerator: u64,
    /// Approval-threshold denominator applied to this result.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub approval_threshold_denominator: u64,
    /// Final deterministic decision.
    pub approved: bool,
}

impl ValidationFeeFinalizationEvidenceV1 {
    /// Recompute the approval decision encoded by this evidence.
    #[must_use]
    pub fn recomputed_approval(&self) -> bool {
        if self.approval_threshold_denominator == 0 {
            return false;
        }
        let Some(turnout) = self
            .approve
            .checked_add(self.reject)
            .and_then(|value| value.checked_add(self.abstain))
        else {
            return false;
        };
        let Some(approve) = self
            .approve
            .checked_mul(u128::from(self.approval_threshold_denominator))
        else {
            return false;
        };
        let Some(required) = turnout.checked_mul(u128::from(self.approval_threshold_numerator))
        else {
            return false;
        };
        turnout >= self.min_turnout && approve >= required
    }
}

/// Typed Parliament and referendum authorization for one enacted policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidationFeeParliamentAuthorizationV1 {
    /// Native proposal identifier.
    pub proposal_id: [u8; 32],
    /// Fingerprint of the exact stored proposal preimage.
    pub proposal_fingerprint: [u8; 32],
    /// Proposal-time commitment to all Parliament body rosters.
    pub proposal_time_roster_root: [u8; 32],
    /// Commitment to the citizen electorate frozen at the referendum start boundary.
    pub plain_electorate_snapshot_root: [u8; 32],
    /// Exact number of citizens in the frozen PLAIN electorate.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub plain_electorate_snapshot_member_count: u64,
    /// Exact height at which the PLAIN electorate was frozen.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub plain_electorate_snapshot_captured_at_height: u64,
    /// Immutable seven-body approval gate used by the electorate eligibility rule.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub plain_electorate_snapshot_approval_gate_height: u64,
    /// Exact referendum window retained in consensus state.
    pub referendum_window: ValidationFeeGovernanceWindowV1,
    /// Deterministic finalized referendum result.
    pub finalization: ValidationFeeFinalizationEvidenceV1,
    /// Height at which the approved policy was appended to the registry.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub enacted_at_height: u64,
}

impl ValidationFeeParliamentAuthorizationV1 {
    /// Return a stable invariant violation, if any.
    #[must_use]
    pub fn invariant_error(&self) -> Option<&'static str> {
        if self.proposal_id == [0; 32]
            || self.proposal_fingerprint == [0; 32]
            || self.proposal_id != self.proposal_fingerprint
        {
            return Some(
                "validation-fee Parliament proposal id and fingerprint must be identical non-zero native identifiers",
            );
        }
        if self.proposal_time_roster_root == [0; 32] {
            return Some("validation-fee Parliament roster commitment must be non-zero");
        }
        if self.plain_electorate_snapshot_root == [0; 32]
            || self.plain_electorate_snapshot_member_count == 0
            || self.plain_electorate_snapshot_member_count > VALIDATION_FEE_PLAIN_MAX_MEMBERS_V1
        {
            return Some(
                "validation-fee PLAIN electorate snapshot root and bounded count must be retained",
            );
        }
        if self.referendum_window.upper < self.referendum_window.lower {
            return Some("validation-fee referendum window is invalid");
        }
        if self.plain_electorate_snapshot_captured_at_height != self.referendum_window.lower
            || self.plain_electorate_snapshot_approval_gate_height
                >= self.plain_electorate_snapshot_captured_at_height
        {
            return Some(
                "validation-fee PLAIN electorate snapshot must be captured at the referendum start after its approval gate",
            );
        }
        if self.finalization.referendum_id != self.proposal_id {
            return Some("validation-fee finalization referendum id differs from the proposal id");
        }
        if self.finalization.finalized_at_height != self.referendum_window.upper {
            return Some(
                "validation-fee finalization height must equal the inclusive referendum end",
            );
        }
        if self.enacted_at_height <= self.finalization.finalized_at_height {
            return Some("validation-fee enactment height must be after referendum finalization");
        }
        if self.finalization.mode != ValidationFeeGovernanceVotingModeV1::Plain {
            return Some("validation-fee governance supports plain referendum voting only");
        }
        if !self.finalization.approved || !self.finalization.recomputed_approval() {
            return Some("validation-fee referendum evidence is not a finalized approval");
        }
        None
    }

    /// Return a stable mismatch against the full retained electorate snapshot, if any.
    #[must_use]
    pub fn plain_electorate_snapshot_anchor_error(
        &self,
        snapshot: &ValidationFeePlainElectorateSnapshotV1,
    ) -> Option<&'static str> {
        if let Some(reason) = snapshot.invariant_error() {
            return Some(reason);
        }
        if self.plain_electorate_snapshot_root != snapshot.roster_root
            || self.plain_electorate_snapshot_member_count != snapshot.member_count
            || self.plain_electorate_snapshot_captured_at_height != snapshot.captured_at_height
            || self.plain_electorate_snapshot_approval_gate_height != snapshot.approval_gate_height
        {
            return Some(
                "validation-fee PLAIN electorate authorization anchors differ from the retained snapshot",
            );
        }
        None
    }
}

/// Exact enacted payout-lifecycle proposal referenced by a validation-fee policy.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidationFeePayoutLifecycleReferenceV1 {
    /// Non-zero lifecycle seal bound into the proposal fingerprint.
    pub lifecycle_seal: [u8; 32],
    /// Full typed Parliament and referendum authorization for the lifecycle proposal.
    pub parliament_authorization: ValidationFeeParliamentAuthorizationV1,
    /// Exact PLAIN electorate rules bound into the lifecycle proposal fingerprint.
    pub plain_electorate_rules: ValidationFeePlainElectorateRulesV1,
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
        if self.plain_electorate_rules.invariant_error().is_some() {
            return Some("validation-fee payout lifecycle PLAIN electorate rules are invalid");
        }
        None
    }
}

/// One entry in the registered validation-fee policy hash chain.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidationFeePolicyRegistryEntryV1 {
    /// Complete governed policy, retained so scheduled policies do not hide
    /// the policy that is effective at the current height.
    pub policy: ValidationFeePolicyV1,
    /// Exact PLAIN electorate rules bound into the policy proposal fingerprint.
    pub plain_electorate_rules: ValidationFeePlainElectorateRulesV1,
    /// Domain-separated policy hash.
    pub policy_hash: [u8; 32],
    /// Typed, independently checkable Parliament and referendum authorization.
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
        plain_electorate_rules: ValidationFeePlainElectorateRulesV1,
        parliament_authorization: ValidationFeeParliamentAuthorizationV1,
        payout_lifecycle: Option<ValidationFeePayoutLifecycleReferenceV1>,
    ) -> Result<Self, norito::Error> {
        let policy_hash = policy.policy_hash()?;
        Ok(Self {
            policy,
            plain_electorate_rules,
            policy_hash,
            parliament_authorization,
            payout_lifecycle,
        })
    }
}

/// On-ledger validation-fee policy registry used to reject rollback and
/// skipped-version policy changes while retaining scheduled policy history.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
    /// The validation-fee module reproduces the frozen V1 proposal preimages locally, so this
    /// authentication remains available in lightweight builds without the `governance` feature.
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
        let chain_id = first.policy.chain_id.clone();
        let genesis_hash = first.policy.genesis_hash;

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
            if entry.policy.chain_id != chain_id || entry.policy.genesis_hash != genesis_hash {
                return Err(ValidationFeePolicyRegistryError::ChainIdentityChanged {
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

    /// Return the latest enacted entry, including a policy scheduled for a
    /// future height.
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
// that do not compile the governance API, but their authorization checks must
// still reproduce the exact governance proposal fingerprint. Explicit V1
// discriminants avoid placeholder variants and bind this private preimage to
// the matching `ProposalKind` wire tags. Governance-enabled parity tests below
// verify the complete encoded bytes.
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
    policy: ValidationFeePolicyV1,
    payout_lifecycle_proposal_id: Option<[u8; 32]>,
    plain_electorate_rules: ValidationFeePlainElectorateRulesV1,
}

#[derive(Encode)]
struct ValidationFeePayoutLifecycleFingerprintPayloadV1 {
    payout_binding: ValidationFeeTreasuryPayoutBindingV1,
    plain_electorate_rules: ValidationFeePlainElectorateRulesV1,
}

fn validation_fee_proposal_fingerprint(proposal: &impl Encode) -> [u8; 32] {
    let encoded = proposal.encode();
    let mut hasher = Blake2bVar::new(32).expect("Blake2bVar length is fixed and valid");
    hasher.update(GOVERNANCE_PROPOSAL_FINGERPRINT_DOMAIN_V1);
    hasher.update(&encoded);
    let mut fingerprint = [0_u8; 32];
    hasher
        .finalize_variable(&mut fingerprint)
        .expect("fingerprint output has the configured Blake2b length");
    fingerprint
}

fn validation_fee_policy_proposal_fingerprint(
    policy: &ValidationFeePolicyV1,
    payout_lifecycle_proposal_id: Option<[u8; 32]>,
    plain_electorate_rules: &ValidationFeePlainElectorateRulesV1,
) -> [u8; 32] {
    validation_fee_proposal_fingerprint(
        &ValidationFeePolicyProposalFingerprintEnvelopeV1::ValidationFeePolicy(
            ValidationFeePolicyFingerprintPayloadV1 {
                policy: policy.clone(),
                payout_lifecycle_proposal_id,
                plain_electorate_rules: plain_electorate_rules.clone(),
            },
        ),
    )
}

fn validation_fee_payout_lifecycle_proposal_fingerprint(
    payout_binding: &ValidationFeeTreasuryPayoutBindingV1,
    plain_electorate_rules: &ValidationFeePlainElectorateRulesV1,
) -> [u8; 32] {
    validation_fee_proposal_fingerprint(
        &ValidationFeePayoutLifecycleProposalFingerprintEnvelopeV1::ValidationFeePayoutLifecycle(
            ValidationFeePayoutLifecycleFingerprintPayloadV1 {
                payout_binding: payout_binding.clone(),
                plain_electorate_rules: plain_electorate_rules.clone(),
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
    if entry.plain_electorate_rules.invariant_error().is_some() {
        return Err(
            ValidationFeePolicyRegistryError::InvalidParliamentAuthorization { policy_version },
        );
    }
    if !validation_fee_authorization_matches_plain_rules(
        &entry.parliament_authorization,
        &entry.plain_electorate_rules,
    ) {
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
                && reference.plain_electorate_rules == entry.plain_electorate_rules
                && validation_fee_authorization_matches_plain_rules(
                    &reference.parliament_authorization,
                    &reference.plain_electorate_rules,
                )
                && reference.parliament_authorization.enacted_at_height
                    <= entry.parliament_authorization.enacted_at_height
                && binding.lifecycle_seal().ok() == Some(reference.lifecycle_seal) =>
        {
            let fingerprint = validation_fee_payout_lifecycle_proposal_fingerprint(
                binding,
                &reference.plain_electorate_rules,
            );
            if reference.parliament_authorization.proposal_id != fingerprint
                || reference.parliament_authorization.proposal_fingerprint != fingerprint
            {
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
        &entry.policy,
        payout_lifecycle_proposal_id,
        &entry.plain_electorate_rules,
    );
    if entry.parliament_authorization.proposal_id != fingerprint
        || entry.parliament_authorization.proposal_fingerprint != fingerprint
    {
        return Err(
            ValidationFeePolicyRegistryError::InvalidParliamentAuthorization { policy_version },
        );
    }
    Ok(())
}

fn validation_fee_authorization_matches_plain_rules(
    authorization: &ValidationFeeParliamentAuthorizationV1,
    rules: &ValidationFeePlainElectorateRulesV1,
) -> bool {
    let Some(span) = authorization
        .referendum_window
        .upper
        .checked_sub(authorization.referendum_window.lower)
        .and_then(|distance| distance.checked_add(1))
    else {
        return false;
    };
    span == rules.ballot_duration_blocks
        && authorization.plain_electorate_snapshot_member_count <= rules.max_members
        && authorization.plain_electorate_snapshot_captured_at_height
            == authorization.referendum_window.lower
        && authorization.plain_electorate_snapshot_approval_gate_height
            < authorization.plain_electorate_snapshot_captured_at_height
        && authorization.finalization.finalized_at_height == authorization.referendum_window.upper
        && authorization.finalization.min_turnout == rules.min_turnout
        && authorization.finalization.approval_threshold_numerator
            == rules.approval_threshold_numerator
        && authorization.finalization.approval_threshold_denominator
            == rules.approval_threshold_denominator
}

/// One exact recipient and share in the atomic treasury-payout effect plan.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
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
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidationFeeTreasuryPayoutBindingV1 {
    /// Immutable deployed pool contract address.
    pub contract_address: ContractAddress,
    /// SHA-256 hash of the exact deployed contract artifact bytes.
    pub code_hash: [u8; 32],
    /// Only public entrypoint allowed to consume reserved validation-fee credit.
    pub entrypoint: Name,
    /// Immutable non-signable contract subject and policy treasury.
    pub treasury_account_id: AccountId,
    /// Exact policy SBD fee asset used by the pool quote leg.
    pub sbd_asset_id: AssetDefinitionId,
    /// Exact XOR asset returned by the pool base leg.
    pub xor_asset_id: AssetDefinitionId,
    /// Exact pool vault receiving SBD and sourcing XOR.
    pub pool_vault_account_id: AccountId,
    /// Exact SBD amount consumed per successful payout tick.
    pub batch_sbd: Quantity,
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
        if self.sbd_asset_id == self.xor_asset_id {
            return Some("validation-fee treasury payout SBD and XOR assets must differ");
        }
        if self.batch_sbd != validation_fee_payout_batch_sbd() {
            return Some("validation-fee treasury payout batch must be exactly 10 SBD");
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

/// Chain-level validation-fee policy.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct ValidationFeePolicyV1 {
    /// Policy schema version.
    pub schema_version: u16,
    /// Chain identifier bound into the policy.
    pub chain_id: ChainId,
    /// Genesis hash bound into the policy.
    pub genesis_hash: [u8; 32],
    /// Monotonic policy version.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub policy_version: u64,
    /// Previous policy hash for policy-chain validation.
    #[norito(default)]
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
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::u64_string"))]
    pub effective_from_height: u64,
    /// Optional last active height.
    #[norito(default)]
    #[cfg_attr(
        feature = "json",
        norito(with = "crate::json_helpers::u64_string::option")
    )]
    pub expires_after_height: Option<u64>,
    /// Explicit exemption classes recognized by this policy.
    #[norito(default)]
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
        if self.genesis_hash == [0; 32] {
            return Some("validation-fee policy genesis hash must be non-zero");
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
                return Some("enabled validation-fee policy amount must be exactly 0.10 SBD");
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
                if binding.sbd_asset_id != self.ds_asset_id {
                    return Some(
                        "validation-fee treasury payout SBD asset must equal the policy fee asset",
                    );
                }
                if binding.batch_sbd.scale() > u32::from(self.ds_scale) {
                    return Some(
                        "validation-fee treasury payout SBD batch exceeds the policy asset scale",
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

/// Return the exact SBD payout batch amount.
#[must_use]
pub fn validation_fee_payout_batch_sbd() -> Quantity {
    VALIDATION_FEE_PAYOUT_BATCH_SBD
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
    use std::str::FromStr as _;

    use iroha_crypto::{Algorithm, KeyPair};

    use super::*;
    #[cfg(feature = "governance")]
    use crate::governance::types::{
        ProposalKind, ValidationFeePayoutLifecycleProposal, ValidationFeePolicyProposal,
    };
    use crate::{domain::DomainId, name::Name};

    const TEST_AUTHORIZATION_STRIDE: u64 = 10_000;

    fn account(seed: u8) -> AccountId {
        let key_pair =
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("key pair");
        AccountId::new(key_pair.public_key().clone())
    }

    fn fee_asset() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("fees", "validation").expect("domain id"),
            Name::from_str("fee").expect("asset name"),
        )
    }

    fn xor_asset() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("xor", "validation").expect("domain id"),
            Name::from_str("xor").expect("asset name"),
        )
    }

    fn plain_electorate_rules() -> ValidationFeePlainElectorateRulesV1 {
        ValidationFeePlainElectorateRulesV1 {
            voting_asset_id: "5dHF5UNffENuEg9mhjYwY1jcZ1K5"
                .parse()
                .expect("voting asset id"),
            bond_escrow_account: account(90),
            slash_receiver_account: account(91),
            ballot_amount: 150_u64.into(),
            ballot_duration_blocks: 3_600,
            citizenship_amount: 10_000_u64.into(),
            max_members: VALIDATION_FEE_PLAIN_MAX_MEMBERS_V1,
            conviction_step_blocks: 100,
            max_conviction: 6,
            min_turnout: 1,
            approval_threshold_numerator: 1,
            approval_threshold_denominator: 2,
            eligibility_rule:
                ValidationFeePlainElectorateEligibilityRuleV1::ProposalOperatorAtOrBeforeGateOthersAfterGate,
        }
    }

    fn payout_binding() -> ValidationFeeTreasuryPayoutBindingV1 {
        let contract_address: ContractAddress =
            "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7"
                .parse()
                .expect("contract address");
        ValidationFeeTreasuryPayoutBindingV1 {
            treasury_account_id: contract_address.subject_id(),
            contract_address,
            code_hash: [0x11; 32],
            entrypoint: Name::from_str("autonomous_validation_fee_tick").expect("entrypoint"),
            sbd_asset_id: fee_asset(),
            xor_asset_id: xor_asset(),
            pool_vault_account_id: account(2),
            batch_sbd: validation_fee_payout_batch_sbd(),
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

    fn authorization(proposal_id: [u8; 32], marker: u8) -> ValidationFeeParliamentAuthorizationV1 {
        let lower = u64::from(marker).saturating_mul(TEST_AUTHORIZATION_STRIDE);
        let upper = lower
            .checked_add(plain_electorate_rules().ballot_duration_blocks - 1)
            .expect("test referendum end");
        ValidationFeeParliamentAuthorizationV1 {
            proposal_id,
            proposal_fingerprint: proposal_id,
            proposal_time_roster_root: [marker.wrapping_add(1); 32],
            plain_electorate_snapshot_root: [marker.wrapping_add(2); 32],
            plain_electorate_snapshot_member_count: 1,
            plain_electorate_snapshot_captured_at_height: lower,
            plain_electorate_snapshot_approval_gate_height: lower.saturating_sub(1),
            referendum_window: ValidationFeeGovernanceWindowV1 { lower, upper },
            finalization: ValidationFeeFinalizationEvidenceV1 {
                referendum_id: proposal_id,
                finalized_at_height: upper,
                mode: ValidationFeeGovernanceVotingModeV1::Plain,
                approve: 1,
                reject: 0,
                abstain: 0,
                min_turnout: 1,
                approval_threshold_numerator: 1,
                approval_threshold_denominator: 2,
                approved: true,
            },
            enacted_at_height: upper.checked_add(1).expect("test enactment height"),
        }
    }

    fn policy_effective_height(version: u64) -> u64 {
        version
            .checked_mul(TEST_AUTHORIZATION_STRIDE)
            .and_then(|lower| lower.checked_add(plain_electorate_rules().ballot_duration_blocks))
            .and_then(|enacted| enacted.checked_add(VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS))
            .expect("test policy effective height")
    }

    fn policy(version: u64, previous_policy_hash: Option<[u8; 32]>) -> ValidationFeePolicyV1 {
        ValidationFeePolicyV1 {
            schema_version: VALIDATION_FEE_POLICY_SCHEMA_VERSION,
            chain_id: ChainId::from("parliament-test"),
            genesis_hash: [7; 32],
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
        let plain_electorate_rules = plain_electorate_rules();
        let proposal_id =
            validation_fee_policy_proposal_fingerprint(&policy, None, &plain_electorate_rules);
        ValidationFeePolicyRegistryEntryV1::from_enactment(
            policy,
            plain_electorate_rules,
            authorization(proposal_id, marker),
            None,
        )
        .expect("policy hash")
    }

    struct PayoutLifecyclePolicyFixture {
        policy: ValidationFeePolicyV1,
        plain_electorate_rules: ValidationFeePlainElectorateRulesV1,
        lifecycle_seal: [u8; 32],
    }

    fn payout_lifecycle_policy_fixture() -> PayoutLifecyclePolicyFixture {
        let binding = payout_binding();
        let plain_electorate_rules = plain_electorate_rules();
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
            plain_electorate_rules,
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

    fn rebind_authorization(
        authorization: &mut ValidationFeeParliamentAuthorizationV1,
        proposal_id: [u8; 32],
    ) {
        authorization.proposal_id = proposal_id;
        authorization.proposal_fingerprint = proposal_id;
        authorization.finalization.referendum_id = proposal_id;
    }

    #[test]
    fn validation_fee_identity_hashes_ignore_and_restore_ambient_flags() {
        let policy = policy(1, None);
        let binding = payout_binding();
        let registry = ValidationFeePolicyRegistryV1 {
            registered_policies: vec![entry(policy.clone(), 1)],
        };
        let proposal_operator = account(7);
        let electorate = ValidationFeePlainElectorateSnapshotV1::from_canonical_members(
            [0x42; 32],
            proposal_operator.clone(),
            200,
            100,
            vec![ValidationFeePlainElectorateMemberV1 {
                account_id: proposal_operator,
                bonded_height: 100,
                bonded_amount: 10_000_u64.into(),
            }],
        )
        .expect("canonical PLAIN electorate snapshot");
        let baseline = (
            policy.policy_hash().expect("policy hash"),
            binding.lifecycle_seal().expect("lifecycle seal"),
            registry.snapshot_hash().expect("registry snapshot hash"),
            electorate.checked_roster_root().expect("electorate root"),
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
                electorate
                    .checked_roster_root()
                    .expect("ambient electorate root"),
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
    fn plain_electorate_rules_roundtrip_exact_first_release_json() {
        let rules = plain_electorate_rules();
        assert_eq!(rules.invariant_error(), None);

        let json = norito::json::to_json(&rules).expect("serialize PLAIN electorate rules");
        assert_eq!(
            json,
            format!(
                concat!(
                    r#"{{"voting_asset_id":"5dHF5UNffENuEg9mhjYwY1jcZ1K5","#,
                    r#""bond_escrow_account":"{}","slash_receiver_account":"{}","#,
                    r#""ballot_amount":"150","ballot_duration_blocks":"3600","#,
                    r#""citizenship_amount":"10000","max_members":"256","#,
                    r#""conviction_step_blocks":"100","max_conviction":"6","#,
                    r#""min_turnout":"1","approval_threshold_numerator":"1","#,
                    r#""approval_threshold_denominator":"2","#,
                    r#""eligibility_rule":{{"rule":"proposal_operator_at_or_before_gate_others_after_gate","value":null}}}}"#
                ),
                rules.bond_escrow_account, rules.slash_receiver_account,
            )
        );
        let decoded_json: ValidationFeePlainElectorateRulesV1 =
            norito::json::from_json(&json).expect("deserialize PLAIN electorate rules");
        assert_eq!(decoded_json, rules);

        let bytes = norito::to_bytes(&rules).expect("encode PLAIN electorate rules");
        let decoded_norito: ValidationFeePlainElectorateRulesV1 =
            norito::decode_from_bytes(&bytes).expect("decode PLAIN electorate rules");
        assert_eq!(decoded_norito, rules);
    }

    #[test]
    fn plain_electorate_rules_reject_invalid_voting_parameters() {
        let rules = plain_electorate_rules();
        for malformed in [
            {
                let mut value = rules.clone();
                value.ballot_amount = Quantity::zero();
                value
            },
            {
                let mut value = rules.clone();
                value.ballot_duration_blocks = 0;
                value
            },
            {
                let mut value = rules.clone();
                value.citizenship_amount = Quantity::zero();
                value
            },
            {
                let mut value = rules.clone();
                value.max_members = 0;
                value
            },
            {
                let mut value = rules.clone();
                value.max_members = VALIDATION_FEE_PLAIN_MAX_MEMBERS_V1 + 1;
                value
            },
            {
                let mut value = rules.clone();
                value.conviction_step_blocks = 0;
                value
            },
            {
                let mut value = rules.clone();
                value.max_conviction = 0;
                value
            },
            {
                let mut value = rules.clone();
                value.min_turnout = 0;
                value
            },
            {
                let mut value = rules.clone();
                value.approval_threshold_numerator = 0;
                value
            },
            {
                let mut value = rules;
                value.approval_threshold_numerator = 3;
                value.approval_threshold_denominator = 2;
                value
            },
        ] {
            assert!(
                malformed.invariant_error().is_some(),
                "malformed PLAIN electorate rules must be rejected"
            );
        }
    }

    #[test]
    fn plain_electorate_snapshot_is_canonical_and_context_bound() {
        let proposal_id = [0x42; 32];
        let proposal_operator = account(7);
        let other_citizen = account(8);
        let approval_gate_height = 100;
        let captured_at_height = 200;
        let mut members = vec![
            ValidationFeePlainElectorateMemberV1 {
                account_id: proposal_operator.clone(),
                bonded_height: approval_gate_height,
                bonded_amount: 10_000_u64.into(),
            },
            ValidationFeePlainElectorateMemberV1 {
                account_id: other_citizen.clone(),
                bonded_height: approval_gate_height + 1,
                bonded_amount: 10_000_u64.into(),
            },
        ];
        members.sort_by(|left, right| left.account_id.cmp(&right.account_id));
        let snapshot = ValidationFeePlainElectorateSnapshotV1::from_canonical_members(
            proposal_id,
            proposal_operator.clone(),
            captured_at_height,
            approval_gate_height,
            members,
        )
        .expect("canonical PLAIN electorate snapshot");

        assert_eq!(snapshot.invariant_error(), None);
        assert_eq!(
            snapshot.context_error(proposal_id, &proposal_operator, &plain_electorate_rules()),
            None
        );
        assert_eq!(
            snapshot.checked_roster_root().expect("snapshot root"),
            snapshot.roster_root
        );
        assert!(snapshot.contains(&proposal_operator));
        assert!(snapshot.contains(&other_citizen));
        assert!(!snapshot.contains(&account(9)));

        let mut self_custody_rules = plain_electorate_rules();
        self_custody_rules.bond_escrow_account = proposal_operator.clone();
        assert_eq!(
            snapshot.context_error(proposal_id, &proposal_operator, &self_custody_rules),
            Some("validation-fee PLAIN custody accounts cannot belong to the voting electorate")
        );
        let mut slash_receiver_rules = plain_electorate_rules();
        slash_receiver_rules.slash_receiver_account = other_citizen.clone();
        assert_eq!(
            snapshot.context_error(proposal_id, &proposal_operator, &slash_receiver_rules),
            Some("validation-fee PLAIN custody accounts cannot belong to the voting electorate")
        );

        let mut reordered = snapshot.clone();
        reordered.members.reverse();
        assert!(reordered.invariant_error().is_some());

        let mut tampered = snapshot.clone();
        tampered
            .members
            .iter_mut()
            .find(|member| member.account_id == other_citizen)
            .expect("other citizen")
            .bonded_amount = 10_001_u64.into();
        assert_eq!(
            tampered.invariant_error(),
            Some("validation-fee PLAIN electorate snapshot root is invalid")
        );

        let mut narrower_rules = plain_electorate_rules();
        narrower_rules.max_members = 1;
        assert!(
            snapshot
                .context_error(proposal_id, &proposal_operator, &narrower_rules)
                .is_some()
        );
        assert!(
            snapshot
                .context_error([0x43; 32], &proposal_operator, &plain_electorate_rules())
                .is_some()
        );
    }

    #[cfg(feature = "governance")]
    #[test]
    fn lightweight_validation_fee_fingerprints_match_governance_proposal_bytes() {
        let payout_binding = payout_binding();
        let plain_electorate_rules = plain_electorate_rules();
        let lifecycle_governance =
            ProposalKind::ValidationFeePayoutLifecycle(ValidationFeePayoutLifecycleProposal {
                payout_binding: payout_binding.clone(),
                plain_electorate_rules: plain_electorate_rules.clone(),
            });
        let lifecycle_lightweight =
            ValidationFeePayoutLifecycleProposalFingerprintEnvelopeV1::ValidationFeePayoutLifecycle(
                ValidationFeePayoutLifecycleFingerprintPayloadV1 {
                    payout_binding: payout_binding.clone(),
                    plain_electorate_rules: plain_electorate_rules.clone(),
                },
            );
        assert_eq!(
            lifecycle_lightweight.encode(),
            lifecycle_governance.encode()
        );
        assert_eq!(
            validation_fee_payout_lifecycle_proposal_fingerprint(
                &payout_binding,
                &plain_electorate_rules,
            ),
            lifecycle_governance.fingerprint()
        );

        let lifecycle_id = lifecycle_governance.fingerprint();
        let mut governed_policy = policy(1, None);
        governed_policy.treasury_payout_binding = Some(payout_binding);
        for payout_lifecycle_proposal_id in [None, Some(lifecycle_id)] {
            let policy_governance =
                ProposalKind::ValidationFeePolicy(ValidationFeePolicyProposal {
                    policy: governed_policy.clone(),
                    payout_lifecycle_proposal_id,
                    plain_electorate_rules: plain_electorate_rules.clone(),
                });
            let policy_lightweight =
                ValidationFeePolicyProposalFingerprintEnvelopeV1::ValidationFeePolicy(
                    ValidationFeePolicyFingerprintPayloadV1 {
                        policy: governed_policy.clone(),
                        payout_lifecycle_proposal_id,
                        plain_electorate_rules: plain_electorate_rules.clone(),
                    },
                );
            assert_eq!(policy_lightweight.encode(), policy_governance.encode());
            assert_eq!(
                validation_fee_policy_proposal_fingerprint(
                    &governed_policy,
                    payout_lifecycle_proposal_id,
                    &plain_electorate_rules,
                ),
                policy_governance.fingerprint()
            );
        }
    }

    #[test]
    fn lightweight_validation_fee_preimages_use_frozen_v1_tags() {
        let policy = ValidationFeePolicyProposalFingerprintEnvelopeV1::ValidationFeePolicy(
            ValidationFeePolicyFingerprintPayloadV1 {
                policy: policy(1, None),
                payout_lifecycle_proposal_id: None,
                plain_electorate_rules: plain_electorate_rules(),
            },
        );
        let lifecycle =
            ValidationFeePayoutLifecycleProposalFingerprintEnvelopeV1::ValidationFeePayoutLifecycle(
                ValidationFeePayoutLifecycleFingerprintPayloadV1 {
                    payout_binding: payout_binding(),
                    plain_electorate_rules: plain_electorate_rules(),
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
                Some("enabled validation-fee policy amount must be exactly 0.10 SBD")
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
                value.batch_sbd = "9.99".parse().expect("quantity");
                value
            },
            {
                let mut value = binding.clone();
                value.batch_sbd = "10.01".parse().expect("quantity");
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
    fn lifecycle_seal_is_derived_and_fingerprint_binds_exact_binding() {
        let binding = payout_binding();
        let plain_electorate_rules = plain_electorate_rules();
        let seal = binding.lifecycle_seal().expect("lifecycle seal");
        assert_ne!(seal, [0; 32]);
        let proposal_fingerprint =
            validation_fee_payout_lifecycle_proposal_fingerprint(&binding, &plain_electorate_rules);

        let mut changed_binding = binding.clone();
        changed_binding.code_hash[0] ^= 1;
        let changed_seal = changed_binding
            .lifecycle_seal()
            .expect("changed lifecycle seal");
        let changed_fingerprint = validation_fee_payout_lifecycle_proposal_fingerprint(
            &changed_binding,
            &plain_electorate_rules,
        );

        assert_ne!(seal, changed_seal);
        assert_ne!(proposal_fingerprint, changed_fingerprint);

        let mut changed_rules = plain_electorate_rules;
        changed_rules.ballot_duration_blocks += 1;
        assert_ne!(
            proposal_fingerprint,
            validation_fee_payout_lifecycle_proposal_fingerprint(&binding, &changed_rules,)
        );
    }

    #[test]
    fn payout_policy_requires_matching_enacted_lifecycle_reference() {
        let PayoutLifecyclePolicyFixture {
            policy: payout_policy,
            plain_electorate_rules,
            lifecycle_seal: seal,
        } = payout_lifecycle_policy_fixture();

        let missing = ValidationFeePolicyRegistryV1 {
            registered_policies: vec![
                ValidationFeePolicyRegistryEntryV1::from_enactment(
                    payout_policy.clone(),
                    plain_electorate_rules.clone(),
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
                    plain_electorate_rules.clone(),
                    authorization([0x10; 32], 10),
                    Some(ValidationFeePayoutLifecycleReferenceV1 {
                        lifecycle_seal: bad_seal,
                        parliament_authorization: authorization([0x11; 32], 11),
                        plain_electorate_rules: plain_electorate_rules.clone(),
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
            validation_fee_payout_lifecycle_proposal_fingerprint(binding, &plain_electorate_rules);
        let policy_id = validation_fee_policy_proposal_fingerprint(
            &payout_policy,
            Some(lifecycle_id),
            &plain_electorate_rules,
        );
        let valid = ValidationFeePolicyRegistryV1 {
            registered_policies: vec![
                ValidationFeePolicyRegistryEntryV1::from_enactment(
                    payout_policy,
                    plain_electorate_rules.clone(),
                    authorization(policy_id, 10),
                    Some(ValidationFeePayoutLifecycleReferenceV1 {
                        lifecycle_seal: seal,
                        parliament_authorization: authorization(lifecycle_id, 9),
                        plain_electorate_rules,
                    }),
                )
                .expect("registry entry"),
            ],
        };
        valid.validate().expect("exact typed proposal fingerprints");

        let mut rebound_lifecycle = valid.clone();
        let lifecycle_authorization = &mut rebound_lifecycle.registered_policies[0]
            .payout_lifecycle
            .as_mut()
            .expect("lifecycle")
            .parliament_authorization;
        rebind_authorization(lifecycle_authorization, [0xA1; 32]);
        assert_invalid_payout_lifecycle_reference(&rebound_lifecycle);

        let mut rebound_policy = valid;
        let policy_authorization =
            &mut rebound_policy.registered_policies[0].parliament_authorization;
        rebind_authorization(policy_authorization, [0xA2; 32]);
        assert_invalid_policy_authorization(&rebound_policy);
    }

    #[test]
    fn parliament_authorization_requires_exact_approved_window_evidence() {
        let valid = authorization([0x12; 32], 12);
        assert_eq!(valid.invariant_error(), None);

        let mut post_window_enactment = valid;
        post_window_enactment.referendum_window.upper =
            post_window_enactment.finalization.finalized_at_height;
        post_window_enactment.enacted_at_height = post_window_enactment
            .finalization
            .finalized_at_height
            .saturating_add(3_600);
        assert_eq!(
            post_window_enactment.invariant_error(),
            None,
            "an approved referendum must remain enactable after its closed voting window"
        );

        let mut equal_finalization = valid;
        equal_finalization.enacted_at_height = equal_finalization.finalization.finalized_at_height;
        assert_eq!(
            equal_finalization.invariant_error(),
            Some("validation-fee enactment height must be after referendum finalization")
        );

        let mut before_finalization = valid;
        before_finalization.enacted_at_height = before_finalization
            .finalization
            .finalized_at_height
            .saturating_sub(1);
        assert_eq!(
            before_finalization.invariant_error(),
            Some("validation-fee enactment height must be after referendum finalization")
        );

        let mut outside_window = valid;
        outside_window.finalization.finalized_at_height =
            outside_window.referendum_window.upper.saturating_add(1);
        assert!(outside_window.invariant_error().is_some());

        let mut fabricated_approval = valid;
        fabricated_approval.finalization.approve = 0;
        assert!(fabricated_approval.invariant_error().is_some());

        let mut wrong_referendum = valid;
        wrong_referendum.finalization.referendum_id = [0xAA; 32];
        assert!(wrong_referendum.invariant_error().is_some());
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
