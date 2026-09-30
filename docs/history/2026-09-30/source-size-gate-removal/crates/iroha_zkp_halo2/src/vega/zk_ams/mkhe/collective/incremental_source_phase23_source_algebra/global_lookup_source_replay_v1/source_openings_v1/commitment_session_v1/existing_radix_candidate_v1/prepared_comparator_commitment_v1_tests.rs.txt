//! Canonical top-role custody, exact inventory coordinates and actual MSM checks.
use super::*;
use crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::TestPreparedComparatorV1;

#[test]
fn both_top_roles_use_exact_shared_inventory_and_stop_before_delta() {
    let mut seen = std::collections::BTreeSet::new();
    for ordinal in 0..688 {
        let coordinate = top_coordinate_v1(ordinal).unwrap();
        assert_eq!(
            coordinate,
            comparator_signed_coordinate_v1(u32::from(ordinal)).unwrap()
        );
        assert_eq!(coordinate.global_ordinal, 12_040 + u32::from(ordinal));
        assert_eq!(coordinate.purpose_ordinal, u32::from(ordinal % 344));
        assert!(seen.insert(coordinate.global_ordinal));
    }
    assert_eq!(seen.len(), 688);
    assert_eq!(
        top_coordinate_v1(343).unwrap().purpose,
        GlobalLookupCommitmentPurposeV1::ComparatorDifferenceTop
    );
    assert_eq!(
        top_coordinate_v1(344).unwrap().purpose,
        GlobalLookupCommitmentPurposeV1::ComparatorSumTop
    );
    for ordinal in [688, 6_879, 7_223, u16::MAX] {
        assert!(top_coordinate_v1(ordinal).is_err());
    }
    let delta = commitment_coordinate_v1(12_728).unwrap();
    assert_eq!(
        delta.purpose,
        GlobalLookupCommitmentPurposeV1::ComparatorDifferenceDigit
    );
    assert_eq!(delta.purpose_ordinal, 0);
    assert_eq!(delta.purpose.count_v1(), 5_848);
    assert_eq!(
        comparator_signed_coordinate_v1(688).unwrap().global_ordinal,
        18_576
    );
    assert_eq!(TOP_RETAINED_BLINDING_BYTES_V1, 22_016);
}

#[test]
fn original_session_samples_and_retains_same_rho_for_actual_sparse_top_values_and_prepared_opening_tail()
 {
    let _guard = crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::prepared_commitment_test_guard_v1();
    let fixture = TestPreparedComparatorV1::new_v1();
    // Existing full-shape patterned inventory fixture is not live source proof.
    let previous = super::super::tests::complete_patterned_candidate_v1();
    let candidate_root = previous.candidate_root;
    let blinding_root = previous.blinding_root;
    let owner_root = previous.owner_binding_digest;
    let low_pointer = previous.blindings.as_slice().as_ptr();
    let mut owner = RnsNativeComparatorTopCommitmentsV1::begin_v1(previous).unwrap();
    owner.require_position_v1(0).unwrap();
    for wrong in [1, 343, 344, 687, 688] {
        assert!(owner.require_position_v1(wrong).is_err());
    }
    let live = owner.live.as_mut().unwrap();
    assert_eq!(live.retained_low.blindings.as_slice().as_ptr(), low_pointer);
    assert_eq!(live.retained_low.candidate_root, candidate_root);
    assert_eq!(live.retained_low.blinding_root, blinding_root);
    assert_eq!(live.retained_low.owner_binding_digest, owner_root);
    assert!(live.retained_low.append_permit.is_some());
    let session = live.session.live.as_mut().unwrap();
    session.next_purpose_ordinal = 1;
    assert!(owner.require_position_v1(0).is_err());
    owner
        .live
        .as_mut()
        .unwrap()
        .session
        .live
        .as_mut()
        .unwrap()
        .next_purpose_ordinal = 0;
    owner
        .live
        .as_mut()
        .unwrap()
        .session
        .live
        .as_mut()
        .unwrap()
        .proof_session_context_digest[31] ^= 1;
    assert!(owner.require_position_v1(0).is_err());
    owner
        .live
        .as_mut()
        .unwrap()
        .session
        .live
        .as_mut()
        .unwrap()
        .proof_session_context_digest[31] ^= 1;
    let (next_owner, tail) = owner.commit_prepared_v1(&fixture.statement_v1(0)).unwrap();
    owner = next_owner;
    let tail = tail.into_chunk_v1(0).unwrap();
    let live = owner.live.as_ref().unwrap();
    assert_eq!(live.next_plane, 1);
    assert_eq!(live.blindings.len(), 1);
    let rho = live.blindings.as_slice()[0];
    assert_eq!(
        rho.to_be_bytes(),
        hex_literal::hex!("79de46b3c5b379a7a844b2fe9af0acab590df7af6fb678bc2d251e9da2515575")
    );
    let session = live.session.live.as_ref().unwrap();
    assert_eq!(session.next_global_ordinal, 12_041);
    let ticket = session.inventory.slots[12_040].as_ref().unwrap();
    assert_eq!(ticket.coordinate, top_coordinate_v1(0).unwrap());
    assert_eq!(&tail.as_slice_v1()[..32], &rho.to_be_bytes());
    assert_eq!(&tail.as_slice_v1()[32..65], &ticket.point_wire);
    assert!(tail.as_slice_v1()[65..].iter().all(|byte| *byte == 0));
    assert_eq!(
        ticket.point_wire,
        TestPreparedComparatorV1::expected_v1(rho)
            .to_non_identity_wire_bytes()
            .unwrap()
    );
    assert_ne!(
        ticket.point_wire,
        TestPreparedComparatorV1::expected_v1(Scalar::from_u64(1))
            .to_non_identity_wire_bytes()
            .unwrap()
    );
    assert_eq!(live.retained_low.blindings.as_slice().as_ptr(), low_pointer);
    owner.require_position_v1(1).unwrap();
    assert!(owner.commit_prepared_v1(&fixture.statement_v1(0)).is_err());
}

#[test]
fn last_sum_top_adoption_uses_actual_msm_then_stops_at_delta_and_prepared_opening_tail() {
    let _guard = crate::vega::zk_ams::mkhe::collective::incremental_source::incremental_source_phase23::radix_range_v2::prepared_commitment_test_guard_v1();
    let fixture = TestPreparedComparatorV1::for_ordinal_v1(687);
    let previous = super::super::tests::complete_patterned_candidate_v1();
    let mut owner = RnsNativeComparatorTopCommitmentsV1::begin_v1(previous).unwrap();
    {
        // Explicit test-only prior state: these 687 points are patterned, not
        // commitments to authenticated values. Only the final adoption below
        // executes the actual sampler -> prepared values -> secret MSM path.
        // The original session sampler still advances once for each prior rho.
        let live = owner.live.as_mut().unwrap();
        let session = live.session.live.as_mut().unwrap();
        let point_wire = Point::canonical_generator()
            .unwrap()
            .to_non_identity_wire_bytes()
            .unwrap();
        for ordinal in 0..687 {
            let coordinate = top_coordinate_v1(ordinal).unwrap();
            let (_chunk, scalar) =
                sample_blinding_v1(&mut session.entropy, coordinate.global_ordinal).unwrap();
            let slot = &mut session.inventory.slots[coordinate.global_ordinal as usize];
            assert!(slot.is_none());
            *slot = Some(GlobalLookupCommitmentTicketV1 {
                coordinate,
                point_wire,
            });
            live.blindings.push(scalar.get());
        }
        live.next_plane = 687;
        let next = top_coordinate_v1(687).unwrap();
        session.next_global_ordinal = next.global_ordinal;
        session.next_purpose = next.purpose;
        session.next_purpose_ordinal = next.purpose_ordinal;
        validate_top_progress_v1(live).unwrap();
    }
    owner.require_position_v1(687).unwrap();
    let (next_owner, tail) = owner
        .commit_prepared_v1(&fixture.statement_v1(687))
        .unwrap();
    owner = next_owner;
    let tail = tail.into_chunk_v1(687).unwrap();
    let live = owner.live.as_ref().unwrap();
    assert_eq!(live.next_plane, 688);
    assert_eq!(live.blindings.len(), 688);
    let rho = live.blindings.as_slice()[687];
    assert_eq!(
        rho.to_be_bytes(),
        hex_literal::hex!("d71fcebf0c534c077fee4dbf8b60ad2c19e211f58c471bce0d4b8e03079d376e")
    );
    let session = live.session.live.as_ref().unwrap();
    let ticket = session.inventory.slots[12_727].as_ref().unwrap();
    assert_eq!(ticket.coordinate, top_coordinate_v1(687).unwrap());
    assert_eq!(&tail.as_slice_v1()[..32], &rho.to_be_bytes());
    assert_eq!(&tail.as_slice_v1()[32..65], &ticket.point_wire);
    assert!(tail.as_slice_v1()[65..].iter().all(|byte| *byte == 0));
    assert_eq!(
        ticket.coordinate.purpose,
        GlobalLookupCommitmentPurposeV1::ComparatorSumTop
    );
    assert_eq!(ticket.coordinate.purpose_ordinal, 343);
    assert_eq!(
        ticket.point_wire,
        TestPreparedComparatorV1::expected_v1(rho)
            .to_non_identity_wire_bytes()
            .unwrap()
    );
    assert_eq!(session.next_global_ordinal, 12_728);
    assert_eq!(
        session.next_purpose,
        GlobalLookupCommitmentPurposeV1::ComparatorDifferenceDigit
    );
    assert_eq!(session.next_purpose_ordinal, 0);
    assert!(session.inventory.slots[12_728].is_none());
    let beta = comparator_signed_coordinate_v1(688).unwrap();
    assert_eq!(beta.global_ordinal, 18_576);
    assert!(session.inventory.slots[beta.global_ordinal as usize].is_none());
    validate_top_progress_v1(live).unwrap();
    assert!(owner.require_position_v1(687).is_err());
    assert!(owner.require_position_v1(688).is_err());
    assert!(
        owner
            .commit_prepared_v1(&fixture.statement_v1(688))
            .is_err()
    );
}

#[test]
fn private_transition_has_no_point_or_entropy_injection_or_cursor_reset() {
    let source = include_str!("prepared_comparator_commitment_v1.rs");
    let compact = source.split_whitespace().collect::<String>();
    let commit = compact
        .split("fncommit_prepared_v1(")
        .nth(1)
        .unwrap()
        .split("fnvalidate_top_progress_v1")
        .next()
        .unwrap();
    for (first, last) in [
        ("self.live.take()", "statement.require_ordinal_v1"),
        ("require_top_position_v1", "sample_blinding_v1"),
        ("sample_blinding_v1", "statement.commitment_v1"),
        ("statement.commitment_v1", "*slot=Some"),
        ("*slot=Some", "live.blindings.push(scalar.get())"),
    ] {
        assert!(commit.find(first).unwrap() < commit.find(last).unwrap());
    }
    for forbidden in [
        "fn into_parts",
        "original_random:",
        "Infallible",
        "point: &",
        "fn scalar",
        "derive(Clone",
        "derive(Debug",
        "mem::forget",
        "next_global_ordinal = 344",
    ] {
        assert!(!source.contains(forbidden), "forbidden surface {forbidden}");
    }
    assert!(source.contains("owner.validate_v1()"));
    assert!(source.contains("let RnsNativeExistingRadixCandidateOwnerV1"));
    assert!(source.lines().count() <= 400);
    assert!(source.len() <= 24_000);
}
