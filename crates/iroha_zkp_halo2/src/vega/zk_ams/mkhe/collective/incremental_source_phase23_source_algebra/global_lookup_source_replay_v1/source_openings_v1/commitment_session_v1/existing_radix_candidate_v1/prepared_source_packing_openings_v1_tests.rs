//! Original inventory/mask projection controls; fixtures are not source authority.
use super::*;
use crate::generalized_bulletproof::ProofSuite;
use crate::vega::bulletproof_t256::{
    ZkAmsT256BulletproofSuiteV1, zeroizing_t256_scalar_vec_drop_count_v1,
};
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::prepared_commitment_test_guard_v1;

type FixtureOwner = RnsNativeSmallSignedCommitmentsV1<core::convert::Infallible>;

fn complete_fixture_v1() -> FixtureOwner {
    let previous = super::super::tests::continuation_fixture_before_v1(7_224);
    let mut owner = FixtureOwner::begin_v1(previous).unwrap();
    super::super::tests::seed_signed_before_v1(&mut owner, SIGNED_AFTER_PLANE_V1);
    owner
}

fn live_v1(owner: &FixtureOwner) -> &SmallSignedCommitmentsLiveV1<core::convert::Infallible> {
    owner.live.as_ref().unwrap()
}

#[test]
fn packing_projection_maps_group_major_masks_to_purpose_major_inventory() {
    for group in 0..344 {
        for digit in 0..17 {
            let coordinate = difference_low_coordinate_v1(group, digit).unwrap();
            assert_eq!(coordinate.wire_ordinal as usize, group * 34 + digit);
            assert_eq!(
                coordinate.inventory_ordinal as usize,
                344 + group * 17 + digit
            );
            assert_eq!(coordinate.purpose_ordinal as usize, group * 17 + digit);
        }
    }
    for ordinal in 0..1_032 {
        let (record, role, plane) = signed_owner_coordinate_v1(ordinal).unwrap();
        let coordinate = signed_inventory_coordinate_v1(record, role, plane).unwrap();
        assert_eq!(coordinate.global_ordinal as usize, 25_112 + ordinal);
        assert_eq!(
            coordinate.purpose,
            GlobalLookupCommitmentPurposeV1::SmallSigned
        );
        assert_eq!(coordinate.purpose_ordinal as usize, ordinal);
    }
    for (group, digit) in [(344, 0), (0, 17), (usize::MAX, 0), (0, usize::MAX)] {
        assert!(difference_low_coordinate_v1(group, digit).is_err());
    }
    assert!(signed_owner_coordinate_v1(1_032).is_err());
    assert!(signed_owner_coordinate_v1(usize::MAX).is_err());
    assert!(signed_inventory_coordinate_v1(43, RnsNativeSignedSourceRoleV1::R, 0).is_err());
    assert!(signed_inventory_coordinate_v1(0, RnsNativeSignedSourceRoleV1::E1, 8).is_err());
    assert_eq!(DERIVED_MASK_BYTES_V1, 44_032);
}

#[test]
fn packing_projection_retains_original_inventory_and_exact_masks_without_advancing_qmask() {
    let _guard = prepared_commitment_test_guard_v1();
    let owner = complete_fixture_v1();
    let live = live_v1(&owner);
    let session = live
        .continuation
        .difference
        .top
        .session
        .live
        .as_ref()
        .unwrap();
    let original_inventory = session.inventory.slots.as_ptr();
    let original_low = live
        .continuation
        .difference
        .top
        .retained_low
        .blindings
        .as_slice()
        .as_ptr();
    let original_signed = live.blindings.as_slice().as_ptr();
    let original_context = session.proof_session_context_digest;
    let original_points: Vec<_> = session
        .inventory
        .slots
        .iter()
        .map(|ticket| {
            ticket
                .as_ref()
                .map(|ticket| (ticket.coordinate, ticket.point_wire))
        })
        .collect();
    let owner = owner.prepare_source_packing_openings_v1().unwrap();
    let live = live_v1(&owner);
    let material = live.packing_openings.as_ref().unwrap();
    let session = live
        .continuation
        .difference
        .top
        .session
        .live
        .as_ref()
        .unwrap();
    assert_eq!(session.inventory.slots.as_ptr(), original_inventory);
    assert_eq!(
        live.continuation
            .difference
            .top
            .retained_low
            .blindings
            .as_slice()
            .as_ptr(),
        original_low
    );
    assert_eq!(live.blindings.as_slice().as_ptr(), original_signed);
    assert_eq!(session.proof_session_context_digest, original_context);
    assert_eq!(
        session
            .inventory
            .slots
            .iter()
            .map(|ticket| ticket
                .as_ref()
                .map(|ticket| (ticket.coordinate, ticket.point_wire)))
            .collect::<Vec<_>>(),
        original_points
    );
    assert_eq!(
        (
            session.next_global_ordinal,
            session.next_purpose,
            session.next_purpose_ordinal
        ),
        (27_176, GlobalLookupCommitmentPurposeV1::QMaskDigit, 0)
    );
    assert_eq!(material.masks.len(), 1_376);
    assert_ne!(material.point_root, [0; 32]);
    for group in 0..344 {
        // Independent reverse-Horner reconstruction, selecting D rather than S.
        let top = &live.continuation.difference.top;
        let mut expected = top.blindings.as_slice()[group];
        for digit in (0..17).rev() {
            expected *= Scalar::from_u64(1 << 15);
            expected += top.retained_low.blindings.as_slice()[group * 34 + digit];
        }
        assert_eq!(material.masks.as_slice()[group], expected);
    }
    assert_eq!(
        &material.masks.as_slice()[344..],
        &live.blindings.as_slice()[..1_032]
    );
    let before_drop = zeroizing_t256_scalar_vec_drop_count_v1();
    assert!(owner.prepare_source_packing_openings_v1().is_err());
    // Repeated preparation consumes the original session and prepared masks.
    assert!(zeroizing_t256_scalar_vec_drop_count_v1() >= before_drop + 6);
}

#[test]
fn packing_projection_matches_actual_basis_commitments_at_both_group_boundaries() {
    let _guard = prepared_commitment_test_guard_v1();
    let mut owner = complete_fixture_v1();
    let generators = ZkAmsT256BulletproofSuiteV1::generators();
    for group in [0, 343] {
        let live = owner.live.as_mut().unwrap();
        let top = &mut live.continuation.difference.top;
        for digit in 0..17 {
            let coordinate = difference_low_coordinate_v1(group, digit).unwrap();
            let rho = top.retained_low.blindings.as_slice()[coordinate.wire_ordinal as usize];
            let point = generators.g_bold[0].mul_scalar(Scalar::from_u64(digit as u64 + 1))
                + generators.g_bold[16_383].mul_scalar(Scalar::from_u64(17 - digit as u64))
                + generators.h.mul_scalar(rho);
            top.session.live.as_mut().unwrap().inventory.slots
                [coordinate.inventory_ordinal as usize]
                .as_mut()
                .unwrap()
                .point_wire = point.to_non_identity_wire_bytes().unwrap();
        }
        let rho = top.blindings.as_slice()[group];
        let top_point = generators.g_bold[0] + generators.h.mul_scalar(rho);
        let coordinate = top_coordinate_v1(group as u16).unwrap();
        top.session.live.as_mut().unwrap().inventory.slots[coordinate.global_ordinal as usize]
            .as_mut()
            .unwrap()
            .point_wire = top_point.to_non_identity_wire_bytes().unwrap();
        let mask = derive_difference_mask_v1(live, group).unwrap();
        let view = OriginalPackingCommitmentViewV1 { live };
        let mut reconstructed = view.packing_difference_top_v1(group).unwrap();
        let mut value_first = Scalar::one();
        let mut value_last = Scalar::zero();
        for digit in (0..17).rev() {
            reconstructed = reconstructed.mul_scalar(Scalar::from_u64(1 << 15))
                + view.packing_difference_low_v1(group, digit).unwrap();
            value_first *= Scalar::from_u64(1 << 15);
            value_first += Scalar::from_u64(digit as u64 + 1);
            value_last *= Scalar::from_u64(1 << 15);
            value_last += Scalar::from_u64(17 - digit as u64);
        }
        let expected = generators.g_bold[0].mul_scalar(value_first)
            + generators.g_bold[16_383].mul_scalar(value_last)
            + generators.h.mul_scalar(mask.get());
        assert_eq!(reconstructed, expected);
        assert_ne!(reconstructed, expected + generators.g_bold[0]);
    }
}

#[test]
fn packing_projection_rejects_missing_reordered_and_malformed_original_tickets() {
    let _guard = prepared_commitment_test_guard_v1();
    let mut owner = complete_fixture_v1();
    for group in [0, 343] {
        let coordinate = difference_low_coordinate_v1(group, 16).unwrap();
        let index = coordinate.inventory_ordinal as usize;
        let live = owner.live.as_mut().unwrap();
        let original = live
            .continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .unwrap()
            .inventory
            .slots[index]
            .take()
            .unwrap();
        assert!(derive_difference_mask_v1(live, group).is_err());
        live.continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .unwrap()
            .inventory
            .slots[index] = Some(GlobalLookupCommitmentTicketV1 {
            coordinate: commitment_coordinate_v1(coordinate.inventory_ordinal + 1).unwrap(),
            point_wire: original.point_wire,
        });
        assert!(derive_difference_mask_v1(live, group).is_err());
        live.continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .unwrap()
            .inventory
            .slots[index] = Some(GlobalLookupCommitmentTicketV1 {
            coordinate: original.coordinate,
            point_wire: [0; 33],
        });
        assert!(derive_difference_mask_v1(live, group).is_err());
        live.continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .unwrap()
            .inventory
            .slots[index] = Some(original);
    }
    // A malformed signed ticket fails the same complete consuming transition.
    owner
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
        .inventory
        .slots[25_112]
        .as_mut()
        .unwrap()
        .point_wire = [0; 33];
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    assert!(owner.prepare_source_packing_openings_v1().is_err());
    assert!(zeroizing_t256_scalar_vec_drop_count_v1() >= before + 6);
}

#[test]
fn packing_projection_refuses_incomplete_original_signed_stage() {
    let _guard = prepared_commitment_test_guard_v1();
    let previous = super::super::tests::continuation_fixture_before_v1(7_224);
    let owner = FixtureOwner::begin_v1(previous).unwrap();
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    assert!(owner.prepare_source_packing_openings_v1().is_err());
    assert!(zeroizing_t256_scalar_vec_drop_count_v1() > before);
}

#[test]
fn packing_projection_accepts_honest_cancelling_masks_without_resampling() {
    let _guard = prepared_commitment_test_guard_v1();
    let mut owner = complete_fixture_v1();
    let h = ZkAmsT256BulletproofSuiteV1::generators().h;
    let live = owner.live.as_mut().unwrap();
    let top = &mut live.continuation.difference.top;
    let mut sum = Scalar::zero();
    let mut weight = Scalar::one();
    for digit in 0..17 {
        let coordinate = difference_low_coordinate_v1(0, digit).unwrap();
        top.retained_low.blindings.as_mut_slice()[coordinate.wire_ordinal as usize] = Scalar::one();
        top.session.live.as_mut().unwrap().inventory.slots[coordinate.inventory_ordinal as usize]
            .as_mut()
            .unwrap()
            .point_wire = h.to_non_identity_wire_bytes().unwrap();
        sum += weight;
        weight *= Scalar::from_u64(1 << 15);
    }
    let cancelling = Scalar::zero() - sum * weight.inverse().unwrap();
    assert!(!cancelling.is_zero());
    top.blindings.as_mut_slice()[0] = cancelling;
    let coordinate = top_coordinate_v1(0).unwrap();
    top.session.live.as_mut().unwrap().inventory.slots[coordinate.global_ordinal as usize]
        .as_mut()
        .unwrap()
        .point_wire = h
        .mul_scalar(cancelling)
        .to_non_identity_wire_bytes()
        .unwrap();
    let mask = derive_difference_mask_v1(live, 0).unwrap();
    assert!(mask.get().is_zero());
    let view = OriginalPackingCommitmentViewV1 { live };
    let mut point = view.packing_difference_top_v1(0).unwrap();
    for digit in (0..17).rev() {
        point = point.mul_scalar(Scalar::from_u64(1 << 15))
            + view.packing_difference_low_v1(0, digit).unwrap();
    }
    assert!(point.is_identity());
}
