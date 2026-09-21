//! Actual delta MSM/session adoption and strict stage/failure boundary tests.
use super::*;
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::{TestPreparedDifferenceDigitV1, prepared_commitment_test_guard_v1};

// Later-stage fixture setup preserves the exact original session, every rho,
// prior synthetic point and final validated roots. It does not repeat D/S token
// validation (covered by its own suite) 23,392 times for each later-stage test.
fn complete_prior_low_fixture_v1()
-> RnsNativeExistingRadixCandidateOwnerV1<core::convert::Infallible> {
    let session = super::super::super::tests::source_complete_session_v1(
        [0x31; 32],
        [0x41; 32],
        TestEntropyFaultV1::None,
    );
    let points = super::super::super::tests::patterned_points_v1();
    let mut assembly = session.into_existing_radix_candidate_assembly_v1().unwrap();
    let live = assembly.live.as_mut().unwrap();
    let session = live.session.live.as_mut().unwrap();
    for ordinal in 0..EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 as u32 {
        let coordinate = existing_radix_candidate_coordinate_v1(ordinal).unwrap();
        let (_chunk, scalar) =
            sample_blinding_v1(&mut session.entropy, coordinate.inventory_ordinal).unwrap();
        let point_wire = session
            .inventory
            .adopt_existing_radix_candidate_v1(coordinate, &points[ordinal as usize % points.len()])
            .unwrap();
        absorb_existing_radix_candidate_point_v1(
            &mut live.candidate_root_hash,
            coordinate,
            &point_wire,
        )
        .unwrap();
        let scalar_wire = ExistingRadixSecretScalarWireV1::new_v1(scalar.as_ref());
        absorb_existing_radix_blinding_v1(
            &mut live.blinding_root_hash,
            coordinate,
            scalar_wire.as_ref_v1(),
        );
        live.blindings.push(scalar.get());
    }
    live.next_wire_ordinal = EXISTING_RADIX_CANDIDATE_POINT_COUNT_V1 as u32;
    let owner = assembly.finish_v1().unwrap();
    // Existing full-token-loop KATs: shortcuts apply only to synthetic earlier
    // setup; the same final owner validator checks every stored prior opening.
    assert_eq!(
        owner.candidate_root,
        hex_literal::hex!("b593dd370462a64cf69be83bd86ebd28634a55591e63d2d400bc3511c9280453")
    );
    assert_eq!(
        owner.blinding_root,
        hex_literal::hex!("4005e2608b61b63d215ff7b0b21b2fece110c1e2813b3943c81c72a97db3c835")
    );
    assert_eq!(
        owner.owner_binding_digest,
        hex_literal::hex!("dd55236fb876d36d02b4a924697110281a367613781e415375b60bdb6b964cea")
    );
    owner
}

pub(super) fn complete_top_fixture_v1()
-> RnsNativeComparatorTopCommitmentsV1<core::convert::Infallible> {
    let previous = complete_prior_low_fixture_v1();
    let mut owner = RnsNativeComparatorTopCommitmentsV1::begin_v1(previous).unwrap();
    let live = owner.live.as_mut().unwrap();
    let session = live.session.live.as_mut().unwrap();
    let point_wire = Point::canonical_generator()
        .unwrap()
        .to_non_identity_wire_bytes()
        .unwrap();
    // Full prior shape and actual original-session rho samples. These synthetic
    // prior points are not source witnesses or production qualification.
    for ordinal in 0..TOP_PLANE_COUNT_V1 {
        let coordinate = top_coordinate_v1(ordinal).unwrap();
        let (_chunk, scalar) =
            sample_blinding_v1(&mut session.entropy, coordinate.global_ordinal).unwrap();
        session.inventory.slots[coordinate.global_ordinal as usize] =
            Some(GlobalLookupCommitmentTicketV1 {
                coordinate,
                point_wire,
            });
        live.blindings.push(scalar.get());
    }
    live.next_plane = TOP_PLANE_COUNT_V1;
    let next = difference_coordinate_v1(0).unwrap();
    session.next_global_ordinal = next.global_ordinal;
    session.next_purpose = next.purpose;
    session.next_purpose_ordinal = next.purpose_ordinal;
    validate_top_progress_v1(live).unwrap();
    owner
}

#[test]
fn exact_5848_inventory_slots_follow_top_bits_and_end_before_beta() {
    for ordinal in 0..5_848 {
        let coordinate = difference_coordinate_v1(ordinal).unwrap();
        assert_eq!(coordinate.global_ordinal, 12_728 + u32::from(ordinal));
        assert_eq!(coordinate.purpose_ordinal, u32::from(ordinal));
        assert_eq!(
            coordinate.purpose,
            GlobalLookupCommitmentPurposeV1::ComparatorDifferenceDigit
        );
    }
    for ordinal in [5_848, 5_849, u16::MAX] {
        assert!(difference_coordinate_v1(ordinal).is_err());
    }
    let next = commitment_coordinate_v1(18_576).unwrap();
    assert_eq!(
        next.purpose,
        GlobalLookupCommitmentPurposeV1::ComparatorBorrow
    );
    assert_eq!(next.purpose_ordinal, 0);
    assert_eq!(DIFFERENCE_RETAINED_BLINDING_BYTES_V1, 187_136);
}

#[test]
fn delta_ingress_requires_exact_completed_top_and_empty_first_slot() {
    let _guard = prepared_commitment_test_guard_v1();
    let previous = complete_prior_low_fixture_v1();
    let incomplete = RnsNativeComparatorTopCommitmentsV1::begin_v1(previous).unwrap();
    assert!(RnsNativeDifferenceCommitmentsV1::validate_start_v1(&incomplete).is_err());
    assert!(RnsNativeDifferenceCommitmentsV1::begin_v1(incomplete).is_err());
    let mut top = complete_top_fixture_v1();
    RnsNativeDifferenceCommitmentsV1::validate_start_v1(&top).unwrap();
    let live = top.live.as_mut().unwrap();
    live.session.live.as_mut().unwrap().next_purpose_ordinal = 1;
    assert!(RnsNativeDifferenceCommitmentsV1::validate_start_v1(&top).is_err());
    let live = top.live.as_mut().unwrap();
    let session = live.session.live.as_mut().unwrap();
    session.next_purpose_ordinal = 0;
    session.inventory.slots[12_728] = Some(GlobalLookupCommitmentTicketV1 {
        coordinate: difference_coordinate_v1(0).unwrap(),
        point_wire: Point::canonical_generator()
            .unwrap()
            .to_non_identity_wire_bytes()
            .unwrap(),
    });
    assert!(RnsNativeDifferenceCommitmentsV1::begin_v1(top).is_err());
}

#[test]
fn first_actual_delta_msm_retains_original_prior_material_rho_and_source_binding() {
    let _guard = prepared_commitment_test_guard_v1();
    let fixture = TestPreparedDifferenceDigitV1::new_v1(0);
    let top = complete_top_fixture_v1();
    let previous = top.live.as_ref().unwrap();
    let low_pointer = previous.retained_low.blindings.as_slice().as_ptr();
    let top_pointer = previous.blindings.as_slice().as_ptr();
    let root = previous.retained_low.candidate_root;
    let point_root = previous
        .session
        .live
        .as_ref()
        .unwrap()
        .inventory
        .adopted_source_commitments_root_v1([0x32; 32])
        .unwrap();
    let mut owner = RnsNativeDifferenceCommitmentsV1::begin_v1(top).unwrap();
    assert!(owner.require_complete_v1().is_err());
    owner = owner.commit_prepared_v1(&fixture.statement_v1(0)).unwrap();
    owner
        .validate_completed_source_prefix_v1([0x53; 32], [0x32; 32], point_root, [0x52; 32])
        .unwrap();
    assert!(
        owner
            .validate_completed_source_prefix_v1([0x53; 32], [0x33; 32], point_root, [0x52; 32])
            .is_err()
    );
    let live = owner.live.as_ref().unwrap();
    assert_eq!(live.next_plane, 1);
    assert_eq!(live.blindings.len(), 1);
    assert_eq!(
        live.top.retained_low.blindings.as_slice().as_ptr(),
        low_pointer
    );
    assert_eq!(live.top.blindings.as_slice().as_ptr(), top_pointer);
    assert_eq!(live.top.retained_low.candidate_root, root);
    assert!(live.top.retained_low.append_permit.is_some());
    let rho = live.blindings.as_slice()[0];
    assert!(!rho.is_zero());
    let ticket = live.top.session.live.as_ref().unwrap().inventory.slots[12_728]
        .as_ref()
        .unwrap();
    assert_eq!(ticket.coordinate, difference_coordinate_v1(0).unwrap());
    assert_eq!(
        ticket.point_wire,
        TestPreparedDifferenceDigitV1::expected_commitment_v1(rho)
            .to_non_identity_wire_bytes()
            .unwrap()
    );
    assert_ne!(
        ticket.point_wire,
        TestPreparedDifferenceDigitV1::expected_commitment_v1(crate::vega::VegaT256ScalarV1::one())
            .to_non_identity_wire_bytes()
            .unwrap()
    );
    assert!(owner.commit_prepared_v1(&fixture.statement_v1(0)).is_err());
}

#[test]
fn last_actual_delta_msm_completes_exactly_at_beta_without_skipping() {
    let _guard = prepared_commitment_test_guard_v1();
    let fixture = TestPreparedDifferenceDigitV1::new_v1(5_847);
    let mut owner = RnsNativeDifferenceCommitmentsV1::begin_v1(complete_top_fixture_v1()).unwrap();
    let live = owner.live.as_mut().unwrap();
    let session = live.top.session.live.as_mut().unwrap();
    let point_wire = Point::canonical_generator()
        .unwrap()
        .to_non_identity_wire_bytes()
        .unwrap();
    // Seed the earlier fixture slots only; the last adoption below executes the
    // real source-derived digit projection and secret MSM.
    for ordinal in 0..5_847 {
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
    live.next_plane = 5_847;
    let next = difference_coordinate_v1(5_847).unwrap();
    session.next_global_ordinal = next.global_ordinal;
    session.next_purpose = next.purpose;
    session.next_purpose_ordinal = next.purpose_ordinal;
    validate_difference_progress_v1(live).unwrap();
    owner = owner
        .commit_prepared_v1(&fixture.statement_v1(5_847))
        .unwrap();
    owner.require_complete_v1().unwrap();
    let live = owner.live.as_ref().unwrap();
    let session = live.top.session.live.as_ref().unwrap();
    let rho = live.blindings.as_slice()[5_847];
    assert_eq!(
        session.inventory.slots[18_575].as_ref().unwrap().point_wire,
        TestPreparedDifferenceDigitV1::expected_commitment_v1(rho)
            .to_non_identity_wire_bytes()
            .unwrap()
    );
    assert_eq!(session.next_global_ordinal, 18_576);
    assert_eq!(
        session.next_purpose,
        GlobalLookupCommitmentPurposeV1::ComparatorBorrow
    );
    assert!(session.inventory.slots[18_576].is_none());
    assert!(
        owner
            .commit_prepared_v1(&fixture.statement_v1(5_848))
            .is_err()
    );
}

#[test]
fn wrong_ordinal_entropy_error_zero_and_unwind_close_delta_owner() {
    let _guard = prepared_commitment_test_guard_v1();
    let fixture = TestPreparedDifferenceDigitV1::new_v1(0);
    let owner = RnsNativeDifferenceCommitmentsV1::begin_v1(complete_top_fixture_v1()).unwrap();
    assert!(owner.commit_prepared_v1(&fixture.statement_v1(1)).is_err());
    for fault in [
        TestEntropyFaultV1::ErrorAt(12_728),
        TestEntropyFaultV1::ZeroAt(12_728),
        TestEntropyFaultV1::PanicAt(12_728),
    ] {
        let mut owner =
            RnsNativeDifferenceCommitmentsV1::begin_v1(complete_top_fixture_v1()).unwrap();
        let GlobalLookupProofSessionEntropySourceV1::TestOnly(entropy) = &mut owner
            .live
            .as_mut()
            .unwrap()
            .top
            .session
            .live
            .as_mut()
            .unwrap()
            .entropy
        else {
            unreachable!()
        };
        entropy.fault = fault;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            owner.commit_prepared_v1(&fixture.statement_v1(0))
        }));
        assert!(match result {
            Ok(result) => result.is_err(),
            Err(_) => true,
        });
    }
}
