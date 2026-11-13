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
//! - [`SubmitAnonymousParliamentTransitionV1`] carries the account-signed facts and intents
//!   of design §8.1: risk escalation, early-close intents, the two inclusion-derived
//!   sortition registrations, invitation responses, absences and endorsements.
//! - [`super::RegisterParliamentCredentialV1`] accepts a binding seat with its credential
//!   (design §3.2).
//!
//! A transaction never applies a lifecycle transition. Qualification, draws, invitation
//! closes, phase changes, finding conclusions, ballot openings and closures,
//! certification and enactment run in the positioned block-start pass (design §8.1,
//! `specs/sccp.md` §4.7), so the baseline's transitions for them have no counterpart here.
//! There is no ballot transition: a binding body has exactly one ballot, and ballots are
//! signerless inputs (design §3.4 to §3.7).
//!
//! The payloads shared with the baseline are the baseline's own types. They are frozen
//! with the baseline and therefore cannot change under this family either.
//!
//! Both interfaces are registered as tolerant instructions of the
//! `iroha.governance.parliament.anonymous.v1` addition. Until a governed change set activates
//! the family they resolve as inactive, and a release without them keeps their envelopes
//! opaque.

use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::{AnonymousGovernanceAttemptV1, BodyDecisionModeV1, anonymous_governance_attempt_id_v1};
use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize,
    governance::types::{
        BodyInstanceId, DeliberationPhaseV1, GovernanceAttemptId, ProposalContentId, ProposalKind,
        RiskTierV1,
    },
    governance_fingerprint::{ANONYMOUS_PARLIAMENT_TRANSITION_DIGEST_V1, fingerprint},
    isi::governance::{
        ParliamentEndorsePublicFindingV1, ParliamentEscalateRiskV1, ParliamentInvitationDecisionV1,
        ParliamentLifecycleTransitionV1, ParliamentRecordAttemptAbsenceV1,
        ParliamentRecordInvitationResponseV1,
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

impl crate::seal::Instruction for CreateAnonymousParliamentAttemptV1 {}

impl PartialOrd for CreateAnonymousParliamentAttemptV1 {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        // ProposalKind has no field ordering; match the baseline attempt's
        // canonical instruction ordering without changing its proposal types.
        Some(self.encode().cmp(&other.encode()))
    }
}

impl CreateAnonymousParliamentAttemptV1 {
    /// Wire identifier of the interface, a tolerant registration of the anonymous family's
    /// addition.
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

/// The intent to end one deliberation phase of a sealed body before its deadline.
///
/// It names the exact phase it closes, so it can never close a later phase than the one
/// its signer saw: once the body has left `phase`, Core refuses it. Reflection is never
/// shortened (design §8.1).
#[derive(
    Clone,
    Copy,
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
    name = "iroha_data_model::governance::anonymous_ballot::AnonymousClosePhaseEarlyV1"
)]
pub struct AnonymousClosePhaseEarlyV1 {
    /// Sealed body whose current phase the intent closes.
    pub body_instance_id: BodyInstanceId,
    /// The phase the body is in: Orientation, Evidence, Questions, Responses or
    /// Deliberation.
    pub phase: DeliberationPhaseV1,
}

impl AnonymousClosePhaseEarlyV1 {
    /// Whether an early-close intent may name `phase`: a deliberation phase before
    /// Reflection.
    #[must_use]
    pub const fn closes_early(phase: DeliberationPhaseV1) -> bool {
        matches!(
            phase,
            DeliberationPhaseV1::Orientation
                | DeliberationPhaseV1::Evidence
                | DeliberationPhaseV1::Questions
                | DeliberationPhaseV1::Responses
                | DeliberationPhaseV1::Deliberation
        )
    }
}

/// One account-signed fact or intent of an anonymous Parliament attempt (design §8.1).
///
/// The executing height and time are absent: Core supplies those of the containing block.
#[derive(
    Clone,
    Copy,
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
    /// Escalate the risk tier before the initial sortition registration locks it.
    #[codec(index = 0)]
    EscalateRisk(ParliamentEscalateRiskV1),
    /// End qualification before its deadline; the pass applies it.
    #[codec(index = 1)]
    CloseQualificationEarly,
    /// Register the initial generation, derived from committed State and the containing
    /// height.
    #[codec(index = 2)]
    RegisterInitialSortition,
    /// Register the next generation of every body whose election ended without a roster,
    /// derived from committed State and the containing height.
    #[codec(index = 3)]
    RegisterSortitionRetry,
    /// Record the authority's response to its invitation: a decline of any seat, or the
    /// acceptance of a public-body seat. A binding seat is accepted by
    /// [`super::RegisterParliamentCredentialV1`], never here.
    #[codec(index = 4)]
    RecordInvitationResponse(ParliamentRecordInvitationResponseV1),
    /// End the named deliberation phase of a sealed body before its deadline; the pass
    /// applies it.
    #[codec(index = 5)]
    ClosePhaseEarly(AnonymousClosePhaseEarlyV1),
    /// Record a public-body member's own absence. Core refuses it for a binding body,
    /// where non-participation appears only in turnout (design §7).
    #[codec(index = 6)]
    RecordAttemptAbsence(ParliamentRecordAttemptAbsenceV1),
    /// Record one seated member's endorsement of a public nonbinding finding.
    #[codec(index = 7)]
    EndorsePublicFinding(ParliamentEndorsePublicFindingV1),
}

impl AnonymousParliamentTransitionV1 {
    /// The same transition in the baseline's closed form, for the transitions whose payload
    /// is one of the baseline's: their state-independent rules are the baseline's rules.
    /// The family's own intents have no baseline form.
    fn shared_form(&self) -> Option<ParliamentLifecycleTransitionV1> {
        use ParliamentLifecycleTransitionV1 as Shared;
        match self {
            Self::EscalateRisk(payload) => Some(Shared::EscalateRisk(*payload)),
            Self::RegisterInitialSortition => Some(Shared::RegisterInitialSortition),
            Self::RecordInvitationResponse(payload) => {
                Some(Shared::RecordInvitationResponse(*payload))
            }
            Self::RecordAttemptAbsence(payload) => Some(Shared::RecordAttemptAbsence(*payload)),
            Self::EndorsePublicFinding(payload) => Some(Shared::EndorsePublicFinding(*payload)),
            Self::CloseQualificationEarly
            | Self::RegisterSortitionRetry
            | Self::ClosePhaseEarly(_) => None,
        }
    }

    /// Reject a malformed or cross-attempt transition before stateful execution.
    ///
    /// Authority, phase, time, candidate corpus and proof checks need committed state and
    /// stay with Core.
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
        if let Some(shared) = self.shared_form() {
            return shared.validate_static_for_attempt(governance_attempt_id);
        }
        if governance_attempt_id.as_bytes() == &[0; 32] {
            return Err("governance attempt id must be non-zero");
        }
        if let Self::ClosePhaseEarly(intent) = self {
            if intent.body_instance_id.as_bytes() == &[0; 32] {
                return Err("body instance id must be non-zero");
            }
            if !AnonymousClosePhaseEarlyV1::closes_early(intent.phase) {
                return Err("only a deliberation phase before Reflection closes early");
            }
        }
        Ok(())
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
/// outcome: Core derives every height, time, snapshot, roster and budget it implies. Risk
/// escalation and the early-close intents need the `CanManageParliament` permission; the
/// two sortition registrations are permissionless (design §8.1 and §10).
#[derive(
    Clone,
    Copy,
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

impl crate::seal::Instruction for SubmitAnonymousParliamentTransitionV1 {}

impl SubmitAnonymousParliamentTransitionV1 {
    /// Wire identifier of the interface, a tolerant registration of the anonymous family's
    /// addition.
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
                AssignmentId, BodyElectionAttemptId, GovernanceAttemptStatusV1, GovernanceStageV1,
                ParliamentBody,
            },
        },
        isi::{
            InstructionBox, OpaqueInstructionV1,
            governance::{
                CreateParliamentGovernanceAttemptV1, SubmitParliamentLifecycleTransitionV1,
            },
        },
    };

    const ATTEMPT: GovernanceAttemptId = GovernanceAttemptId::new([0x33; 32]);
    const ELECTION: BodyElectionAttemptId = BodyElectionAttemptId::new([0x44; 32]);
    const BODY: BodyInstanceId = BodyInstanceId::new([0x55; 32]);

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

    fn close_phase(
        body_instance_id: BodyInstanceId,
        phase: DeliberationPhaseV1,
    ) -> AnonymousParliamentTransitionV1 {
        AnonymousParliamentTransitionV1::ClosePhaseEarly(AnonymousClosePhaseEarlyV1 {
            body_instance_id,
            phase,
        })
    }

    fn every_transition() -> Vec<AnonymousParliamentTransitionV1> {
        use AnonymousParliamentTransitionV1 as Transition;
        vec![
            Transition::EscalateRisk(ParliamentEscalateRiskV1 {
                target: RiskTierV1::Constitutional,
            }),
            Transition::CloseQualificationEarly,
            Transition::RegisterInitialSortition,
            Transition::RegisterSortitionRetry,
            response(
                ParliamentBody::RulesCommittee,
                ParliamentInvitationDecisionV1::Accept,
            ),
            close_phase(BODY, DeliberationPhaseV1::Evidence),
            Transition::RecordAttemptAbsence(ParliamentRecordAttemptAbsenceV1 {
                body_instance_id: BODY,
                assignment_id: AssignmentId::new([0xaa; 32]),
            }),
            Transition::EndorsePublicFinding(ParliamentEndorsePublicFindingV1 {
                body_instance_id: BODY,
                result_root: [0xbb; 32],
            }),
        ]
    }

    /// The typed instruction travels under its wire ID and decodes back to itself; a frame the
    /// decoder refuses stays an opaque envelope, as on a release without the instruction.
    fn assert_registered<T>(instruction: T, wire_id: &'static str, payload: Vec<u8>)
    where
        T: Into<InstructionBox> + Clone + PartialEq + core::fmt::Debug + 'static,
    {
        let boxed: InstructionBox = instruction.clone().into();
        assert_eq!(crate::isi::instruction_wire_id(&boxed), Some(wire_id));
        let wire = norito::encode_canonical(&boxed).expect("encode registered instruction");
        let decoded = norito::decode_canonical::<InstructionBox>(&wire).expect("decode");
        assert_eq!(decoded.as_any().downcast_ref::<T>(), Some(&instruction));
        assert_eq!(norito::encode_canonical(&decoded).expect("re-encode"), wire);

        let mut trailing = payload;
        trailing.push(0);
        let envelope: InstructionBox = OpaqueInstructionV1::new(wire_id, trailing)
            .expect("bounded payload")
            .into();
        let wire = norito::encode_canonical(&envelope).expect("encode envelope");
        let decoded = norito::decode_canonical::<InstructionBox>(&wire).expect("decode envelope");
        assert!(decoded.as_opaque().is_some(), "{wire_id} refused frame");
        assert_eq!(norito::encode_canonical(&decoded).expect("re-encode"), wire);
    }

    #[test]
    fn creation_comparison_distinguishes_retry_and_proposal_content() {
        let original = CreateAnonymousParliamentAttemptV1 {
            proposal: proposal(),
            attempt_sequence: 0,
        };
        assert_eq!(
            original.partial_cmp(&original.clone()),
            Some(core::cmp::Ordering::Equal)
        );
        let retry = CreateAnonymousParliamentAttemptV1 {
            attempt_sequence: 1,
            ..original.clone()
        };
        let mut other_content = original.clone();
        let ProposalKind::DeployContract(deployment) = &mut other_content.proposal else {
            panic!("fixture must be a contract deployment");
        };
        deployment.code_hash = crate::governance::types::ContractCodeHash::new([0xcc; 32]);
        for other in [retry, other_content] {
            let order = original
                .partial_cmp(&other)
                .expect("instructions are comparable");
            assert_ne!(order, core::cmp::Ordering::Equal);
            assert_eq!(other.partial_cmp(&original), Some(order.reverse()));
        }
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
        assert_registered(
            instruction,
            CreateAnonymousParliamentAttemptV1::WIRE_ID,
            bytes,
        );
    }

    #[test]
    fn every_transition_roundtrips_under_its_registered_wire_id() {
        let transitions = every_transition();
        assert_eq!(transitions.len(), 8, "one fixture per variant");
        for (index, transition) in transitions.into_iter().enumerate() {
            // The discriminant is the variant index.
            assert_eq!(
                transition.encode()[..4],
                u32::try_from(index).expect("small").to_le_bytes()
            );
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
            assert_registered(
                instruction,
                SubmitAnonymousParliamentTransitionV1::WIRE_ID,
                bytes,
            );
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
    fn an_early_close_names_one_deliberation_phase_before_reflection() {
        for phase in [
            DeliberationPhaseV1::Orientation,
            DeliberationPhaseV1::Evidence,
            DeliberationPhaseV1::Questions,
            DeliberationPhaseV1::Responses,
            DeliberationPhaseV1::Deliberation,
        ] {
            assert!(AnonymousClosePhaseEarlyV1::closes_early(phase));
            assert_eq!(
                close_phase(BODY, phase).validate_static_for_attempt(ATTEMPT),
                Ok(()),
                "{phase:?}"
            );
        }
        // Reflection and the baseline's ballot phase are never shortened.
        for phase in [DeliberationPhaseV1::Reflection, DeliberationPhaseV1::Vote] {
            assert!(!AnonymousClosePhaseEarlyV1::closes_early(phase));
            assert_eq!(
                close_phase(BODY, phase).validate_static_for_attempt(ATTEMPT),
                Err("only a deliberation phase before Reflection closes early"),
                "{phase:?}"
            );
        }
        assert_eq!(
            close_phase(BodyInstanceId::new([0; 32]), DeliberationPhaseV1::Evidence)
                .validate_static_for_attempt(ATTEMPT),
            Err("body instance id must be non-zero")
        );
    }

    #[test]
    fn the_shared_payload_rules_are_the_baseline_rules() {
        use AnonymousParliamentTransitionV1 as Transition;

        // An inert enclosing attempt, for every transition.
        for transition in every_transition() {
            assert!(
                transition
                    .validate_static_for_attempt(GovernanceAttemptId::new([0; 32]))
                    .is_err(),
                "{transition:?}"
            );
        }
        // An inert body instance or assignment of a shared payload.
        for transition in [
            Transition::RecordAttemptAbsence(ParliamentRecordAttemptAbsenceV1 {
                body_instance_id: BODY,
                assignment_id: AssignmentId::new([0; 32]),
            }),
            Transition::EndorsePublicFinding(ParliamentEndorsePublicFindingV1 {
                body_instance_id: BodyInstanceId::new([0; 32]),
                result_root: [0xbb; 32],
            }),
            Transition::EndorsePublicFinding(ParliamentEndorsePublicFindingV1 {
                body_instance_id: BODY,
                result_root: [0; 32],
            }),
        ] {
            let shared = transition
                .shared_form()
                .expect("a shared payload")
                .validate_static_for_attempt(ATTEMPT);
            assert!(shared.is_err(), "{transition:?}");
            assert_eq!(
                transition.validate_static_for_attempt(ATTEMPT),
                shared,
                "{transition:?}"
            );
        }
        // The family's own intents have no baseline form; every other transition has one.
        for transition in every_transition() {
            let own = matches!(
                transition,
                Transition::CloseQualificationEarly
                    | Transition::RegisterSortitionRetry
                    | Transition::ClosePhaseEarly(_)
            );
            assert_eq!(transition.shared_form().is_none(), own, "{transition:?}");
        }
        // The submit instruction applies the same rules under its own attempt.
        assert_eq!(
            SubmitAnonymousParliamentTransitionV1 {
                governance_attempt_id: ATTEMPT,
                transition: Transition::RegisterSortitionRetry,
            }
            .validate_static(),
            Ok(())
        );
        assert!(
            SubmitAnonymousParliamentTransitionV1 {
                governance_attempt_id: GovernanceAttemptId::new([0; 32]),
                transition: Transition::CloseQualificationEarly,
            }
            .validate_static()
            .is_err()
        );
    }

    #[test]
    fn transitions_apply_nothing_and_the_family_has_its_own_digest_domain() {
        // The baseline's pass-applied transitions, its timed-OVN transitions and unknown
        // tags do not decode: no transaction applies a lifecycle transition.
        for tag in [
            "CompleteQualification",
            "RegisterSortitionRequest",
            "ConsumeSortitionPulseBatch",
            "BeginInvitationAcceptance",
            "FailBodyElectionNoRoster",
            "SealBodyRoster",
            "AdvanceBodyPhase",
            "FailPublicFindingNoResult",
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
                r#"{"transition":"CloseQualificationEarly","extra":1}"#
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
            if let Some(shared) = transition.shared_form() {
                assert_ne!(transition.digest_v1(), shared.digest_v1(), "{transition:?}");
            }
        }
    }
}
