//! Exact beta/m continuation, original-opening custody and actual-MSM controls.
use super::*;
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::{TestPreparedComparatorV1, prepared_commitment_test_guard_v1};

fn fill_difference_fixture_v1(
    owner: &mut RnsNativeDifferenceCommitmentsV1<core::convert::Infallible>,
) {
    let live = owner.live.as_mut().unwrap();
    let session = live.top.session.live.as_mut().unwrap();
    let point_wire = Point::canonical_generator()
        .unwrap()
        .to_non_identity_wire_bytes()
        .unwrap();
    // Earlier points are full-shape fixtures, not authenticated source proofs.
    for ordinal in 0..DIFFERENCE_PLANE_COUNT_V1 {
        let coordinate = difference_coordinate_v1(ordinal).unwrap();
        let (_chunk, scalar) =
            sample_blinding_v1(&mut session.entropy, coordinate.global_ordinal).unwrap();
        session.inventory.slots[coordinate.global_ordinal as usize] =
            Some(GlobalLookupCommitmentTicketV1 {
                coordinate,
                point_wire,
            });
        live.blindings.push(scalar.get());
    }
    live.next_plane = DIFFERENCE_PLANE_COUNT_V1;
    let next = continuation_coordinate_v1(CONTINUATION_FIRST_PLANE_V1).unwrap();
    session.next_global_ordinal = next.global_ordinal;
    session.next_purpose = next.purpose;
    session.next_purpose_ordinal = next.purpose_ordinal;
    owner.require_complete_v1().unwrap();
}

fn complete_difference_fixture_v1() -> RnsNativeDifferenceCommitmentsV1<core::convert::Infallible> {
    let top = super::super::tests::complete_top_fixture_v1();
    let mut owner = RnsNativeDifferenceCommitmentsV1::begin_v1(top).unwrap();
    fill_difference_fixture_v1(&mut owner);
    owner
}

fn seed_continuation_before_v1(
    owner: &mut RnsNativeComparatorContinuationV1<core::convert::Infallible>,
    until: u16,
) {
    let live = owner.live.as_mut().unwrap();
    let session = live.difference.top.session.live.as_mut().unwrap();
    let point_wire = Point::canonical_generator()
        .unwrap()
        .to_non_identity_wire_bytes()
        .unwrap();
    for ordinal in live.next_plane..until {
        let coordinate = continuation_coordinate_v1(ordinal).unwrap();
        let (_chunk, scalar) =
            sample_blinding_v1(&mut session.entropy, coordinate.global_ordinal).unwrap();
        assert!(session.inventory.slots[coordinate.global_ordinal as usize].is_none());
        session.inventory.slots[coordinate.global_ordinal as usize] =
            Some(GlobalLookupCommitmentTicketV1 {
                coordinate,
                point_wire,
            });
        live.blindings.push(scalar.get());
    }
    live.next_plane = until;
    let next = continuation_coordinate_v1(until).unwrap();
    session.next_global_ordinal = next.global_ordinal;
    session.next_purpose = next.purpose;
    session.next_purpose_ordinal = next.purpose_ordinal;
    validate_continuation_progress_v1(live).unwrap();
}

#[test]
fn all_6536_continuation_positions_match_shared_beta_then_m_inventory() {
    for ordinal in 688..7_224 {
        let coordinate = continuation_coordinate_v1(ordinal).unwrap();
        assert_eq!(coordinate.global_ordinal, 18_576 + u32::from(ordinal - 688));
        assert_eq!(
            coordinate.purpose,
            if ordinal < 6_880 {
                GlobalLookupCommitmentPurposeV1::ComparatorBorrow
            } else {
                GlobalLookupCommitmentPurposeV1::ComparatorMixedTop
            }
        );
        assert_eq!(
            coordinate.purpose_ordinal,
            if ordinal < 6_880 {
                u32::from(ordinal - 688)
            } else {
                u32::from(ordinal - 6_880)
            }
        );
    }
    for ordinal in [0, 687, 7_224, 9_287, u16::MAX] {
        assert!(continuation_coordinate_v1(ordinal).is_err());
    }
    assert_eq!(CONTINUATION_RETAINED_BLINDING_BYTES_V1, 209_152);
    let next = commitment_coordinate_v1(25_112).unwrap();
    assert_eq!(
        (next.purpose, next.purpose_ordinal),
        (GlobalLookupCommitmentPurposeV1::SmallSigned, 0)
    );
}

#[test]
fn continuation_refuses_incomplete_delta_wrong_logical_start_and_occupied_slot() {
    let _guard = prepared_commitment_test_guard_v1();
    let mut delta =
        RnsNativeDifferenceCommitmentsV1::begin_v1(super::super::tests::complete_top_fixture_v1())
            .unwrap();
    assert!(RnsNativeComparatorContinuationV1::validate_start_v1(&delta, 688).is_err());
    fill_difference_fixture_v1(&mut delta);
    RnsNativeComparatorContinuationV1::validate_start_v1(&delta, 688).unwrap();
    for ordinal in [0, 687, 689, 6_880, 7_224] {
        assert!(RnsNativeComparatorContinuationV1::validate_start_v1(&delta, ordinal).is_err());
    }
    let live = delta.live.as_mut().unwrap();
    let session = live.top.session.live.as_mut().unwrap();
    session.next_purpose_ordinal = 1;
    assert!(RnsNativeComparatorContinuationV1::validate_start_v1(&delta, 688).is_err());
    let session = delta
        .live
        .as_mut()
        .unwrap()
        .top
        .session
        .live
        .as_mut()
        .unwrap();
    session.next_purpose_ordinal = 0;
    session.inventory.slots[18_576] = Some(GlobalLookupCommitmentTicketV1 {
        coordinate: continuation_coordinate_v1(688).unwrap(),
        point_wire: Point::canonical_generator()
            .unwrap()
            .to_non_identity_wire_bytes()
            .unwrap(),
    });
    assert!(RnsNativeComparatorContinuationV1::begin_v1(delta).is_err());
}

#[test]
fn actual_first_beta_retains_original_prior_openings_and_sampled_rho() {
    let _guard = prepared_commitment_test_guard_v1();
    let fixture = TestPreparedComparatorV1::for_ordinal_v1(688);
    let delta = complete_difference_fixture_v1();
    let previous = delta.live.as_ref().unwrap();
    let low_pointer = previous.top.retained_low.blindings.as_slice().as_ptr();
    let top_pointer = previous.top.blindings.as_slice().as_ptr();
    let delta_pointer = previous.blindings.as_slice().as_ptr();
    let candidate_root = previous.top.retained_low.candidate_root;
    let point_root = previous
        .top
        .session
        .live
        .as_ref()
        .unwrap()
        .inventory
        .adopted_source_commitments_root_v1([0x32; 32])
        .unwrap();
    let owner = RnsNativeComparatorContinuationV1::begin_v1(delta).unwrap();
    owner.require_position_v1(688).unwrap();
    assert!(owner.require_position_v1(689).is_err());
    let owner = owner
        .commit_prepared_v1(&fixture.statement_v1(688))
        .unwrap();
    owner.require_position_v1(689).unwrap();
    owner
        .validate_completed_source_prefix_v1([0x53; 32], [0x32; 32], point_root, [0x52; 32])
        .unwrap();
    assert!(
        owner
            .validate_completed_source_prefix_v1([0x53; 32], [0x33; 32], point_root, [0x52; 32])
            .is_err()
    );
    let live = owner.live.as_ref().unwrap();
    assert_eq!(
        live.difference
            .top
            .retained_low
            .blindings
            .as_slice()
            .as_ptr(),
        low_pointer
    );
    assert_eq!(
        live.difference.top.blindings.as_slice().as_ptr(),
        top_pointer
    );
    assert_eq!(live.difference.blindings.as_slice().as_ptr(), delta_pointer);
    assert_eq!(
        live.difference.top.retained_low.candidate_root,
        candidate_root
    );
    assert!(live.difference.top.retained_low.append_permit.is_some());
    let rho = live.blindings.as_slice()[0];
    let ticket = live
        .difference
        .top
        .session
        .live
        .as_ref()
        .unwrap()
        .inventory
        .slots[18_576]
        .as_ref()
        .unwrap();
    assert_eq!(ticket.coordinate, continuation_coordinate_v1(688).unwrap());
    assert_eq!(
        ticket.point_wire,
        TestPreparedComparatorV1::expected_v1(rho)
            .to_non_identity_wire_bytes()
            .unwrap()
    );
    assert_ne!(
        ticket.point_wire,
        TestPreparedComparatorV1::expected_v1(crate::vega::VegaT256ScalarV1::one())
            .to_non_identity_wire_bytes()
            .unwrap()
    );
    assert!(
        owner
            .commit_prepared_v1(&fixture.statement_v1(688))
            .is_err()
    );
}

#[test]
fn actual_beta_m_boundary_and_final_m_stop_before_signed_values() {
    let _guard = prepared_commitment_test_guard_v1();
    let mut owner =
        RnsNativeComparatorContinuationV1::begin_v1(complete_difference_fixture_v1()).unwrap();
    for ordinal in [6_879, 6_880, 7_223] {
        seed_continuation_before_v1(&mut owner, ordinal);
        owner.require_position_v1(ordinal).unwrap();
        let fixture = TestPreparedComparatorV1::for_ordinal_v1(ordinal);
        owner = owner
            .commit_prepared_v1(&fixture.statement_v1(ordinal))
            .unwrap();
        let live = owner.live.as_ref().unwrap();
        let rho = live.blindings.as_slice()[usize::from(ordinal - 688)];
        let coordinate = continuation_coordinate_v1(ordinal).unwrap();
        let ticket = live
            .difference
            .top
            .session
            .live
            .as_ref()
            .unwrap()
            .inventory
            .slots[coordinate.global_ordinal as usize]
            .as_ref()
            .unwrap();
        assert_eq!(ticket.coordinate, coordinate);
        assert_eq!(
            ticket.point_wire,
            TestPreparedComparatorV1::expected_v1(rho)
                .to_non_identity_wire_bytes()
                .unwrap()
        );
    }
    let live = owner.live.as_ref().unwrap();
    validate_continuation_progress_v1(live).unwrap();
    assert_eq!(live.next_plane, 7_224);
    assert_eq!(live.blindings.len(), 6_536);
    let session = live.difference.top.session.live.as_ref().unwrap();
    assert_eq!(session.next_global_ordinal, 25_112);
    assert_eq!(
        session.next_purpose,
        GlobalLookupCommitmentPurposeV1::SmallSigned
    );
    assert!(session.inventory.slots[25_112].is_none());
    assert!(owner.require_position_v1(7_224).is_err());
    assert!(
        owner
            .commit_prepared_v1(
                &TestPreparedComparatorV1::for_ordinal_v1(7_223).statement_v1(7_224)
            )
            .is_err()
    );
}

#[test]
fn continuation_rejects_missing_owner_and_failed_entropy_without_replacement() {
    let _guard = prepared_commitment_test_guard_v1();
    let empty = RnsNativeComparatorContinuationV1::<core::convert::Infallible> { live: None };
    assert!(empty.require_position_v1(688).is_err());
    assert!(
        empty
            .validate_completed_source_prefix_v1([0; 32], [0; 32], [0; 32], [0; 32])
            .is_err()
    );
    let fixture = TestPreparedComparatorV1::for_ordinal_v1(688);
    assert!(
        empty
            .commit_prepared_v1(&fixture.statement_v1(688))
            .is_err()
    );
    let mut owner =
        RnsNativeComparatorContinuationV1::begin_v1(complete_difference_fixture_v1()).unwrap();
    let GlobalLookupProofSessionEntropySourceV1::TestOnly(entropy) = &mut owner
        .live
        .as_mut()
        .unwrap()
        .difference
        .top
        .session
        .live
        .as_mut()
        .unwrap()
        .entropy
    else {
        unreachable!()
    };
    entropy.fault = TestEntropyFaultV1::ErrorAt(18_576);
    assert!(
        owner
            .commit_prepared_v1(&fixture.statement_v1(688))
            .is_err()
    );
}
