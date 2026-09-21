//! Canonical tail geometry and refusal tests; no authenticated-source authority.
use super::*;

fn admitted_session_fixture_v1(ordinal: u16) -> GlobalLookupCommitmentSessionLiveV1<Infallible> {
    let mut owner = GlobalLookupCommitmentSessionV1::test_only_v1([0x31; 32], [0x41; 32]).unwrap();
    let mut session = owner.live.take().unwrap();
    let coordinate = comparator_signed_coordinate_v1(u32::from(ordinal)).unwrap();
    session.inventory.slots[coordinate.global_ordinal as usize] =
        Some(GlobalLookupCommitmentTicketV1 {
            coordinate,
            point_wire: Point::canonical_generator()
                .unwrap()
                .to_non_identity_wire_bytes()
                .unwrap(),
        });
    let next = commitment_coordinate_v1(coordinate.global_ordinal + 1).unwrap();
    session.next_global_ordinal = next.global_ordinal;
    session.next_purpose = next.purpose;
    session.next_purpose_ordinal = next.purpose_ordinal;
    session
}

// Geometry fixture only: its point is not claimed to commit to source values.
pub(super) fn admitted_tail_fixture_v1(ordinal: u16) -> PreparedPlaneOpeningTailV1 {
    PreparedPlaneOpeningTailV1::from_admitted_v1(
        &admitted_session_fixture_v1(ordinal),
        comparator_signed_coordinate_v1(u32::from(ordinal)).unwrap(),
        &Scalar::from_u64(0x0102_0304_0506_0708),
    )
    .unwrap()
}

#[test]
fn prepared_opening_tail_all_9288_coordinates_are_exact_and_other_purposes_refused() {
    for ordinal in 0..9_288_u32 {
        let canonical = comparator_signed_coordinate_v1(ordinal).unwrap();
        assert_eq!(tail_plane_ordinal_v1(canonical).unwrap(), ordinal as u16);
        let mut changed = canonical;
        changed.global_ordinal += 1;
        assert!(tail_plane_ordinal_v1(changed).is_err());
        let mut changed = canonical;
        changed.purpose_ordinal = u32::MAX;
        assert!(tail_plane_ordinal_v1(changed).is_err());
    }
    for physical in [0, 344, 12_728, 27_176] {
        assert!(tail_plane_ordinal_v1(commitment_coordinate_v1(physical).unwrap()).is_err());
    }
}

#[test]
fn prepared_opening_tail_exact_big_endian_scalar_point_and_zero_padding() {
    for ordinal in [
        0, 343, 344, 687, 688, 6_879, 6_880, 7_223, 7_224, 8_255, 8_256, 9_287,
    ] {
        let tail = admitted_tail_fixture_v1(ordinal);
        let drops = PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1();
        let chunk = tail.into_chunk_v1(ordinal).unwrap();
        assert_eq!(
            PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1(),
            drops + 1
        );
        let bytes = chunk.as_slice_v1();
        assert_eq!(bytes.len(), 16_384);
        assert_eq!(&bytes[..24], &[0; 24]);
        assert_eq!(&bytes[24..32], &[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(
            &bytes[32..65],
            &Point::canonical_generator()
                .unwrap()
                .to_non_identity_wire_bytes()
                .unwrap()
        );
        assert!(bytes[65..].iter().all(|byte| *byte == 0));
    }
}

#[test]
fn prepared_opening_tail_refuses_wrong_session_cursor_ticket_zero_rho_and_point() {
    let coordinate = comparator_signed_coordinate_v1(688).unwrap();
    for fault in 0..6 {
        let mut session = admitted_session_fixture_v1(688);
        match fault {
            0 => session.next_global_ordinal += 1,
            1 => session.next_purpose_ordinal += 1,
            2 => session.pending_source = Some(coordinate),
            3 => session.inventory.slots[coordinate.global_ordinal as usize] = None,
            4 => {
                session.inventory.slots[coordinate.global_ordinal as usize]
                    .as_mut()
                    .unwrap()
                    .coordinate
                    .purpose_ordinal += 1
            }
            _ => session.inventory.slots[coordinate.global_ordinal as usize]
                .as_mut()
                .unwrap()
                .point_wire
                .fill(0),
        }
        assert!(
            PreparedPlaneOpeningTailV1::from_admitted_v1(&session, coordinate, &Scalar::one())
                .is_err()
        );
    }
    assert!(
        PreparedPlaneOpeningTailV1::from_admitted_v1(
            &admitted_session_fixture_v1(688),
            coordinate,
            &Scalar::zero()
        )
        .is_err()
    );
}

#[test]
fn prepared_opening_tail_wrong_ordinal_and_unwind_dispose_scalar_owner() {
    let tail = admitted_tail_fixture_v1(8_256);
    let drops = PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1();
    assert!(tail.into_chunk_v1(7_224).is_err());
    assert_eq!(
        PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1(),
        drops + 1
    );
    let tail = admitted_tail_fixture_v1(8_256);
    let drops = PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _tail = tail;
            panic!("injected tail-consumer unwind");
        }))
        .is_err()
    );
    assert_eq!(
        PreparedPlaneOpeningTailV1::test_scalar_owner_drop_count_v1(),
        drops + 1
    );
}
