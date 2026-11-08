//! Deterministic reducer fixtures for authority budget and termination regressions.

use super::*;
use crate::governance::parliament::{
    MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1, PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1,
    ParliamentAttemptStateV1, sorafs_authority_attempt_policy_v1,
};
use iroha_data_model::{
    governance::types::{
        BeaconSessionId, BodyElectionAttemptId, GovernanceAttemptV1, GovernanceStageV1,
        SortitionRequestId, SortitionRequestV1, parliament_candidate_root_v1,
    },
    isi::governance::ParliamentSortitionRequestRegistrationV1,
};

/// Exhaust the real shared reducer through its bounded capacity-failure generations.
pub(crate) fn exhausted_attempt(
    proposal: &SorafsAuthorityProposalV1,
    first_request_height: u64,
) -> ParliamentAttemptStateV1 {
    let content = proposal.proposal_content_id();
    let id = GovernanceAttemptId::derive_v1(content, 0);
    let (risk_tier, required) = sorafs_authority_attempt_policy_v1();
    let mut attempt = ParliamentAttemptStateV1::try_new(
        GovernanceAttemptV1 {
            id,
            proposal_content_id: content,
            sequence: 0,
            risk_tier,
            stage: GovernanceStageV1::Qualification,
            status: GovernanceAttemptStatusV1::Active,
        },
        PARLIAMENT_GOVERNANCE_POLICY_VERSION_V1,
        1,
        proposal.grid.epoch_length_blocks,
        proposal.effect_preimage_hash_v1(),
        admitted_expected_head(proposal),
        required.clone(),
    )
    .unwrap();
    attempt.complete_qualification(id).unwrap();
    let beacon = BeaconSessionId::for_network_v1(&proposal.network);
    for sequence in 0..=MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1 {
        let request_height =
            first_request_height + u64::from(sequence) * proposal.grid.epoch_length_blocks;
        let pulse_height = attempt
            .next_sortition_generation_pulse_height_v1(request_height, beacon)
            .unwrap();
        let registrations = required
            .iter()
            .map(|required| {
                let mut request = SortitionRequestV1 {
                    id: SortitionRequestId::new([0; 32]),
                    governance_attempt_id: id,
                    body_election_attempt_id: BodyElectionAttemptId::derive_v1(
                        id,
                        required.body,
                        sequence,
                    ),
                    body: required.body,
                    candidate_root: parliament_candidate_root_v1(id, required.body, &[]),
                    candidate_count: 0,
                    target_seats: 3,
                    request_height,
                    pulse_height,
                    beacon_session_id: beacon,
                };
                request.id = request.canonical_id();
                ParliamentSortitionRequestRegistrationV1 { sequence, request }
            })
            .collect();
        attempt
            .record_hidden_sortition_capacity_failure_batch(id, registrations, &[])
            .unwrap();
    }
    assert_eq!(
        attempt.attempt().status,
        GovernanceAttemptStatusV1::Rejected
    );
    assert_eq!(
        attempt.randomness_redraws_used_v1().unwrap(),
        MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1
    );
    attempt.validate().unwrap();
    attempt
}
