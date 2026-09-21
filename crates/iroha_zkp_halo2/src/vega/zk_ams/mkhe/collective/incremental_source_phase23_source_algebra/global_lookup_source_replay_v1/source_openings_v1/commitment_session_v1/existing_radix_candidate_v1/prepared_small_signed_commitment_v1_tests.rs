//! Exact signed inventory, original-opening custody and actual-MSM controls.
use super::*;
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::{TestPreparedSmallSignedV1, prepared_commitment_test_guard_v1};

pub(super) fn continuation_fixture_before_v1(
    until: u16,
) -> RnsNativeComparatorContinuationV1<core::convert::Infallible> {
    assert!((688..=7_224).contains(&until));
    let top = super::super::super::tests::complete_top_fixture_v1();
    let mut delta = RnsNativeDifferenceCommitmentsV1::begin_v1(top).unwrap();
    let live = delta.live.as_mut().unwrap();
    let session = live.top.session.live.as_mut().unwrap();
    let point_wire = Point::canonical_generator()
        .unwrap()
        .to_non_identity_wire_bytes()
        .unwrap();
    // Full earlier inventory shape and original entropy, with synthetic points.
    // These fixtures are not authenticated source production or complete proofs.
    for ordinal in 0..DIFFERENCE_PLANE_COUNT_V1 {
        let coordinate = difference_coordinate_v1(ordinal).unwrap();
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
    live.next_plane = DIFFERENCE_PLANE_COUNT_V1;
    let next = commitment_coordinate_v1(DIFFERENCE_AFTER_ORDINAL_V1).unwrap();
    session.next_global_ordinal = next.global_ordinal;
    session.next_purpose = next.purpose;
    session.next_purpose_ordinal = next.purpose_ordinal;
    delta.require_complete_v1().unwrap();
    let mut owner = RnsNativeComparatorContinuationV1::begin_v1(delta).unwrap();
    let live = owner.live.as_mut().unwrap();
    let session = live.difference.top.session.live.as_mut().unwrap();
    for ordinal in 688..until {
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
    let next = commitment_coordinate_v1(18_576 + u32::from(until - 688)).unwrap();
    session.next_global_ordinal = next.global_ordinal;
    session.next_purpose = next.purpose;
    session.next_purpose_ordinal = next.purpose_ordinal;
    validate_continuation_progress_v1(live).unwrap();
    owner
}

pub(super) fn seed_signed_before_v1(
    owner: &mut RnsNativeSmallSignedCommitmentsV1<core::convert::Infallible>,
    until: u16,
) {
    let live = owner.live.as_mut().unwrap();
    let session = live
        .continuation
        .difference
        .top
        .session
        .live
        .as_mut()
        .unwrap();
    let point_wire = Point::canonical_generator()
        .unwrap()
        .to_non_identity_wire_bytes()
        .unwrap();
    // Skip only intermediate synthetic commitments. Actual tested endpoints
    // retain their production-generated points and original-session rho values.
    for ordinal in live.next_plane..until {
        let coordinate = signed_commitment_coordinate_v1(ordinal).unwrap();
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
    let next = commitment_coordinate_v1(
        SIGNED_FIRST_INVENTORY_V1 + u32::from(until - SIGNED_FIRST_PLANE_V1),
    )
    .unwrap();
    session.next_global_ordinal = next.global_ordinal;
    session.next_purpose = next.purpose;
    session.next_purpose_ordinal = next.purpose_ordinal;
    validate_small_signed_progress_v1(live).unwrap();
}

#[test]
fn signed_slots_follow_complete_beta_m_and_stop_before_qmask() {
    for ordinal in 7_224..9_288 {
        let coordinate = signed_commitment_coordinate_v1(ordinal).unwrap();
        assert_eq!(
            coordinate.global_ordinal,
            25_112 + u32::from(ordinal - 7_224)
        );
        assert_eq!(
            coordinate.purpose_ordinal,
            u32::from((ordinal - 7_224) % 1_032)
        );
        assert_eq!(
            coordinate.purpose,
            if ordinal < 8_256 {
                GlobalLookupCommitmentPurposeV1::SmallSigned
            } else {
                GlobalLookupCommitmentPurposeV1::SmallNegativeMagnitude
            }
        );
    }
    for ordinal in [0, 7_223, 9_288, u16::MAX] {
        assert!(signed_commitment_coordinate_v1(ordinal).is_err());
    }
    let next = commitment_coordinate_v1(27_176).unwrap();
    assert_eq!(
        (next.purpose, next.purpose_ordinal),
        (GlobalLookupCommitmentPurposeV1::QMaskDigit, 0)
    );
    assert_eq!(SIGNED_RETAINED_BLINDING_BYTES_V1, 66_048);
    let generator = Point::canonical_generator().unwrap();
    let wire = generator.to_non_identity_wire_bytes().unwrap();
    require_nonidentity_derived_positive_v1(&wire, &generator).unwrap();
    assert!(
        require_nonidentity_derived_positive_v1(&wire, &(Point::identity() - generator)).is_err()
    );
    assert!(require_nonidentity_derived_positive_v1(&[0; 33], &generator).is_err());
}

#[test]
fn signed_stage_rejects_incomplete_prior_owner_wrong_start_and_occupied_slot() {
    let _guard = prepared_commitment_test_guard_v1();
    let previous = continuation_fixture_before_v1(7_223);
    assert!(RnsNativeSmallSignedCommitmentsV1::validate_start_v1(&previous, 7_224).is_err());
    assert!(RnsNativeSmallSignedCommitmentsV1::begin_v1(previous).is_err());
    let mut previous = continuation_fixture_before_v1(7_224);
    RnsNativeSmallSignedCommitmentsV1::validate_start_v1(&previous, 7_224).unwrap();
    for ordinal in [0, 7_223, 7_225, 8_256, 9_288] {
        assert!(RnsNativeSmallSignedCommitmentsV1::validate_start_v1(&previous, ordinal).is_err());
    }
    let session = previous
        .live
        .as_mut()
        .unwrap()
        .difference
        .top
        .session
        .live
        .as_mut()
        .unwrap();
    session.next_purpose_ordinal = 1;
    assert!(RnsNativeSmallSignedCommitmentsV1::validate_start_v1(&previous, 7_224).is_err());
    let session = previous
        .live
        .as_mut()
        .unwrap()
        .difference
        .top
        .session
        .live
        .as_mut()
        .unwrap();
    session.next_purpose_ordinal = 0;
    session.inventory.slots[25_112] = Some(GlobalLookupCommitmentTicketV1 {
        coordinate: signed_commitment_coordinate_v1(7_224).unwrap(),
        point_wire: Point::canonical_generator()
            .unwrap()
            .to_non_identity_wire_bytes()
            .unwrap(),
    });
    assert!(RnsNativeSmallSignedCommitmentsV1::begin_v1(previous).is_err());
}

#[test]
fn actual_first_last_signed_and_negative_points_preserve_original_openings_and_derived_sum_and_prepared_opening_tail()
 {
    let _guard = prepared_commitment_test_guard_v1();
    let previous = continuation_fixture_before_v1(7_224);
    let live = previous.live.as_ref().unwrap();
    let pointers = [
        live.difference
            .top
            .retained_low
            .blindings
            .as_slice()
            .as_ptr(),
        live.difference.top.blindings.as_slice().as_ptr(),
        live.difference.blindings.as_slice().as_ptr(),
        live.blindings.as_slice().as_ptr(),
    ];
    let candidate_root = live.difference.top.retained_low.candidate_root;
    let point_root = live
        .difference
        .top
        .session
        .live
        .as_ref()
        .unwrap()
        .inventory
        .adopted_source_commitments_root_v1([0x32; 32])
        .unwrap();
    let mut owner = RnsNativeSmallSignedCommitmentsV1::begin_v1(previous).unwrap();
    assert!(owner.require_complete_v1().is_err());
    for ordinal in [7_224, 8_255, 8_256, 9_287] {
        seed_signed_before_v1(&mut owner, ordinal);
        owner.require_position_v1(ordinal).unwrap();
        assert!(owner.require_position_v1(ordinal + 1).is_err());
        let fixture = TestPreparedSmallSignedV1::for_ordinal_v1(ordinal);
        let (next_owner, tail) = owner
            .commit_prepared_v1(&fixture.statement_v1(ordinal))
            .unwrap();
        owner = next_owner;
        let tail = tail.into_chunk_v1(ordinal).unwrap();
        let live = owner.live.as_ref().unwrap();
        let rho = live.blindings.as_slice()[usize::from(ordinal - 7_224)];
        let session = live
            .continuation
            .difference
            .top
            .session
            .live
            .as_ref()
            .unwrap();
        let coordinate = signed_commitment_coordinate_v1(ordinal).unwrap();
        let ticket = session.inventory.slots[coordinate.global_ordinal as usize]
            .as_ref()
            .unwrap();
        assert_eq!(ticket.coordinate, coordinate);
        assert_eq!(&tail.as_slice_v1()[..32], &rho.to_be_bytes());
        assert_eq!(&tail.as_slice_v1()[32..65], &ticket.point_wire);
        assert!(tail.as_slice_v1()[65..].iter().all(|byte| *byte == 0));
        assert_eq!(
            ticket.point_wire,
            TestPreparedSmallSignedV1::expected_v1(ordinal, rho)
                .to_non_identity_wire_bytes()
                .unwrap()
        );
        if ordinal >= 8_256 {
            let unit = usize::from(ordinal - 8_256);
            let signed = session.inventory.slots[25_112 + unit].as_ref().unwrap();
            let signed_point =
                Point::from_non_identity_wire_bytes_exact(&signed.point_wire).unwrap();
            let negative_point =
                Point::from_non_identity_wire_bytes_exact(&ticket.point_wire).unwrap();
            // Both endpoints of this pair were actual source-value MSMs. The
            // independent positive equation uses the original two rho values.
            let rho_x = live.blindings.as_slice()[unit];
            let expected_plus =
                TestPreparedSmallSignedV1::expected_positive_v1(7_224 + unit as u16, rho_x + rho);
            assert_eq!(
                (signed_point + negative_point)
                    .to_non_identity_wire_bytes()
                    .unwrap(),
                expected_plus.to_non_identity_wire_bytes().unwrap()
            );
        }
    }
    owner.require_complete_v1().unwrap();
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
        [
            live.continuation
                .difference
                .top
                .retained_low
                .blindings
                .as_slice()
                .as_ptr(),
            live.continuation
                .difference
                .top
                .blindings
                .as_slice()
                .as_ptr(),
            live.continuation.difference.blindings.as_slice().as_ptr(),
            live.continuation.blindings.as_slice().as_ptr()
        ],
        pointers
    );
    assert_eq!(
        live.continuation.difference.top.retained_low.candidate_root,
        candidate_root
    );
    assert!(
        live.continuation
            .difference
            .top
            .retained_low
            .append_permit
            .is_some()
    );
    assert_eq!(live.blindings.len(), 2_064);
    let session = live
        .continuation
        .difference
        .top
        .session
        .live
        .as_ref()
        .unwrap();
    assert_eq!(session.next_global_ordinal, 27_176);
    assert!(session.inventory.slots[27_176].is_none());
    assert!(owner.require_position_v1(9_288).is_err());
    assert!(
        owner
            .commit_prepared_v1(
                &TestPreparedSmallSignedV1::for_ordinal_v1(9_287).statement_v1(9_287)
            )
            .is_err()
    );
}

#[test]
fn signed_entropy_error_zero_and_unwind_consume_original_owner() {
    let _guard = prepared_commitment_test_guard_v1();
    let fixture = TestPreparedSmallSignedV1::for_ordinal_v1(7_224);
    for fault in [
        TestEntropyFaultV1::ErrorAt(25_112),
        TestEntropyFaultV1::ZeroAt(25_112),
        TestEntropyFaultV1::PanicAt(25_112),
    ] {
        let mut owner =
            RnsNativeSmallSignedCommitmentsV1::begin_v1(continuation_fixture_before_v1(7_224))
                .unwrap();
        let GlobalLookupProofSessionEntropySourceV1::TestOnly(entropy) = &mut owner
            .live
            .as_mut()
            .unwrap()
            .continuation
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
        let panic_expected = matches!(fault, TestEntropyFaultV1::PanicAt(_));
        entropy.fault = fault;
        let mut owner = Some(owner);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            owner
                .take()
                .unwrap()
                .commit_prepared_v1(&fixture.statement_v1(7_224))
        }));
        assert!(owner.is_none());
        match result {
            Ok(result) => {
                assert!(!panic_expected);
                assert!(result.is_err());
            }
            Err(_) => assert!(panic_expected),
        }
    }
    let empty = RnsNativeSmallSignedCommitmentsV1::<core::convert::Infallible> { live: None };
    assert!(empty.require_position_v1(7_224).is_err());
    assert!(empty.require_complete_v1().is_err());
    assert!(
        empty
            .validate_completed_source_prefix_v1([0; 32], [0; 32], [0; 32], [0; 32])
            .is_err()
    );
    assert!(
        empty
            .commit_prepared_v1(&fixture.statement_v1(7_224))
            .is_err()
    );
}

#[test]
fn derived_positive_identity_and_wrong_signed_ticket_fail_closed() {
    let _guard = prepared_commitment_test_guard_v1();
    for corrupt_coordinate in [false, true] {
        let mut owner =
            RnsNativeSmallSignedCommitmentsV1::begin_v1(continuation_fixture_before_v1(7_224))
                .unwrap();
        seed_signed_before_v1(&mut owner, 8_256);
        let live = owner.live.as_mut().unwrap();
        let session = live
            .continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .unwrap();
        // Recover the deterministic fixture's next sample independently; do
        // not advance or replace the retained production owner/RNG.
        let mut entropy =
            GlobalLookupProofSessionEntropySourceV1::<core::convert::Infallible>::TestOnly(
                DeterministicProofSessionEntropyV1 {
                    seed: [0x41; 32],
                    fault: TestEntropyFaultV1::None,
                },
            );
        let (_chunk, rho) = sample_blinding_v1(&mut entropy, 26_144).unwrap();
        let expected_negative = TestPreparedSmallSignedV1::expected_v1(8_256, rho.get());
        let signed = session.inventory.slots[25_112].as_mut().unwrap();
        if corrupt_coordinate {
            signed.coordinate.purpose_ordinal += 1;
        } else {
            signed.point_wire = (Point::identity() - expected_negative)
                .to_non_identity_wire_bytes()
                .unwrap();
        }
        let fixture = TestPreparedSmallSignedV1::for_ordinal_v1(8_256);
        assert!(
            owner
                .commit_prepared_v1(&fixture.statement_v1(8_256))
                .is_err()
        );
    }
}
