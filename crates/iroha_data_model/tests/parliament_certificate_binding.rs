//! Public-boundary tests for self-contained SORA Parliament certificates.

use iroha_data_model::governance::types::{
    AssignmentId, BallotAttemptId, BeaconPulseId, BeaconSessionId, BodyElectionAttemptId,
    BodyInstanceId, GovernanceAttemptId, GovernanceCertificateErrorV1, GovernanceCertificateV1,
    GovernanceExpectedHeadAbsentV1, GovernanceExpectedHeadV1, ParliamentAggregateOutcomeV1,
    ParliamentAggregateTallyV1, ParliamentBallotCertificateBindingV1, ParliamentBody,
    ParliamentBodyCertificateBindingV1, ParliamentPublicFindingCertificateBindingV1,
    ProposalContentId, RiskTierV1, SortitionRequestV1, TleKeySessionId, TleSessionId,
    parliament_ballot_result_root_v1, parliament_public_finding_endorsement_root_v1,
};

fn policy_jury_binding(
    governance_attempt_id: GovernanceAttemptId,
) -> ParliamentBodyCertificateBindingV1 {
    let election_attempt_id =
        BodyElectionAttemptId::derive_v1(governance_attempt_id, ParliamentBody::PolicyJury, 0);
    let beacon_session_id = BeaconSessionId::new([0x31; 32]);
    let sortition_request = SortitionRequestV1::try_new_canonical(
        governance_attempt_id,
        election_attempt_id,
        ParliamentBody::PolicyJury,
        [0x32; 32],
        3,
        3,
        10,
        11,
        beacon_session_id,
        None,
    )
    .expect("canonical Policy Jury request");
    let roster_root = [0x33; 32];
    let body_instance_id = BodyInstanceId::derive_v1(election_attempt_id, roster_root);
    let ballot_attempt_id = BallotAttemptId::derive_v1(body_instance_id, 0);
    let tle_key_session_id = TleKeySessionId::new([0x34; 32]);
    let release_beacon_session_id = BeaconSessionId::new([0x35; 32]);
    let release_height = 30;
    let opening_deadline_height = 35;
    let tally = ParliamentAggregateTallyV1 {
        original_seats: 3,
        accepted_ballots: 2,
        aye: 2,
        nay: 0,
        abstain: 0,
    };
    let outcome = ParliamentAggregateOutcomeV1::Approved;
    let opening_root = [0x36; 32];
    let result_height = 31;
    let result_root = parliament_ballot_result_root_v1(
        governance_attempt_id,
        body_instance_id,
        ballot_attempt_id,
        opening_root,
        tally,
        outcome,
        result_height,
    );

    ParliamentBodyCertificateBindingV1 {
        body_instance_id,
        election_attempt_id,
        election_attempt_sequence: 0,
        sortition_request_id: sortition_request.id,
        sortition_request,
        body: ParliamentBody::PolicyJury,
        original_seats: 3,
        beacon_session_id,
        beacon_pulse_id: BeaconPulseId::new([0x37; 32]),
        roster_root,
        assignment_root: [0x38; 32],
        result_root,
        result_height,
        public_finding: None,
        ballot: Some(ParliamentBallotCertificateBindingV1 {
            ballot_attempt_id,
            ballot_attempt_sequence: 0,
            tle_session_id: TleSessionId::derive_v1(
                ballot_attempt_id,
                tle_key_session_id,
                release_beacon_session_id,
                release_height,
            ),
            tle_key_session_id,
            registration_root: [0x39; 32],
            dropout_root: [0x3A; 32],
            survivor_root: [0x3B; 32],
            corpus_root: [0x3C; 32],
            no_recovery_root: [0x3D; 32],
            timed_commitment_root: [0x3E; 32],
            release_beacon_session_id,
            registered_at_height: 20,
            registration_close_height: 22,
            survivor_freeze_height: 24,
            commitment_close_height: 26,
            registration_closed_at_height: 22,
            survivors_frozen_at_height: 24,
            commitment_closed_at_height: 26,
            max_ballot_retries: 3,
            max_corpus_entries: 1_000,
            release_height,
            opening_deadline_height,
            release_pulse_id: BeaconPulseId::new([0x3F; 32]),
            opening_height: 30,
            opening_root,
            tally,
            outcome,
        }),
    }
}

fn public_finding_binding(
    governance_attempt_id: GovernanceAttemptId,
) -> ParliamentBodyCertificateBindingV1 {
    let election_attempt_id =
        BodyElectionAttemptId::derive_v1(governance_attempt_id, ParliamentBody::RulesCommittee, 0);
    let beacon_session_id = BeaconSessionId::new([0x41; 32]);
    let sortition_request = SortitionRequestV1::try_new_canonical(
        governance_attempt_id,
        election_attempt_id,
        ParliamentBody::RulesCommittee,
        [0x42; 32],
        3,
        3,
        10,
        11,
        beacon_session_id,
        None,
    )
    .expect("canonical Rules Committee request");
    let roster_root = [0x43; 32];
    let body_instance_id = BodyInstanceId::derive_v1(election_attempt_id, roster_root);
    let result_root = [0x44; 32];
    let endorsing_assignments = vec![AssignmentId::new([0x45; 32]), AssignmentId::new([0x46; 32])];
    let endorsement_root = parliament_public_finding_endorsement_root_v1(
        governance_attempt_id,
        body_instance_id,
        result_root,
        &endorsing_assignments,
    );

    ParliamentBodyCertificateBindingV1 {
        body_instance_id,
        election_attempt_id,
        election_attempt_sequence: 0,
        sortition_request_id: sortition_request.id,
        sortition_request,
        body: ParliamentBody::RulesCommittee,
        original_seats: 3,
        beacon_session_id,
        beacon_pulse_id: BeaconPulseId::new([0x47; 32]),
        roster_root,
        assignment_root: [0x48; 32],
        result_root,
        result_height: 19,
        public_finding: Some(ParliamentPublicFindingCertificateBindingV1 {
            endorsement_root,
            endorsing_assignments,
            endorsements: 2,
            quorum: 2,
        }),
        ballot: None,
    }
}

#[test]
fn public_finding_certificate_carries_recomputable_exact_quorum() {
    let proposal_content_id = ProposalContentId::new([0x51; 32]);
    let governance_attempt_id = GovernanceAttemptId::derive_v1(proposal_content_id, 0);
    let certificate = GovernanceCertificateV1 {
        proposal_content_id,
        governance_attempt_id,
        governance_attempt_sequence: 0,
        risk_tier: RiskTierV1::Standard,
        body_bindings: vec![
            public_finding_binding(governance_attempt_id),
            policy_jury_binding(governance_attempt_id),
        ],
        policy_version: 1,
        effect_preimage_hash: [0x52; 32],
        expected_head: GovernanceExpectedHeadV1::Absent(GovernanceExpectedHeadAbsentV1 {
            subject_id: [0x53; 32],
        }),
        certified_at_height: 40,
        enact_at_height: 41,
    };

    certificate
        .validate()
        .expect("self-contained exact public quorum must validate");
    let encoded = norito::to_bytes(&certificate).expect("encode Parliament certificate");
    assert_eq!(
        norito::decode_from_bytes::<GovernanceCertificateV1>(&encoded)
            .expect("decode Parliament certificate"),
        certificate
    );

    let mut reordered = certificate.clone();
    reordered.body_bindings[0]
        .public_finding
        .as_mut()
        .expect("public finding")
        .endorsing_assignments
        .swap(0, 1);
    assert_eq!(
        reordered.validate(),
        Err(GovernanceCertificateErrorV1::InvalidPublicFinding)
    );

    let mut missing = certificate;
    missing.body_bindings[0]
        .public_finding
        .as_mut()
        .expect("public finding")
        .endorsing_assignments
        .pop();
    assert_eq!(
        missing.validate(),
        Err(GovernanceCertificateErrorV1::InvalidPublicFinding)
    );
}
