//! Ordered certificate validation and independent resource-bound regressions.

use super::*;

fn policy_ballot_fixture(body_instance_id: BodyInstanceId) -> ParliamentBallotCertificateBindingV1 {
    let ballot_attempt_sequence = 0;
    let ballot_attempt_id = BallotAttemptId::derive_v1(body_instance_id, ballot_attempt_sequence);
    let release_beacon_session_id = BeaconSessionId::new([0x85; 32]);
    let tle_key_session_id = TleKeySessionId::new([0x8E; 32]);
    let release_height = 1_757;
    ParliamentBallotCertificateBindingV1 {
        ballot_attempt_id,
        ballot_attempt_sequence,
        tle_session_id: TleSessionId::derive_v1(
            ballot_attempt_id,
            tle_key_session_id,
            release_beacon_session_id,
            release_height,
        ),
        tle_key_session_id,
        registration_root: [0x81; 32],
        dropout_root: [0x82; 32],
        survivor_root: [0x83; 32],
        corpus_root: [0x6D; 32],
        no_recovery_root: [0x6E; 32],
        timed_commitment_root: [0x84; 32],
        release_beacon_session_id,
        registered_at_height: 140,
        registration_close_height: 641,
        survivor_freeze_height: 1_141,
        commitment_close_height: 1_157,
        registration_closed_at_height: 641,
        survivors_frozen_at_height: 1_141,
        commitment_closed_at_height: 1_157,
        max_ballot_retries: 3,
        max_corpus_entries: 500,
        release_height,
        opening_deadline_height: 2_357,
        release_pulse_id: BeaconPulseId::new([0x7D; 32]),
        opening_height: release_height,
        opening_root: [0x7E; 32],
        tally: ParliamentAggregateTallyV1 {
            original_seats: 500,
            accepted_ballots: 334,
            aye: 200,
            nay: 100,
            abstain: 34,
        },
        outcome: ParliamentAggregateOutcomeV1::Approved,
    }
}

fn governance_certificate_fixture() -> GovernanceCertificateV1 {
    let proposal_content_id = ProposalContentId::new([0x61; 32]);
    let governance_attempt_sequence = 0;
    let governance_attempt_id =
        GovernanceAttemptId::derive_v1(proposal_content_id, governance_attempt_sequence);
    let election_attempt_sequence = 0;
    let election_attempt_id = BodyElectionAttemptId::derive_v1(
        governance_attempt_id,
        ParliamentBody::PolicyJury,
        election_attempt_sequence,
    );
    let sortition_request = SortitionRequestV1::try_new_canonical(
        governance_attempt_id,
        election_attempt_id,
        ParliamentBody::PolicyJury,
        [0x80; 32],
        1_000,
        500,
        100,
        101,
        BeaconSessionId::new([0x66; 32]),
        None,
    )
    .expect("canonical Policy Jury request");
    let roster_root = [0x68; 32];
    let body_instance_id = BodyInstanceId::derive_v1(election_attempt_id, roster_root);
    let ballot = policy_ballot_fixture(body_instance_id);
    let result_height = 1_800;
    let result_root = parliament_ballot_result_root_v1(
        governance_attempt_id,
        body_instance_id,
        ballot.ballot_attempt_id,
        ballot.opening_root,
        ballot.tally,
        ballot.outcome,
        result_height,
    );
    GovernanceCertificateV1 {
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
            original_seats: ballot.tally.original_seats,
            beacon_session_id: BeaconSessionId::new([0x66; 32]),
            beacon_pulse_id: BeaconPulseId::new([0x67; 32]),
            roster_root,
            assignment_root: [0x69; 32],
            result_root,
            result_height,
            public_finding: None,
            ballot: Some(ballot),
        }],
        policy_version: PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1,
        effect_preimage_hash: [0x6F; 32],
        expected_head: GovernanceExpectedHeadV1::Present(GovernanceExpectedHeadPresentV1 {
            subject_id: [0x70; 32],
            version: 3,
            head_root: [0x71; 32],
        }),
        certified_at_height: result_height,
        enact_at_height: result_height + 1,
    }
}

fn add_public_finding(certificate: &mut GovernanceCertificateV1) {
    let attempt = certificate.governance_attempt_id;
    let election_attempt_sequence = 0;
    let election_attempt_id = BodyElectionAttemptId::derive_v1(
        attempt,
        ParliamentBody::RulesCommittee,
        election_attempt_sequence,
    );
    let request = SortitionRequestV1::try_new_canonical(
        attempt,
        election_attempt_id,
        ParliamentBody::RulesCommittee,
        [0xB0; 32],
        3,
        3,
        80,
        81,
        BeaconSessionId::new([0xB1; 32]),
        None,
    )
    .expect("canonical public-finding request");
    let roster_root = [0xB2; 32];
    let body_instance_id = BodyInstanceId::derive_v1(election_attempt_id, roster_root);
    let result_root = [0xB3; 32];
    let endorsing_assignments = vec![AssignmentId::new([0xB4; 32]), AssignmentId::new([0xB5; 32])];
    let endorsement_root = parliament_public_finding_endorsement_root_v1(
        attempt,
        body_instance_id,
        result_root,
        &endorsing_assignments,
    );
    certificate.body_bindings.insert(
        0,
        ParliamentBodyCertificateBindingV1 {
            body_instance_id,
            election_attempt_id,
            election_attempt_sequence,
            sortition_request_id: request.id,
            sortition_request: request,
            body: ParliamentBody::RulesCommittee,
            original_seats: 3,
            beacon_session_id: BeaconSessionId::new([0xB1; 32]),
            beacon_pulse_id: BeaconPulseId::new([0xB6; 32]),
            roster_root,
            assignment_root: [0xB7; 32],
            result_root,
            result_height: 90,
            public_finding: Some(ParliamentPublicFindingCertificateBindingV1 {
                endorsement_root,
                endorsing_assignments,
                endorsements: 2,
                quorum: 2,
            }),
            ballot: None,
        },
    );
}

fn refresh_policy_result(certificate: &mut GovernanceCertificateV1) {
    let binding = &mut certificate.body_bindings[0];
    let ballot = binding.ballot.expect("fixture has a Policy Jury ballot");
    binding.result_root = parliament_ballot_result_root_v1(
        certificate.governance_attempt_id,
        binding.body_instance_id,
        ballot.ballot_attempt_id,
        ballot.opening_root,
        ballot.tally,
        ballot.outcome,
        binding.result_height,
    );
}

#[test]
fn certificate_context_precedes_body_validation() {
    let valid = governance_certificate_fixture();
    assert_eq!(valid.validate(), Ok(()));
    let mut candidate = valid.clone();
    candidate.effect_preimage_hash = [0; 32];
    candidate.governance_attempt_sequence = MAX_PARLIAMENT_GOVERNANCE_ATTEMPT_RETRIES_V1 + 1;
    candidate.body_bindings.clear();
    candidate.certified_at_height = 0;
    candidate.expected_head = GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
        subject_id: [0; 32],
    });
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::ZeroBinding)
    );
    candidate.effect_preimage_hash = valid.effect_preimage_hash;
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::NonCanonicalIdentifier)
    );
    candidate.governance_attempt_id = GovernanceAttemptId::derive_v1(
        candidate.proposal_content_id,
        candidate.governance_attempt_sequence,
    );
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::RetryLimitExceeded)
    );
    candidate.governance_attempt_sequence = valid.governance_attempt_sequence;
    candidate.governance_attempt_id = valid.governance_attempt_id;
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::EmptyBodyBindings)
    );
    candidate.body_bindings.clone_from(&valid.body_bindings);
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::InvalidLifecycle)
    );
    candidate.certified_at_height = valid.certified_at_height;
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::InvalidExpectedHead)
    );
    candidate.expected_head = valid.expected_head;
    assert_eq!(candidate.validate(), Ok(()));
}

#[test]
fn body_identity_precedes_evidence_and_ballot_identity() {
    let valid = governance_certificate_fixture();
    let mut candidate = valid.clone();
    candidate.body_bindings[0].roster_root = [0; 32];
    candidate.body_bindings[0].sortition_request_id = SortitionRequestId::new([0xA1; 32]);
    candidate.body_bindings[0].original_seats = 501;
    candidate.body_bindings[0].election_attempt_sequence = MAX_PARLIAMENT_SORTITION_RETRIES_V1 + 1;
    candidate.body_bindings[0].ballot = None;
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::ZeroBinding)
    );
    candidate.body_bindings[0].roster_root = valid.body_bindings[0].roster_root;
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::SortitionRequestMismatch)
    );
    candidate.body_bindings[0].sortition_request_id = valid.body_bindings[0].sortition_request_id;
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::InvalidSeatCount)
    );
    candidate.body_bindings[0].original_seats = valid.body_bindings[0].original_seats;
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::RetryLimitExceeded)
    );
    candidate.body_bindings[0].election_attempt_sequence =
        valid.body_bindings[0].election_attempt_sequence;
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::MissingBindingBallot)
    );
    let mut ballot = valid.body_bindings[0]
        .ballot
        .expect("fixture policy ballot");
    ballot.corpus_root = [0; 32];
    ballot.ballot_attempt_id = BallotAttemptId::new([0xA2; 32]);
    candidate.body_bindings[0].ballot = Some(ballot);
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::ZeroBinding)
    );
    candidate.body_bindings[0]
        .ballot
        .as_mut()
        .unwrap()
        .corpus_root = [0x6D; 32];
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::NonCanonicalIdentifier)
    );
    candidate.body_bindings[0].ballot = valid.body_bindings[0].ballot;
    assert_eq!(candidate.validate(), Ok(()));
}

#[test]
fn ballot_lifecycle_and_tally_precede_result_and_cross_pulse_checks() {
    let valid = governance_certificate_fixture();
    let mut candidate = valid.clone();
    let pulse = candidate.body_bindings[0].beacon_pulse_id;
    let ballot = candidate.body_bindings[0].ballot.as_mut().unwrap();
    ballot.registered_at_height = 0;
    ballot.tally.aye += 1;
    ballot.outcome = ParliamentAggregateOutcomeV1::Rejected;
    ballot.release_pulse_id = pulse;
    candidate.body_bindings[0].result_root[0] ^= 1;
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::InvalidLifecycle)
    );
    candidate.body_bindings[0]
        .ballot
        .as_mut()
        .unwrap()
        .registered_at_height = 140;
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::InvalidTally(
            ParliamentTallyErrorV1::CountSumMismatch {
                accepted_ballots: 334,
                counted_ballots: 335
            }
        ))
    );
    candidate.body_bindings[0]
        .ballot
        .as_mut()
        .unwrap()
        .tally
        .aye -= 1;
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::TallyOutcomeMismatch)
    );
    candidate.body_bindings[0].ballot.as_mut().unwrap().outcome =
        ParliamentAggregateOutcomeV1::Approved;
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::BallotResultRootMismatch)
    );
    candidate.body_bindings[0].result_root = valid.body_bindings[0].result_root;
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::DuplicateBinding)
    );
    candidate.body_bindings[0].ballot = valid.body_bindings[0].ballot;
    assert_eq!(candidate.validate(), Ok(()));
}

#[test]
fn earlier_public_evidence_precedes_later_body_identity() {
    let mut valid = governance_certificate_fixture();
    add_public_finding(&mut valid);
    assert_eq!(valid.validate(), Ok(()));
    let mut candidate = valid.clone();
    candidate.body_bindings[0].public_finding = None;
    candidate.body_bindings[1].roster_root = [0; 32];
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::MissingPublicFinding)
    );
    candidate.body_bindings[0].public_finding = valid.body_bindings[0].public_finding.clone();
    candidate.body_bindings[0]
        .public_finding
        .as_mut()
        .unwrap()
        .endorsements = 1;
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::InvalidPublicFinding)
    );
    candidate.body_bindings[0].public_finding = valid.body_bindings[0].public_finding.clone();
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::ZeroBinding)
    );
    candidate.body_bindings[1].roster_root = valid.body_bindings[1].roster_root;
    assert_eq!(candidate.validate(), Ok(()));
}

#[test]
fn ballot_resource_windows_reject_each_short_boundary() {
    let valid = governance_certificate_fixture();
    for window in 0..3 {
        let mut candidate = valid.clone();
        let ballot = candidate.body_bindings[0].ballot.as_mut().unwrap();
        match window {
            0 => {
                ballot.registration_close_height -= 1;
                ballot.registration_closed_at_height = ballot.registration_close_height;
            }
            1 => {
                ballot.survivor_freeze_height -= 1;
                ballot.survivors_frozen_at_height = ballot.survivor_freeze_height;
            }
            2 => {
                ballot.commitment_close_height -= 1;
                ballot.commitment_closed_at_height = ballot.commitment_close_height;
            }
            _ => unreachable!("three fixed resource windows"),
        }
        assert_eq!(
            candidate.validate(),
            Err(GovernanceCertificateErrorV1::InvalidLifecycle)
        );
    }
    for entries in [0, 499] {
        let mut candidate = valid.clone();
        candidate.body_bindings[0]
            .ballot
            .as_mut()
            .unwrap()
            .max_corpus_entries = entries;
        assert_eq!(
            candidate.validate(),
            Err(GovernanceCertificateErrorV1::InvalidLifecycle)
        );
    }
    assert_eq!(valid.validate(), Ok(()));
}

#[test]
fn canonical_ballot_result_still_requires_approval_and_emergency_threshold() {
    let mut candidate = governance_certificate_fixture();
    let ballot = candidate.body_bindings[0].ballot.as_mut().unwrap();
    ballot.tally.aye = 100;
    ballot.tally.nay = 200;
    ballot.outcome = ParliamentAggregateOutcomeV1::Rejected;
    refresh_policy_result(&mut candidate);
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::NonApprovingBallot)
    );
    candidate.risk_tier = RiskTierV1::Emergency;
    let ballot = candidate.body_bindings[0].ballot.as_mut().unwrap();
    ballot.tally.aye = 300;
    ballot.tally.nay = 34;
    ballot.tally.abstain = 0;
    ballot.outcome = ParliamentAggregateOutcomeV1::Approved;
    refresh_policy_result(&mut candidate);
    assert_eq!(
        candidate.validate(),
        Err(GovernanceCertificateErrorV1::EmergencyPolicyJuryThreshold)
    );
    let ballot = candidate.body_bindings[0].ballot.as_mut().unwrap();
    ballot.tally.aye = 334;
    ballot.tally.nay = 0;
    refresh_policy_result(&mut candidate);
    assert_eq!(candidate.validate(), Ok(()));
}

fn resize_policy_schedule(certificate: &mut GovernanceCertificateV1, entries: u32) {
    let binding = &mut certificate.body_bindings[0];
    let ballot = binding.ballot.as_mut().expect("fixture policy ballot");
    ballot.max_corpus_entries = entries;
    ballot.registration_close_height = ballot
        .registered_at_height
        .checked_add(u64::from(entries))
        .and_then(|height| height.checked_add(1))
        .expect("a u32 corpus fits the u64 registration schedule");
    ballot.registration_closed_at_height = ballot.registration_close_height;
    ballot.survivor_freeze_height = ballot
        .registration_close_height
        .checked_add(u64::from(entries))
        .expect("a u32 corpus fits the u64 survivor schedule");
    ballot.survivors_frozen_at_height = ballot.survivor_freeze_height;
    ballot.commitment_close_height = ballot
        .survivor_freeze_height
        .checked_add(parliament_timed_ovn_required_chunk_blocks_v1(entries))
        .expect("a u32 corpus fits the u64 commitment schedule");
    ballot.commitment_closed_at_height = ballot.commitment_close_height;
    ballot.release_height = ballot
        .commitment_close_height
        .checked_add(600)
        .expect("release height");
    ballot.opening_height = ballot.release_height;
    ballot.opening_deadline_height = ballot
        .release_height
        .checked_add(600)
        .expect("opening deadline");
    ballot.tle_session_id = TleSessionId::derive_v1(
        ballot.ballot_attempt_id,
        ballot.tle_key_session_id,
        ballot.release_beacon_session_id,
        ballot.release_height,
    );
    binding.result_height = ballot
        .release_height
        .checked_add(43)
        .expect("result height");
    certificate.certified_at_height = binding.result_height;
    certificate.enact_at_height = binding
        .result_height
        .checked_add(1)
        .expect("enactment height");
    refresh_policy_result(certificate);
}

#[test]
fn ballot_corpus_cap_is_independent_of_adequate_windows() {
    let valid = governance_certificate_fixture();
    for entries in [
        500,
        MAX_PARLIAMENT_BALLOT_CORPUS_ENTRIES_V1,
        MAX_PARLIAMENT_BALLOT_CORPUS_ENTRIES_V1 + 1,
        u32::MAX,
    ] {
        let mut candidate = valid.clone();
        resize_policy_schedule(&mut candidate, entries);
        let expected = if entries <= MAX_PARLIAMENT_BALLOT_CORPUS_ENTRIES_V1 {
            Ok(())
        } else {
            Err(GovernanceCertificateErrorV1::InvalidLifecycle)
        };
        assert_eq!(
            candidate.validate(),
            expected,
            "configured corpus {entries}"
        );
    }
}
