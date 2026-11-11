//! Attempt creation and lifecycle transitions of the anonymous Parliament definition
//! family (design §12.3).
//!
//! An anonymous attempt is created through its own interface and keeps its own State from
//! creation; an attempt of the frozen baseline is never switched. The family has three
//! interfaces:
//!
//! - [`CreateAnonymousParliamentAttemptV1`] admits exact proposal content. The caller
//!   selects neither the purpose of the attempt nor its policy: Core records the purpose
//!   from the network's original genesis policy or the release qualification, and freezes
//!   the policy from committed parameters.
//! - [`SubmitAnonymousParliamentTransitionV1`] carries the account-signed transitions that
//!   remain: qualification, sortition, invitations, public-body deliberation and findings.
//! - [`super::RegisterParliamentCredentialV1`] accepts a binding seat with its credential
//!   (design §3.2).
//!
//! There is no ballot transition. A binding body has exactly one ballot; it opens and
//! closes in the positioned block-start pass, and ballots are signerless inputs (design
//! §3.4 to §3.7). The timed-OVN transitions of the baseline have no counterpart here.
//!
//! The shared payloads are the baseline's own types. They are frozen with the baseline
//! and therefore cannot change under this family either.
//!
//! **Nothing here is registered.** Until a governed change set activates the family, its
//! wire identifiers resolve as inactive and these payloads travel only inside the ordinary
//! opaque envelope.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::{AnonymousGovernanceAttemptV1, BodyDecisionModeV1, anonymous_governance_attempt_id_v1};
use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize,
    governance::types::{GovernanceAttemptId, ProposalContentId, ProposalKind, RiskTierV1},
    governance_fingerprint::{ANONYMOUS_PARLIAMENT_TRANSITION_DIGEST_V1, fingerprint},
    isi::governance::{
        ParliamentAdvanceBodyPhaseV1, ParliamentBeginInvitationAcceptanceV1,
        ParliamentConsumeSortitionPulseBatchV1, ParliamentEndorsePublicFindingV1,
        ParliamentEscalateRiskV1, ParliamentFailBodyElectionNoRosterV1,
        ParliamentFailPublicFindingNoResultV1, ParliamentInvitationDecisionV1,
        ParliamentLifecycleTransitionV1, ParliamentRecordAttemptAbsenceV1,
        ParliamentRecordInvitationResponseV1, ParliamentRegisterSortitionRequestV1,
        ParliamentSealBodyRosterV1,
    },
};

/// Create one retryable anonymous Parliament attempt for exact immutable proposal content.
///
/// The instruction carries no caller-selected identifier, purpose, status, stage, policy,
/// effect hash, compare-and-set head or body pipeline. Core derives every one of them from
/// the proposal and committed state.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::governance::anonymous_ballot::CreateAnonymousParliamentAttemptV1"
)]
pub struct CreateAnonymousParliamentAttemptV1 {
    /// Exact typed proposal whose fingerprint every retry shares.
    pub proposal: ProposalKind,
    /// Zero-based end-to-end retry sequence for this proposal content.
    pub attempt_sequence: u32,
}

impl CreateAnonymousParliamentAttemptV1 {
    /// Wire identifier of the interface; no registry or selection activates it yet.
    pub const WIRE_ID: &'static str = "iroha.governance.parliament.anonymous.attempt.create.v1";

    /// The immutable proposal-content identifier.
    #[must_use]
    pub fn proposal_content_id(&self) -> ProposalContentId {
        ProposalContentId::derive_v1(&self.proposal)
    }

    /// The only valid identifier of this attempt. It is the family's own derivation, so it
    /// never equals the baseline identifier of the same proposal and sequence.
    #[must_use]
    pub fn governance_attempt_id(&self) -> GovernanceAttemptId {
        anonymous_governance_attempt_id_v1(self.proposal_content_id(), self.attempt_sequence)
    }

    /// The initial attempt snapshot under a policy-derived risk tier.
    #[must_use]
    pub fn canonical_attempt(&self, risk_tier: RiskTierV1) -> AnonymousGovernanceAttemptV1 {
        AnonymousGovernanceAttemptV1::new_v1(
            self.proposal_content_id(),
            self.attempt_sequence,
            risk_tier,
        )
    }
}

/// One account-signed transition of an anonymous Parliament attempt.
///
/// The executing height and time are absent: Core supplies those of the containing block.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(tag = "transition", content = "payload", deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::governance::anonymous_ballot::AnonymousParliamentTransitionV1"
)]
pub enum AnonymousParliamentTransitionV1 {
    /// Escalate the risk tier before the Policy Jury sortition freezes it.
    #[codec(index = 0)]
    EscalateRisk(ParliamentEscalateRiskV1),
    /// Finish qualification and enter the first required body.
    #[codec(index = 1)]
    CompleteQualification,
    /// Register the first complete sortition generation from consensus-derived inputs.
    #[codec(index = 2)]
    RegisterInitialSortition,
    /// Register one atomic immutable request batch for a future beacon pulse.
    #[codec(index = 3)]
    RegisterSortitionRequest(ParliamentRegisterSortitionRequestV1),
    /// Consume a finalized threshold-beacon pulse for a complete request batch.
    #[codec(index = 4)]
    ConsumeSortitionPulseBatch(ParliamentConsumeSortitionPulseBatchV1),
    /// Derive the deterministic draw plan and begin invitation acceptance.
    #[codec(index = 5)]
    BeginInvitationAcceptance(ParliamentBeginInvitationAcceptanceV1),
    /// Record the authority's response to its invitation: a decline of any seat, or the
    /// acceptance of a public-body seat. A binding seat is accepted by
    /// [`super::RegisterParliamentCredentialV1`], never here.
    #[codec(index = 6)]
    RecordInvitationResponse(ParliamentRecordInvitationResponseV1),
    /// Terminally record an election whose invitation window closed without a viable
    /// roster.
    #[codec(index = 7)]
    FailBodyElectionNoRoster(ParliamentFailBodyElectionNoRosterV1),
    /// Seal a canonical roster into a body instance; a binding body also seals its
    /// credential list (design §3.3).
    #[codec(index = 8)]
    SealBodyRoster(ParliamentSealBodyRosterV1),
    /// Advance one sealed body by exactly one deliberation phase.
    #[codec(index = 9)]
    AdvanceBodyPhase(ParliamentAdvanceBodyPhaseV1),
    /// Record a public-body member's own absence. Core refuses it for a binding body,
    /// where non-participation appears only in turnout (design §7).
    #[codec(index = 10)]
    RecordAttemptAbsence(ParliamentRecordAttemptAbsenceV1),
    /// Record one seated member's endorsement of a public nonbinding finding.
    #[codec(index = 11)]
    EndorsePublicFinding(ParliamentEndorsePublicFindingV1),
    /// Terminally record expiry of a public-finding endorsement window.
    #[codec(index = 12)]
    FailPublicFindingNoResult(ParliamentFailPublicFindingNoResultV1),
}

impl AnonymousParliamentTransitionV1 {
    /// The same transition in the baseline's closed form: every payload here is one of the
    /// baseline's, so its state-independent rules are the baseline's rules.
    fn shared_form(&self) -> ParliamentLifecycleTransitionV1 {
        use ParliamentLifecycleTransitionV1 as Shared;
        match self {
            Self::EscalateRisk(payload) => Shared::EscalateRisk(*payload),
            Self::CompleteQualification => Shared::CompleteQualification,
            Self::RegisterInitialSortition => Shared::RegisterInitialSortition,
            Self::RegisterSortitionRequest(payload) => {
                Shared::RegisterSortitionRequest(payload.clone())
            }
            Self::ConsumeSortitionPulseBatch(payload) => {
                Shared::ConsumeSortitionPulseBatch(payload.clone())
            }
            Self::BeginInvitationAcceptance(payload) => Shared::BeginInvitationAcceptance(*payload),
            Self::RecordInvitationResponse(payload) => Shared::RecordInvitationResponse(*payload),
            Self::FailBodyElectionNoRoster(payload) => Shared::FailBodyElectionNoRoster(*payload),
            Self::SealBodyRoster(payload) => Shared::SealBodyRoster(*payload),
            Self::AdvanceBodyPhase(payload) => Shared::AdvanceBodyPhase(*payload),
            Self::RecordAttemptAbsence(payload) => Shared::RecordAttemptAbsence(*payload),
            Self::EndorsePublicFinding(payload) => Shared::EndorsePublicFinding(*payload),
            Self::FailPublicFindingNoResult(payload) => Shared::FailPublicFindingNoResult(*payload),
        }
    }

    /// Reject a malformed, unbounded or cross-attempt transition before stateful execution.
    ///
    /// Authority, phase, height, time, candidate corpus and proof checks need committed
    /// state and stay with Core.
    ///
    /// # Errors
    /// A stable message for the first violated state-independent rule.
    pub fn validate_static_for_attempt(
        &self,
        governance_attempt_id: GovernanceAttemptId,
    ) -> Result<(), &'static str> {
        if let Self::RecordInvitationResponse(payload) = self
            && payload.decision == ParliamentInvitationDecisionV1::Accept
            && BodyDecisionModeV1::of(payload.body) == BodyDecisionModeV1::AnonymousBallot
        {
            return Err("a binding seat is accepted by registering its credential");
        }
        self.shared_form()
            .validate_static_for_attempt(governance_attempt_id)
    }

    /// A domain-separated digest of the exact transition. The domain is the family's own:
    /// a transition and its baseline form never share a digest.
    #[must_use]
    pub fn digest_v1(&self) -> [u8; 32] {
        fingerprint(ANONYMOUS_PARLIAMENT_TRANSITION_DIGEST_V1, self)
    }
}

/// Submit one transition for an existing anonymous Parliament attempt.
///
/// A member's own invitation response, absence declaration and public-finding endorsement
/// are bound to the signed authority. A transition carries a fact or an intent, never an
/// outcome: Core derives or rechecks every height, time, snapshot, roster and budget it
/// names. Which of the remaining transitions are permissionless liveness triggers and
/// which need the `CanManageParliament` permission is Core's rule; the `NoRoster` retry
/// generation stays permissionless (design §10).
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(
    name = "iroha_data_model::governance::anonymous_ballot::SubmitAnonymousParliamentTransitionV1"
)]
pub struct SubmitAnonymousParliamentTransitionV1 {
    /// Attempt whose reducer must consume the transition.
    pub governance_attempt_id: GovernanceAttemptId,
    /// Exact closed transition payload.
    pub transition: AnonymousParliamentTransitionV1,
}

impl SubmitAnonymousParliamentTransitionV1 {
    /// Wire identifier of the interface; no registry or selection activates it yet.
    pub const WIRE_ID: &'static str = "iroha.governance.parliament.anonymous.transition.submit.v1";

    /// Reject a malformed or cross-attempt transition before stateful execution.
    ///
    /// # Errors
    /// As [`AnonymousParliamentTransitionV1::validate_static_for_attempt`].
    pub fn validate_static(&self) -> Result<(), &'static str> {
        self.transition
            .validate_static_for_attempt(self.governance_attempt_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        governance::{
            anonymous_ballot::test_fixtures::proposal,
            types::{
                AssignmentId, BeaconPulseId, BeaconSessionId, BodyElectionAttemptId,
                BodyInstanceId, DeliberationPhaseV1, GovernanceAttemptStatusV1, GovernanceStageV1,
                ParliamentBody, SortitionRequestId, SortitionRequestV1,
            },
        },
        isi::{
            InstructionBox, OpaqueInstructionV1,
            governance::{
                CreateParliamentGovernanceAttemptV1, ParliamentSortitionRequestRegistrationV1,
                SubmitParliamentLifecycleTransitionV1,
            },
        },
    };

    const ATTEMPT: GovernanceAttemptId = GovernanceAttemptId::new([0x33; 32]);
    const ELECTION: BodyElectionAttemptId = BodyElectionAttemptId::new([0x44; 32]);
    const BODY: BodyInstanceId = BodyInstanceId::new([0x55; 32]);

    fn request() -> SortitionRequestV1 {
        SortitionRequestV1 {
            id: SortitionRequestId::new([0x66; 32]),
            governance_attempt_id: ATTEMPT,
            body_election_attempt_id: ELECTION,
            body: ParliamentBody::PolicyJury,
            candidate_root: [0x77; 32],
            candidate_count: 200,
            target_seats: 150,
            request_height: 10,
            pulse_height: 20,
            beacon_session_id: BeaconSessionId::new([0x88; 32]),
        }
    }

    fn response(
        body: ParliamentBody,
        decision: ParliamentInvitationDecisionV1,
    ) -> AnonymousParliamentTransitionV1 {
        AnonymousParliamentTransitionV1::RecordInvitationResponse(
            ParliamentRecordInvitationResponseV1 {
                election_attempt_id: ELECTION,
                body,
                decision,
            },
        )
    }

    fn every_transition() -> Vec<AnonymousParliamentTransitionV1> {
        use AnonymousParliamentTransitionV1 as Transition;
        vec![
            Transition::EscalateRisk(ParliamentEscalateRiskV1 {
                target: RiskTierV1::Constitutional,
            }),
            Transition::CompleteQualification,
            Transition::RegisterInitialSortition,
            Transition::RegisterSortitionRequest(ParliamentRegisterSortitionRequestV1 {
                requests: vec![ParliamentSortitionRequestRegistrationV1 {
                    sequence: 0,
                    request: request(),
                }],
            }),
            Transition::ConsumeSortitionPulseBatch(ParliamentConsumeSortitionPulseBatchV1 {
                request_ids: vec![request().id],
                beacon_session_id: BeaconSessionId::new([0x88; 32]),
                pulse_height: 20,
                pulse_id: BeaconPulseId::new([0x99; 32]),
            }),
            Transition::BeginInvitationAcceptance(ParliamentBeginInvitationAcceptanceV1 {
                election_attempt_id: ELECTION,
            }),
            response(
                ParliamentBody::RulesCommittee,
                ParliamentInvitationDecisionV1::Accept,
            ),
            Transition::FailBodyElectionNoRoster(ParliamentFailBodyElectionNoRosterV1 {
                election_attempt_id: ELECTION,
            }),
            Transition::SealBodyRoster(ParliamentSealBodyRosterV1 {
                election_attempt_id: ELECTION,
            }),
            Transition::AdvanceBodyPhase(ParliamentAdvanceBodyPhaseV1 {
                body_instance_id: BODY,
                target: DeliberationPhaseV1::Evidence,
            }),
            Transition::RecordAttemptAbsence(ParliamentRecordAttemptAbsenceV1 {
                body_instance_id: BODY,
                assignment_id: AssignmentId::new([0xaa; 32]),
            }),
            Transition::EndorsePublicFinding(ParliamentEndorsePublicFindingV1 {
                body_instance_id: BODY,
                result_root: [0xbb; 32],
            }),
            Transition::FailPublicFindingNoResult(ParliamentFailPublicFindingNoResultV1 {
                body_instance_id: BODY,
            }),
        ]
    }

    fn assert_opaque(wire_id: &'static str, payload: Vec<u8>) {
        let envelope: InstructionBox = OpaqueInstructionV1::new(wire_id, payload)
            .expect("bounded payload")
            .into();
        let wire = norito::encode_canonical(&envelope).expect("encode envelope");
        let decoded = norito::decode_canonical::<InstructionBox>(&wire).expect("decode envelope");
        assert!(
            decoded.as_opaque().is_some(),
            "{wire_id} has no registry or dispatch activation"
        );
        assert_eq!(norito::encode_canonical(&decoded).expect("re-encode"), wire);
    }

    #[test]
    fn creation_derives_the_family_identifier_and_the_initial_snapshot() {
        let instruction = CreateAnonymousParliamentAttemptV1 {
            proposal: proposal(),
            attempt_sequence: 3,
        };
        let content = ProposalContentId::derive_v1(&instruction.proposal);
        assert_eq!(instruction.proposal_content_id(), content);
        assert_eq!(
            instruction.governance_attempt_id(),
            anonymous_governance_attempt_id_v1(content, 3)
        );

        // The baseline attempt of the same proposal and retry is another attempt.
        let baseline = CreateParliamentGovernanceAttemptV1 {
            proposal: proposal(),
            attempt_sequence: 3,
        };
        assert_eq!(baseline.proposal_content_id(), content);
        assert_ne!(
            instruction.governance_attempt_id(),
            baseline.governance_attempt_id()
        );
        assert_ne!(
            CreateAnonymousParliamentAttemptV1::WIRE_ID,
            CreateParliamentGovernanceAttemptV1::WIRE_ID
        );

        let attempt = instruction.canonical_attempt(RiskTierV1::Constitutional);
        assert!(attempt.has_canonical_id());
        assert_eq!(attempt.id, instruction.governance_attempt_id());
        assert_eq!(attempt.proposal_content_id, content);
        assert_eq!(attempt.sequence, 3);
        assert_eq!(attempt.risk_tier, RiskTierV1::Constitutional);
        assert_eq!(attempt.stage, GovernanceStageV1::Qualification);
        assert_eq!(attempt.status, GovernanceAttemptStatusV1::Active);

        let bytes = norito::encode_canonical(&instruction).expect("encode");
        assert_eq!(
            norito::decode_canonical::<CreateAnonymousParliamentAttemptV1>(&bytes).expect("decode"),
            instruction
        );
        let json = norito::json::to_json(&instruction).expect("json");
        assert_eq!(
            norito::json::from_str::<CreateAnonymousParliamentAttemptV1>(&json).expect("from json"),
            instruction
        );
        assert_opaque(CreateAnonymousParliamentAttemptV1::WIRE_ID, bytes);
    }

    #[test]
    fn every_transition_roundtrips_and_stays_opaque() {
        let transitions = every_transition();
        assert_eq!(transitions.len(), 13, "one fixture per variant");
        for transition in transitions {
            let instruction = SubmitAnonymousParliamentTransitionV1 {
                governance_attempt_id: ATTEMPT,
                transition,
            };
            let bytes = norito::encode_canonical(&instruction).expect("encode");
            assert_eq!(
                norito::decode_canonical::<SubmitAnonymousParliamentTransitionV1>(&bytes)
                    .expect("decode"),
                instruction
            );
            let json = norito::json::to_json(&instruction).expect("json");
            assert_eq!(
                norito::json::from_str::<SubmitAnonymousParliamentTransitionV1>(&json)
                    .expect("from json"),
                instruction
            );
            assert_opaque(SubmitAnonymousParliamentTransitionV1::WIRE_ID, bytes);
        }
        assert_ne!(
            SubmitAnonymousParliamentTransitionV1::WIRE_ID,
            SubmitParliamentLifecycleTransitionV1::WIRE_ID
        );
    }

    #[test]
    fn a_binding_seat_is_never_accepted_by_a_bare_response() {
        for body in [ParliamentBody::PolicyJury, ParliamentBody::ConfirmationJury] {
            assert_eq!(
                response(body, ParliamentInvitationDecisionV1::Accept)
                    .validate_static_for_attempt(ATTEMPT),
                Err("a binding seat is accepted by registering its credential"),
                "{body:?}"
            );
            // Declining a binding seat needs no credential.
            assert_eq!(
                response(body, ParliamentInvitationDecisionV1::Decline)
                    .validate_static_for_attempt(ATTEMPT),
                Ok(()),
                "{body:?}"
            );
        }
        for decision in [
            ParliamentInvitationDecisionV1::Accept,
            ParliamentInvitationDecisionV1::Decline,
        ] {
            assert_eq!(
                response(ParliamentBody::RulesCommittee, decision)
                    .validate_static_for_attempt(ATTEMPT),
                Ok(()),
                "{decision:?}"
            );
        }
    }

    #[test]
    fn the_shared_payload_rules_are_the_baseline_rules() {
        use AnonymousParliamentTransitionV1 as Transition;

        // An inert enclosing attempt.
        assert!(
            Transition::CompleteQualification
                .validate_static_for_attempt(GovernanceAttemptId::new([0; 32]))
                .is_err()
        );
        // An inert election, body instance or assignment.
        for transition in [
            Transition::BeginInvitationAcceptance(ParliamentBeginInvitationAcceptanceV1 {
                election_attempt_id: BodyElectionAttemptId::new([0; 32]),
            }),
            Transition::SealBodyRoster(ParliamentSealBodyRosterV1 {
                election_attempt_id: BodyElectionAttemptId::new([0; 32]),
            }),
            Transition::AdvanceBodyPhase(ParliamentAdvanceBodyPhaseV1 {
                body_instance_id: BodyInstanceId::new([0; 32]),
                target: DeliberationPhaseV1::Evidence,
            }),
            Transition::RecordAttemptAbsence(ParliamentRecordAttemptAbsenceV1 {
                body_instance_id: BODY,
                assignment_id: AssignmentId::new([0; 32]),
            }),
        ] {
            let shared = transition
                .shared_form()
                .validate_static_for_attempt(ATTEMPT);
            assert!(shared.is_err(), "{transition:?}");
            assert_eq!(
                transition.validate_static_for_attempt(ATTEMPT),
                shared,
                "{transition:?}"
            );
        }
        // A sortition request of another attempt.
        let foreign = Transition::RegisterSortitionRequest(ParliamentRegisterSortitionRequestV1 {
            requests: vec![ParliamentSortitionRequestRegistrationV1 {
                sequence: 0,
                request: request(),
            }],
        });
        assert_eq!(
            foreign.validate_static_for_attempt(GovernanceAttemptId::new([0x34; 32])),
            Err("sortition request governance attempt id does not match the enclosing attempt")
        );
        // The submit instruction applies the same rules under its own attempt.
        assert_eq!(
            SubmitAnonymousParliamentTransitionV1 {
                governance_attempt_id: ATTEMPT,
                transition: Transition::CompleteQualification,
            }
            .validate_static(),
            Ok(())
        );
        assert!(
            SubmitAnonymousParliamentTransitionV1 {
                governance_attempt_id: GovernanceAttemptId::new([0; 32]),
                transition: Transition::CompleteQualification,
            }
            .validate_static()
            .is_err()
        );
    }

    #[test]
    fn the_family_has_no_ballot_transition_and_its_own_digest_domain() {
        // The baseline's timed-OVN transitions and its unknown tags do not decode.
        for tag in [
            "RegisterBallotAttempt",
            "RegisterBallotParticipant",
            "FreezeTimedOvnCorpus",
            "FinalizeOpenedBallot",
            "Unknown",
        ] {
            let json = format!(r#"{{"transition":"{tag}","payload":null}}"#);
            assert!(
                norito::json::from_str::<AnonymousParliamentTransitionV1>(&json).is_err(),
                "{tag}"
            );
        }
        assert!(
            norito::json::from_str::<AnonymousParliamentTransitionV1>(
                r#"{"transition":"CompleteQualification","extra":1}"#
            )
            .is_err()
        );

        // Distinct transitions have distinct digests, and none is its baseline digest.
        let transitions = every_transition();
        let digests: std::collections::BTreeSet<_> = transitions
            .iter()
            .map(AnonymousParliamentTransitionV1::digest_v1)
            .collect();
        assert_eq!(digests.len(), transitions.len());
        for transition in &transitions {
            assert_ne!(
                transition.digest_v1(),
                transition.shared_form().digest_v1(),
                "{transition:?}"
            );
        }
    }
}
