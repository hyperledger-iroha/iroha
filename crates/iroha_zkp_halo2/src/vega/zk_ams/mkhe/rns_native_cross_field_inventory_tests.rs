use super::*;
use crate::vega::derive_t256_generators_v1;

fn point_bytes_v1() -> [u8; POINT_BYTES_V1] {
    let point =
        derive_t256_generators_v1(b"rns-native-cross-field-inventory-test", 1).expect("point")[0];
    let mut encoded = [0_u8; POINT_BYTES_V1];
    point
        .write_non_identity_wire_bytes_ref(&mut encoded)
        .expect("canonical point");
    encoded
}

fn canonical_inventory_v1() -> Vec<u8> {
    let point = point_bytes_v1();
    let mut inventory = Vec::with_capacity(INVENTORY_BYTES_V1);
    for _ in 0..INVENTORY_POINTS_V1 {
        inventory.extend_from_slice(&point);
    }
    assert_eq!(inventory.len(), INVENTORY_BYTES_V1);
    inventory
}

fn canonical_wire_v1(prior_context_digest: [u8; DIGEST_BYTES_V1], continuation: &[u8]) -> Vec<u8> {
    let inventory = canonical_inventory_v1();
    let inventory_root =
        canonical_inventory_root_v1(prior_context_digest, &inventory).expect("inventory root");
    let continuation_digest =
        canonical_continuation_digest_v1(prior_context_digest, inventory_root, continuation)
            .expect("continuation digest");
    let total = HEADER_BYTES_V1 + inventory.len() + continuation.len() + CODEC_DIGEST_BYTES_V1;
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(&INVENTORY_MAGIC_V1);
    bytes.push(INVENTORY_VERSION_V1);
    bytes.push(INVENTORY_FLAGS_V1);
    bytes.extend_from_slice(&(HEADER_BYTES_V1 as u16).to_be_bytes());
    bytes.extend_from_slice(&(total as u32).to_be_bytes());
    bytes.extend_from_slice(&[
        ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u8,
        REPETITIONS_V1 as u8,
        RECORDS_V1 as u8,
        BLOCKS_PER_RECORD_V1 as u8,
        ZK_AMS_MKHE_RNS_NATIVE_RADIX_LOG2_V1,
        RADIX_DIGITS_V1 as u8,
        Q_MASK_DIGITS_V1 as u8,
        POINT_BYTES_V1 as u8,
    ]);
    for count in [
        COMPARATOR_POINTS_V1,
        SMALL_SOURCE_POINTS_V1,
        Q_MASK_POINTS_V1,
        INVENTORY_POINTS_V1,
    ] {
        bytes.extend_from_slice(&(count as u32).to_be_bytes());
    }
    bytes.extend_from_slice(&prior_context_digest);
    bytes.extend_from_slice(&inventory_root);
    bytes.extend_from_slice(&continuation_digest);
    bytes.extend_from_slice(&(continuation.len() as u32).to_be_bytes());
    assert_eq!(bytes.len(), HEADER_BYTES_V1);
    bytes.extend_from_slice(&inventory);
    bytes.extend_from_slice(continuation);
    let codec_digest = codec_digest_v1(&bytes);
    bytes.extend_from_slice(&codec_digest);
    assert_eq!(bytes.len(), total);
    bytes
}

#[test]
fn forty_limb_inventory_geometry_has_one_exact_role_order() {
    assert_eq!(
        inventory_coordinate_v1(0),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorDifferenceTop,
            owner: 0,
            column: 0,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(COMPARATOR_GROUPS_V1 - 1),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorDifferenceTop,
            owner: COMPARATOR_GROUPS_V1 - 1,
            column: 0,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(COMPARATOR_GROUPS_V1),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorSumTop,
            owner: 0,
            column: 0,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(COMPARATOR_TOP_POINTS_V1),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorDifferenceDigit,
            owner: 0,
            column: 0,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(COMPARATOR_TOP_POINTS_V1 + 17),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorMixedTop,
            owner: 0,
            column: 0,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(COMPARATOR_TOP_POINTS_V1 + 18),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorBorrow,
            owner: 0,
            column: 0,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(
            COMPARATOR_TOP_POINTS_V1
                + (COMPARATOR_GROUPS_V1 - 1) * COMPARATOR_POINTS_PER_GROUP_V1
                + 35,
        ),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorBorrow,
            owner: COMPARATOR_GROUPS_V1 - 1,
            column: 17,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(COMPARATOR_POINTS_V1 - 1),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::ComparatorDifferenceInverse,
            owner: COMPARATOR_GROUPS_V1 - 1,
            column: 16,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(COMPARATOR_POINTS_V1),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::SmallSigned,
            owner: 0,
            column: 0,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(COMPARATOR_POINTS_V1 + SMALL_SOURCE_POINTS_V1),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::QMaskDigit,
            owner: 0,
            column: 0,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(INVENTORY_POINTS_V1 - 1),
        Ok(InventoryCoordinateV1 {
            role: InventoryPointRoleV1::QMaskComplementInverse,
            owner: Q_MASK_BLOCKS_V1 - 1,
            column: 3,
        })
    );
    assert_eq!(
        inventory_coordinate_v1(INVENTORY_POINTS_V1),
        Err(RnsNativeCrossFieldInventoryErrorV1::InvalidGeometry)
    );
}

#[test]
fn statement8_small_source_accessor_uses_exact_raw_roles_and_derived_positive() {
    let mut inventory = canonical_inventory_v1();
    let signed = Point::from_non_identity_wire_bytes_exact(&point_bytes_v1())
        .expect("canonical signed point");
    let commitments = small_source_product_commitments_v1(&inventory, 0)
        .expect("first small-source commitment tuple");
    assert!(commitments.signed == signed);
    assert!(commitments.negative_magnitude == signed);
    assert!(commitments.positive == signed + signed);
    assert!(small_source_product_commitments_v1(&inventory, SMALL_SOURCE_BLOCKS_V1 - 1).is_some());
    assert!(small_source_product_commitments_v1(&inventory, SMALL_SOURCE_BLOCKS_V1).is_none());
    assert!(small_source_product_commitments_v1(&inventory[..inventory.len() - 1], 0).is_none());

    let mut opposite = [0_u8; POINT_BYTES_V1];
    (-signed)
        .write_non_identity_wire_bytes_ref(&mut opposite)
        .expect("canonical opposite point");
    let negative_ordinal = COMPARATOR_POINTS_V1 + 1;
    let negative_offset = negative_ordinal * POINT_BYTES_V1;
    inventory[negative_offset..negative_offset + POINT_BYTES_V1].copy_from_slice(&opposite);
    assert!(small_source_product_commitments_v1(&inventory, 0).is_none());
}

#[test]
fn statement4_subtraction_accessor_selects_only_delta_and_beta_zero_through_sixteen() {
    let mut inventory = canonical_inventory_v1();
    let points = derive_t256_generators_v1(b"rns-native-statement4-subtraction-accessor", 39)
        .expect("statement-4 accessor points");
    let write_point = |inventory: &mut [u8], ordinal: usize, point: Point| {
        let mut encoded = [0_u8; POINT_BYTES_V1];
        point
            .write_non_identity_wire_bytes_ref(&mut encoded)
            .expect("canonical statement-4 point");
        let offset = ordinal * POINT_BYTES_V1;
        inventory[offset..offset + POINT_BYTES_V1].copy_from_slice(&encoded);
    };

    let first = COMPARATOR_TOP_POINTS_V1;
    for column in 0..COMPARATOR_SUBTRACTION_DIGITS_V1 {
        write_point(&mut inventory, first + column, points[column]);
        write_point(
            &mut inventory,
            first + 18 + column,
            points[COMPARATOR_SUBTRACTION_DIGITS_V1 + column],
        );
    }
    for (local, point) in [(17, points[34]), (35, points[35]), (36, points[36])] {
        write_point(&mut inventory, first + local, point);
    }

    let first_group = comparator_subtraction_commitments_v1(&inventory, 0)
        .expect("first comparator subtraction tuple");
    assert_eq!(
        first_group.difference_digits.as_slice(),
        &points[..COMPARATOR_SUBTRACTION_DIGITS_V1]
    );
    assert_eq!(
        first_group.borrows.as_slice(),
        &points[COMPARATOR_SUBTRACTION_DIGITS_V1..2 * COMPARATOR_SUBTRACTION_DIGITS_V1]
    );
    for excluded in &points[34..37] {
        assert!(!first_group.difference_digits.contains(excluded));
        assert!(!first_group.borrows.contains(excluded));
    }

    let last =
        COMPARATOR_TOP_POINTS_V1 + (COMPARATOR_GROUPS_V1 - 1) * COMPARATOR_POINTS_PER_GROUP_V1;
    write_point(&mut inventory, last + 16, points[37]);
    write_point(&mut inventory, last + 18 + 16, points[38]);
    let last_group = comparator_subtraction_commitments_v1(&inventory, COMPARATOR_GROUPS_V1 - 1)
        .expect("last comparator subtraction tuple");
    assert_eq!(last_group.difference_digits[16], points[37]);
    assert_eq!(last_group.borrows[16], points[38]);
    assert!(comparator_subtraction_commitments_v1(&inventory, COMPARATOR_GROUPS_V1).is_none());
    assert!(comparator_subtraction_commitments_v1(&inventory[..inventory.len() - 1], 0).is_none());
}

#[test]
fn q_mask_linear_accessor_selects_digits_and_complements_but_not_inverses() {
    let mut inventory = canonical_inventory_v1();
    let points = derive_t256_generators_v1(b"rns-native-q-mask-linear-accessor", 8)
        .expect("q-mask accessor points");
    let first = COMPARATOR_POINTS_V1 + SMALL_SOURCE_POINTS_V1;
    for (local, point) in [
        (0, points[0]),
        (1, points[1]),
        (2, points[2]),
        (3, points[3]),
        (8, points[4]),
        (9, points[5]),
        (10, points[6]),
        (11, points[7]),
    ] {
        let mut encoded = [0_u8; POINT_BYTES_V1];
        point
            .write_non_identity_wire_bytes_ref(&mut encoded)
            .expect("canonical q-mask point");
        let offset = (first + local) * POINT_BYTES_V1;
        inventory[offset..offset + POINT_BYTES_V1].copy_from_slice(&encoded);
    }
    let commitments =
        q_mask_linear_commitments_v1(&inventory, 0).expect("first q-mask linear tuple");
    assert_eq!(commitments.digits.as_slice(), &points[..4]);
    assert_eq!(commitments.complement_digits.as_slice(), &points[4..]);
    assert!(q_mask_linear_commitments_v1(&inventory, Q_MASK_BLOCKS_V1 - 1).is_some());
    assert!(q_mask_linear_commitments_v1(&inventory, Q_MASK_BLOCKS_V1).is_none());
    assert!(q_mask_linear_commitments_v1(&inventory[..inventory.len() - 1], 0).is_none());
}

#[test]
fn global_lookup_inverse_accessors_alias_exact_post_z_roles_and_boundaries() {
    let mut inventory = canonical_inventory_v1();
    let points = derive_t256_generators_v1(b"rns-native-global-lookup-inverse-accessors", 25)
        .expect("global lookup inverse accessor points");
    let write_point = |inventory: &mut [u8], ordinal: usize, point: Point| {
        let mut encoded = [0_u8; POINT_BYTES_V1];
        point
            .write_non_identity_wire_bytes_ref(&mut encoded)
            .expect("canonical global lookup inverse point");
        let offset = ordinal * POINT_BYTES_V1;
        inventory[offset..offset + POINT_BYTES_V1].copy_from_slice(&encoded);
    };

    let comparator_first = COMPARATOR_TOP_POINTS_V1;
    let comparator_last =
        COMPARATOR_TOP_POINTS_V1 + (COMPARATOR_GROUPS_V1 - 1) * COMPARATOR_POINTS_PER_GROUP_V1;
    write_point(&mut inventory, comparator_first + 36, points[0]);
    write_point(&mut inventory, comparator_last + 52, points[1]);
    write_point(&mut inventory, comparator_first + 35, points[24]);
    assert_eq!(
        comparator_difference_inverse_v1(&inventory, 0, 0),
        Some(points[0])
    );
    assert_eq!(
        comparator_difference_inverse_v1(
            &inventory,
            COMPARATOR_GROUPS_V1 - 1,
            COMPARATOR_SUBTRACTION_DIGITS_V1 - 1,
        ),
        Some(points[1])
    );
    assert_eq!(
        comparator_difference_inverse_v1(&inventory, COMPARATOR_GROUPS_V1, 0),
        None
    );
    assert_eq!(
        comparator_difference_inverse_v1(&inventory, 0, COMPARATOR_SUBTRACTION_DIGITS_V1),
        None
    );

    let small_first = COMPARATOR_POINTS_V1;
    let small_last =
        COMPARATOR_POINTS_V1 + (SMALL_SOURCE_BLOCKS_V1 - 1) * SMALL_SOURCE_POINTS_PER_BLOCK_V1;
    write_point(&mut inventory, small_first, points[24]);
    write_point(&mut inventory, small_first + 2, points[2]);
    write_point(&mut inventory, small_first + 3, points[3]);
    write_point(&mut inventory, small_last + 2, points[4]);
    write_point(&mut inventory, small_last + 3, points[5]);
    assert_eq!(
        small_source_lookup_inverses_v1(&inventory, 0),
        Some((points[2], points[3]))
    );
    assert_eq!(
        small_source_lookup_inverses_v1(&inventory, SMALL_SOURCE_BLOCKS_V1 - 1),
        Some((points[4], points[5]))
    );
    assert_eq!(
        small_source_lookup_inverses_v1(&inventory, SMALL_SOURCE_BLOCKS_V1),
        None
    );

    let q_mask_first = COMPARATOR_POINTS_V1 + SMALL_SOURCE_POINTS_V1;
    for column in 0..Q_MASK_DIGITS_V1 {
        write_point(&mut inventory, q_mask_first + column, points[24]);
        write_point(
            &mut inventory,
            q_mask_first + 4 + column,
            points[6 + column],
        );
        write_point(
            &mut inventory,
            q_mask_first + 12 + column,
            points[10 + column],
        );
    }
    let q_mask_last = q_mask_first + (Q_MASK_BLOCKS_V1 - 1) * Q_MASK_POINTS_PER_BLOCK_V1;
    for column in 0..Q_MASK_DIGITS_V1 {
        write_point(
            &mut inventory,
            q_mask_last + 4 + column,
            points[14 + column],
        );
        write_point(
            &mut inventory,
            q_mask_last + 12 + column,
            points[18 + column],
        );
    }
    let first_q_mask = q_mask_lookup_inverses_v1(&inventory, 0).expect("first q-mask inverses");
    assert_eq!(first_q_mask.digit_inverses.as_slice(), &points[6..10]);
    assert_eq!(first_q_mask.complement_inverses.as_slice(), &points[10..14]);
    assert!(!first_q_mask.digit_inverses.contains(&points[24]));
    assert!(!first_q_mask.complement_inverses.contains(&points[24]));
    let last_q_mask =
        q_mask_lookup_inverses_v1(&inventory, Q_MASK_BLOCKS_V1 - 1).expect("last q-mask inverses");
    assert_eq!(last_q_mask.digit_inverses.as_slice(), &points[14..18]);
    assert_eq!(last_q_mask.complement_inverses.as_slice(), &points[18..22]);
    assert!(q_mask_lookup_inverses_v1(&inventory, Q_MASK_BLOCKS_V1).is_none());

    let short = &inventory[..inventory.len() - 1];
    assert!(comparator_difference_inverse_v1(short, 0, 0).is_none());
    assert!(small_source_lookup_inverses_v1(short, 0).is_none());
    assert!(q_mask_lookup_inverses_v1(short, 0).is_none());
}

#[test]
fn authenticated_qpcs_grid_is_exact_canonical_and_limb_major() {
    let mut bytes = vec![0_u8; QPCS_EVALUATION_BYTES_V1];
    let relation = (ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 - 1) * REPETITIONS_V1 + (REPETITIONS_V1 - 1);
    let offset = relation * 16;
    let modulus = ZK_AMS_MKHE_RNS_NATIVE_MODULI_V1[ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 - 1];
    bytes[offset..offset + 8].copy_from_slice(&(modulus - 1).to_be_bytes());
    bytes[offset + 8..offset + 16].copy_from_slice(&(modulus - 2).to_be_bytes());
    let grid = CanonicalQpcsEvaluationGridV1::from_authenticated_bytes_v1(&bytes).expect("grid");
    assert_eq!(
        grid.get_v1(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 - 1, REPETITIONS_V1 - 1),
        Some(CanonicalQpcsEvaluationV1 {
            product: modulus - 1,
            opening_quotient: modulus - 2,
        })
    );
    assert_eq!(grid.get_v1(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1, 0), None);
    assert_eq!(grid.get_v1(0, REPETITIONS_V1), None);

    bytes[offset..offset + 8].copy_from_slice(&modulus.to_be_bytes());
    assert_eq!(
        CanonicalQpcsEvaluationGridV1::from_authenticated_bytes_v1(&bytes).map(|_| ()),
        Err(RnsNativeCrossFieldInventoryErrorV1::InvalidQpcsEvaluation)
    );
    assert_eq!(
        CanonicalQpcsEvaluationGridV1::from_authenticated_bytes_v1(&bytes[..bytes.len() - 1])
            .map(|_| ()),
        Err(RnsNativeCrossFieldInventoryErrorV1::InvalidQpcsEvaluation)
    );
}

#[test]
fn proof_body_codec_is_exact_capped_canonical_and_context_bound() {
    let context = [0x42; DIGEST_BYTES_V1];
    let continuation = b"future-streaming-sparse-product-proof";
    let bytes = canonical_wire_v1(context, continuation);
    let view = CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(&bytes, context)
        .expect("canonical proof body");
    assert_eq!(view.prior_context_digest, context);
    assert_eq!(view.inventory.len(), INVENTORY_BYTES_V1);
    assert_eq!(view.continuation, continuation);
    assert_ne!(view.inventory_root, [0; DIGEST_BYTES_V1]);
    assert_ne!(view.continuation_digest, [0; DIGEST_BYTES_V1]);
    assert_ne!(view.codec_digest, [0; DIGEST_BYTES_V1]);

    assert_eq!(
        CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(&bytes, [0x43; 32])
            .map(|_| ()),
        Err(RnsNativeCrossFieldInventoryErrorV1::InvalidHeader)
    );
    assert!(
        CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(
            &bytes[..bytes.len() - 1],
            context
        )
        .is_err()
    );
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(
        CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(&trailing, context).is_err()
    );

    let oversized = vec![0_u8; PROOF_MAX_BYTES_V1 + 1];
    assert_eq!(
        CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(&oversized, context)
            .map(|_| ()),
        Err(RnsNativeCrossFieldInventoryErrorV1::ProofCapExceeded)
    );
}

#[test]
fn proof_body_rejects_geometry_point_and_continuation_substitution() {
    let context = [0x52; DIGEST_BYTES_V1];
    let bytes = canonical_wire_v1(context, b"continuation");

    let mut geometry = bytes.clone();
    geometry[12] = 39;
    assert_eq!(
        CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(&geometry, context)
            .map(|_| ()),
        Err(RnsNativeCrossFieldInventoryErrorV1::InvalidGeometry)
    );

    let mut point = bytes.clone();
    point[HEADER_BYTES_V1..HEADER_BYTES_V1 + POINT_BYTES_V1].fill(0);
    assert_eq!(
        CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(&point, context).map(|_| ()),
        Err(RnsNativeCrossFieldInventoryErrorV1::InvalidPoint)
    );

    let mut continuation = bytes.clone();
    let continuation_offset = HEADER_BYTES_V1 + INVENTORY_BYTES_V1;
    continuation[continuation_offset] ^= 1;
    assert_eq!(
        CrossFieldInventoryProofViewV1::from_canonical_bytes_exact_v1(&continuation, context)
            .map(|_| ()),
        Err(RnsNativeCrossFieldInventoryErrorV1::InvalidIntegrity)
    );
}

#[test]
fn production_boundary_is_private_move_only_non_authorizing_and_fail_closed() {
    let source = include_str!("rns_native_cross_field_inventory.rs");
    let declaration = "pub(super) struct RnsNativeCrossFieldInventoryPrerequisiteV1";
    let declaration_offset = source.find(declaration).expect("stage declaration");
    let attributes = source[..declaration_offset]
        .rsplit_once("\n\n")
        .map_or(&source[..declaration_offset], |(_, block)| block);
    let stage = source[declaration_offset + declaration.len()..]
        .split_once("\n}\n")
        .map(|(body, _)| body)
        .expect("stage body");
    assert!(!attributes.contains("derive(Clone"));
    assert!(!attributes.contains("derive(Copy"));
    assert!(!stage.contains("pub fn"));
    assert!(!stage.contains("Verified"));
    assert!(!stage.contains("Release"));
    assert!(source.contains("Point::from_non_identity_wire_bytes_exact"));
    assert!(
        source.contains("COMPARATOR_BOOLEAN_DISJOINT_PRODUCT_ARGUMENT_AVAILABLE_V1: bool = true")
    );
    assert!(source.contains("RANGE_AND_CARRY_RELATIONS_VERIFIED_V1: bool = false"));
    assert!(source.contains("CANONICAL_Q_MASK_RELATIONS_VERIFIED_V1: bool = false"));
    assert!(source.contains("GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1: bool = false"));
    assert!(stage.contains("terminal_transcript_digest: [u8; DIGEST_BYTES_V1]"));
    assert!(source.contains("terminal_transcript_digest: transcript.transcript_digest()"));
    assert!(source.contains("pub(super) const fn terminal_transcript_digest_v1(&self)"));

    let parent = include_str!("../mkhe.rs");
    assert_eq!(
        parent
            .matches("mod rns_native_cross_field_inventory;")
            .count(),
        1
    );
    assert!(!parent.contains("pub use rns_native_cross_field_inventory"));
    assert!(!parent.contains("phase23_rns_link"));

    let composite = include_str!("rns_native_composite_verifier.rs");
    assert!(composite.contains("StageUnavailable"));
    assert!(composite.contains("CrossFieldGlobalLookup"));
    let qpcs = include_str!("rns_native_qpcs_fri_complete.rs");
    assert!(qpcs.contains("pub(super) fn authenticate_rns_native_qpcs_fri_complete_v1"));
    assert!(qpcs.contains("Successful verification remains non-authorizing"));
}
