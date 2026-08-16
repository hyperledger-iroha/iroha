//! Typed governance instructions.
//!
//! These canonical data-model types are serialized into transactions and are executed by the
//! corresponding core governance paths. They also define the exact CLI and Torii draft surfaces;
//! endpoint-local aliases are not part of the instruction format.
#[cfg(feature = "governance")]
use crate::governance::types::ParliamentBody;
use std::{string::String, vec::Vec};
#[cfg(not(feature = "governance"))]
type ParliamentBody = ();
#[cfg(not(feature = "governance"))]
pub use self::at_window_placeholder::AtWindow;
#[cfg(feature = "governance")]
pub use crate::governance::types::AtWindow;
#[cfg(test)]
use crate::isi::bridge::SccpRouteGovernanceActionV1;
use crate::{
    isi::sorafs::SorafsProviderGovernanceActionV1,
    prelude::*,
    runtime::RuntimeUpgradeManifest,
    smart_contract::manifest::ManifestProvenance,
    validation_fee::{
        ValidationFeePlainElectorateRulesV1, ValidationFeePolicyV1,
        ValidationFeeTreasuryPayoutBindingV1,
    },
};
use iroha_primitives::numeric::Quantity;
use norito::codec::{Decode, Encode};
#[cfg(not(feature = "governance"))]
mod at_window_placeholder {
    use super::*;
    #[derive(
        Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, iroha_schema::IntoSchema,
    )]
    #[cfg_attr(
        feature = "json",
        derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
    )]
    /// Inclusive governance enactment window expressed in block heights.
    pub struct AtWindow {
        /// Lower bound (inclusive) of the enactment window.
        pub lower: u64,
        /// Upper bound (inclusive) of the enactment window.
        pub upper: u64,
    }
}
/// Voting mode for a referendum
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, iroha_schema::IntoSchema,
)]
pub enum VotingMode {
    /// Zero-knowledge voting flow (default ballot type).
    Zk,
    /// Plain-text quadratic voting flow.
    Plain,
}
#[cfg(feature = "json")]
impl norito::json::JsonSerialize for VotingMode {
    fn json_serialize(&self, out: &mut String) {
        norito::json::write_json_string(
            match self {
                Self::Zk => "Zk",
                Self::Plain => "Plain",
            },
            out,
        );
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_json_string_to(
            match self {
                Self::Zk => "Zk",
                Self::Plain => "Plain",
            },
            out,
        )
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for VotingMode {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "Zk" => Ok(Self::Zk),
            "Plain" => Ok(Self::Plain),
            other => Err(norito::json::Error::unknown_field(other.to_owned())),
        }
    }
}
/// Council derivation method.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Default,
    Encode,
    Decode,
    iroha_schema::IntoSchema,
)]
pub enum CouncilDerivationKind {
    /// Derived automatically from deterministic bonded-citizen sortition.
    Sortition,
    /// Supplied explicitly by an authorized parliament administrator.
    #[default]
    Manual,
}
#[cfg(feature = "json")]
impl norito::json::JsonSerialize for CouncilDerivationKind {
    fn json_serialize(&self, out: &mut String) {
        let label = match self {
            CouncilDerivationKind::Sortition => "Sortition",
            CouncilDerivationKind::Manual => "Manual",
        };
        norito::json::write_json_string(label, out);
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        let label = match self {
            CouncilDerivationKind::Sortition => "Sortition",
            CouncilDerivationKind::Manual => "Manual",
        };
        norito::json::write_json_string_to(label, out)
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for CouncilDerivationKind {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "Sortition" => Ok(CouncilDerivationKind::Sortition),
            "Manual" => Ok(CouncilDerivationKind::Manual),
            other => Err(norito::json::Error::unknown_field(other.to_owned())),
        }
    }
}
/// Propose deployment of an IVM bytecode (`.to`) by hash
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct ProposeDeployContract {
    /// Canonical public contract address targeted by the proposal.
    pub contract_address: crate::smart_contract::ContractAddress,
    /// Blake2b-32 hash of the compiled `.to` bytecode slated for deployment (lowercase hex).
    pub code_hash_hex: String,
    /// Blake2b-32 hash of the ABI surface expected by the host (lowercase hex).
    pub abi_hash_hex: String,
    /// ABI version (e.g., "1") supplied by the proposer.
    pub abi_version: String,
    /// Optional enactment window override (inclusive)
    pub window: Option<AtWindow>,
    /// Optional voting mode for the referendum created by this proposal (default Zk)
    pub mode: Option<VotingMode>,
    /// Optional manifest provenance to attest the contract manifest on enactment.
    pub manifest_provenance: Option<ManifestProvenance>,
}
impl crate::seal::Instruction for ProposeDeployContract {}
/// Propose a runtime upgrade manifest through governance.
///
/// Ledger admission requires an exact `CanProposeRuntimeUpgrade` permission whose ABI version and
/// hash match the manifest; contract-deployment permissions do not authorize runtime upgrades.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct ProposeRuntimeUpgradeProposal {
    /// Canonical runtime-upgrade manifest payload.
    pub manifest: RuntimeUpgradeManifest,
    /// Optional referendum window override (inclusive).
    pub window: Option<AtWindow>,
    /// Optional voting mode for the referendum created by this proposal (default Zk).
    pub mode: Option<VotingMode>,
}
impl crate::seal::Instruction for ProposeRuntimeUpgradeProposal {}
/// Propose one closed SCCP registry action through governance.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct ProposeSccpRouteGovernance {
    /// Complete network- and action-bound referendum preimage.
    pub anchor: crate::isi::bridge::SccpRouteGovernanceAnchorV1,
    /// Optional referendum window override (inclusive).
    pub window: Option<AtWindow>,
    /// Optional voting mode for the referendum created by this proposal (default Zk).
    pub mode: Option<VotingMode>,
}
impl crate::seal::Instruction for ProposeSccpRouteGovernance {}
/// Propose one closed `SoraFS` provider-owner transition through governance.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct ProposeSorafsProviderGovernance {
    /// Exact compare-and-set owner transition to execute if enacted.
    pub action: SorafsProviderGovernanceActionV1,
    /// Optional referendum window override (inclusive).
    pub window: Option<AtWindow>,
    /// Optional voting mode for the referendum created by this proposal (default ZK).
    pub mode: Option<VotingMode>,
}
impl crate::seal::Instruction for ProposeSorafsProviderGovernance {}
/// Propose one validation-fee policy through SORA Parliament.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct ProposeValidationFeePolicy {
    /// Complete policy to append if Parliament and the referendum approve it.
    pub policy: ValidationFeePolicyV1,
    /// Exact enacted payout lifecycle required when the policy carries a payout binding.
    pub payout_lifecycle_proposal_id: Option<[u8; 32]>,
    /// Optional inclusive referendum voting window override.
    pub referendum_window: Option<AtWindow>,
    /// Optional voting mode for the referendum (default `Plain`; `Zk` is rejected).
    pub mode: Option<VotingMode>,
    /// Exact PLAIN electorate contract bound into the proposal fingerprint.
    pub plain_electorate_rules: ValidationFeePlainElectorateRulesV1,
}
impl crate::seal::Instruction for ProposeValidationFeePolicy {}
/// Propose one exact validation-fee payout lifecycle through SORA Parliament.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct ProposeValidationFeePayoutLifecycle {
    /// Exact treasury payout binding authorized by this lifecycle.
    ///
    /// Consensus derives the non-zero lifecycle seal from this complete
    /// binding before accepting the proposal.
    pub payout_binding: ValidationFeeTreasuryPayoutBindingV1,
    /// Optional inclusive referendum voting window override.
    pub referendum_window: Option<AtWindow>,
    /// Optional voting mode for the referendum (default `Plain`; `Zk` is rejected).
    pub mode: Option<VotingMode>,
    /// Exact PLAIN electorate contract bound into the proposal fingerprint.
    pub plain_electorate_rules: ValidationFeePlainElectorateRulesV1,
}
impl crate::seal::Instruction for ProposeValidationFeePayoutLifecycle {}
/// Cast a ZK ballot (default voting mode)
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct CastZkBallot {
    /// Canonical V1 election/referendum selector.
    pub election_id: String,
    /// Base64-encoded proof bytes (envelope routing determines backend)
    pub proof_b64: String,
    /// Closed V1 JSON public-input object encoded as UTF-8 for Norito.
    ///
    /// The object accepts only `root_hint`, `owner`, `amount`, `duration_blocks`, `direction`, and
    /// `nullifier`; `amount` is an exact canonical non-negative [`Quantity`] string.
    pub public_inputs_json: String,
}
impl crate::seal::Instruction for CastZkBallot {}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(deny_unknown_fields))]
/// Canonical V1 ZK ballot proof envelope.
///
/// Opaque container for the ballot proof and minimal public context.
pub struct BallotProof {
    /// Proof backend tag (e.g., "halo2/ipa" or "halo2/pasta/tiny-add").
    pub backend: iroha_schema::Ident,
    /// Opaque proof envelope bytes (ZK1 or H2* container).
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub envelope_bytes: Vec<u8>,
    /// Optional eligibility root hint (32-byte) to bind verification to a known root.
    /// JSON uses a lowercase hex string (optional 0x or blake2b32: prefix).
    #[cfg_attr(
        feature = "json",
        norito(json = "crate::json_helpers::fixed_bytes_hex::option")
    )]
    pub root_hint: Option<[u8; 32]>,
    /// Optional owner account id (when the circuit commits to it in public inputs).
    pub owner: Option<crate::account::AccountId>,
    /// Optional nullifier hint (32-byte) derived from the proof's commitment.
    /// JSON uses a lowercase hex string (optional 0x or blake2b32: prefix).
    #[cfg_attr(
        feature = "json",
        norito(json = "crate::json_helpers::fixed_bytes_hex::option")
    )]
    pub nullifier: Option<[u8; 32]>,
    /// Optional exact lock amount hint.
    pub amount: Option<Quantity>,
    /// Optional lock duration hint in blocks.
    pub duration_blocks: Option<u64>,
    /// Optional direction hint (Aye/Nay/Abstain).
    pub direction: Option<String>,
}
/// Cast a non‑ZK quadratic ballot (optional mode)
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct CastPlainBallot {
    /// Canonical V1 selector of the referendum this ballot targets.
    pub referendum_id: String,
    /// Account submitting the ballot.
    pub owner: AccountId,
    /// Quadratic voting credit amount committed by the ballot.
    pub amount: Quantity,
    /// Duration of the lock in blocks.
    pub duration_blocks: u64,
    /// 0=Aye, 1=Nay, 2=Abstain
    pub direction: u8,
}
impl crate::seal::Instruction for CastPlainBallot {}
/// Enact an approved referendum (host validates certificate separately)
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, iroha_schema::IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct EnactReferendum {
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    /// Identifier of the referendum to enact.
    pub referendum_id: [u8; 32],
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    /// Blake2b-32 hash of the referendum preimage (proposal payload).
    pub preimage_hash: [u8; 32],
    /// Window describing when enactment is valid.
    pub at_window: AtWindow,
}
impl crate::seal::Instruction for EnactReferendum {}
/// Enact a finalized threshold referendum for one exact SCCP route action.
///
/// Unlike the generic referendum instruction, this surface carries the full closed preimage and
/// Core checks it against the stored proposal, exact [`NetworkId`], and canonical referendum id
/// before applying any registry mutation.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, iroha_schema::IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[cfg_attr(feature = "json", norito(no_fast_from_json))]
#[norito(deny_unknown_fields)]
pub struct EnactSccpRouteGovernance {
    /// Canonical identifier derived from the complete SCCP anchor.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub referendum_id: [u8; 32],
    /// Full network- and action-bound preimage approved by the referendum.
    pub anchor: crate::isi::bridge::SccpRouteGovernanceAnchorV1,
    /// Exact finalized enactment window.
    pub at_window: AtWindow,
}
impl crate::seal::Instruction for EnactSccpRouteGovernance {}
/// Finalize a referendum: compute tally and emit Approved/Rejected events
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct FinalizeReferendum {
    /// Identifier of the referendum to finalize.
    pub referendum_id: String,
    /// Deterministic proposal id this referendum governs
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub proposal_id: [u8; 32],
}
impl crate::seal::Instruction for FinalizeReferendum {}
/// Record a council approval for a governance proposal.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema,
)]
pub struct ApproveGovernanceProposal {
    /// Parliament body granting the approval (defaults to Agenda Council).
    #[norito(default)]
    pub body: ParliamentBody,
    /// Deterministic proposal id (Blake2b-32) being approved by the council.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub proposal_id: [u8; 32],
}
impl crate::seal::Instruction for ApproveGovernanceProposal {}
/// Equal citizen decision recorded by a seated Parliament member.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, iroha_schema::IntoSchema,
)]
pub enum ParliamentDecision {
    /// Support the proposal at this Parliament stage.
    Approve,
    /// Reject the proposal at this Parliament stage.
    Reject,
    /// Record presence without supporting or rejecting the proposal.
    Abstain,
}
impl ParliamentDecision {
    /// Stable lowercase label used by JSON clients.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Reject => "reject",
            Self::Abstain => "abstain",
        }
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonSerialize for ParliamentDecision {
    fn json_serialize(&self, out: &mut String) {
        norito::json::write_json_string(self.as_str(), out);
    }
    fn json_serialize_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::write_json_string_to(self.as_str(), out)
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for ParliamentDecision {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = parser.parse_string()?;
        match value.as_str() {
            "approve" => Ok(Self::Approve),
            "reject" => Ok(Self::Reject),
            "abstain" => Ok(Self::Abstain),
            other => Err(norito::json::Error::UnknownField {
                field: other.to_owned(),
            }),
        }
    }
}
/// Cast an equal signed Parliament ballot for a proposal stage.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, iroha_schema::IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct CastParliamentBallot {
    /// Parliament body receiving the ballot.
    pub body: ParliamentBody,
    /// Deterministic proposal id (Blake2b-32) being decided by the body.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
    pub proposal_id: [u8; 32],
    /// Equal citizen decision signed by the transaction authority.
    pub decision: ParliamentDecision,
}
impl crate::seal::Instruction for CastParliamentBallot {}
/// Persist a council membership for an epoch.
///
/// This instruction records an explicitly administered `members` roster for `epoch` in the WSV.
/// Selection metadata is derived by the ledger and is not accepted from the caller.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct PersistCouncilForEpoch {
    /// Epoch index
    pub epoch: u64,
    /// Council members in deterministic order
    pub members: Vec<crate::account::AccountId>,
    /// Alternates that can replace members who decline or are ineligible.
    #[norito(default)]
    pub alternates: Vec<crate::account::AccountId>,
}
impl crate::seal::Instruction for PersistCouncilForEpoch {}
/// Discipline event recorded for a citizen assigned to a governance role.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, iroha_schema::IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize),
    norito(tag = "event", content = "value", rename_all = "kebab-case")
)]
pub enum CitizenServiceEvent {
    /// Citizen declined the assignment.
    Decline,
    /// Citizen failed to appear for the assignment.
    NoShow,
    /// Citizen committed misconduct during the assignment.
    Misconduct,
}
impl core::cmp::PartialOrd for CitizenServiceEvent {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl core::cmp::Ord for CitizenServiceEvent {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        (*self as u8).cmp(&(*other as u8))
    }
}
/// Record a citizen service discipline event (decline, no-show, misconduct) for a role/epoch.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct RecordCitizenServiceOutcome {
    /// Citizen account receiving the record.
    pub owner: AccountId,
    /// Epoch index associated with the assignment.
    pub epoch: u64,
    /// Governance role label (e.g., "council", "`policy_jury`").
    pub role: String,
    /// Recorded event kind.
    pub event: CitizenServiceEvent,
}
impl crate::seal::Instruction for RecordCitizenServiceOutcome {}
/// Bond the configured citizenship amount to join the citizen registry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct RegisterCitizen {
    /// Account receiving citizenship.
    pub owner: AccountId,
    /// Amount to bond (must meet or exceed the configured floor).
    pub amount: Quantity,
}
impl crate::seal::Instruction for RegisterCitizen {}
/// Unbond and remove a citizen from the registry.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct UnregisterCitizen {
    /// Account to remove from the registry.
    pub owner: AccountId,
}
impl crate::seal::Instruction for UnregisterCitizen {}
/// Slash a governance bond lock for a referendum.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct SlashGovernanceLock {
    /// Canonical V1 selector of the referendum whose lock is being slashed.
    pub referendum_id: String,
    /// Account whose bond lock will be reduced.
    pub owner: AccountId,
    /// Exact amount to slash from the lock.
    pub amount: Quantity,
    /// Human-readable reason recorded with the slash event.
    pub reason: String,
}
impl crate::seal::Instruction for SlashGovernanceLock {}
/// Restitute a previously slashed governance bond lock.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Encode, Decode, iroha_schema::IntoSchema)]
pub struct RestituteGovernanceLock {
    /// Canonical V1 selector of the referendum whose lock is being restored.
    pub referendum_id: String,
    /// Account receiving the restitution.
    pub owner: AccountId,
    /// Exact amount to restore to the lock.
    pub amount: Quantity,
    /// Human-readable reason recorded with the restitution event.
    pub reason: String,
}
impl crate::seal::Instruction for RestituteGovernanceLock {}
fn governance_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}
macro_rules! impl_governance_decode_from_slice {
    ($ty:ty { $($field:ident : $field_ty:ty),+ $(,)? }) => {
        impl<'a> norito::core::DecodeFromSlice<'a> for $ty {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
                let flags = governance_decode_flags();
                if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
                    return super::decode_packed_instruction_payload::<Self>(bytes);
                }
                let mut offset = 0usize;
                $(
                    let $field = super::decode_aos_canonical_field::<$field_ty>(
                        super::read_aos_field(bytes, &mut offset, flags)?,
                        flags,
                    )?;
                )+
                if offset != bytes.len() {
                    return Err(norito::core::Error::LengthMismatch);
                }
                norito::core::note_payload_access(bytes, offset);
                Ok((Self { $($field),+ }, offset))
            }
        }
    };
}
impl_governance_decode_from_slice!(ProposeDeployContract {
    contract_address: crate::smart_contract::ContractAddress,
    code_hash_hex: String,
    abi_hash_hex: String,
    abi_version: String,
    window: Option<AtWindow>,
    mode: Option<VotingMode>,
    manifest_provenance: Option<ManifestProvenance>,
});
impl_governance_decode_from_slice!(ProposeRuntimeUpgradeProposal {
    manifest: RuntimeUpgradeManifest,
    window: Option<AtWindow>,
    mode: Option<VotingMode>,
});
impl_governance_decode_from_slice!(ProposeSccpRouteGovernance {
    anchor: crate::isi::bridge::SccpRouteGovernanceAnchorV1,
    window: Option<AtWindow>,
    mode: Option<VotingMode>,
});
impl_governance_decode_from_slice!(ProposeSorafsProviderGovernance {
    action: SorafsProviderGovernanceActionV1,
    window: Option<AtWindow>,
    mode: Option<VotingMode>,
});
impl_governance_decode_from_slice!(ProposeValidationFeePolicy {
    policy: ValidationFeePolicyV1,
    payout_lifecycle_proposal_id: Option<[u8; 32]>,
    referendum_window: Option<AtWindow>,
    mode: Option<VotingMode>,
    plain_electorate_rules: ValidationFeePlainElectorateRulesV1,
});
impl_governance_decode_from_slice!(ProposeValidationFeePayoutLifecycle {
    payout_binding: ValidationFeeTreasuryPayoutBindingV1,
    referendum_window: Option<AtWindow>,
    mode: Option<VotingMode>,
    plain_electorate_rules: ValidationFeePlainElectorateRulesV1,
});
impl_governance_decode_from_slice!(CastZkBallot {
    election_id: String,
    proof_b64: String,
    public_inputs_json: String,
});
impl_governance_decode_from_slice!(CastPlainBallot {
    referendum_id: String,
    owner: AccountId,
    amount: Quantity,
    duration_blocks: u64,
    direction: u8,
});
impl_governance_decode_from_slice!(SlashGovernanceLock {
    referendum_id: String,
    owner: AccountId,
    amount: Quantity,
    reason: String,
});
impl_governance_decode_from_slice!(RestituteGovernanceLock {
    referendum_id: String,
    owner: AccountId,
    amount: Quantity,
    reason: String,
});
impl_governance_decode_from_slice!(EnactReferendum {
    referendum_id: [u8; 32],
    preimage_hash: [u8; 32],
    at_window: AtWindow,
});
impl_governance_decode_from_slice!(EnactSccpRouteGovernance {
    referendum_id: [u8; 32],
    anchor: crate::isi::bridge::SccpRouteGovernanceAnchorV1,
    at_window: AtWindow,
});
impl_governance_decode_from_slice!(FinalizeReferendum {
    referendum_id: String,
    proposal_id: [u8; 32],
});
impl_governance_decode_from_slice!(ApproveGovernanceProposal {
    body: ParliamentBody,
    proposal_id: [u8; 32],
});
impl_governance_decode_from_slice!(CastParliamentBallot {
    body: ParliamentBody,
    proposal_id: [u8; 32],
    decision: ParliamentDecision,
});
impl_governance_decode_from_slice!(PersistCouncilForEpoch {
    epoch: u64,
    members: Vec<crate::account::AccountId>,
    alternates: Vec<crate::account::AccountId>,
});
impl_governance_decode_from_slice!(RecordCitizenServiceOutcome {
    owner: AccountId,
    epoch: u64,
    role: String,
    event: CitizenServiceEvent,
});
impl_governance_decode_from_slice!(RegisterCitizen {
    owner: AccountId,
    amount: Quantity,
});
impl_governance_decode_from_slice!(UnregisterCitizen { owner: AccountId });
#[cfg(test)]
mod tests {
    use super::*;
    use crate::isi::test_support::{
        assert_registry_decodes_type_name as assert_registry_decodes, assert_slice_roundtrip,
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_primitives::numeric::Numeric;
    use norito::core::DecodeFromSlice;
    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("derive checked governance fixture account keypair");
        AccountId::new(key_pair.public_key().clone())
    }
    fn window() -> AtWindow {
        AtWindow {
            lower: 10,
            upper: 20,
        }
    }
    #[cfg(feature = "json")]
    fn assert_exact_json<T: norito::json::JsonSerialize>(value: &T) {
        let legacy = norito::json::to_json(value).expect("serialize legacy JSON");
        assert_eq!(
            norito::json::to_json_bounded(value, legacy.len()).expect("serialize at exact bound"),
            legacy
        );
        assert_eq!(
            norito::json::to_json_bounded(value, legacy.len() - 1),
            Err(norito::json::BoundedJsonError::BodyTooLarge)
        );
    }
    fn contract_address() -> crate::smart_contract::ContractAddress {
        "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
            .parse()
            .expect("contract address")
    }
    fn runtime_manifest() -> RuntimeUpgradeManifest {
        RuntimeUpgradeManifest {
            name: "runtime-upgrade".to_string(),
            description: "isi roundtrip".to_string(),
            abi_version: 1,
            abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
            added_syscalls: Vec::new(),
            added_pointer_types: Vec::new(),
            start_height: 100,
            end_height: 200,
            sbom_digests: Vec::new(),
            slsa_attestation: Vec::new(),
            provenance: Vec::new(),
        }
    }
    fn sccp_route_action() -> SccpRouteGovernanceActionV1 {
        SccpRouteGovernanceActionV1::Remove(crate::bridge::SccpRouteKeyV1 {
            lane_id: crate::bridge::SccpLaneIdV1 {
                source: crate::bridge::SccpNetworkV1::EthereumSepolia,
                target: crate::bridge::SccpNetworkV1::SoraTaira,
            },
            route_id: "taira_eth_xor".to_owned(),
            asset_key: "xor".to_owned(),
            revision: 1,
        })
    }
    fn sccp_route_anchor() -> crate::isi::bridge::SccpRouteGovernanceAnchorV1 {
        crate::isi::bridge::SccpRouteGovernanceAnchorV1 {
            network_id: NetworkId::from_genesis_hash(
                HashOf::<crate::block::BlockHeader>::from_untyped_unchecked(Hash::new(
                    b"SCCP governance instruction fixture network",
                )),
            ),
            action: sccp_route_action(),
        }
    }
    fn sorafs_provider_action() -> SorafsProviderGovernanceActionV1 {
        SorafsProviderGovernanceActionV1::Establish(
            crate::isi::sorafs::EstablishSorafsProviderOwnerV1 {
                provider_id: crate::sorafs::capacity::ProviderId::new([0x51; 32]),
                owner: account(1),
            },
        )
    }
    #[derive(Encode)]
    struct ForgedCastPlainBallot {
        referendum_id: String,
        owner: AccountId,
        amount: Numeric,
        duration_blocks: u64,
        direction: u8,
    }
    #[test]
    fn governance_amount_rejects_negative_numeric_payload() {
        let encoded = ForgedCastPlainBallot {
            referendum_id: "referendum-1".to_owned(),
            owner: account(1),
            amount: Numeric::new(-1_i32, 0),
            duration_blocks: 100,
            direction: 0,
        }
        .encode();
        assert!(CastPlainBallot::decode_from_slice(&encoded).is_err());
    }
    #[test]
    fn encode_roundtrip_basic() {
        let p = ProposeDeployContract {
            contract_address: contract_address(),
            code_hash_hex: "aa".repeat(32),
            abi_hash_hex: "bb".repeat(32),
            abi_version: "1".into(),
            window: Some(window()),
            mode: Some(VotingMode::Zk),
            manifest_provenance: None,
        };
        let enc = norito::codec::Encode::encode(&p);
        let mut cur = enc.as_slice();
        let dec = ProposeDeployContract::decode(&mut cur).unwrap();
        assert_eq!(p, dec);
    }
    #[cfg(feature = "json")]
    #[test]
    fn voting_mode_json_is_canonical_and_rejects_aliases() {
        assert_exact_json(&VotingMode::Zk);
        assert_exact_json(&VotingMode::Plain);
        assert_eq!(
            norito::json::to_json(&VotingMode::Zk).expect("serialize Zk voting mode"),
            "\"Zk\""
        );
        assert_eq!(
            norito::json::to_json(&VotingMode::Plain).expect("serialize Plain voting mode"),
            "\"Plain\""
        );
        assert_eq!(
            norito::json::from_str::<VotingMode>("\"Zk\"").expect("decode canonical Zk"),
            VotingMode::Zk
        );
        assert_eq!(
            norito::json::from_str::<VotingMode>("\"Plain\"").expect("decode canonical Plain"),
            VotingMode::Plain
        );
        for alias in ["zk", "plain", "PLAIN", " Zk", "Zk ", "Quadratic"] {
            let json = format!("\"{alias}\"");
            assert!(
                norito::json::from_str::<VotingMode>(&json).is_err(),
                "noncanonical voting mode alias must reject: {alias:?}"
            );
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn parliament_decision_json_is_exact_lowercase() {
        for (decision, label) in [
            (ParliamentDecision::Approve, "approve"),
            (ParliamentDecision::Reject, "reject"),
            (ParliamentDecision::Abstain, "abstain"),
        ] {
            assert_exact_json(&decision);
            let json = format!("\"{label}\"");
            assert_eq!(
                norito::json::to_json(&decision).expect("serialize parliament decision"),
                json
            );
            assert_eq!(
                norito::json::from_str::<ParliamentDecision>(&json)
                    .expect("decode canonical parliament decision"),
                decision
            );
        }
        for alias in ["Approve", "Reject", "Abstain", " approve", "approve "] {
            let json = format!("\"{alias}\"");
            assert!(
                norito::json::from_str::<ParliamentDecision>(&json).is_err(),
                "noncanonical parliament decision alias must reject: {alias:?}"
            );
        }
    }
    #[cfg(feature = "json")]
    #[test]
    fn council_derivation_json_has_exact_checked_bound() {
        assert_exact_json(&CouncilDerivationKind::Sortition);
        assert_exact_json(&CouncilDerivationKind::Manual);
    }
    #[test]
    fn runtime_upgrade_proposal_roundtrip() {
        let ins = ProposeRuntimeUpgradeProposal {
            manifest: runtime_manifest(),
            window: Some(window()),
            mode: Some(VotingMode::Plain),
        };
        let enc = norito::codec::Encode::encode(&ins);
        let mut cur = enc.as_slice();
        let dec = ProposeRuntimeUpgradeProposal::decode(&mut cur).unwrap();
        assert_eq!(ins, dec);
    }
    #[test]
    fn sccp_route_governance_proposal_roundtrip() {
        let ins = ProposeSccpRouteGovernance {
            anchor: sccp_route_anchor(),
            window: Some(window()),
            mode: Some(VotingMode::Plain),
        };
        let enc = norito::codec::Encode::encode(&ins);
        let mut cur = enc.as_slice();
        let dec = ProposeSccpRouteGovernance::decode(&mut cur).unwrap();
        assert_eq!(ins, dec);
    }
    #[test]
    fn sorafs_provider_governance_proposal_roundtrip() {
        let instruction = ProposeSorafsProviderGovernance {
            action: sorafs_provider_action(),
            window: Some(window()),
            mode: Some(VotingMode::Zk),
        };
        let encoded = instruction.encode();
        let decoded = ProposeSorafsProviderGovernance::decode(&mut encoded.as_slice())
            .expect("decode SoraFS provider-governance proposal");
        assert_eq!(instruction, decoded);
    }
    #[test]
    fn at_window_roundtrip() {
        let win = AtWindow { lower: 1, upper: 2 };
        let enc = norito::codec::Encode::encode(&win);
        let mut cur = enc.as_slice();
        let dec = AtWindow::decode(&mut cur).unwrap();
        assert_eq!(win, dec);
    }
    #[test]
    fn approve_proposal_roundtrip() {
        let ins = ApproveGovernanceProposal {
            body: ParliamentBody::AgendaCouncil,
            proposal_id: [0xAA; 32],
        };
        let enc = norito::codec::Encode::encode(&ins);
        let mut cur = enc.as_slice();
        let dec = ApproveGovernanceProposal::decode(&mut cur).unwrap();
        assert_eq!(ins, dec);
    }
    #[test]
    fn at_window_decodes_from_slice_via_norito() {
        let window = AtWindow {
            lower: 10,
            upper: 42,
        };
        let bytes = norito::codec::Encode::encode(&window);
        let (decoded, used) = <AtWindow as DecodeFromSlice>::decode_from_slice(&bytes)
            .expect("decode_from_slice should succeed");
        assert_eq!(decoded, window);
        assert_eq!(used, bytes.len());
    }
    #[test]
    #[allow(clippy::too_many_lines)]
    fn governance_decode_from_slice_roundtrips() {
        assert_slice_roundtrip(ProposeDeployContract {
            contract_address: contract_address(),
            code_hash_hex: "aa".repeat(32),
            abi_hash_hex: "bb".repeat(32),
            abi_version: "1".into(),
            window: Some(window()),
            mode: Some(VotingMode::Zk),
            manifest_provenance: None,
        });
        assert_slice_roundtrip(ProposeRuntimeUpgradeProposal {
            manifest: runtime_manifest(),
            window: Some(window()),
            mode: Some(VotingMode::Plain),
        });
        assert_slice_roundtrip(ProposeSccpRouteGovernance {
            anchor: sccp_route_anchor(),
            window: Some(window()),
            mode: Some(VotingMode::Plain),
        });
        assert_slice_roundtrip(ProposeSorafsProviderGovernance {
            action: sorafs_provider_action(),
            window: Some(window()),
            mode: Some(VotingMode::Zk),
        });
        assert_slice_roundtrip(CastZkBallot {
            election_id: "referendum-1".to_owned(),
            proof_b64: "AQID".to_owned(),
            public_inputs_json: "{\"vote\":\"aye\"}".to_owned(),
        });
        assert_slice_roundtrip(CastPlainBallot {
            referendum_id: "referendum-1".to_owned(),
            owner: account(1),
            amount: 1_000_u64.into(),
            duration_blocks: 100,
            direction: 0,
        });
        assert_slice_roundtrip(SlashGovernanceLock {
            referendum_id: "referendum-1".to_owned(),
            owner: account(1),
            amount: 100_u64.into(),
            reason: "misconduct".to_owned(),
        });
        assert_slice_roundtrip(RestituteGovernanceLock {
            referendum_id: "referendum-1".to_owned(),
            owner: account(1),
            amount: 50_u64.into(),
            reason: "appeal accepted".to_owned(),
        });
        assert_slice_roundtrip(EnactReferendum {
            referendum_id: [0x11; 32],
            preimage_hash: [0x22; 32],
            at_window: window(),
        });
        assert_slice_roundtrip(FinalizeReferendum {
            referendum_id: "referendum-1".to_owned(),
            proposal_id: [0x33; 32],
        });
        assert_slice_roundtrip(ApproveGovernanceProposal {
            body: ParliamentBody::AgendaCouncil,
            proposal_id: [0x44; 32],
        });
        assert_slice_roundtrip(CastParliamentBallot {
            body: ParliamentBody::PolicyJury,
            proposal_id: [0x45; 32],
            decision: ParliamentDecision::Reject,
        });
        assert_slice_roundtrip(PersistCouncilForEpoch {
            epoch: 7,
            members: vec![account(1), account(2)],
            alternates: vec![account(3)],
        });
        assert_slice_roundtrip(RecordCitizenServiceOutcome {
            owner: account(1),
            epoch: 7,
            role: "policy_jury".to_owned(),
            event: CitizenServiceEvent::NoShow,
        });
        assert_slice_roundtrip(RegisterCitizen {
            owner: account(1),
            amount: 2_000_u64.into(),
        });
        assert_slice_roundtrip(UnregisterCitizen { owner: account(1) });
    }
    #[test]
    fn governance_default_registry_decodes_type_names() {
        let registry = crate::isi::registry::default();
        assert_registry_decodes(
            &registry,
            ProposeDeployContract {
                contract_address: contract_address(),
                code_hash_hex: "aa".repeat(32),
                abi_hash_hex: "bb".repeat(32),
                abi_version: "1".into(),
                window: Some(window()),
                mode: Some(VotingMode::Zk),
                manifest_provenance: None,
            },
        );
        assert_registry_decodes(
            &registry,
            ProposeRuntimeUpgradeProposal {
                manifest: runtime_manifest(),
                window: Some(window()),
                mode: Some(VotingMode::Plain),
            },
        );
        assert_registry_decodes(
            &registry,
            ProposeSorafsProviderGovernance {
                action: sorafs_provider_action(),
                window: Some(window()),
                mode: Some(VotingMode::Zk),
            },
        );
        assert_registry_decodes(
            &registry,
            CastPlainBallot {
                referendum_id: "referendum-1".to_owned(),
                owner: account(1),
                amount: 1_000_u64.into(),
                duration_blocks: 100,
                direction: 0,
            },
        );
        assert_registry_decodes(
            &registry,
            CastParliamentBallot {
                body: ParliamentBody::PolicyJury,
                proposal_id: [0x45; 32],
                decision: ParliamentDecision::Approve,
            },
        );
        assert_registry_decodes(
            &registry,
            PersistCouncilForEpoch {
                epoch: 7,
                members: vec![account(1), account(2)],
                alternates: Vec::new(),
            },
        );
        assert_registry_decodes(
            &registry,
            RegisterCitizen {
                owner: account(1),
                amount: 2_000_u64.into(),
            },
        );
    }
}
