//! Tests of the anonymous attempt reducer: purpose isolation, the result pipeline,
//! certification and the restore check.

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::{
    NetworkId,
    block::BlockHeader,
    governance::{
        anonymous_ballot::{
            BallotClosureErrorV1, BallotScopeV1, Digest512, PARLIAMENT_BALLOT_QUALIFICATION_V1,
            ParliamentBallotParametersV1, ParliamentCandidateTestingPolicyV1,
            ParliamentDecisionRuleV1, ParliamentLifecycleParametersV1, ProofProfileIdV1,
            anonymous_body_instance_id_v1, anonymous_governance_attempt_id_v1,
        },
        types::{
            AssignmentId, BeaconPulseId, BeaconSessionId, BodyElectionAttemptId,
            GovernanceExpectedHeadAbsentV1, GovernanceExpectedHeadPresentV1,
            ParliamentPublicFindingCertificateBindingV1, ProposalContentId, SortitionRequestV1,
            parliament_public_finding_endorsement_root_v1, parliament_quorum_seats_v1,
        },
    },
};

use super::*;

const CONTENT: ProposalContentId = ProposalContentId::new([0x01; 32]);
const SUBJECT: [u8; 32] = [0xe0; 32];
const JURY_SEATS: u16 = 150;
/// The Policy Jury closes here; a narrow approval commits the Confirmation request here.
const POLICY_CLOSED: GovernanceAnchorV1 = anchor(90_000, 0, 87_401_500);
const CONFIRMATION_CLOSED: GovernanceAnchorV1 = anchor(190_000, 1, 187_401_500);

const fn anchor(height: u64, position: u32, timestamp_ms: u64) -> GovernanceAnchorV1 {
    GovernanceAnchorV1 {
        height,
        position,
        timestamp_ms,
    }
}

fn candidate() -> ParliamentBallotPurposeV1 {
    ParliamentBallotPurposeV1::CandidateTesting(ParliamentCandidateTestingPolicyV1::CANDIDATE_V1)
}

/// The contract-lifecycle pipeline with its conditional Confirmation Jury.
fn pipeline() -> Vec<ParliamentBody> {
    vec![
        ParliamentBody::RulesCommittee,
        ParliamentBody::AgendaCouncil,
        ParliamentBody::InterestPanel,
        ParliamentBody::ReviewPanel,
        ParliamentBody::OversightCommittee,
        ParliamentBody::PolicyJury,
        ParliamentBody::ConfirmationJury,
    ]
}

fn public_bodies() -> Vec<ParliamentBody> {
    pipeline()
        .into_iter()
        .filter(|body| BodyDecisionModeV1::of(*body) == BodyDecisionModeV1::PublicFinding)
        .collect()
}

fn policy() -> FrozenParliamentAttemptPolicyV1 {
    FrozenParliamentAttemptPolicyV1::freeze_v1(
        &pipeline(),
        &ParliamentLifecycleParametersV1::default(),
        Some(&ParliamentBallotParametersV1::default()),
        1_000,
        3_600,
    )
    .expect("default attempt policy")
}

fn attempt(sequence: u32, risk_tier: RiskTierV1) -> AnonymousGovernanceAttemptV1 {
    AnonymousGovernanceAttemptV1::new_v1(CONTENT, sequence, risk_tier)
}

fn head() -> GovernanceExpectedHeadV1 {
    GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
        subject_id: SUBJECT,
    })
}

fn new_state(risk_tier: RiskTierV1) -> AnonymousParliamentAttemptStateV1 {
    AnonymousParliamentAttemptStateV1::try_new(
        candidate(),
        attempt(0, risk_tier),
        policy(),
        [0xd0; 32],
        head(),
    )
    .expect("new attempt")
}

fn id(state: &AnonymousParliamentAttemptStateV1) -> GovernanceAttemptId {
    state.attempt().id
}

/// A sealed body of the attempt `governance_attempt_id` whose request was committed at
/// `request_height`.
fn sealed(
    governance_attempt_id: GovernanceAttemptId,
    body: ParliamentBody,
    seats: u16,
    request_height: u64,
) -> AnonymousSealedBodyV1 {
    let election_attempt_id = BodyElectionAttemptId::derive_v1(governance_attempt_id, body, 0);
    let target_seats = policy()
        .required_body(body)
        .expect("required body")
        .target_seats;
    let sortition_request = SortitionRequestV1::try_new_canonical(
        governance_attempt_id,
        election_attempt_id,
        body,
        [0x70; 32],
        2_000,
        u32::from(target_seats),
        request_height,
        request_height + 5,
        BeaconSessionId::new([0x71; 32]),
        None,
    )
    .expect("canonical request");
    let mut root = [0x80; 64];
    root[0] = body as u8;
    let roster_root = Digest512::new(root);
    let mut pulse = [0x90; 32];
    pulse[0] = body as u8;
    AnonymousSealedBodyV1 {
        body_instance_id: anonymous_body_instance_id_v1(election_attempt_id, &roster_root),
        election_attempt_id,
        election_attempt_sequence: 0,
        sortition_request,
        body,
        seats,
        beacon_pulse_id: BeaconPulseId::new(pulse),
        roster_root,
        assignment_root: [0xa0; 32],
    }
}

/// The finding exactly a quorum of `sealed` endorsed.
fn finding(
    governance_attempt_id: GovernanceAttemptId,
    sealed: &AnonymousSealedBodyV1,
) -> AnonymousPublicFindingResultV1 {
    let quorum = parliament_quorum_seats_v1(u32::from(sealed.seats));
    let endorsing_assignments: Vec<_> = (1..=quorum)
        .map(|index| {
            let mut bytes = [0xb0; 32];
            bytes[..4].copy_from_slice(&index.to_be_bytes());
            AssignmentId::new(bytes)
        })
        .collect();
    let result_root = [0xc0; 32];
    AnonymousPublicFindingResultV1 {
        result_root,
        finding: ParliamentPublicFindingCertificateBindingV1 {
            endorsement_root: parliament_public_finding_endorsement_root_v1(
                governance_attempt_id,
                sealed.body_instance_id,
                result_root,
                &endorsing_assignments,
            ),
            endorsing_assignments,
            endorsements: quorum,
            quorum,
        },
    }
}

/// The closure record of `sealed` with the given counts under `risk_tier`, closed at
/// `closed`. Its outcome and narrow flag are the decision rule's.
fn closure(
    state: &AnonymousParliamentAttemptStateV1,
    sealed: &AnonymousSealedBodyV1,
    (aye, nay, abstain): (u32, u32, u32),
    closed: GovernanceAnchorV1,
) -> BallotClosureRecordV1 {
    let risk_tier = state.attempt().risk_tier;
    let decision = ParliamentDecisionRuleV1::V1
        .decide(sealed.body, risk_tier, sealed.seats, aye, nay, abstain)
        .expect("valid counts");
    let casting_window_ms = state
        .policy()
        .binding
        .expect("binding policy")
        .casting_window_ms;
    // Casting opens after the pulse that drew the jury and closes one window after its
    // clock, just before `closed`.
    let opened_height = sealed.sortition_request.pulse_height + 45;
    let casting_clock_timestamp_ms = closed.timestamp_ms - casting_window_ms - 500;
    BallotClosureRecordV1 {
        network_id: NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([0x5a; Hash::LENGTH]),
        )),
        proposal_content_id: CONTENT,
        proposal_digest: Digest512::new([0x02; 64]),
        governance_attempt_id: id(state),
        election_attempt_id: sealed.election_attempt_id,
        body_instance_id: sealed.body_instance_id,
        body_role: sealed.body,
        scope: BallotScopeV1::new([0x44; 64]),
        seats: sealed.seats,
        profile_id: ProofProfileIdV1::V1,
        policy_hash: Digest512::new([0xaa; 64]),
        list_digest: Digest512::new([0x99; 64]),
        registration_mapping_digest: Digest512::new([0x04; 64]),
        opening_anchor: anchor(opened_height, 2, casting_clock_timestamp_ms - 1_000),
        casting_clock_height: opened_height + 1,
        casting_clock_timestamp_ms,
        casting_window_ms,
        closing_ms: casting_clock_timestamp_ms + casting_window_ms,
        closure_anchor: closed,
        corpus_root: Digest512::new([0x05; 64]),
        accepted_ballots: aye + nay + abstain,
        aye,
        nay,
        abstain,
        outcome: decision.outcome,
        narrow: decision.narrow,
    }
}

fn assert_restores(state: &AnonymousParliamentAttemptStateV1) {
    assert_eq!(state.validate(), Ok(()), "{:?}", state.attempt());
    let bytes = norito::encode_canonical(state).expect("encode");
    let restored =
        norito::decode_canonical::<AnonymousParliamentAttemptStateV1>(&bytes).expect("decode");
    assert_eq!(&restored, state);
    assert_eq!(restored.validate(), Ok(()));
}

/// An attempt past qualification with its risk locked, every public body and the Policy
/// Jury sealed, and every public finding recorded: at the Policy Jury stage.
fn at_policy_jury(risk_tier: RiskTierV1) -> AnonymousParliamentAttemptStateV1 {
    let mut state = new_state(risk_tier);
    let attempt = id(&state);
    state.complete_qualification(attempt).expect("qualified");
    state.lock_risk(attempt).expect("risk locked");
    for body in public_bodies() {
        state
            .seal_body(attempt, sealed(attempt, body, 9, 10))
            .expect("public body sealed");
    }
    state
        .seal_body(
            attempt,
            sealed(attempt, ParliamentBody::PolicyJury, JURY_SEATS, 10),
        )
        .expect("Policy Jury sealed");
    assert_restores(&state);
    for body in public_bodies() {
        assert_eq!(state.attempt().stage, stage_for_body(body));
        let result = finding(attempt, state.sealed_body(body).expect("sealed"));
        state
            .record_public_finding(attempt, body, result, 40)
            .expect("finding recorded");
        assert_restores(&state);
    }
    assert_eq!(state.attempt().stage, GovernanceStageV1::PolicyJury);
    state
}

fn close_policy(
    state: &mut AnonymousParliamentAttemptStateV1,
    counts: (u32, u32, u32),
) -> Result<AnonymousBallotDecisionV1, AnonymousParliamentErrorV1> {
    let jury = *state
        .sealed_body(ParliamentBody::PolicyJury)
        .expect("Policy Jury sealed");
    let record = closure(state, &jury, counts, POLICY_CLOSED);
    state.record_ballot_closure(id(state), record)
}

/// A narrowly approved attempt with its Confirmation Jury sealed by the closure's block.
fn at_confirmation_jury() -> AnonymousParliamentAttemptStateV1 {
    let mut state = at_policy_jury(RiskTierV1::Standard);
    assert_eq!(
        close_policy(&mut state, (51, 49, 0)),
        Ok(AnonymousBallotDecisionV1::ConfirmationRequired)
    );
    let attempt = id(&state);
    state
        .seal_body(
            attempt,
            sealed(
                attempt,
                ParliamentBody::ConfirmationJury,
                JURY_SEATS,
                POLICY_CLOSED.height,
            ),
        )
        .expect("Confirmation Jury sealed");
    assert_restores(&state);
    state
}

#[test]
fn a_clear_approval_certifies_the_attempt_from_its_recorded_results() {
    let mut state = at_policy_jury(RiskTierV1::Standard);
    assert!(!state.is_terminal());
    assert_eq!(
        close_policy(&mut state, (120, 20, 0)),
        Ok(AnonymousBallotDecisionV1::Certified)
    );
    assert_eq!(state.attempt().status, GovernanceAttemptStatusV1::Certified);
    assert_eq!(state.attempt().stage, GovernanceStageV1::Enactment);
    assert!(!state.confirmation_required());
    assert_restores(&state);

    let certificate = state.certificate().expect("certificate");
    assert_eq!(certificate.validate(), Ok(()));
    assert_eq!(certificate.purpose, candidate());
    assert_eq!(certificate.policy, policy());
    assert_eq!(certificate.certified_at(), Some(POLICY_CLOSED));
    // One binding per public body and the Policy Jury; no Confirmation Jury.
    let bound: Vec<_> = certificate
        .body_bindings
        .iter()
        .map(|binding| binding.sealed.body)
        .collect();
    let mut expected = public_bodies();
    expected.push(ParliamentBody::PolicyJury);
    assert_eq!(bound, expected);
    for binding in &certificate.body_bindings {
        assert_eq!(state.result(binding.sealed.body), Some(binding));
        assert_eq!(
            state.sealed_body(binding.sealed.body),
            Some(&binding.sealed)
        );
    }
}

#[test]
fn a_candidate_certificate_is_never_executed() {
    let mut state = at_policy_jury(RiskTierV1::Standard);
    close_policy(&mut state, (120, 20, 0)).expect("certified");
    let attempt = id(&state);
    let certified = state.clone();
    // Long after the enactment delay.
    let late = anchor(900_000, 0, u64::MAX / 2);
    let refused =
        AnonymousParliamentErrorV1::Purpose(ParliamentBallotPurposeErrorV1::CandidateTesting);

    assert_eq!(state.mark_enacted(attempt, late), Err(refused.clone()));
    assert_eq!(
        state.mark_superseded(
            attempt,
            late,
            GovernanceExpectedHeadV1::Present(GovernanceExpectedHeadPresentV1 {
                subject_id: SUBJECT,
                version: 1,
                head_root: [0xe1; 32],
            }),
        ),
        Err(refused.clone())
    );
    assert_eq!(
        state.mark_execution_failed(attempt, late),
        Err(refused.clone())
    );
    assert_eq!(state, certified, "a refused execution changes nothing");

    // The certificate is the end of a candidate attempt.
    assert!(state.is_terminal());
    assert_eq!(state.enactment_due_ms(), None);
    assert_eq!(state.execution(), None);
    assert_eq!(PARLIAMENT_BALLOT_QUALIFICATION_V1, None);

    // A State that claims an execution is not one this reducer produces.
    for status in [
        GovernanceAttemptStatusV1::Enacted,
        GovernanceAttemptStatusV1::Superseded,
        GovernanceAttemptStatusV1::ExecutionFailed,
    ] {
        let mut forged = certified.clone();
        forged.attempt.status = status;
        forged.execution = Some(AnonymousExecutionV1 {
            at: late,
            outcome: AnonymousExecutionOutcomeV1::Enacted,
        });
        assert_eq!(forged.validate(), Err(refused.clone()), "{status:?}");
    }
    // Relabelling the purpose does not help: this release names no qualification, and
    // the certificate no longer is the one the results give.
    let mut relabelled = certified;
    relabelled.purpose = ParliamentBallotPurposeV1::Qualified(Digest512::new([0x42; 64]));
    assert_eq!(
        relabelled.validate(),
        Err(AnonymousParliamentErrorV1::Purpose(
            ParliamentBallotPurposeErrorV1::Unqualified
        ))
    );
}

#[test]
fn execution_is_due_on_the_governance_clock_and_superseded_only_by_another_head() {
    let mut state = at_policy_jury(RiskTierV1::Standard);
    close_policy(&mut state, (120, 20, 0)).expect("certified");
    let certificate = state.certificate().expect("certificate");
    let due_ms = certificate.enact_at_ms().expect("due time");
    assert_eq!(
        due_ms,
        POLICY_CLOSED.timestamp_ms + policy().lifecycle_policy.enactment_delay_ms
    );

    assert_eq!(
        execution_is_due(certificate, anchor(90_001, 0, due_ms - 1)),
        Err(AnonymousParliamentErrorV1::EnactmentNotDue {
            due_ms,
            now_ms: due_ms - 1
        })
    );
    assert_eq!(
        execution_is_due(certificate, anchor(90_001, 0, due_ms)),
        Ok(())
    );
    assert!(due_ms > POLICY_CLOSED.timestamp_ms, "the frozen delay is positive");
    // An item follows the certifying closure: never an earlier block, whatever its time
    // claims, and in the certifying block only later in the same pass at the block's time.
    for not_after in [
        anchor(POLICY_CLOSED.height - 1, 0, due_ms),
        anchor(
            POLICY_CLOSED.height,
            POLICY_CLOSED.position,
            POLICY_CLOSED.timestamp_ms,
        ),
        anchor(POLICY_CLOSED.height, POLICY_CLOSED.position + 1, due_ms),
    ] {
        assert_eq!(
            execution_is_due(certificate, not_after),
            Err(AnonymousParliamentErrorV1::ExecutionAnchor),
            "{not_after:?}"
        );
    }
    // Later in the certifying pass the item follows, and waits for the frozen delay.
    assert_eq!(
        execution_is_due(
            certificate,
            anchor(
                POLICY_CLOSED.height,
                POLICY_CLOSED.position + 1,
                POLICY_CLOSED.timestamp_ms
            )
        ),
        Err(AnonymousParliamentErrorV1::EnactmentNotDue {
            due_ms,
            now_ms: POLICY_CLOSED.timestamp_ms
        })
    );

    assert_eq!(
        head_supersedes(certificate, head()),
        Err(AnonymousParliamentErrorV1::ExpectedHeadUnchanged)
    );
    let present = |subject_id, version, head_root| {
        GovernanceExpectedHeadV1::Present(GovernanceExpectedHeadPresentV1 {
            subject_id,
            version,
            head_root,
        })
    };
    assert_eq!(
        head_supersedes(certificate, present(SUBJECT, 1, [0xe1; 32])),
        Ok(())
    );
    for invalid in [
        present([0xe9; 32], 1, [0xe1; 32]),
        present(SUBJECT, 0, [0xe1; 32]),
        present(SUBJECT, 1, [0; 32]),
    ] {
        assert_eq!(
            head_supersedes(certificate, invalid),
            Err(AnonymousParliamentErrorV1::InvalidSupersedingHead)
        );
    }
}

#[test]
fn a_narrow_policy_approval_needs_its_confirmation_jury() {
    // A Confirmation Jury is not sealable before a narrow approval requires it.
    let mut early = at_policy_jury(RiskTierV1::Standard);
    let attempt = id(&early);
    let confirmation = sealed(
        attempt,
        ParliamentBody::ConfirmationJury,
        JURY_SEATS,
        POLICY_CLOSED.height,
    );
    assert_eq!(
        early.seal_body(attempt, confirmation),
        Err(AnonymousParliamentErrorV1::ConfirmationNotRequired)
    );

    let mut state = at_confirmation_jury();
    assert!(state.confirmation_required());
    assert_eq!(state.attempt().status, GovernanceAttemptStatusV1::Active);
    assert_eq!(state.attempt().stage, GovernanceStageV1::ConfirmationJury);
    assert_eq!(state.certificate(), None);

    let jury = *state
        .sealed_body(ParliamentBody::ConfirmationJury)
        .expect("sealed");
    let record = closure(&state, &jury, (120, 20, 0), CONFIRMATION_CLOSED);
    assert_eq!(
        state.record_ballot_closure(attempt, record),
        Ok(AnonymousBallotDecisionV1::Certified)
    );
    assert_restores(&state);
    let certificate = state.certificate().expect("certificate");
    assert_eq!(certificate.certified_at(), Some(CONFIRMATION_CLOSED));
    assert!(certificate.body(ParliamentBody::ConfirmationJury).is_some());

    // A Confirmation Jury that does not approve rejects the attempt.
    let mut rejected = at_confirmation_jury();
    let record = closure(&rejected, &jury, (20, 120, 0), CONFIRMATION_CLOSED);
    assert_eq!(
        rejected.record_ballot_closure(attempt, record),
        Ok(AnonymousBallotDecisionV1::Rejected(
            BallotOutcomeV1::Rejected
        ))
    );
    assert_eq!(
        rejected.rejection(),
        Some(&AnonymousAttemptRejectionV1::Ballot(Box::new(record)))
    );
    assert!(rejected.is_terminal());
    assert_restores(&rejected);
}

#[test]
fn a_confirmation_jury_is_the_fresh_draw_the_narrow_approval_admitted() {
    use AnonymousParliamentErrorV1 as Error;

    let mut state = at_policy_jury(RiskTierV1::Standard);
    close_policy(&mut state, (51, 49, 0)).expect("narrow approval");
    let attempt = id(&state);
    let mismatch =
        Error::Certificate(AnonymousGovernanceCertificateErrorV1::ConfirmationJuryMismatch);
    let fresh = sealed(
        attempt,
        ParliamentBody::ConfirmationJury,
        JURY_SEATS,
        POLICY_CLOSED.height,
    );

    // The first Confirmation request is committed by the Policy closure's block.
    for request_height in [POLICY_CLOSED.height - 1, POLICY_CLOSED.height + 1] {
        let stale = sealed(
            attempt,
            ParliamentBody::ConfirmationJury,
            JURY_SEATS,
            request_height,
        );
        assert_eq!(state.seal_body(attempt, stale), Err(mismatch.clone()));
    }
    // It never reuses the Policy Jury's pulse.
    let mut same_pulse = fresh;
    same_pulse.beacon_pulse_id = state
        .sealed_body(ParliamentBody::PolicyJury)
        .expect("sealed")
        .beacon_pulse_id;
    assert_eq!(state.seal_body(attempt, same_pulse), Err(mismatch));
    assert_eq!(state.sealed_body(ParliamentBody::ConfirmationJury), None);

    state.seal_body(attempt, fresh).expect("fresh draw sealed");
    assert_restores(&state);

    // A restored Confirmation Jury is held to the same rule.
    let mut restored = state;
    restored
        .sealed
        .get_mut(&ParliamentBody::ConfirmationJury)
        .expect("sealed")
        .beacon_pulse_id = same_pulse.beacon_pulse_id;
    assert!(restored.validate().is_err());
}

#[test]
fn any_other_closure_rejects_the_attempt_for_good() {
    for (counts, outcome) in [
        ((20, 120, 0), BallotOutcomeV1::Rejected),
        ((60, 60, 0), BallotOutcomeV1::Rejected),
        ((40, 0, 0), BallotOutcomeV1::NoQuorum),
    ] {
        let mut state = at_policy_jury(RiskTierV1::Standard);
        assert_eq!(
            close_policy(&mut state, counts),
            Ok(AnonymousBallotDecisionV1::Rejected(outcome)),
            "{counts:?}"
        );
        assert_eq!(state.attempt().status, GovernanceAttemptStatusV1::Rejected);
        assert_eq!(state.attempt().stage, GovernanceStageV1::PolicyJury);
        assert!(matches!(
            state.rejection(),
            Some(AnonymousAttemptRejectionV1::Ballot(record)) if record.outcome == outcome
        ));
        assert_eq!(state.result(ParliamentBody::PolicyJury), None);
        assert_eq!(state.certificate(), None);
        assert!(state.is_terminal());
        assert_restores(&state);

        // A binding jury has exactly one ballot: nothing follows a rejection.
        let attempt = id(&state);
        let inactive = AnonymousParliamentErrorV1::NotActive {
            status: GovernanceAttemptStatusV1::Rejected,
        };
        assert_eq!(
            close_policy(&mut state, (120, 20, 0)),
            Err(inactive.clone())
        );
        assert_eq!(
            state.escalate_risk(attempt, RiskTierV1::Emergency),
            Err(inactive.clone())
        );
        assert_eq!(
            state.mark_enacted(attempt, anchor(900_000, 0, u64::MAX / 2)),
            Err(AnonymousParliamentErrorV1::NotCertified)
        );
    }
}

#[test]
fn a_public_body_without_a_finding_rejects_the_attempt() {
    use AnonymousParliamentErrorV1 as Error;

    let mut state = new_state(RiskTierV1::Standard);
    let attempt = id(&state);
    state.complete_qualification(attempt).expect("qualified");
    let rules = sealed(attempt, ParliamentBody::RulesCommittee, 9, 10);
    let failure = |body, height| AnonymousPublicFindingFailureV1 {
        body,
        kind: AnonymousPublicFindingFailureKindV1::WindowExpired,
        height,
    };

    // Only the sealed public body at the attempt's stage can fail, after its draw.
    assert_eq!(
        state.fail_public_finding(attempt, failure(ParliamentBody::RulesCommittee, 40)),
        Err(Error::BodyNotSealed {
            body: ParliamentBody::RulesCommittee
        })
    );
    state.seal_body(attempt, rules).expect("sealed");
    assert_eq!(
        state.fail_public_finding(attempt, failure(ParliamentBody::AgendaCouncil, 40)),
        Err(Error::Stage {
            expected: GovernanceStageV1::Agenda,
            actual: GovernanceStageV1::Rules,
        })
    );
    assert_eq!(
        state.fail_public_finding(attempt, failure(ParliamentBody::PolicyJury, 40)),
        Err(Error::DecisionMode {
            body: ParliamentBody::PolicyJury
        })
    );
    assert_eq!(
        state.fail_public_finding(
            attempt,
            failure(
                ParliamentBody::RulesCommittee,
                rules.sortition_request.pulse_height
            )
        ),
        Err(Error::FailureBeforeSortition)
    );

    for kind in [
        AnonymousPublicFindingFailureKindV1::QuorumUnreachable,
        AnonymousPublicFindingFailureKindV1::WindowExpired,
    ] {
        let mut failed = state.clone();
        let failure = AnonymousPublicFindingFailureV1 {
            body: ParliamentBody::RulesCommittee,
            kind,
            height: 40,
        };
        failed
            .fail_public_finding(attempt, failure)
            .expect("failed");
        assert_eq!(failed.attempt().status, GovernanceAttemptStatusV1::Rejected);
        assert_eq!(failed.attempt().stage, GovernanceStageV1::Rules);
        assert_eq!(
            failed.rejection(),
            Some(&AnonymousAttemptRejectionV1::PublicFinding(failure))
        );
        assert!(failed.is_terminal());
        assert_restores(&failed);
        // Nothing follows: not a late finding, not a second failure.
        let inactive = Error::NotActive {
            status: GovernanceAttemptStatusV1::Rejected,
        };
        assert_eq!(
            failed.record_public_finding(
                attempt,
                ParliamentBody::RulesCommittee,
                finding(attempt, &rules),
                41
            ),
            Err(inactive.clone())
        );
        assert_eq!(failed.fail_public_finding(attempt, failure), Err(inactive));

        // A restored rejection names the body at the stage the attempt stopped at.
        let mut misplaced = failed.clone();
        misplaced.attempt.stage = GovernanceStageV1::Agenda;
        assert!(misplaced.validate().is_err());
        let mut unrecorded = failed;
        unrecorded.rejection = None;
        assert_eq!(
            unrecorded.validate(),
            Err(Error::Restore { field: "rejection" })
        );
    }
}

#[test]
fn results_follow_the_pipeline_and_belong_to_their_sealed_bodies() {
    use AnonymousParliamentErrorV1 as Error;

    let mut state = new_state(RiskTierV1::Standard);
    let attempt = id(&state);
    let other = anonymous_governance_attempt_id_v1(CONTENT, 1);
    let rules = sealed(attempt, ParliamentBody::RulesCommittee, 9, 10);
    let agenda = sealed(attempt, ParliamentBody::AgendaCouncil, 9, 10);

    assert_eq!(
        state.complete_qualification(other),
        Err(Error::WrongAttempt)
    );
    state.complete_qualification(attempt).expect("qualified");
    assert_eq!(
        state.complete_qualification(attempt),
        Err(Error::Stage {
            expected: GovernanceStageV1::Qualification,
            actual: GovernanceStageV1::Rules,
        })
    );

    // Nothing is recorded for an unsealed body.
    assert_eq!(
        state.record_public_finding(
            attempt,
            ParliamentBody::RulesCommittee,
            finding(attempt, &rules),
            40
        ),
        Err(Error::BodyNotSealed {
            body: ParliamentBody::RulesCommittee
        })
    );
    state.seal_body(attempt, rules).expect("sealed");
    assert_eq!(
        state.seal_body(attempt, rules),
        Err(Error::BodySealed {
            body: ParliamentBody::RulesCommittee
        })
    );
    // Bodies drawn in one batch share their pulse.
    let mut agenda = agenda;
    agenda.beacon_pulse_id = rules.beacon_pulse_id;
    // A request for another seat target than the frozen one draws another body.
    let mut off_target = agenda;
    off_target.sortition_request = SortitionRequestV1::try_new_canonical(
        attempt,
        agenda.election_attempt_id,
        ParliamentBody::AgendaCouncil,
        [0x70; 32],
        2_000,
        9,
        10,
        15,
        BeaconSessionId::new([0x71; 32]),
        None,
    )
    .expect("canonical request");
    assert_eq!(
        state.seal_body(attempt, off_target),
        Err(Error::Certificate(
            AnonymousGovernanceCertificateErrorV1::SortitionRequestMismatch
        ))
    );
    // A body of another attempt is not this attempt's.
    assert_eq!(
        state.seal_body(attempt, sealed(other, ParliamentBody::AgendaCouncil, 9, 10)),
        Err(Error::Certificate(
            AnonymousGovernanceCertificateErrorV1::SortitionRequestMismatch
        ))
    );
    state.seal_body(attempt, agenda).expect("sealed");

    // Bodies seal in any order, and decide in pipeline order.
    assert_eq!(
        state.record_public_finding(
            attempt,
            ParliamentBody::AgendaCouncil,
            finding(attempt, &agenda),
            40
        ),
        Err(Error::Stage {
            expected: GovernanceStageV1::Agenda,
            actual: GovernanceStageV1::Rules,
        })
    );
    // A finding short of its quorum, or before its sortition pulse, is no result.
    let mut short = finding(attempt, &rules);
    short.finding.endorsing_assignments.pop();
    short.finding.endorsements -= 1;
    assert_eq!(
        state.record_public_finding(attempt, ParliamentBody::RulesCommittee, short, 40),
        Err(Error::Certificate(
            AnonymousGovernanceCertificateErrorV1::InvalidPublicFinding
        ))
    );
    assert_eq!(
        state.record_public_finding(
            attempt,
            ParliamentBody::RulesCommittee,
            finding(attempt, &rules),
            rules.sortition_request.pulse_height
        ),
        Err(Error::Certificate(
            AnonymousGovernanceCertificateErrorV1::InvalidLifecycle
        ))
    );
    // A jury never decides by a public finding, nor a public body by a ballot.
    assert_eq!(
        state.record_public_finding(
            attempt,
            ParliamentBody::PolicyJury,
            finding(attempt, &rules),
            40
        ),
        Err(Error::DecisionMode {
            body: ParliamentBody::PolicyJury
        })
    );
    let mut public_closure = closure(
        &state,
        &sealed(attempt, ParliamentBody::PolicyJury, JURY_SEATS, 10),
        (120, 20, 0),
        POLICY_CLOSED,
    );
    public_closure.body_role = ParliamentBody::RulesCommittee;
    assert_eq!(
        state.record_ballot_closure(attempt, public_closure),
        Err(Error::DecisionMode {
            body: ParliamentBody::RulesCommittee
        })
    );
    assert_restores(&state);
}

#[test]
fn a_closure_is_the_record_of_the_sealed_jury_under_the_locked_policy() {
    use AnonymousParliamentErrorV1 as Error;

    let state = at_policy_jury(RiskTierV1::Standard);
    let attempt = id(&state);
    let jury = *state
        .sealed_body(ParliamentBody::PolicyJury)
        .expect("sealed");
    let record = closure(&state, &jury, (120, 20, 0), POLICY_CLOSED);
    let refused = |edit: &dyn Fn(&mut BallotClosureRecordV1)| {
        let mut state = state.clone();
        let mut record = record;
        edit(&mut record);
        let result = state.record_ballot_closure(attempt, record);
        assert_eq!(state, at_policy_jury(RiskTierV1::Standard));
        result.expect_err("refused closure")
    };
    let context =
        |field| Error::Certificate(AnonymousGovernanceCertificateErrorV1::BallotContext { field });

    assert_eq!(
        refused(&|record| record.proposal_content_id = ProposalContentId::new([0x02; 32])),
        context("proposal_content_id")
    );
    assert_eq!(
        refused(
            &|record| record.governance_attempt_id = anonymous_governance_attempt_id_v1(CONTENT, 1)
        ),
        context("governance_attempt_id")
    );
    assert_eq!(
        refused(&|record| record.body_instance_id =
            anonymous_body_instance_id_v1(jury.election_attempt_id, &Digest512::new([0x13; 64]))),
        context("body_instance_id")
    );
    assert_eq!(refused(&|record| record.seats -= 1), context("seats"));
    // Casting cannot open before the pulse that drew the jury, whatever the outcome.
    let pulse_height = jury.sortition_request.pulse_height;
    for counts in [(120, 20, 0), (20, 120, 0), (40, 0, 0)] {
        let mut early = closure(&state, &jury, counts, POLICY_CLOSED);
        early.opening_anchor.height = pulse_height;
        early.casting_clock_height = pulse_height + 1;
        let mut refusing = state.clone();
        assert_eq!(
            refusing.record_ballot_closure(attempt, early),
            Err(context("opening_anchor")),
            "{counts:?}"
        );
        assert_eq!(refusing, state);
    }
    // An outcome the counts do not give, and a window the policy did not freeze.
    assert!(matches!(
        refused(&|record| record.outcome = BallotOutcomeV1::Rejected),
        Error::Ballot(AnonymousBallotBindingErrorV1::Closure(
            BallotClosureErrorV1::Outcome { .. }
        ))
    ));
    assert!(matches!(
        refused(&|record| {
            record.casting_window_ms += 1;
            record.closing_ms += 1;
        }),
        Error::Ballot(AnonymousBallotBindingErrorV1::Closure(
            BallotClosureErrorV1::PolicyMismatch {
                field: "casting_window_ms"
            }
        ))
    ));
    // The Confirmation Jury's stage has not come.
    assert_eq!(
        refused(&|record| record.body_role = ParliamentBody::ConfirmationJury),
        Error::Stage {
            expected: GovernanceStageV1::ConfirmationJury,
            actual: GovernanceStageV1::PolicyJury,
        }
    );
}

#[test]
fn the_risk_tier_rises_until_it_is_locked_and_then_decides_the_ballot() {
    use AnonymousParliamentErrorV1 as Error;

    let mut state = new_state(RiskTierV1::Standard);
    let attempt = id(&state);
    assert_eq!(
        state.escalate_risk(attempt, RiskTierV1::Standard),
        Err(Error::RiskEscalationReplay)
    );
    assert_eq!(
        state.escalate_risk(attempt, RiskTierV1::Routine),
        Err(Error::RiskDowngrade)
    );
    state
        .escalate_risk(attempt, RiskTierV1::Constitutional)
        .expect("escalated");
    assert_eq!(state.attempt().risk_tier, RiskTierV1::Constitutional);

    // A binding jury is sealed only under a locked tier, at or above its floor.
    let jury = sealed(attempt, ParliamentBody::PolicyJury, JURY_SEATS, 10);
    assert_eq!(
        state.seal_body(attempt, jury),
        Err(Error::RiskTierNotLocked)
    );
    state.lock_risk(attempt).expect("locked");
    assert!(state.risk_locked());
    assert_eq!(state.lock_risk(attempt), Err(Error::RiskTierLocked));
    assert_eq!(
        state.escalate_risk(attempt, RiskTierV1::Emergency),
        Err(Error::RiskTierLocked)
    );
    assert_eq!(
        state.seal_body(attempt, sealed(attempt, ParliamentBody::PolicyJury, 2, 10)),
        Err(Error::BelowSealFloor {
            seats: 2,
            minimum: 3
        })
    );
    state.seal_body(attempt, jury).expect("sealed");
    assert_restores(&state);

    // An emergency Policy Jury needs two thirds of its seats in favour: the same counts
    // that approve a standard attempt reject an emergency one.
    let counts = (80, 20, 0);
    let mut standard = at_policy_jury(RiskTierV1::Standard);
    assert_eq!(
        close_policy(&mut standard, counts),
        Ok(AnonymousBallotDecisionV1::Certified)
    );
    let mut emergency = at_policy_jury(RiskTierV1::Emergency);
    assert_eq!(
        close_policy(&mut emergency, counts),
        Ok(AnonymousBallotDecisionV1::Rejected(
            BallotOutcomeV1::Rejected
        ))
    );
    // A record decided under another tier than the locked one is not this attempt's.
    let decided_as_standard = at_policy_jury(RiskTierV1::Standard);
    let jury = *decided_as_standard
        .sealed_body(ParliamentBody::PolicyJury)
        .expect("sealed");
    let record = closure(&decided_as_standard, &jury, counts, POLICY_CLOSED);
    assert_eq!(record.outcome, BallotOutcomeV1::Approved);
    let mut locked_as_emergency = at_policy_jury(RiskTierV1::Emergency);
    let attempt = id(&locked_as_emergency);
    assert!(matches!(
        locked_as_emergency.record_ballot_closure(attempt, record),
        Err(Error::Ballot(AnonymousBallotBindingErrorV1::Closure(
            BallotClosureErrorV1::Outcome { .. }
        )))
    ));
}

#[test]
fn creation_takes_only_the_family_identifier_a_recognized_purpose_and_a_binding_policy() {
    use AnonymousParliamentErrorV1 as Error;

    let create = |purpose, attempt, policy, effect, head| {
        AnonymousParliamentAttemptStateV1::try_new(purpose, attempt, policy, effect, head)
    };
    let valid = attempt(0, RiskTierV1::Standard);
    assert_restores(&create(candidate(), valid, policy(), [0xd0; 32], head()).expect("valid"));

    // The baseline derivation of the identifier is another attempt's.
    let mut baseline = valid;
    baseline.id = GovernanceAttemptId::derive_v1(CONTENT, 0);
    assert_eq!(
        create(candidate(), baseline, policy(), [0xd0; 32], head()),
        Err(Error::ImmutableBinding)
    );
    assert_eq!(
        create(candidate(), valid, policy(), [0; 32], head()),
        Err(Error::ImmutableBinding)
    );
    assert_eq!(
        create(
            candidate(),
            valid,
            policy(),
            [0xd0; 32],
            GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
                subject_id: [0; 32]
            })
        ),
        Err(Error::ImmutableBinding)
    );

    // No release qualification exists, so no production attempt can be created.
    assert_eq!(
        create(
            ParliamentBallotPurposeV1::Qualified(Digest512::new([0x42; 64])),
            valid,
            policy(),
            [0xd0; 32],
            head()
        ),
        Err(Error::Purpose(ParliamentBallotPurposeErrorV1::Unqualified))
    );

    let over_limit = attempt(u32::MAX, RiskTierV1::Standard);
    assert_eq!(
        create(candidate(), over_limit, policy(), [0xd0; 32], head()),
        Err(Error::RetryLimitExceeded)
    );

    let mut started = valid;
    started.stage = GovernanceStageV1::Rules;
    assert_eq!(
        create(candidate(), started, policy(), [0xd0; 32], head()),
        Err(Error::NotInitial)
    );

    // A pipeline without binding juries is not an attempt of this family.
    let public_only = FrozenParliamentAttemptPolicyV1::freeze_v1(
        &public_bodies(),
        &ParliamentLifecycleParametersV1::default(),
        None,
        1_000,
        3_600,
    )
    .expect("public pipeline");
    assert_eq!(
        create(candidate(), valid, public_only, [0xd0; 32], head()),
        Err(Error::Policy(
            FrozenBallotPolicyErrorV1::MissingBindingPolicy
        ))
    );
}

#[test]
fn a_restored_state_is_exactly_one_the_reducer_produces() {
    use AnonymousParliamentErrorV1 as Error;

    let mut certified = at_policy_jury(RiskTierV1::Standard);
    close_policy(&mut certified, (120, 20, 0)).expect("certified");
    let active = at_policy_jury(RiskTierV1::Standard);

    // A missing or surplus result.
    let mut dropped = active.clone();
    dropped.results.remove(&ParliamentBody::RulesCommittee);
    assert_eq!(dropped.validate(), Err(Error::Restore { field: "results" }));
    let mut skipped = active.clone();
    skipped.attempt.stage = GovernanceStageV1::Review;
    assert_eq!(skipped.validate(), Err(Error::Restore { field: "results" }));

    // A result of another sealed body than the one State holds.
    let mut resealed = active.clone();
    resealed
        .sealed
        .get_mut(&ParliamentBody::RulesCommittee)
        .expect("sealed")
        .assignment_root = [0xa1; 32];
    assert_eq!(
        resealed.validate(),
        Err(Error::Restore { field: "results" })
    );

    // A certificate on an active attempt, none on a certified one, and another one.
    let mut premature = active.clone();
    premature.certificate = certified.certificate.clone();
    assert_eq!(
        premature.validate(),
        Err(Error::Restore {
            field: "certificate"
        })
    );
    let mut stripped = certified.clone();
    stripped.certificate = None;
    assert_eq!(
        stripped.validate(),
        Err(Error::Restore {
            field: "certificate"
        })
    );
    let mut altered = certified.clone();
    altered
        .certificate
        .as_mut()
        .expect("certificate")
        .effect_preimage_hash = [0xd1; 32];
    assert_eq!(
        altered.validate(),
        Err(Error::Restore {
            field: "certificate"
        })
    );

    // A rejection without its closure, and a closure that approved.
    let mut rejected = active.clone();
    close_policy(&mut rejected, (20, 120, 0)).expect("rejected");
    let mut unrecorded = rejected.clone();
    unrecorded.rejection = None;
    assert_eq!(
        unrecorded.validate(),
        Err(Error::Restore { field: "rejection" })
    );
    let jury = *active
        .sealed_body(ParliamentBody::PolicyJury)
        .expect("sealed");
    let mut approving = rejected;
    approving.rejection = Some(AnonymousAttemptRejectionV1::Ballot(Box::new(closure(
        &active,
        &jury,
        (120, 20, 0),
        POLICY_CLOSED,
    ))));
    assert_eq!(
        approving.validate(),
        Err(Error::Restore { field: "rejection" })
    );

    // A binding jury sealed without the risk lock, or a stage outside the pipeline.
    let mut unlocked = active.clone();
    unlocked.risk_locked = false;
    assert_eq!(unlocked.validate(), Err(Error::RiskTierNotLocked));
    let mut off_pipeline = active;
    off_pipeline.attempt.stage = GovernanceStageV1::Mpc;
    assert_eq!(
        off_pipeline.validate(),
        Err(Error::Restore { field: "stage" })
    );
}
