//! Governance domain events exposed via the governance data model.
//!
//! These events mirror the lifecycle described in `gov.md` and the roadmap,
//! providing typed Norito-serialisable payloads which higher layers can emit
//! once the corresponding execution paths are implemented.

use std::string::String;

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use crate::{
    account::AccountId,
    governance::types::{ParliamentBodies, ProposalId, VoteChoice},
};

/// Root/sudo execution outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub enum SudoExecutionResult {
    /// The wrapped call executed successfully.
    Success,
    /// The wrapped call failed with a deterministic error/trap code.
    Failure(SudoFailure),
}

/// Metadata describing a failed sudo execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct SudoFailure {
    /// Deterministic error/trap code emitted by the host.
    pub error_code: u32,
}

/// Governance event payloads.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub enum GovernanceEvent {
    /// Root/sudo execution completed.
    SudoExecuted(SudoExecuted),
    /// New referendum proposed.
    ReferendumProposed(ReferendumProposed),
    /// Referendum opened for voting.
    ReferendumOpened(ReferendumOpened),
    /// Vote cast for a referendum.
    VoteCast(VoteCast),
    /// Referendum tally computed.
    ReferendumTallied(ReferendumTallied),
    /// Referendum scheduled for enactment.
    GovernanceScheduled(GovernanceScheduled),
    /// Referendum enacted on chain.
    GovernanceEnacted(GovernanceEnacted),
    /// Governance execution failed inside the enactment path.
    GovernanceExecutionFailed(GovernanceExecutionFailed),
    /// Parliament bodies selected for an epoch.
    ParliamentSelected(ParliamentSelected),
    /// Parliament enactment executed.
    ParliamentEnacted(ParliamentEnacted),
    /// Parliament enactment failed to execute.
    ParliamentExecutionFailed(ParliamentExecutionFailed),
    /// Parliament could not complete its obligation before timeout.
    ParliamentTimeout(ParliamentTimeout),
    /// Parliament vetoed a proposal or preimage.
    ParliamentVetoed(ParliamentVetoed),
    /// Parliament member removed for cause.
    ParliamentMemberEjected(ParliamentMemberEjected),
    /// Submitted enactment certificate rejected.
    CertificateRejected(CertificateRejected),
    /// Referendum or enactment requires rescheduling.
    RescheduleRequired(RescheduleRequired),
    /// Fast-track decision granted to a referendum.
    FastTrackGranted(FastTrackGranted),
    /// Deposit slashed for a referendum.
    DepositSlashed(DepositSlashed),
}

/// Metadata emitted when sudo executes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct SudoExecuted {
    /// Version tag for the event payload.
    pub event_version: u16,
    /// Identifier of the proposal or context this execution belongs to.
    pub proposal_id: ProposalId,
    /// Blake2b-256 hash of the approving signature set.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub sig_set_hash: [u8; 32],
    /// Block height where the sudo action executed.
    pub at_height: u64,
    /// Result of executing the wrapped call.
    pub result: SudoExecutionResult,
}

/// Referendum proposal submitted.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct ReferendumProposed {
    /// Deterministic proposal identifier.
    pub proposal_id: ProposalId,
    /// Account that submitted the proposal.
    pub proposer: AccountId,
    /// Deposit locked while the referendum is active.
    pub deposit: u128,
}

/// Referendum opened for voting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct ReferendumOpened {
    /// Referendum identifier.
    pub referendum_id: ProposalId,
    /// Inclusive start height for voting.
    pub start: u64,
    /// Exclusive end height for voting.
    pub end: u64,
}

/// Vote recorded for a referendum.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct VoteCast {
    /// Referendum identifier.
    pub referendum_id: ProposalId,
    /// Account casting the vote.
    pub voter: AccountId,
    /// Conviction index applied to the vote.
    pub conviction: u8,
    /// Choice made by the voter.
    pub choice: VoteChoice,
}

/// Referendum tally summarising outcomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct ReferendumTallied {
    /// Referendum identifier.
    pub referendum_id: ProposalId,
    /// Conviction-weighted Aye total.
    pub aye: u128,
    /// Conviction-weighted Nay total.
    pub nay: u128,
    /// Conviction-weighted Abstain total.
    pub abstain: u128,
    /// Total turnout recorded for the tally.
    pub turnout: u128,
    /// Whether the referendum met approval criteria.
    pub approved: bool,
}

/// Referendum scheduled for execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct GovernanceScheduled {
    /// Referendum identifier.
    pub referendum_id: ProposalId,
    /// Lower bound of the execution window (inclusive).
    pub at_window_lower: u64,
    /// Upper bound of the execution window (inclusive).
    pub at_window_upper: u64,
}

/// Referendum enacted successfully.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct GovernanceEnacted {
    /// Referendum identifier.
    pub referendum_id: ProposalId,
    /// Block height at which enactment occurred.
    pub at_height: u64,
}

/// Referendum enactment failed to execute.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct GovernanceExecutionFailed {
    /// Referendum identifier.
    pub referendum_id: ProposalId,
    /// Blake2b-256 hash of the preimage acted upon.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub preimage_hash: [u8; 32],
    /// Block height where the failure occurred.
    pub at_height: u64,
    /// Deterministic error/trap code from the host.
    pub error_code: u32,
}

/// Parliament bodies selected for an epoch.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct ParliamentSelected {
    /// Epoch index the selection applies to.
    pub selection_epoch: u64,
    /// Body configuration snapshot persisted by the selection.
    pub bodies: ParliamentBodies,
}

/// Parliament execution succeeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct ParliamentEnacted {
    /// Epoch index the enactment applies to.
    pub selection_epoch: u64,
    /// Block height where enactment succeeded.
    pub at_height: u64,
}

/// Parliament execution failed with a deterministic error.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct ParliamentExecutionFailed {
    /// Epoch index the failure applies to.
    pub selection_epoch: u64,
    /// Blake2b-256 hash of the enactment preimage.
    #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
    pub preimage_hash: [u8; 32],
    /// Block height where the failure occurred.
    pub at_height: u64,
    /// Deterministic error/trap code.
    pub error_code: u32,
}

/// Parliament house type for timeout reporting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub enum ParliamentHouse {
    /// Rules Committee.
    RulesCommittee,
    /// Agenda Council.
    AgendaCouncil,
    /// Interest Panel.
    InterestPanel,
    /// Review Panel.
    ReviewPanel,
    /// Policy Jury.
    PolicyJury,
    /// Oversight Committee.
    OversightCommittee,
    /// Final MPC/FMA gate.
    FmaCommittee,
    /// Enactment House.
    Enactment,
}

/// Parliament missed its deadline and timed out.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct ParliamentTimeout {
    /// Epoch index the timeout applies to.
    pub selection_epoch: u64,
    /// Specific house that timed out.
    pub house: ParliamentHouse,
    /// Deterministic reason for the timeout.
    pub reason: String,
}

/// Parliament vetoed a proposal or manifest.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct ParliamentVetoed {
    /// Epoch index the veto applies to.
    pub selection_epoch: u64,
    /// Deterministic reason describing the veto.
    pub reason: String,
}

/// Parliament member removed for cause.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct ParliamentMemberEjected {
    /// Account removed from parliament.
    pub member: AccountId,
    /// Deterministic reason for removal.
    pub reason: String,
}

/// Enactment certificate rejected during validation.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct CertificateRejected {
    /// Deterministic reason describing the rejection.
    pub reason: String,
}

/// Enactment requires rescheduling.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct RescheduleRequired {
    /// Referendum or schedule subject that must be resubmitted.
    pub subject_id: ProposalId,
    /// Deterministic reason for the reschedule.
    pub reason: String,
}

/// Fast-track decision granted by parliament review house.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct FastTrackGranted {
    /// Referendum identifier.
    pub referendum_id: ProposalId,
    /// Previous enactment delay (in blocks).
    pub old_delay: u64,
    /// New enactment delay after fast-track (in blocks).
    pub new_delay: u64,
}

/// Deposit slashed for a referendum due to policy breaches.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
pub struct DepositSlashed {
    /// Referendum identifier associated with the slash.
    pub referendum_id: ProposalId,
    /// Amount forfeited from the deposit.
    pub amount: u128,
    /// Deterministic reason for the slash.
    pub reason: String,
}

/// Prelude exports for convenience.
pub mod prelude {
    pub use super::{
        CertificateRejected, DepositSlashed, FastTrackGranted, GovernanceEnacted, GovernanceEvent,
        GovernanceExecutionFailed, GovernanceScheduled, ParliamentEnacted,
        ParliamentExecutionFailed, ParliamentHouse, ParliamentMemberEjected, ParliamentSelected,
        ParliamentTimeout, ParliamentVetoed, ReferendumOpened, ReferendumProposed,
        ReferendumTallied, RescheduleRequired, SudoExecuted, SudoExecutionResult, SudoFailure,
        VoteCast,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_proposal_id(byte: u8) -> ProposalId {
        ProposalId([byte; 32])
    }

    #[test]
    fn sudo_executed_roundtrip() {
        let event = GovernanceEvent::SudoExecuted(SudoExecuted {
            event_version: 1,
            proposal_id: sample_proposal_id(0x11),
            sig_set_hash: [0xAA; 32],
            at_height: 1337,
            result: SudoExecutionResult::Failure(SudoFailure { error_code: 42 }),
        });

        let framed = norito::to_bytes(&event).expect("encode governance event");
        let decoded =
            norito::decode_from_bytes::<GovernanceEvent>(&framed).expect("decode governance event");
        assert_eq!(decoded, event);
    }

    #[test]
    fn referendum_tallied_roundtrip() {
        let event = GovernanceEvent::ReferendumTallied(ReferendumTallied {
            referendum_id: sample_proposal_id(0x22),
            aye: 10,
            nay: 4,
            abstain: 2,
            turnout: 16,
            approved: true,
        });

        let framed = norito::to_bytes(&event).expect("encode referendum tally");
        let decoded =
            norito::decode_from_bytes::<GovernanceEvent>(&framed).expect("decode referendum tally");
        assert_eq!(decoded, event);
    }

    #[test]
    fn parliament_timeout_roundtrip() {
        let event = GovernanceEvent::ParliamentTimeout(ParliamentTimeout {
            selection_epoch: 7,
            house: ParliamentHouse::ReviewPanel,
            reason: "missing quorum".to_owned(),
        });

        let framed = norito::to_bytes(&event).expect("encode parliament timeout");
        let decoded = norito::decode_from_bytes::<GovernanceEvent>(&framed)
            .expect("decode parliament timeout");
        assert_eq!(decoded, event);
    }
}
