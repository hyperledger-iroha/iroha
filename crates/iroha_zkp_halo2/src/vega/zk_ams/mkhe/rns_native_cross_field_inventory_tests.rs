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
    assert_eq!(commitments.signed, signed);
    assert_eq!(commitments.negative_magnitude, signed);
    assert_eq!(commitments.positive, signed + signed);
    assert!(
        small_source_product_commitments_v1(&inventory, SMALL_SOURCE_BLOCKS_V1 - 1).is_some()
    );
    assert!(small_source_product_commitments_v1(&inventory, SMALL_SOURCE_BLOCKS_V1).is_none());
    assert!(
        small_source_product_commitments_v1(&inventory[..inventory.len() - 1], 0).is_none()
    );

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

    let parent = include_str!("../mkhe.rs");
    assert_eq!(
        parent
            .matches("mod rns_native_cross_field_inventory;")
            .count(),
        1
    );
    assert!(!parent.contains("pub use rns_native_cross_field_inventory"));

    let retired = include_str!("phase23_rns_link_cross_field_v2.rs");
    assert!(retired.contains("const LIMBS_V2: usize = 38;"));
    assert!(retired.contains("SOURCE_SET_BOUND_V2: bool = false"));
    assert!(retired.contains("CANONICAL_Q_MASK_SET_BOUND_V2: bool = false"));
    let vector_backend = include_str!(
        "global_lookup_statement_v1/vector_arithmetic_plane_openings_v1/vector_arithmetic_proof_codec_v2.rs"
    );
    assert!(vector_backend.contains("VECTOR_ARITHMETIC_STREAMING_BACKEND_READY_V2: bool = false"));

    let composite = include_str!("rns_native_composite_verifier.rs");
    assert!(composite.contains("StageUnavailable"));
    assert!(composite.contains("CrossFieldGlobalLookup"));
}
