//! Original retained rho/ticket comparisons; synthetic source is not qualification.
use super::*;
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::prepared_commitment_test_guard_v1;
type Owner = RnsNativeSmallSignedCommitmentsV1<core::convert::Infallible>;
fn owner_v1() -> Owner {
    let previous = super::tests::continuation_fixture_before_v1(7_224);
    let mut owner = Owner::begin_v1(previous).unwrap();
    super::tests::seed_signed_before_v1(&mut owner, SIGNED_AFTER_PLANE_V1);
    owner
}
fn chunk_v1(
    owner: &RnsNativeStoredPlaneReplayV1<core::convert::Infallible>,
    ordinal: u16,
) -> ConfidentialSpoolChunkV1 {
    let live = &owner.live;
    let expected = if ordinal < 688 {
        live.continuation.difference.top.blindings.as_slice()[usize::from(ordinal)]
    } else if ordinal < 7_224 {
        live.continuation.blindings.as_slice()[usize::from(ordinal - 688)]
    } else {
        live.blindings.as_slice()[usize::from(ordinal - 7_224)]
    };
    let coordinate = comparator_signed_coordinate_v1(u32::from(ordinal)).unwrap();
    let ticket = live
        .continuation
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
    let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(16_384).unwrap();
    let bytes: &mut [u8; 32] = (&mut chunk.as_mut_slice_v1()[..32]).try_into().unwrap();
    expected.write_le_bytes_ref(bytes);
    bytes.reverse();
    chunk.as_mut_slice_v1()[32..65].copy_from_slice(&ticket.point_wire);
    chunk
}

#[test]
fn stored_tail_linkage_uses_every_original_rho_range_and_the_delta_ticket_gap() {
    let _guard = prepared_commitment_test_guard_v1();
    let owner = owner_v1();
    let original_inventory = owner
        .live
        .as_ref()
        .unwrap()
        .continuation
        .difference
        .top
        .session
        .live
        .as_ref()
        .unwrap()
        .inventory
        .slots
        .as_ptr();
    let original_rhos = owner.live.as_ref().unwrap().blindings.as_slice().as_ptr();
    SOURCE_PREFIX_ROOT_VALIDATIONS_V1.with(|count| count.set(0));
    let owner = owner.into_stored_plane_replay_v1().unwrap();
    assert_eq!(
        SOURCE_PREFIX_ROOT_VALIDATIONS_V1.with(core::cell::Cell::get),
        1
    );
    let live = &owner.live;
    let inventory = live
        .continuation
        .difference
        .top
        .session
        .live
        .as_ref()
        .unwrap()
        .inventory
        .slots
        .as_ptr();
    assert_eq!(inventory, original_inventory);
    assert_eq!(owner.live.blindings.as_slice().as_ptr(), original_rhos);
    for ordinal in 0..9_288 {
        let coordinate = comparator_signed_coordinate_v1(u32::from(ordinal)).unwrap();
        assert_eq!(
            coordinate.global_ordinal,
            if ordinal < 688 {
                12_040 + u32::from(ordinal)
            } else {
                18_576 + u32::from(ordinal - 688)
            }
        );
        let chunk = chunk_v1(&owner, ordinal);
        owner
            .validate_stored_plane_tail_v1(ordinal, chunk.as_slice_v1())
            .unwrap();
    }
    assert_eq!(
        owner
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_ref()
            .unwrap()
            .inventory
            .slots
            .as_ptr(),
        inventory
    );
    // Every tail keeps its original checks, but no tail rehashes 344 source
    // tickets. Only the consuming admission traversed the original prefix.
    assert_eq!(
        SOURCE_PREFIX_ROOT_VALIDATIONS_V1.with(core::cell::Cell::get),
        1
    );
    assert_eq!(owner.live.blindings.as_slice().as_ptr(), original_rhos);
}

#[test]
fn stored_tail_linkage_rejects_valid_but_wrong_rho_point_coordinate_and_padding() {
    let _guard = prepared_commitment_test_guard_v1();
    let mut owner = owner_v1().into_stored_plane_replay_v1().unwrap();
    for ordinal in [
        0, 343, 344, 687, 688, 6_879, 6_880, 7_223, 7_224, 8_255, 8_256, 9_287,
    ] {
        for fault in 0..5 {
            let mut chunk = chunk_v1(&owner, ordinal);
            match fault {
                0 => chunk.as_mut_slice_v1()[..32].fill(0),
                1 => chunk.as_mut_slice_v1()[..32]
                    .copy_from_slice(&crate::vega::VEGA_T256_SCALAR_MODULUS_BE_V1),
                2 => chunk.as_mut_slice_v1()[32..65].copy_from_slice(
                    &(Point::canonical_generator().unwrap()
                        + Point::canonical_generator().unwrap())
                    .to_non_identity_wire_bytes()
                    .unwrap(),
                ),
                3 => chunk.as_mut_slice_v1()[16_383] = 1,
                _ => {
                    let original = Scalar::from_be_bytes_exact_ref(
                        chunk.as_slice_v1()[..32].try_into().unwrap(),
                    )
                    .unwrap();
                    let changed = original + Scalar::one();
                    let encoded: &mut [u8; 32] =
                        (&mut chunk.as_mut_slice_v1()[..32]).try_into().unwrap();
                    changed.write_le_bytes_ref(encoded);
                    encoded.reverse();
                }
            }
            assert!(
                owner
                    .validate_stored_plane_tail_v1(ordinal, chunk.as_slice_v1())
                    .is_err()
            );
        }
        let chunk = chunk_v1(&owner, ordinal);
        assert!(
            owner
                .validate_stored_plane_tail_v1(ordinal, &chunk.as_slice_v1()[..16_383])
                .is_err()
        );
        assert!(
            owner
                .validate_stored_plane_tail_v1(9_288, chunk.as_slice_v1())
                .is_err()
        );
        let coordinate = comparator_signed_coordinate_v1(u32::from(ordinal)).unwrap();
        let session = owner
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .unwrap();
        session.inventory.slots[coordinate.global_ordinal as usize]
            .as_mut()
            .unwrap()
            .coordinate
            .purpose_ordinal += 1;
        assert!(
            owner
                .validate_stored_plane_tail_v1(ordinal, chunk.as_slice_v1())
                .is_err()
        );
        owner
            .live
            .continuation
            .difference
            .top
            .session
            .live
            .as_mut()
            .unwrap()
            .inventory
            .slots[coordinate.global_ordinal as usize]
            .as_mut()
            .unwrap()
            .coordinate = coordinate;
    }
}

#[test]
fn stored_tail_linkage_admission_rejects_incomplete_poisoned_and_rebound_source() {
    use crate::vega::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1;
    let _guard = prepared_commitment_test_guard_v1();
    assert!(Owner { live: None }.into_stored_plane_replay_v1().is_err());
    for fault in 0..4 {
        let mut owner = owner_v1();
        let live = owner.live.as_mut().unwrap();
        match fault {
            0 => live.next_plane -= 1,
            1 => {
                live.continuation
                    .difference
                    .top
                    .session
                    .live
                    .as_mut()
                    .unwrap()
                    .next_global_ordinal -= 1
            }
            2 => {
                live.continuation
                    .difference
                    .top
                    .session
                    .live
                    .as_mut()
                    .unwrap()
                    .proof_session_context_digest[0] ^= 1
            }
            _ => {
                live.continuation
                    .difference
                    .top
                    .session
                    .live
                    .as_mut()
                    .unwrap()
                    .inventory
                    .slots[0]
                    .as_mut()
                    .unwrap()
                    .point_wire[0] ^= 1
            }
        }
        let before = zeroizing_t256_scalar_vec_drop_count_v1();
        assert!(owner.into_stored_plane_replay_v1().is_err());
        assert!(zeroizing_t256_scalar_vec_drop_count_v1() > before);
    }
}

#[test]
fn stored_tail_linkage_replay_unwind_drops_original_zeroizing_masks() {
    use crate::vega::bulletproof_t256::zeroizing_t256_scalar_vec_drop_count_v1;
    let _guard = prepared_commitment_test_guard_v1();
    let owner = owner_v1().into_stored_plane_replay_v1().unwrap();
    let before = zeroizing_t256_scalar_vec_drop_count_v1();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        let _original_owner = owner;
        panic!("intentional immutable replay owner unwind");
    }));
    assert!(result.is_err());
    assert!(zeroizing_t256_scalar_vec_drop_count_v1() > before);
}
