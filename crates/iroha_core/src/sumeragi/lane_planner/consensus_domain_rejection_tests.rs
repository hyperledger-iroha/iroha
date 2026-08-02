#[test]
fn lane_consensus_domains_reject_blank_base_mode_tag() {
    assert_eq!(
        plan_lane_consensus_domains(
            &routing_for_lane_dataspaces(&[(1, 11)]),
            &accepted_schedule(&[0]),
            &[committee(1, 11, vec![test_peer(1)], None)],
            "  ",
        ),
        Err(LaneConsensusDomainError::BlankBaseModeTag)
    );
}

#[test]
fn lane_consensus_domains_reject_action_index_out_of_bounds() {
    assert_eq!(
        plan_lane_consensus_domains(
            &routing_for_lane_dataspaces(&[(1, 11)]),
            &accepted_schedule(&[1]),
            &[committee(1, 11, vec![test_peer(1)], None)],
            "permissioned",
        ),
        Err(LaneConsensusDomainError::ActionIndexOutOfBounds {
            index: 1,
            routing_decisions: 1
        })
    );
}

#[test]
fn lane_consensus_domains_reject_inconsistent_accepted_lane_dataspace() {
    assert_eq!(
        plan_lane_consensus_domains(
            &routing_for_lane_dataspaces(&[(1, 11), (1, 12)]),
            &accepted_schedule(&[0, 1]),
            &[committee(1, 11, vec![test_peer(1)], None)],
            "permissioned",
        ),
        Err(LaneConsensusDomainError::AcceptedLaneDataspaceMismatch {
            lane_id: LaneId::new(1),
            expected: DataSpaceId::new(11),
            actual: DataSpaceId::new(12),
        })
    );
}

#[test]
fn lane_consensus_domains_reject_duplicate_committee() {
    assert_eq!(
        plan_lane_consensus_domains(
            &routing_for_lane_dataspaces(&[(1, 11)]),
            &accepted_schedule(&[0]),
            &[
                committee(1, 11, vec![test_peer(1)], None),
                committee(1, 11, vec![test_peer(2)], None),
            ],
            "permissioned",
        ),
        Err(LaneConsensusDomainError::DuplicateLaneCommittee {
            lane_id: LaneId::new(1)
        })
    );
}

#[test]
fn lane_consensus_domains_reject_missing_committee_for_accepted_lane() {
    assert_eq!(
        plan_lane_consensus_domains(
            &routing_for_lane_dataspaces(&[(1, 11)]),
            &accepted_schedule(&[0]),
            &[],
            "permissioned",
        ),
        Err(LaneConsensusDomainError::MissingLaneCommittee {
            lane_id: LaneId::new(1)
        })
    );
}

#[test]
fn lane_consensus_domains_reject_committee_dataspace_mismatch() {
    assert_eq!(
        plan_lane_consensus_domains(
            &routing_for_lane_dataspaces(&[(1, 11)]),
            &accepted_schedule(&[0]),
            &[committee(1, 12, vec![test_peer(1)], None)],
            "permissioned",
        ),
        Err(LaneConsensusDomainError::CommitteeDataspaceMismatch {
            lane_id: LaneId::new(1),
            expected: DataSpaceId::new(11),
            actual: DataSpaceId::new(12),
        })
    );
}

#[test]
fn lane_consensus_domains_reject_empty_duplicate_and_invalid_quorum_committees() {
    let routing = routing_for_lane_dataspaces(&[(1, 11)]);
    assert_eq!(
        plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(1, 11, Vec::new(), None)],
            "permissioned",
        ),
        Err(LaneConsensusDomainError::EmptyValidatorSet {
            lane_id: LaneId::new(1)
        })
    );

    let duplicate = test_peer(1);
    assert_eq!(
        plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(1, 11, vec![duplicate.clone(), duplicate], None)],
            "permissioned",
        ),
        Err(LaneConsensusDomainError::DuplicateValidator {
            lane_id: LaneId::new(1)
        })
    );

    assert_eq!(
        plan_lane_consensus_domains(
            &routing,
            &accepted_schedule(&[0]),
            &[committee(1, 11, vec![test_peer(1), test_peer(2)], Some(3))],
            "permissioned",
        ),
        Err(LaneConsensusDomainError::InvalidQuorum {
            lane_id: LaneId::new(1),
            validator_count: 2,
            min_quorum: 3,
        })
    );
}
