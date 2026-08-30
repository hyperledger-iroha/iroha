//! Governance lifecycle events for the data event stream.
pub use self::model::*;
use super::*;
use iroha_data_model_derive::model;
#[model]
mod model {
    use super::*;
    use crate::{
        governance::types::{
            BallotAttemptId, BallotAttemptStatusV1, BodyInstanceId, BodyInstanceStatusV1,
            GovernanceAttemptId, GovernanceAttemptStatusV1, GovernanceCertificateId,
            GovernanceExpectedHeadV1, GovernanceStageV1, ParliamentAggregateOutcomeV1,
            ParliamentAggregateTallyV1, ParliamentBodies, ParliamentBody,
            ParliamentConcentrationWarningV1, ParliamentNoResultKindV1, ProposalContentId,
            RiskTierV1,
        },
        isi::governance::{
            ParliamentAutomaticExecutionOutcomeV1, ParliamentLifecycleTransitionKindV1,
        },
    };
    use iroha_primitives::numeric::Quantity;
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
        /// Compatibility event emitted by the retired manual epoch-council instruction.
        CouncilPersisted(GovernanceCouncilPersisted),
        /// Compatibility event emitted by the retired independent epoch-council subsystem.
        ParliamentSelected(GovernanceParliamentSelected),
        /// A canonical attempt was created from immutable proposal content.
        ParliamentAttemptCreated(GovernanceParliamentAttemptCreated),
        /// One typed reducer transition was accepted and applied.
        ParliamentLifecycleTransitionApplied(GovernanceParliamentLifecycleTransitionApplied),
        /// A retryable Parliament governance attempt changed lifecycle state.
        ParliamentAttemptTransitioned(GovernanceParliamentAttemptTransitioned),
        /// A sealed Parliament body instance changed lifecycle state.
        ParliamentBodyTransitioned(GovernanceParliamentBodyTransitioned),
        /// A hidden Parliament ballot attempt changed lifecycle state.
        ParliamentBallotTransitioned(GovernanceParliamentBallotTransitioned),
        /// A nonempty feasible roster was sealed below its requested diversity or size.
        ParliamentConcentrationWarning(GovernanceParliamentConcentrationWarning),
        /// A hidden Parliament aggregate result was finalized.
        ParliamentAggregateFinalized(GovernanceParliamentAggregateFinalized),
        /// A current-roster QC installed, rotated, or retired a threshold key session.
        ThresholdKeyLifecycleApplied(GovernanceThresholdKeyLifecycleAppliedV1),
        /// A complete V1 governance certificate was issued automatically.
        ParliamentCertificateIssued(GovernanceParliamentCertificateIssued),
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

    /// Public audit record for one QC-authorized threshold-key lifecycle action.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceThresholdKeyLifecycleAppliedV1 {
        /// Exact lifecycle action authenticated by the current validator roster.
        pub action: crate::isi::consensus_keys::ThresholdKeyLifecycleActionV1,
        /// Exact threshold-key session identifier.
        pub session_id: [u8; 32],
        /// Exact finalized DKG transcript commitment.
        pub transcript_hash: [u8; 32],
        /// Committed height at which the lifecycle action took effect.
        pub effective_height: u64,
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
        pub amount: Quantity,
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
        pub amount: Quantity,
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
        pub amount: Quantity,
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
        /// Exact amount slashed from the lock.
        pub amount: Quantity,
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
        /// Exact amount restored to the lock.
        pub amount: Quantity,
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
        pub amount: Quantity,
    }
    /// Citizen registry entry removed (bond returned).
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceCitizenRevoked {
        /// Account removed from the registry.
        pub owner: crate::account::AccountId,
        /// Amount returned from escrow.
        pub amount: Quantity,
    }
    /// Compatibility payload from the retired manual epoch-council instruction.
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
        /// Total eligible candidates considered by the historical subsystem.
        #[norito(default)]
        pub candidates_count: u32,
        /// Derivation method.
        pub derived_by: crate::isi::governance::CouncilDerivationKind,
    }
    /// Compatibility payload from the retired independent epoch-council subsystem.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceParliamentSelected {
        /// Epoch index associated with the selection.
        pub selection_epoch: u64,
        /// Body rosters for the epoch.
        pub bodies: ParliamentBodies,
    }
    /// Canonical creation of one retryable end-to-end Parliament attempt.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceParliamentAttemptCreated {
        /// Immutable proposal content shared by every retry.
        pub proposal_content_id: ProposalContentId,
        /// Canonical identifier derived from content and retry sequence.
        pub governance_attempt_id: GovernanceAttemptId,
        /// Zero-based retry sequence for the proposal content.
        pub attempt_sequence: u32,
        /// Policy-derived initial risk tier.
        pub risk_tier: RiskTierV1,
        /// Governance policy version frozen for the attempt.
        pub policy_version: u64,
        /// Hash of the exact deterministic effect preimage.
        pub effect_preimage_hash: [u8; 32],
        /// Compare-and-set head frozen when the attempt was created.
        pub expected_head: GovernanceExpectedHeadV1,
        /// Block height creating the attempt.
        pub at_height: u64,
    }
    /// Bounded audit record for one accepted Parliament reducer command or
    /// consensus-derived certificate-execution outcome.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceParliamentLifecycleTransitionApplied {
        /// Immutable proposal content being processed.
        pub proposal_content_id: ProposalContentId,
        /// Retry attempt that consumed the transition.
        pub governance_attempt_id: GovernanceAttemptId,
        /// Stable bounded classification of the public transition or automatic outcome.
        pub transition_kind: ParliamentLifecycleTransitionKindV1,
        /// Core-derived bounded no-result class, present only when a body terminalizes without a result.
        pub no_result_kind: Option<ParliamentNoResultKindV1>,
        /// Typed consensus-owned terminal outcome, absent for submitted transitions.
        pub automatic_outcome: Option<ParliamentAutomaticExecutionOutcomeV1>,
        /// Domain-separated digest of the submitted transition or automatic outcome.
        ///
        /// A no-result transition carries no caller-selected failure class; its
        /// separate `no_result_kind` field records that Core-derived evidence.
        pub transition_digest: [u8; 32],
        /// Certificate produced by this transition, when applicable.
        pub certificate_id: Option<GovernanceCertificateId>,
        /// Block height applying the transition.
        pub at_height: u64,
    }
    /// End-to-end Parliament attempt lifecycle transition.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceParliamentAttemptTransitioned {
        /// Immutable proposal content shared by every retry.
        pub proposal_content_id: ProposalContentId,
        /// Retry attempt whose state changed.
        pub governance_attempt_id: GovernanceAttemptId,
        /// Sequential stage occupied after the transition.
        pub stage: GovernanceStageV1,
        /// Attempt status after the transition.
        pub status: GovernanceAttemptStatusV1,
        /// Block height applying the transition.
        pub at_height: u64,
    }
    /// Sealed Parliament body lifecycle transition.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceParliamentBodyTransitioned {
        /// End-to-end governance attempt served by the body.
        pub governance_attempt_id: GovernanceAttemptId,
        /// Body instance whose state changed.
        pub body_instance_id: BodyInstanceId,
        /// Parliament role of the body instance.
        pub body: ParliamentBody,
        /// Body status after the transition.
        pub status: BodyInstanceStatusV1,
        /// Block height applying the transition.
        pub at_height: u64,
    }
    /// Hidden Parliament ballot lifecycle transition.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceParliamentBallotTransitioned {
        /// Body instance whose formal decision the ballot determines.
        pub body_instance_id: BodyInstanceId,
        /// Ballot attempt whose state changed.
        pub ballot_attempt_id: BallotAttemptId,
        /// Ballot status after the transition.
        pub status: BallotAttemptStatusV1,
        /// Block height applying the transition.
        pub at_height: u64,
    }
    /// Roster concentration warning emitted at body sealing.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceParliamentConcentrationWarning {
        /// Canonical warning details.
        pub warning: ParliamentConcentrationWarningV1,
        /// Block height at which the undersized or concentrated roster was sealed.
        pub at_height: u64,
    }
    /// Final hidden aggregate result for one Parliament ballot attempt.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceParliamentAggregateFinalized {
        /// Immutable proposal content being decided.
        pub proposal_content_id: ProposalContentId,
        /// End-to-end governance retry attempt.
        pub governance_attempt_id: GovernanceAttemptId,
        /// Body instance contributing the result.
        pub body_instance_id: BodyInstanceId,
        /// Hidden ballot attempt that produced the aggregate.
        pub ballot_attempt_id: BallotAttemptId,
        /// Canonical aggregate counts.
        pub tally: ParliamentAggregateTallyV1,
        /// Final aggregate outcome.
        pub outcome: ParliamentAggregateOutcomeV1,
        /// Whether this Policy Jury result triggers a disjoint Confirmation Jury.
        pub requires_confirmation: bool,
        /// Block height at which the aggregate was finalized.
        pub at_height: u64,
    }
    /// Automatic V1 governance certificate issuance event.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceParliamentCertificateIssued {
        /// Content hash of the complete governance certificate.
        pub certificate_id: GovernanceCertificateId,
        /// Immutable proposal content authorized by the certificate.
        pub proposal_content_id: ProposalContentId,
        /// Successful end-to-end attempt that produced the certificate.
        pub governance_attempt_id: GovernanceAttemptId,
        /// Block height at which the certificate was finalized.
        pub certified_at_height: u64,
        /// Exact deterministic enactment height.
        pub enact_at_height: u64,
    }
    /// Parliament approval recorded payload.
    #[derive(
        Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, iroha_schema::IntoSchema,
    )]
    pub struct GovernanceParliamentApprovalRecorded {
        /// Proposal id receiving an approval.
        #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::fixed_bytes"))]
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
        /// Exact amount slashed from the citizenship bond.
        pub slashed: Quantity,
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
    GovernanceParliamentAttemptCreated,
    GovernanceParliamentLifecycleTransitionApplied,
    GovernanceParliamentAttemptTransitioned,
    GovernanceParliamentBodyTransitioned,
    GovernanceParliamentBallotTransitioned,
    GovernanceParliamentConcentrationWarning,
    GovernanceParliamentAggregateFinalized,
    GovernanceThresholdKeyLifecycleAppliedV1,
    GovernanceParliamentCertificateIssued,
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
        GovernanceParliamentAggregateFinalized, GovernanceParliamentApprovalRecorded,
        GovernanceParliamentAttemptCreated, GovernanceParliamentAttemptTransitioned,
        GovernanceParliamentBallotTransitioned, GovernanceParliamentBodyTransitioned,
        GovernanceParliamentCertificateIssued, GovernanceParliamentConcentrationWarning,
        GovernanceParliamentLifecycleTransitionApplied, GovernanceParliamentSelected,
        GovernanceProposalApproved, GovernanceProposalEnacted, GovernanceProposalRejected,
        GovernanceProposalSubmitted, GovernanceReferendumClosed, GovernanceReferendumOpened,
        GovernanceSlashReason, GovernanceThresholdKeyLifecycleAppliedV1,
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::types::{
        BallotAttemptId, BallotAttemptStatusV1, BodyInstanceId, BodyInstanceStatusV1,
        GovernanceAttemptId, GovernanceAttemptStatusV1, GovernanceCertificateId,
        GovernanceExpectedHeadAbsentV1, GovernanceExpectedHeadV1, GovernanceStageV1,
        ParliamentAggregateOutcomeV1, ParliamentAggregateTallyV1, ParliamentBody,
        ParliamentConcentrationWarningV1, ParliamentNoResultKindV1, ProposalContentId, RiskTierV1,
    };
    use crate::isi::governance::{
        ParliamentAutomaticExecutionFailedV1, ParliamentAutomaticExecutionOutcomeV1,
        ParliamentCloseBallotRegistrationV1, ParliamentLifecycleTransitionKindV1,
        ParliamentLifecycleTransitionV1,
    };

    fn assert_roundtrip(event: GovernanceEvent) {
        let bytes = norito::to_bytes(&event).expect("encode canonical governance event");
        let decoded = norito::decode_from_bytes::<GovernanceEvent>(&bytes)
            .expect("decode canonical governance event");
        assert_eq!(decoded, event);
    }

    #[test]
    fn parliament_v1_lifecycle_events_roundtrip() {
        let proposal_content_id = ProposalContentId::new([0x11; 32]);
        let governance_attempt_id = GovernanceAttemptId::new([0x12; 32]);
        let body_instance_id = BodyInstanceId::new([0x13; 32]);
        let ballot_attempt_id = BallotAttemptId::new([0x14; 32]);
        assert_roundtrip(GovernanceEvent::ParliamentAttemptCreated(
            GovernanceParliamentAttemptCreated {
                proposal_content_id,
                governance_attempt_id,
                attempt_sequence: 0,
                risk_tier: RiskTierV1::Standard,
                policy_version: 1,
                effect_preimage_hash: [0x15; 32],
                expected_head: GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
                    subject_id: [0x16; 32],
                }),
                at_height: 99,
            },
        ));
        assert_roundtrip(GovernanceEvent::ParliamentLifecycleTransitionApplied(
            GovernanceParliamentLifecycleTransitionApplied {
                proposal_content_id,
                governance_attempt_id,
                transition_kind: ParliamentLifecycleTransitionKindV1::CompleteQualification,
                no_result_kind: None,
                automatic_outcome: None,
                transition_digest: ParliamentLifecycleTransitionV1::CompleteQualification
                    .digest_v1(),
                certificate_id: None,
                at_height: 100,
            },
        ));
        assert_roundtrip(GovernanceEvent::ParliamentLifecycleTransitionApplied(
            GovernanceParliamentLifecycleTransitionApplied {
                proposal_content_id,
                governance_attempt_id,
                transition_kind: ParliamentLifecycleTransitionKindV1::FailPublicFindingNoResult,
                no_result_kind: Some(ParliamentNoResultKindV1::PublicFindingDeadlineExpired),
                automatic_outcome: None,
                transition_digest: [0x19; 32],
                certificate_id: None,
                at_height: 101,
            },
        ));
        assert_roundtrip(GovernanceEvent::ParliamentLifecycleTransitionApplied(
            GovernanceParliamentLifecycleTransitionApplied {
                proposal_content_id,
                governance_attempt_id,
                transition_kind: ParliamentLifecycleTransitionKindV1::FailBallotNoResult,
                no_result_kind: Some(ParliamentNoResultKindV1::BallotOpeningDeadlineExpired),
                automatic_outcome: None,
                transition_digest: [0x20; 32],
                certificate_id: None,
                at_height: 101,
            },
        ));
        let automatic_outcome = ParliamentAutomaticExecutionOutcomeV1::ExecutionFailed(
            ParliamentAutomaticExecutionFailedV1 {
                effect_preimage_hash: [0x21; 32],
                failure_root: [0x22; 32],
            },
        );
        assert_roundtrip(GovernanceEvent::ParliamentLifecycleTransitionApplied(
            GovernanceParliamentLifecycleTransitionApplied {
                proposal_content_id,
                governance_attempt_id,
                transition_kind: automatic_outcome.kind(),
                no_result_kind: None,
                automatic_outcome: Some(automatic_outcome),
                transition_digest: automatic_outcome.digest_v1(),
                certificate_id: Some(GovernanceCertificateId::new([0x23; 32])),
                at_height: 102,
            },
        ));
        assert_roundtrip(GovernanceEvent::ParliamentAttemptTransitioned(
            GovernanceParliamentAttemptTransitioned {
                proposal_content_id,
                governance_attempt_id,
                stage: GovernanceStageV1::PolicyJury,
                status: GovernanceAttemptStatusV1::Active,
                at_height: 100,
            },
        ));
        assert_roundtrip(GovernanceEvent::ParliamentBodyTransitioned(
            GovernanceParliamentBodyTransitioned {
                governance_attempt_id,
                body_instance_id,
                body: ParliamentBody::PolicyJury,
                status: BodyInstanceStatusV1::Balloting,
                at_height: 101,
            },
        ));
        assert_roundtrip(GovernanceEvent::ParliamentBallotTransitioned(
            GovernanceParliamentBallotTransitioned {
                body_instance_id,
                ballot_attempt_id,
                status: BallotAttemptStatusV1::AwaitingRelease,
                at_height: 102,
            },
        ));
    }

    #[test]
    fn parliament_lifecycle_audit_event_never_embeds_transition_evidence() {
        let proposal_content_id = ProposalContentId::new([0x17; 32]);
        let governance_attempt_id = GovernanceAttemptId::new([0x18; 32]);
        let transition = ParliamentLifecycleTransitionV1::CloseBallotRegistration(
            ParliamentCloseBallotRegistrationV1 {
                ballot_attempt_id: BallotAttemptId::new([0x19; 32]),
            },
        );
        let event = GovernanceEvent::ParliamentLifecycleTransitionApplied(
            GovernanceParliamentLifecycleTransitionApplied {
                proposal_content_id,
                governance_attempt_id,
                transition_kind: transition.kind(),
                no_result_kind: None,
                automatic_outcome: None,
                transition_digest: transition.digest_v1(),
                certificate_id: None,
                at_height: 103,
            },
        );
        let encoded = norito::to_bytes(&event).expect("encode bounded lifecycle audit event");
        assert!(
            encoded.len() < 512,
            "audit event must not duplicate evidence"
        );
        assert_roundtrip(event);
    }

    #[test]
    fn parliament_v1_result_and_certificate_events_roundtrip() {
        let proposal_content_id = ProposalContentId::new([0x21; 32]);
        let governance_attempt_id = GovernanceAttemptId::new([0x22; 32]);
        let body_instance_id = BodyInstanceId::new([0x23; 32]);
        let ballot_attempt_id = BallotAttemptId::new([0x24; 32]);
        assert_roundtrip(GovernanceEvent::ParliamentConcentrationWarning(
            GovernanceParliamentConcentrationWarning {
                warning: ParliamentConcentrationWarningV1 {
                    body_instance_id,
                    body: ParliamentBody::ConfirmationJury,
                    target_seats: 1_000,
                    sealed_seats: 731,
                    eligible_candidates: 731,
                    cross_body_assignment_cap: 2,
                },
                at_height: 200,
            },
        ));
        assert_roundtrip(GovernanceEvent::ParliamentAggregateFinalized(
            GovernanceParliamentAggregateFinalized {
                proposal_content_id,
                governance_attempt_id,
                body_instance_id,
                ballot_attempt_id,
                tally: ParliamentAggregateTallyV1 {
                    original_seats: 500,
                    accepted_ballots: 334,
                    aye: 170,
                    nay: 160,
                    abstain: 4,
                },
                outcome: ParliamentAggregateOutcomeV1::Approved,
                requires_confirmation: true,
                at_height: 201,
            },
        ));
        assert_roundtrip(GovernanceEvent::ParliamentCertificateIssued(
            GovernanceParliamentCertificateIssued {
                certificate_id: GovernanceCertificateId::new([0x25; 32]),
                proposal_content_id,
                governance_attempt_id,
                certified_at_height: 202,
                enact_at_height: 203,
            },
        ));
    }

    #[test]
    fn threshold_key_lifecycle_event_roundtrips() {
        assert_roundtrip(GovernanceEvent::ThresholdKeyLifecycleApplied(
            GovernanceThresholdKeyLifecycleAppliedV1 {
                action: crate::isi::consensus_keys::ThresholdKeyLifecycleActionV1::InstallGlobalBeaconKey,
                session_id: [0x31; 32],
                transcript_hash: [0x32; 32],
                effective_height: 41,
            },
        ));
    }

    #[test]
    fn parliament_event_schema_excludes_per_voter_plaintext() {
        let retired_variant = ["Parliament", "Ballot", "Recorded"].concat();
        let retired_payload = ["Governance", retired_variant.as_str()].concat();
        let retired_decision = ["Parliament", "Decision"].concat();
        let retired_decision_declaration = ["pub enum ", retired_decision.as_str(), " {"].concat();

        let event_source = include_str!("governance.rs");
        let instruction_source = include_str!("../../isi/governance.rs");
        assert!(!event_source.contains(&retired_variant));
        assert!(!event_source.contains(&retired_payload));
        assert!(!instruction_source.contains(&retired_decision_declaration));

        let schema = <GovernanceEvent as iroha_schema::IntoSchema>::schema();
        let metadata = schema
            .get::<GovernanceEvent>()
            .expect("governance event schema entry");
        let iroha_schema::Metadata::Enum(event_metadata) = metadata else {
            panic!("governance event schema must remain an enum");
        };
        assert!(
            event_metadata
                .variants
                .iter()
                .all(|variant| variant.tag != retired_variant)
        );
        assert!(schema.iter().all(|(_, entry)| {
            entry.type_name != retired_payload && entry.type_name != retired_decision
        }));
    }
}
