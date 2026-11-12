//! Tests of the anonymous attempt reducer's spine: purpose isolation, the result pipeline,
//! certification, execution and the restore check. Every attempt is run through its real
//! lifecycle by the [`super::test_driver::Driver`].

use std::collections::BTreeSet;

use iroha_data_model::governance::{
    anonymous_ballot::{
        BallotClosureErrorV1, Digest512, PARLIAMENT_BALLOT_QUALIFICATION_V1,
        ParliamentLifecycleParametersV1, anonymous_body_instance_id_v1,
        anonymous_governance_attempt_id_v1,
    },
    types::{GovernanceExpectedHeadAbsentV1, GovernanceExpectedHeadPresentV1, ProposalContentId},
};

use super::{
    test_driver::{
        Answer, CONTENT, Driver, SUBJECT, assert_restores, attempt, candidate, head, network,
        pipeline, policy, public_bodies,
    },
    *,
};

/// A certified attempt whose Policy Jury approved clearly.
fn certified() -> Driver {
    let mut driver = Driver::at_policy_reflection(RiskTierV1::Standard);
    assert_eq!(
        driver.close_ballot(ParliamentBody::PolicyJury, (20, 5, 0)),
        Ok(AnonymousBallotDecisionV1::Certified)
    );
    driver
}

/// An attempt whose narrowly approving Policy Jury admitted a Confirmation Jury, now
/// sealed and in Reflection.
fn at_confirmation_reflection() -> Driver {
    let mut driver = Driver::at_policy_reflection(RiskTierV1::Standard);
    assert_eq!(
        driver.close_ballot(ParliamentBody::PolicyJury, (13, 12, 0)),
        Ok(AnonymousBallotDecisionV1::ConfirmationRequired)
    );
    assert_eq!(
        driver.state.attempt().stage,
        GovernanceStageV1::ConfirmationJury
    );
    driver.assert_restores();
    driver.advance_to_slot();
    driver.answer(ParliamentBody::ConfirmationJury, Answer::Primaries);
    assert_eq!(
        test_driver::names(&driver.step()),
        vec![("CloseInvitations", Some(ParliamentBody::ConfirmationJury))]
    );
    driver.deliberate(ParliamentBody::ConfirmationJury);
    driver.assert_restores();
    driver
}

#[test]
fn a_clear_approval_certifies_the_attempt_from_its_recorded_results() {
    let driver = certified();
    let state = &driver.state;
    assert_eq!(state.attempt().status, GovernanceAttemptStatusV1::Certified);
    assert_eq!(state.attempt().stage, GovernanceStageV1::Enactment);
    assert!(!state.confirmation_required());
    driver.assert_restores();

    let certificate = state.certificate().expect("certificate");
    assert_eq!(certificate.validate(), Ok(()));
    assert_eq!(certificate.purpose, candidate());
    assert_eq!(certificate.policy, policy());
    assert_eq!(
        certificate.certified_at(),
        state
            .result(ParliamentBody::PolicyJury)
            .and_then(AnonymousParliamentBodyBindingV1::ballot)
            .map(|ballot| ballot.closure.closure_anchor)
    );
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
    // A certified candidate attempt has no lifecycle item left.
    assert_eq!(state.lifecycle_items(), Ok(Vec::new()));
}

#[test]
fn a_candidate_certificate_is_never_executed() {
    let driver = certified();
    let mut state = driver.state.clone();
    let attempt = driver.id();
    let certified = state.clone();
    // Long after the enactment delay.
    let late = test_driver::anchor(900_000, 0, u64::MAX / 2);
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
        assert_eq!(
            forged.validate(&network()),
            Err(refused.clone()),
            "{status:?}"
        );
    }
    // Relabelling the purpose does not help: this release names no qualification.
    let mut relabelled = certified;
    relabelled.purpose = ParliamentBallotPurposeV1::Qualified(Digest512::new([0x42; 64]));
    assert_eq!(
        relabelled.validate(&network()),
        Err(AnonymousParliamentErrorV1::Purpose(
            ParliamentBallotPurposeErrorV1::Unqualified
        ))
    );
}

#[test]
fn execution_is_due_on_the_governance_clock_and_superseded_only_by_another_head() {
    let driver = certified();
    let certificate = driver.state.certificate().expect("certificate");
    let closed = certificate.certified_at().expect("certified");
    let due_ms = certificate.enact_at_ms().expect("due time");
    assert_eq!(
        due_ms,
        closed.timestamp_ms + policy().lifecycle_policy.enactment_delay_ms
    );
    let anchor = test_driver::anchor;

    assert_eq!(
        execution_is_due(certificate, anchor(closed.height + 1, 0, due_ms - 1)),
        Err(AnonymousParliamentErrorV1::EnactmentNotDue {
            due_ms,
            now_ms: due_ms - 1
        })
    );
    assert_eq!(
        execution_is_due(certificate, anchor(closed.height + 1, 0, due_ms)),
        Ok(())
    );
    // An item follows the certifying closure: never an earlier block, whatever its time
    // claims, and in the certifying block only later in the same pass at the block's time.
    for not_after in [
        anchor(closed.height - 1, 0, due_ms),
        closed,
        anchor(closed.height, closed.position + 1, due_ms),
    ] {
        assert_eq!(
            execution_is_due(certificate, not_after),
            Err(AnonymousParliamentErrorV1::ExecutionAnchor),
            "{not_after:?}"
        );
    }
    assert_eq!(
        execution_is_due(
            certificate,
            anchor(closed.height, closed.position + 1, closed.timestamp_ms)
        ),
        Err(AnonymousParliamentErrorV1::EnactmentNotDue {
            due_ms,
            now_ms: closed.timestamp_ms
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
    let mut driver = at_confirmation_reflection();
    assert!(driver.state.confirmation_required());
    assert_eq!(
        driver.state.attempt().status,
        GovernanceAttemptStatusV1::Active
    );
    assert_eq!(driver.state.certificate(), None);
    // The Confirmation Jury's generation was registered by the Policy closure's block.
    let policy_result = driver
        .state
        .result(ParliamentBody::PolicyJury)
        .expect("Policy result")
        .result_height;
    let confirmation = *driver
        .state
        .sealed_body(ParliamentBody::ConfirmationJury)
        .expect("sealed");
    assert_eq!(confirmation.sortition_request.request_height, policy_result);
    assert_ne!(
        confirmation.beacon_pulse_id,
        driver
            .state
            .sealed_body(ParliamentBody::PolicyJury)
            .expect("sealed")
            .beacon_pulse_id
    );

    let mut rejected = Driver {
        state: driver.state.clone(),
        height: driver.height,
        now_ms: driver.now_ms,
        citizens: driver.citizens.clone(),
    };
    assert_eq!(
        driver.close_ballot(ParliamentBody::ConfirmationJury, (12, 2, 0)),
        Ok(AnonymousBallotDecisionV1::Certified)
    );
    driver.assert_restores();
    let certificate = driver.state.certificate().expect("certificate");
    assert!(certificate.body(ParliamentBody::ConfirmationJury).is_some());
    assert_eq!(
        certificate.certified_at(),
        driver
            .state
            .result(ParliamentBody::ConfirmationJury)
            .and_then(AnonymousParliamentBodyBindingV1::ballot)
            .map(|ballot| ballot.closure.closure_anchor)
    );

    // A Confirmation Jury that does not approve rejects the attempt.
    assert_eq!(
        rejected.close_ballot(ParliamentBody::ConfirmationJury, (2, 12, 0)),
        Ok(AnonymousBallotDecisionV1::Rejected(
            BallotOutcomeV1::Rejected
        ))
    );
    assert!(matches!(
        rejected.state.rejection(),
        Some(AnonymousAttemptRejectionV1::Ballot(record))
            if record.body_role == ParliamentBody::ConfirmationJury
    ));
    assert!(rejected.state.is_terminal());
    rejected.assert_restores();
}

#[test]
fn a_confirmation_jury_is_the_fresh_draw_the_narrow_approval_admitted() {
    let driver = at_confirmation_reflection();
    let state = &driver.state;
    let mismatch = AnonymousParliamentErrorV1::Certificate(
        AnonymousGovernanceCertificateErrorV1::ConfirmationJuryMismatch,
    );
    let fresh = *state
        .sealed_body(ParliamentBody::ConfirmationJury)
        .expect("sealed");
    // The jury's snapshot excluded every sealed Policy Jury member.
    let policy_members: BTreeSet<_> = state
        .body(ParliamentBody::PolicyJury)
        .expect("sealed")
        .seats()
        .iter()
        .map(|seat| seat.member.clone())
        .collect();
    let (generation, _) = state
        .active_election(ParliamentBody::ConfirmationJury)
        .expect("election");
    assert!(
        state.generations()[generation]
            .candidates()
            .iter()
            .all(|candidate| !policy_members.contains(candidate))
    );

    // A seal that reuses the Policy Jury's pulse is not the fresh draw.
    let mut same_pulse = fresh;
    same_pulse.beacon_pulse_id = state
        .sealed_body(ParliamentBody::PolicyJury)
        .expect("sealed")
        .beacon_pulse_id;
    assert_eq!(state.check_seal(&same_pulse), Err(mismatch.clone()));
    // Nor one requested before the Policy closure's block.
    let mut stale = fresh;
    stale.sortition_request.request_height -= 1;
    stale.sortition_request.id = stale.sortition_request.canonical_id();
    assert_eq!(state.check_seal(&stale), Err(mismatch));
    assert_eq!(state.check_seal(&fresh), Ok(()));

    // A restored Confirmation Jury is held to the same rule.
    let mut restored = state.clone();
    restored
        .bodies
        .get_mut(&ParliamentBody::ConfirmationJury)
        .expect("sealed")
        .sealed
        .beacon_pulse_id = same_pulse.beacon_pulse_id;
    assert!(restored.validate(&network()).is_err());
}

#[test]
fn a_narrow_approval_without_fresh_capacity_ends_the_attempt_atomically() {
    // Twenty-six citizens: the Policy Jury seats twenty-five of them, leaving one for the
    // Confirmation Jury, below the anonymity floor.
    let mut driver = Driver::new(RiskTierV1::Standard, 26);
    driver.qualify();
    driver.register_initial();
    driver.advance_to_slot();
    for body in pipeline() {
        if body != ParliamentBody::ConfirmationJury {
            driver.answer(body, Answer::Primaries);
        }
    }
    driver.step();
    for body in public_bodies() {
        driver.conclude_public(body);
    }
    driver.deliberate(ParliamentBody::PolicyJury);
    let generations = driver.state.generations().len();
    assert_eq!(
        driver.close_ballot(ParliamentBody::PolicyJury, (13, 12, 0)),
        Ok(AnonymousBallotDecisionV1::ConfirmationCapacityUnavailable)
    );
    let state = &driver.state;
    assert_eq!(state.attempt().status, GovernanceAttemptStatusV1::Rejected);
    assert_eq!(state.attempt().stage, GovernanceStageV1::PolicyJury);
    // No partial certificate: no Policy result, no Confirmation generation.
    assert_eq!(state.result(ParliamentBody::PolicyJury), None);
    assert_eq!(state.generations().len(), generations);
    assert_eq!(state.certificate(), None);
    assert!(matches!(
        state.rejection(),
        Some(AnonymousAttemptRejectionV1::ConfirmationCapacityUnavailable(record))
            if record.narrow
    ));
    assert!(state.is_terminal());
    driver.assert_restores();
}

#[test]
fn any_other_closure_rejects_the_attempt_for_good() {
    for (counts, outcome) in [
        ((5, 20, 0), BallotOutcomeV1::Rejected),
        ((12, 12, 0), BallotOutcomeV1::Rejected),
        ((10, 0, 0), BallotOutcomeV1::NoQuorum),
    ] {
        let mut driver = Driver::at_policy_reflection(RiskTierV1::Standard);
        assert_eq!(
            driver.close_ballot(ParliamentBody::PolicyJury, counts),
            Ok(AnonymousBallotDecisionV1::Rejected(outcome)),
            "{counts:?}"
        );
        let state = &driver.state;
        assert_eq!(state.attempt().status, GovernanceAttemptStatusV1::Rejected);
        assert_eq!(state.attempt().stage, GovernanceStageV1::PolicyJury);
        assert!(matches!(
            state.rejection(),
            Some(AnonymousAttemptRejectionV1::Ballot(record)) if record.outcome == outcome
        ));
        assert_eq!(state.result(ParliamentBody::PolicyJury), None);
        assert_eq!(state.certificate(), None);
        assert!(state.is_terminal());
        assert_eq!(state.lifecycle_items(), Ok(Vec::new()));
        driver.assert_restores();

        // A binding jury has exactly one ballot: nothing follows a rejection.
        let attempt = driver.id();
        let inactive = AnonymousParliamentErrorV1::NotActive {
            status: GovernanceAttemptStatusV1::Rejected,
        };
        let mut again = driver.state.clone();
        assert_eq!(
            again.escalate_risk(attempt, RiskTierV1::Emergency),
            Err(inactive.clone())
        );
        assert_eq!(
            again.mark_enacted(attempt, test_driver::anchor(900_000, 0, u64::MAX / 2)),
            Err(AnonymousParliamentErrorV1::NotCertified)
        );
        assert_eq!(
            again.register_sortition_retry(
                attempt,
                &network(),
                driver.height,
                driver.now_ms,
                Vec::new()
            ),
            Err(inactive)
        );
    }
}

#[test]
fn a_closure_is_the_record_of_the_sealed_jury_under_the_locked_policy() {
    use AnonymousParliamentErrorV1 as Error;

    let mut driver = Driver::at_policy_reflection(RiskTierV1::Standard);
    let record = driver.close_record(ParliamentBody::PolicyJury, (20, 5, 0));
    let state = driver.state.clone();
    let attempt = driver.id();
    let jury = *state
        .sealed_body(ParliamentBody::PolicyJury)
        .expect("sealed");
    let refused = |edit: &dyn Fn(&mut BallotClosureRecordV1)| {
        let mut changed = state.clone();
        let mut record = record;
        edit(&mut record);
        let result = changed.close_ballot(attempt, record, &network(), Vec::new);
        assert_eq!(changed, state, "a refused closure changes nothing");
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
    // Casting cannot open before the jury's Reflection, whatever the outcome.
    let reflection = state
        .body(ParliamentBody::PolicyJury)
        .and_then(|body| body.current_phase())
        .expect("Reflection")
        .entered_at;
    for (aye, nay) in [(20, 5), (5, 20)] {
        let early = |record: &mut BallotClosureRecordV1| {
            record.aye = aye;
            record.nay = nay;
            record.outcome = if aye > nay {
                BallotOutcomeV1::Approved
            } else {
                BallotOutcomeV1::Rejected
            };
            record.opening_anchor = reflection;
            record.casting_clock_height = reflection.height + 1;
        };
        assert_eq!(refused(&early), Error::BallotChronology);
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
    // A jury never decides by a public finding, nor a public body by a ballot.
    assert_eq!(
        refused(&|record| record.body_role = ParliamentBody::RulesCommittee),
        Error::DecisionMode {
            body: ParliamentBody::RulesCommittee
        }
    );
}

#[test]
fn the_risk_tier_rises_until_the_initial_registration_locks_it() {
    use AnonymousParliamentErrorV1 as Error;

    let mut driver = Driver::new(RiskTierV1::Standard, 40);
    let attempt = driver.id();
    assert_eq!(
        driver.state.escalate_risk(attempt, RiskTierV1::Standard),
        Err(Error::RiskEscalationReplay)
    );
    assert_eq!(
        driver.state.escalate_risk(attempt, RiskTierV1::Routine),
        Err(Error::RiskDowngrade)
    );
    driver
        .state
        .escalate_risk(attempt, RiskTierV1::Constitutional)
        .expect("escalated");
    assert_eq!(driver.state.attempt().risk_tier, RiskTierV1::Constitutional);
    driver.qualify();
    assert!(!driver.state.risk_locked());
    driver.register_initial();
    assert!(driver.state.risk_locked());
    assert_eq!(
        driver.state.escalate_risk(attempt, RiskTierV1::Emergency),
        Err(Error::RiskTierLocked)
    );
    driver.assert_restores();

    // An emergency Policy Jury needs two thirds of its seats in favour: the same counts
    // that approve a standard attempt reject an emergency one.
    let counts = (15, 9, 0);
    let mut standard = Driver::at_policy_reflection(RiskTierV1::Standard);
    assert_eq!(
        standard.close_ballot(ParliamentBody::PolicyJury, counts),
        Ok(AnonymousBallotDecisionV1::Certified)
    );
    let mut emergency = Driver::at_policy_reflection(RiskTierV1::Emergency);
    assert_eq!(
        emergency.close_ballot(ParliamentBody::PolicyJury, counts),
        Ok(AnonymousBallotDecisionV1::Rejected(
            BallotOutcomeV1::Rejected
        ))
    );
    // A record decided under another tier than the locked one is not this attempt's: the
    // counts approve under the standard tier, which the emergency attempt did not lock.
    let mut locked_as_emergency = Driver::at_policy_reflection(RiskTierV1::Emergency);
    let mut record = locked_as_emergency.close_record(ParliamentBody::PolicyJury, (20, 5, 0));
    record.aye = counts.0;
    record.nay = counts.1;
    record.accepted_ballots = counts.0 + counts.1;
    record.outcome = BallotOutcomeV1::Approved;
    record.narrow = false;
    let attempt = locked_as_emergency.id();
    assert!(matches!(
        locked_as_emergency
            .state
            .close_ballot(attempt, record, &network(), Vec::new),
        Err(Error::Ballot(AnonymousBallotBindingErrorV1::Closure(
            BallotClosureErrorV1::Outcome { .. }
        )))
    ));
}

#[test]
fn creation_takes_only_the_family_identifier_a_recognized_purpose_and_a_binding_policy() {
    use AnonymousParliamentErrorV1 as Error;

    let origin = AnonymousAttemptOriginV1 {
        created_at_ms: test_driver::CREATED_MS,
        randomness_redraws_before_attempt: 0,
    };
    let create = |purpose, attempt, policy, effect, head, origin| {
        AnonymousParliamentAttemptStateV1::try_new(
            purpose,
            attempt,
            policy,
            test_driver::PROPOSAL_DIGEST,
            effect,
            head,
            origin,
        )
    };
    let valid = attempt(0, RiskTierV1::Standard);
    let created = create(candidate(), valid, policy(), [0xd0; 32], head(), origin).expect("valid");
    assert_restores(&created);
    assert_eq!(created.randomness_redraws_before_attempt(), 0);
    assert_eq!(
        created.qualification().created_at_ms(),
        test_driver::CREATED_MS
    );

    // The baseline derivation of the identifier is another attempt's.
    let mut baseline = valid;
    baseline.id = GovernanceAttemptId::derive_v1(CONTENT, 0);
    assert_eq!(
        create(candidate(), baseline, policy(), [0xd0; 32], head(), origin),
        Err(Error::ImmutableBinding)
    );
    assert_eq!(
        create(candidate(), valid, policy(), [0; 32], head(), origin),
        Err(Error::ImmutableBinding)
    );
    assert_eq!(
        AnonymousParliamentAttemptStateV1::try_new(
            candidate(),
            valid,
            policy(),
            Digest512::new([0; 64]),
            [0xd0; 32],
            head(),
            origin,
        ),
        Err(Error::ImmutableBinding)
    );
    assert_eq!(created.proposal_digest(), &test_driver::PROPOSAL_DIGEST);
    assert_eq!(
        create(
            candidate(),
            valid,
            policy(),
            [0xd0; 32],
            GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
                subject_id: [0; 32]
            }),
            origin
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
            head(),
            origin
        ),
        Err(Error::Purpose(ParliamentBallotPurposeErrorV1::Unqualified))
    );

    let over_limit = attempt(u32::MAX, RiskTierV1::Standard);
    assert_eq!(
        create(
            candidate(),
            over_limit,
            policy(),
            [0xd0; 32],
            head(),
            origin
        ),
        Err(Error::RetryLimitExceeded)
    );

    let mut started = valid;
    started.stage = GovernanceStageV1::Rules;
    assert_eq!(
        create(candidate(), started, policy(), [0xd0; 32], head(), origin),
        Err(Error::NotInitial)
    );

    // The redraw prefix: a first attempt inherits nothing; a successor inherits less than
    // the ceiling, so it can still draw once.
    let ceiling = policy().sortition_policy.max_randomness_redraws;
    let inherited = |count| AnonymousAttemptOriginV1 {
        created_at_ms: test_driver::CREATED_MS,
        randomness_redraws_before_attempt: count,
    };
    assert_eq!(
        create(
            candidate(),
            valid,
            policy(),
            [0xd0; 32],
            head(),
            inherited(1)
        ),
        Err(Error::RedrawLineage)
    );
    let successor = attempt(1, RiskTierV1::Standard);
    assert!(
        create(
            candidate(),
            successor,
            policy(),
            [0xd0; 32],
            head(),
            inherited(ceiling - 1)
        )
        .is_ok()
    );
    assert_eq!(
        create(
            candidate(),
            successor,
            policy(),
            [0xd0; 32],
            head(),
            inherited(ceiling)
        ),
        Err(Error::RedrawLineage)
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
        create(candidate(), valid, public_only, [0xd0; 32], head(), origin),
        Err(Error::Policy(
            FrozenBallotPolicyErrorV1::MissingBindingPolicy
        ))
    );
}

#[test]
fn a_restored_state_is_exactly_one_the_reducer_produces() {
    use AnonymousParliamentErrorV1 as Error;

    let certified = certified().state;
    let active = Driver::at_policy_reflection(RiskTierV1::Standard).state;
    let validate = |state: &AnonymousParliamentAttemptStateV1| state.validate(&network());

    // A missing or surplus result.
    let mut dropped = active.clone();
    dropped.results.remove(&ParliamentBody::RulesCommittee);
    assert_eq!(validate(&dropped), Err(Error::Restore { field: "results" }));
    let mut skipped = active.clone();
    skipped.attempt.stage = GovernanceStageV1::Review;
    assert_eq!(validate(&skipped), Err(Error::Restore { field: "results" }));

    // A result of another sealed body than the one State holds.
    let mut resealed = active.clone();
    resealed
        .bodies
        .get_mut(&ParliamentBody::RulesCommittee)
        .expect("sealed")
        .sealed
        .assignment_root = [0xa1; 32];
    assert_eq!(
        validate(&resealed),
        Err(Error::Restore { field: "results" })
    );

    // A certificate on an active attempt, none on a certified one, and another one.
    let mut premature = active.clone();
    premature.certificate = certified.certificate.clone();
    assert_eq!(
        validate(&premature),
        Err(Error::Restore {
            field: "certificate"
        })
    );
    let mut stripped = certified.clone();
    stripped.certificate = None;
    assert_eq!(
        validate(&stripped),
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
        validate(&altered),
        Err(Error::Restore {
            field: "certificate"
        })
    );

    // A rejection without its closure, and a closure that approved.
    let mut rejecting = Driver::at_policy_reflection(RiskTierV1::Standard);
    let approval = rejecting.close_record(ParliamentBody::PolicyJury, (20, 5, 0));
    let mut rejection = approval;
    rejection.aye = 5;
    rejection.nay = 20;
    rejection.outcome = BallotOutcomeV1::Rejected;
    let attempt = rejecting.id();
    rejecting
        .state
        .close_ballot(attempt, rejection, &network(), Vec::new)
        .expect("rejected");
    let mut unrecorded = rejecting.state.clone();
    unrecorded.rejection = None;
    assert_eq!(
        validate(&unrecorded),
        Err(Error::Restore { field: "rejection" })
    );
    let mut approving = rejecting.state;
    approving.rejection = Some(AnonymousAttemptRejectionV1::Ballot(Box::new(approval)));
    assert_eq!(
        validate(&approving),
        Err(Error::Restore { field: "rejection" })
    );

    // A binding jury sealed without the risk lock, or a stage outside the pipeline.
    let mut unlocked = active.clone();
    unlocked.risk_locked = false;
    assert_eq!(validate(&unlocked), Err(Error::RiskTierNotLocked));
    let mut off_pipeline = active;
    off_pipeline.attempt.stage = GovernanceStageV1::Mpc;
    assert_eq!(
        validate(&off_pipeline),
        Err(Error::Restore { field: "stage" })
    );
}
