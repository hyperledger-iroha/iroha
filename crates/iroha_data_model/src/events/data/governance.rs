//! Governance lifecycle events for the data event stream.

use iroha_data_model_derive::model;

pub use self::model::*;
use super::*;

#[model]
mod model {
    use super::*;
    use crate::governance::types::{ParliamentBodies, ParliamentBody};

    /// Governance lifecycle events.
    #[derive(
        Debug,
        Clone,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        iroha_data_model_derive::EventSet,
        Decode,
        Encode,
        iroha_schema::IntoSchema,
    )]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type)]
    pub enum GovernanceEvent {
        /// A governance proposal was submitted.
        ProposalSubmitted(GovernanceProposalSubmitted),
        /// A governance proposal was approved by referendum.
        ProposalApproved(GovernanceProposalApproved),
        /// A governance proposal was rejected by referendum.
        ProposalRejected(GovernanceProposalRejected),
        /// A governance token lock was created for a referendum.
        LockCreated(GovernanceLockCreated),
        /// A governance token lock was extended (expiry increased and/or amount increased) for a referendum.
        LockExtended(GovernanceLockExtended),
        /// A governance proposal was enacted.
        ProposalEnacted(GovernanceProposalEnacted),
        /// A ballot was accepted for a referendum (mode-specific fields may be hidden).
        BallotAccepted(GovernanceBallotAccepted),
        /// A ballot was rejected for a referendum (non-consensus; best-effort)
        BallotRejected(GovernanceBallotRejected),
        /// A referendum was opened for voting (status becomes Open).
        ReferendumOpened(GovernanceReferendumOpened),
        /// A referendum was closed (e.g., finalized/decided).
        ReferendumClosed(GovernanceReferendumClosed),
        /// A governance lock expired and was unlocked.
        LockUnlocked(GovernanceLockUnlocked),
        /// Council membership was persisted for an epoch.
        CouncilPersisted(GovernanceCouncilPersisted),
        /// Parliament bodies were derived for an epoch.
        ParliamentSelected(GovernanceParliamentSelected),
        /// A parliament body approval was recorded for a proposal.
        ParliamentApprovalRecorded(GovernanceParliamentApprovalRecorded),
        /// A governance lock was slashed (partial or full) for a referendum.
        LockSlashed(GovernanceLockSlashed),
        /// A governance lock received restitution after appeal.
        LockRestituted(GovernanceLockRestituted),
        /// A citizenship bond was registered.
        CitizenRegistered(GovernanceCitizenRegistered),
        /// A citizenship bond was withdrawn and the citizen was removed.
        CitizenRevoked(GovernanceCitizenRevoked),
        /// A citizen service discipline event was recorded.
        CitizenServiceRecorded(GovernanceCitizenServiceRecorded),
    }

    /// Proposal submitted payload.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceProposalSubmitted {
        /// Deterministic proposal id (blake2b‑32 of content fields)
        pub id: [u8; 32],
        /// Proposer account id
        pub proposer: crate::account::AccountId,
        /// Canonical public contract address targeted by the proposal when applicable.
        pub contract_address: Option<crate::smart_contract::ContractAddress>,
    }

    /// Lock created payload.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceLockCreated {
        /// Referendum identifier
        pub referendum_id: String,
        /// Voter account id
        pub owner: crate::account::AccountId,
        /// Locked amount
        pub amount: u128,
        /// Expiry height (inclusive)
        pub expiry_height: u64,
    }

    /// Lock extended payload.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceLockExtended {
        /// Referendum identifier
        pub referendum_id: String,
        /// Voter account id
        pub owner: crate::account::AccountId,
        /// New locked amount (after extension)
        pub amount: u128,
        /// New expiry height (inclusive)
        pub expiry_height: u64,
    }

    /// Proposal enacted payload.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceProposalEnacted {
        /// Deterministic proposal id
        pub id: [u8; 32],
    }

    /// Proposal approved payload.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceProposalApproved {
        /// Deterministic proposal id
        pub id: [u8; 32],
    }

    /// Proposal rejected payload.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceProposalRejected {
        /// Deterministic proposal id
        pub id: [u8; 32],
    }

    /// Ballot mode (ZK or Plain)
    #[derive(
        Debug,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Default,
        Decode,
        Encode,
        iroha_schema::IntoSchema,
    )]
    pub enum GovernanceBallotMode {
        /// Zero-knowledge ballot (direction and owner hidden)
        #[default]
        Zk,
        /// Transparent, quadratic-weighted ballot
        Plain,
    }

    /// Ballot accepted payload
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceBallotAccepted {
        /// Referendum identifier
        pub referendum_id: String,
        /// Voting mode (ZK/Plain)
        pub mode: GovernanceBallotMode,
        /// Optional weight when available (e.g., Plain mode); None for ZK ballots.
        pub weight: Option<u128>,
    }

    /// Ballot rejected payload
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceBallotRejected {
        /// Referendum identifier
        pub referendum_id: String,
        /// Free-form reason (stable messages preferred)
        pub reason: String,
    }

    /// Reason for slashing or restituting a governance bond.
    #[derive(
        Debug,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Default,
        Decode,
        Encode,
        iroha_schema::IntoSchema,
    )]
    pub enum GovernanceSlashReason {
        /// Duplicate/second ballot detected for the same referendum.
        DoubleVote,
        /// Proof or ballot deemed ineligible (e.g., stale eligibility root).
        IneligibleProof,
        /// Malicious or invalid submission (proof/metadata mismatch).
        Misconduct,
        /// Manual slashing triggered by an operator.
        #[default]
        Manual,
        /// Restitution granted after appeal or correction.
        Restitution,
    }

    /// Referendum opened payload
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceReferendumOpened {
        /// Referendum identifier
        pub id: String,
        /// Enactment window start height (inclusive)
        pub h_start: u64,
        /// Enactment window end height (inclusive)
        pub h_end: u64,
    }

    /// Referendum closed payload
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceReferendumClosed {
        /// Referendum identifier
        pub id: String,
        /// Block height at which the referendum was closed
        pub at_height: u64,
    }

    /// Lock unlocked payload.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceLockUnlocked {
        /// Referendum identifier
        pub referendum_id: String,
        /// Voter account id
        pub owner: crate::account::AccountId,
        /// Amount unlocked
        pub amount: u128,
    }

    /// Lock slashed payload.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceLockSlashed {
        /// Referendum identifier
        pub referendum_id: String,
        /// Voter account id
        pub owner: crate::account::AccountId,
        /// Amount slashed from the lock (smallest units).
        pub amount: u128,
        /// Reason for slashing (typed; use `Manual` for human-only reasons).
        pub reason: GovernanceSlashReason,
        /// Account that now custodians the slashed funds (may equal the escrow).
        pub destination: crate::account::AccountId,
        /// Free-form note attached to the slash decision.
        pub note: String,
    }

    /// Lock restitution payload.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceLockRestituted {
        /// Referendum identifier
        pub referendum_id: String,
        /// Voter account id
        pub owner: crate::account::AccountId,
        /// Amount restored to the lock (smallest units).
        pub amount: u128,
        /// Reason being rectified/appealed.
        pub reason: GovernanceSlashReason,
        /// Free-form note attached to the restitution decision.
        pub note: String,
    }

    /// Citizen registry entry created.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceCitizenRegistered {
        /// Account receiving citizenship.
        pub owner: crate::account::AccountId,
        /// Bonded amount held in escrow.
        pub amount: u128,
    }

    /// Citizen registry entry removed (bond returned).
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceCitizenRevoked {
        /// Account removed from the registry.
        pub owner: crate::account::AccountId,
        /// Amount returned from escrow.
        pub amount: u128,
    }

    /// Council persisted payload.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceCouncilPersisted {
        /// Epoch index
        pub epoch: u64,
        /// Number of members stored
        pub members_count: u32,
        /// Number of alternates stored alongside members.
        #[norito(default)]
        pub alternates_count: u32,
        /// Number of candidates whose VRF proofs verified.
        #[norito(default)]
        pub verified: u32,
        /// Total number of candidates considered (verified + rejected).
        #[norito(default)]
        pub candidates_count: u32,
        /// Derivation method (VRF or Fallback)
        pub derived_by: crate::isi::governance::CouncilDerivationKind,
    }

    /// Parliament selection recorded for an epoch.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceParliamentSelected {
        /// Epoch index associated with the selection.
        pub selection_epoch: u64,
        /// Body rosters for the epoch.
        pub bodies: ParliamentBodies,
    }

    /// Parliament approval recorded payload.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceParliamentApprovalRecorded {
        /// Proposal id receiving an approval.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub proposal_id: [u8; 32],
        /// Epoch of the approving council.
        pub epoch: u64,
        /// Parliament body granting the approval.
        pub body: ParliamentBody,
        /// Number of approvals recorded so far.
        pub approvals: u32,
        /// Quorum required to open the referendum.
        pub required: u32,
    }

    /// Citizen service discipline event payload.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceCitizenServiceRecorded {
        /// Citizen account receiving the record.
        pub owner: crate::account::AccountId,
        /// Epoch associated with the assignment.
        pub epoch: u64,
        /// Governance role label (e.g., `council` or `policy_jury`).
        pub role: String,
        /// Recorded event kind.
        pub event: crate::isi::governance::CitizenServiceEvent,
        /// Amount slashed from the citizenship bond (smallest units).
        pub slashed: u128,
        /// Height until which the citizen remains on cooldown.
        #[norito(default)]
        pub cooldown_until: u64,
    }
}

#[cfg(feature = "json")]
impl_json_via_norito_bytes!(
    GovernanceEvent,
    GovernanceProposalSubmitted,
    GovernanceLockCreated,
    GovernanceLockExtended,
    GovernanceProposalEnacted,
    GovernanceProposalApproved,
    GovernanceProposalRejected,
    GovernanceBallotMode,
    GovernanceBallotAccepted,
    GovernanceBallotRejected,
    GovernanceReferendumOpened,
    GovernanceReferendumClosed,
    GovernanceLockUnlocked,
    GovernanceCouncilPersisted,
    GovernanceParliamentSelected,
    GovernanceParliamentApprovalRecorded,
    GovernanceSlashReason,
    GovernanceLockSlashed,
    GovernanceLockRestituted,
    GovernanceCitizenRegistered,
    GovernanceCitizenRevoked,
    GovernanceCitizenServiceRecorded,
);

/// Prelude exports
pub mod prelude {
    pub use super::{
        GovernanceBallotAccepted, GovernanceBallotMode, GovernanceBallotRejected,
        GovernanceCitizenRegistered, GovernanceCitizenRevoked, GovernanceCitizenServiceRecorded,
        GovernanceCouncilPersisted, GovernanceEvent, GovernanceLockCreated, GovernanceLockExtended,
        GovernanceLockRestituted, GovernanceLockSlashed, GovernanceLockUnlocked,
        GovernanceParliamentApprovalRecorded, GovernanceParliamentSelected,
        GovernanceProposalApproved, GovernanceProposalEnacted, GovernanceProposalRejected,
        GovernanceProposalSubmitted, GovernanceReferendumClosed, GovernanceReferendumOpened,
        GovernanceSlashReason,
    };
}
