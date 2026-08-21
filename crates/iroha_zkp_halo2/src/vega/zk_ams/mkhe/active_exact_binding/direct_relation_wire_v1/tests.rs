use super::predecode_v1::{BOUND_TWO_CHUNK_WIRE_BYTES_V1, MEMBERSHIP_HEADER_BYTES_V1};
use super::*;
use crate::vega::VEGA_T256_SCALAR_MODULUS_BE_V1;
use crate::vega::zk_ams::mkhe::direct_object_transport::{
    ZkAmsMkheDirectObjectKindV1, ZkAmsMkheDirectObjectPointerV1,
};
use crate::vega::zk_ams::mkhe::exact_eight_chunk_membership::{
    DirectRelationBoundOneMembershipRoleV1, DirectRelationBoundTwoMembershipRoleV1,
    canonical_membership_syntax_wire_fixture_for_test, repair_membership_proof_set_digest_for_test,
};
fn pointer(
    kind: ZkAmsMkheDirectObjectKindV1,
    bytes: u64,
    seed: u8,
) -> ZkAmsMkheDirectObjectPointerV1 {
    ZkAmsMkheDirectObjectPointerV1::new(kind, bytes, [seed; 32]).expect("pointer")
}
#[test]
fn every_relation_has_one_exact_layout_and_role_table() {
    let rows = [
        (
            PersistentDirectRelationV1::RkgRoundOne,
            828,
            2,
            0x0f,
            0x30,
            25_248_766,
        ),
        (
            PersistentDirectRelationV1::RkgRoundTwo,
            938,
            3,
            0x13,
            0x2c,
            25_248_876,
        ),
        (
            PersistentDirectRelationV1::RkgNormalize,
            828,
            2,
            0x21,
            0x1e,
            25_248_766,
        ),
        (
            PersistentDirectRelationV1::Galois,
            718,
            1,
            0x05,
            0x3a,
            25_248_656,
        ),
    ];
    for (relation, statement, objects, active, zero, total) in rows {
        assert_eq!(relation.statement_bytes(), statement);
        assert_eq!(relation.object_count(), objects);
        assert_eq!(relation.active_witness_mask(), active);
        assert_eq!(relation.forced_zero_witness_mask(), zero);
        assert_eq!(active & zero, 0);
        assert_eq!(active | zero, 0x3f);
        assert_eq!(HEADER_BYTES_V1 + statement + BODY_BYTES_V1, total);
    }
    assert_eq!(
        MEMBERSHIP_FRAME_OFFSETS_V1,
        [0, 12_291, 24_582, 37_401, 50_220, 63_039]
    );
}
#[test]
fn typed_objects_reject_wrong_kind_or_polynomial_width() {
    let exact = pointer(
        ZkAmsMkheDirectObjectKindV1::RkgH0,
        EXACT_POLYNOMIAL_OBJECT_BYTES_V1,
        1,
    );
    assert!(DirectPolynomialObjectV1::<RkgH0ObjectRoleV1>::new([2; 32], exact).is_ok());
    assert!(DirectPolynomialObjectV1::<RkgH1ObjectRoleV1>::new([2; 32], exact).is_err());
    let short = pointer(
        ZkAmsMkheDirectObjectKindV1::RkgH0,
        EXACT_POLYNOMIAL_OBJECT_BYTES_V1 - 1,
        3,
    );
    assert!(DirectPolynomialObjectV1::<RkgH0ObjectRoleV1>::new([4; 32], short).is_err());
    assert!(DirectPolynomialObjectV1::<RkgH0ObjectRoleV1>::new([0; 32], exact).is_err());
    assert_eq!(2 * EXACT_POLYNOMIAL_OBJECT_BYTES_V1, 79_691_776);
    const {
        assert!(2 * EXACT_POLYNOMIAL_OBJECT_BYTES_V1 > 64 * 1024 * 1024);
    }
}
#[test]
fn membership_slot_statements_bind_relation_slot_bound_and_zero_shape() {
    let core = [7; 32];
    let relations = [
        PersistentDirectRelationV1::RkgRoundOne,
        PersistentDirectRelationV1::RkgRoundTwo,
        PersistentDirectRelationV1::RkgNormalize,
        PersistentDirectRelationV1::Galois,
    ];
    let mut digests = Vec::new();
    for relation in relations {
        for slot in 0..6 {
            let digest = membership_share_statement_digest(relation, core, slot);
            assert_ne!(digest, [0; 32]);
            assert!(!digests.contains(&digest));
            digests.push(digest);
        }
    }
    assert_eq!(digests.len(), 24);
}
#[test]
fn response_and_blind_ordering_widths_are_exact() {
    use super::predecode_v1::{
        blind_response_offset, membership_commitment_point_offset, membership_section_offsets,
        response_offset,
    };
    assert_eq!(4 * 6 * 131_072 * 8, RESPONSE_BYTES_V1);
    assert_eq!(4 * 6 * 8 * 32, BLIND_RESPONSE_BYTES_V1);
    assert_eq!(2 * 12_291 + 4 * 12_819, MEMBERSHIP_BYTES_V1);
    assert_eq!(2 * 8 * (9 + 2 * 15) + 4 * 8 * (9 + 2 * 16), 1_936);
    assert_eq!(6 * 8 * 5, 240);
    assert_eq!(
        response_offset(3, 5, 131_071).unwrap() + 8,
        RESPONSE_BYTES_V1
    );
    assert_eq!(
        blind_response_offset(3, 5, 7).unwrap() + 32,
        BLIND_RESPONSE_BYTES_V1
    );
    assert!(response_offset(4, 0, 0).is_none());
    assert!(blind_response_offset(0, 6, 0).is_none());
    assert_eq!(membership_commitment_point_offset(0, 0), Some(351));
    assert_eq!(membership_commitment_point_offset(1, 0), Some(12_642));
    assert_eq!(membership_commitment_point_offset(2, 0), Some(24_933));
    assert_eq!(membership_commitment_point_offset(5, 7), Some(74_310));
    assert_eq!(
        membership_section_offsets(RKG_ONE_STATEMENT_BYTES_V1),
        [908, 76_766, 25_242_590, 25_248_734, 25_248_766]
    );
    assert_eq!(
        membership_section_offsets(RKG_TWO_STATEMENT_BYTES_V1),
        [1_018, 76_876, 25_242_700, 25_248_844, 25_248_876]
    );
    assert_eq!(
        membership_section_offsets(GALOIS_STATEMENT_BYTES_V1),
        [798, 76_656, 25_242_480, 25_248_624, 25_248_656]
    );
}
#[test]
fn first_message_framing_is_domain_and_relation_separated() {
    let rns = DirectRelationRnsFirstMessageHasherV1::new(PersistentDirectRelationV1::RkgRoundOne);
    let points = [3_u8; RECONSTRUCTED_COMMITMENT_BYTES_V1];
    let commitment =
        commitment_first_message_digest(PersistentDirectRelationV1::RkgRoundOne, &points).unwrap();
    let other_relation =
        DirectRelationRnsFirstMessageHasherV1::new(PersistentDirectRelationV1::RkgRoundTwo);
    assert!(rns.finish().is_err());
    assert!(other_relation.finish().is_err());
    assert_ne!(commitment, [0; 32]);
    assert!(
        commitment_first_message_digest(PersistentDirectRelationV1::RkgRoundOne, &points[..1583])
            .is_err()
    );
    assert!(DirectRelationFirstMessageDigestsV1::new([[1; 32]; 4], [[2; 32]; 4]).is_ok());
    assert!(DirectRelationFirstMessageDigestsV1::new([[0; 32]; 4], [[2; 32]; 4]).is_err());
}
#[test]
fn rns_first_message_stream_rejects_wrong_row_limb_and_width() {
    let mut stream =
        DirectRelationRnsFirstMessageHasherV1::new(PersistentDirectRelationV1::RkgRoundOne);
    assert!(stream.absorb_limb(1, 0, &[]).is_err());
    assert!(stream.absorb_limb(0, 1, &[]).is_err());
    assert!(stream.absorb_limb(0, 0, &[0; 8]).is_err());
    assert!(stream.finish().is_err());
}
#[test]
fn rns_relation_rows_are_frozen_and_padding_rows_are_explicit() {
    assert_eq!(
        PersistentDirectRelationV1::RkgRoundOne.rns_row_tags(),
        ([1, 2, 0x84, 0x85, 0], 4)
    );
    assert_eq!(
        PersistentDirectRelationV1::RkgRoundTwo.rns_row_tags(),
        ([3, 0x82, 0x83, 0x85, 0], 4)
    );
    assert_eq!(
        PersistentDirectRelationV1::RkgNormalize.rns_row_tags(),
        ([4, 0x81, 0x82, 0x83, 0x84], 5)
    );
    assert_eq!(
        PersistentDirectRelationV1::Galois.rns_row_tags(),
        ([5, 0x81, 0x83, 0x84, 0x85], 5)
    );
}
#[test]
fn canonical_header_freezes_every_offset_and_reserved_byte() {
    for relation in [
        PersistentDirectRelationV1::RkgRoundOne,
        PersistentDirectRelationV1::RkgRoundTwo,
        PersistentDirectRelationV1::RkgNormalize,
        PersistentDirectRelationV1::Galois,
    ] {
        let expected =
            ExpectedDirectRelationStatementV1::layout_fixture(relation, [relation as u8; 32]);
        let header = canonical_header(&expected);
        assert_eq!(&header[..4], b"ZAXR");
        assert_eq!(header[4], 1);
        assert_eq!(header[5], relation as u8);
        assert_eq!(&header[44..48], &[0; 4]);
        assert_eq!(&header[48..80], &[relation as u8; 32]);
        assert_eq!(
            u32::from_be_bytes(header[40..44].try_into().unwrap()) as usize,
            HEADER_BYTES_V1 + relation.statement_bytes() + BODY_BYTES_V1
        );
    }
}
#[test]
fn strict_header_preflight_rejects_every_mutation_truncation_and_trailing_byte() {
    use super::predecode_v1::validate_header;
    let expected = ExpectedDirectRelationStatementV1::layout_fixture(
        PersistentDirectRelationV1::RkgRoundTwo,
        [9; 32],
    );
    let total = HEADER_BYTES_V1 + expected.bytes().len() + BODY_BYTES_V1;
    let mut wire = vec![0_u8; total];
    wire[..HEADER_BYTES_V1].copy_from_slice(&canonical_header(&expected));
    assert!(validate_header(&wire, &expected).is_ok());
    for offset in 0..HEADER_BYTES_V1 {
        wire[offset] ^= 1;
        assert!(
            validate_header(&wire, &expected).is_err(),
            "header mutation {offset} accepted"
        );
        wire[offset] ^= 1;
    }
    assert!(validate_header(&wire[..wire.len() - 1], &expected).is_err());
    wire.push(0);
    assert!(validate_header(&wire, &expected).is_err());
}
#[test]
fn response_and_scalar_canonical_boundaries_are_strict() {
    use super::predecode_v1::{validate_blind_scalar, validate_response_word};
    for value in [
        -super::super::RESPONSE_COEFFICIENT_BOUND_V1,
        super::super::RESPONSE_COEFFICIENT_BOUND_V1,
        0,
    ] {
        assert!(validate_response_word(&value.to_be_bytes()).is_ok());
    }
    for value in [
        -super::super::RESPONSE_COEFFICIENT_BOUND_V1 - 1,
        super::super::RESPONSE_COEFFICIENT_BOUND_V1 + 1,
        i64::MIN,
        i64::MAX,
    ] {
        assert!(validate_response_word(&value.to_be_bytes()).is_err());
    }
    assert!(validate_blind_scalar(&[0; 32]).is_ok());
    let mut maximum = VEGA_T256_SCALAR_MODULUS_BE_V1;
    for byte in maximum.iter_mut().rev() {
        if *byte != 0 {
            *byte -= 1;
            break;
        }
        *byte = 0xff;
    }
    assert!(validate_blind_scalar(&maximum).is_ok());
    assert!(validate_blind_scalar(&VEGA_T256_SCALAR_MODULUS_BE_V1).is_err());
    assert!(validate_blind_scalar(&[0xff; 32]).is_err());
}
fn canonical_six_frame_membership_fixture() -> Vec<u8> {
    let mut membership = Vec::with_capacity(MEMBERSHIP_BYTES_V1);
    for slot in 0..2 {
        membership.extend_from_slice(&canonical_membership_syntax_wire_fixture_for_test::<
            DirectRelationBoundOneMembershipRoleV1,
        >(&[slot as u8 + 1], slot * 9));
    }
    for slot in 2..6 {
        membership.extend_from_slice(&canonical_membership_syntax_wire_fixture_for_test::<
            DirectRelationBoundTwoMembershipRoleV1,
        >(&[slot as u8 + 1], slot * 9));
    }
    assert_eq!(membership.len(), MEMBERSHIP_BYTES_V1);
    membership
}
fn assert_preflight_rejects_before_owned_materialization(membership: &[u8]) {
    use super::predecode_v1::{
        owned_membership_materializations_for_test,
        preflight_and_materialize_membership_frames_for_test,
        reset_owned_membership_materializations_for_test,
    };
    reset_owned_membership_materializations_for_test();
    let result = preflight_and_materialize_membership_frames_for_test(membership);
    assert_eq!(result, Err(ZkAmsMkheErrorV1::InvalidWireEncoding));
    assert_eq!(owned_membership_materializations_for_test(), 0);
}
#[test]
fn six_frame_preflight_is_the_only_owned_materialization_gate() {
    use super::predecode_v1::{
        owned_membership_materializations_for_test,
        preflight_and_materialize_membership_frames_for_test,
        reset_owned_membership_materializations_for_test,
    };
    let membership = canonical_six_frame_membership_fixture();
    reset_owned_membership_materializations_for_test();
    preflight_and_materialize_membership_frames_for_test(&membership)
        .expect("canonical six-frame fixture");
    assert_eq!(owned_membership_materializations_for_test(), 1);
}
#[test]
fn early_inner_point_malleability_is_rejected_before_owned_materialization() {
    const INNER_PROOF_OFFSET: usize = 47;
    let mut membership = canonical_six_frame_membership_fixture();
    let point = MEMBERSHIP_HEADER_BYTES_V1 + INNER_PROOF_OFFSET;
    membership[point..point + 33].fill(0);
    membership[point] = 0x20;
    repair_membership_proof_set_digest_for_test::<DirectRelationBoundOneMembershipRoleV1>(
        &mut membership[..DIRECT_BOUND_ONE_MEMBERSHIP_BYTES_V1],
    );
    assert_preflight_rejects_before_owned_materialization(&membership);
}
#[test]
fn late_inner_scalar_malleability_is_rejected_before_owned_materialization() {
    const INNER_PROOF_OFFSET: usize = 47;
    const BOUND_TWO_PROOF_BYTES: usize = 1_513;
    let mut membership = canonical_six_frame_membership_fixture();
    let frame_start = MEMBERSHIP_FRAME_OFFSETS_V1[5];
    let chunk_start = frame_start + MEMBERSHIP_HEADER_BYTES_V1 + 7 * BOUND_TWO_CHUNK_WIRE_BYTES_V1;
    let scalar = chunk_start + INNER_PROOF_OFFSET + BOUND_TWO_PROOF_BYTES - 32;
    membership[scalar..scalar + 32].fill(0xff);
    let frame_end = frame_start + DIRECT_BOUND_TWO_MEMBERSHIP_BYTES_V1;
    repair_membership_proof_set_digest_for_test::<DirectRelationBoundTwoMembershipRoleV1>(
        &mut membership[frame_start..frame_end],
    );
    assert_preflight_rejects_before_owned_materialization(&membership);
}
#[test]
fn predecode_remains_borrowed_move_only_and_release_gates_stay_closed() {
    let source = include_str!("predecode_v1.rs");
    assert!(!source.contains("Vec::"));
    assert!(!source.contains(".reserve("));
    assert!(!source.contains("derive(Clone"));
    assert!(source.contains("struct PreflightedDirectRelationMembershipFramesV1"));
    assert!(!source.contains("pub(super) struct PreflightedDirectRelationMembershipFramesV1"));
    assert!(source.contains("preflighted.materialize()?"));
    assert!(source.contains("responses: &'a [u8]"));
    assert!(source.contains("capability: VerifiedPersistentWitnessDirectRelationUseV1"));
    let active = include_str!("../../active_exact_binding.rs");
    assert!(active.contains("let canonical_complete_wire_certified = false;"));
    let verifier = active
        .split("pub(super) fn verify_and_consume_direct_relation_use_v1")
        .nth(1)
        .and_then(|tail| tail.split("/// Sole production minting boundary").next())
        .expect("final direct verifier body");
    assert!(verifier.contains("capability.validate()?;"));
    assert!(verifier.contains("Err(ZkAmsMkheErrorV1::ReleaseUnavailable)"));
    assert!(!verifier.contains("predecode_direct_relation_proof_v1"));
}
