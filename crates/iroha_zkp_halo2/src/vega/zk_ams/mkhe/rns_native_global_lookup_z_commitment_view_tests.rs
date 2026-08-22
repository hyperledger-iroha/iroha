use std::sync::OnceLock;

use super::*;
use crate::vega::derive_t256_generators_v1;

fn test_points_v1() -> &'static [Point; 16] {
    static POINTS: OnceLock<[Point; 16]> = OnceLock::new();
    POINTS.get_or_init(|| {
        let points =
            derive_t256_generators_v1(b"rns-native-global-lookup-z-commitment-view-tests", 16)
                .expect("global lookup rendezvous test points");
        core::array::from_fn(|index| points[index])
    })
}

fn test_point_v1(index: usize) -> Point {
    test_points_v1()[index % test_points_v1().len()]
}

fn pre_global_capability_v1() -> &'static ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1 {
    static CAPABILITY: OnceLock<ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1> = OnceLock::new();
    CAPABILITY.get_or_init(|| {
        ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1::test_fixture_v1(
            [0xa7; DIGEST_BYTES_V1],
            [0xb8; DIGEST_BYTES_V1],
        )
        .expect("distinct pre-global capability fixture")
    })
}

fn encoded_point_v1(point: Point) -> [u8; POINT_BYTES_V1] {
    encode_point_v1(point).expect("canonical nonidentity test point")
}

fn existing_inverse_bytes_v1() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES
        .get_or_init(|| {
            let mut bytes = Vec::with_capacity(EXISTING_INVERSE_BYTES_V1);
            for ordinal in 0..EXISTING_LOW_PER_ROLE_V1 {
                bytes.extend_from_slice(&encoded_point_v1(test_point_v1(2 + ordinal % 2)));
            }
            for ordinal in 0..EXISTING_LOW_PER_ROLE_V1 {
                bytes.extend_from_slice(&encoded_point_v1(test_point_v1(4 + ordinal % 2)));
            }
            assert_eq!(bytes.len(), EXISTING_INVERSE_BYTES_V1);
            bytes
        })
        .as_slice()
}

fn canonical_wire_v1(residual: &[u8]) -> Vec<u8> {
    assert!(!residual.is_empty());
    assert!(residual.len() <= RNS_NATIVE_GLOBAL_LOOKUP_POST_Z_RESIDUAL_MAX_BYTES_V1);
    let total = HEADER_BYTES_V1
        + PRE_Z_POINT_BYTES_V1
        + EXISTING_INVERSE_BYTES_V1
        + residual.len()
        + CODEC_DIGEST_BYTES_V1;
    let mut wire = Vec::with_capacity(total);
    wire.extend_from_slice(&MAGIC_V1);
    wire.push(VERSION_V1);
    wire.push(FLAGS_V1);
    wire.extend_from_slice(&(HEADER_BYTES_V1 as u16).to_be_bytes());
    wire.extend_from_slice(&(total as u32).to_be_bytes());
    wire.extend_from_slice(&(GROUPS_V1 as u16).to_be_bytes());
    wire.extend_from_slice(&(ZK_AMS_MKHE_RNS_NATIVE_LIMBS_V1 as u16).to_be_bytes());
    wire.extend_from_slice(&[
        LOW_DIGITS_V1 as u8,
        BORROWS_V1 as u8,
        REPETITIONS_V1 as u8,
        PRE_Z_SCALAR_COMMITMENTS_V1 as u8,
        POINT_BYTES_V1 as u8,
        Z_CHALLENGE_ORDINAL_V1,
        MAX_CHALLENGE_ATTEMPTS_V1,
        Q_MASK_DIGITS_V1 as u8,
    ]);
    for value in [
        LOOKUP_TABLE_VALUES_V1 as usize,
        SOURCE_COMMITMENTS_V1,
        PRE_Z_PHYSICAL_COMMITMENTS_V1,
        EXISTING_INVERSE_POINTS_V1,
        ADDED_INVERSE_POINTS_V1,
        GLOBAL_INVERSE_POINTS_V1,
        Q_MASK_BLOCKS_V1,
        residual.len(),
    ] {
        wire.extend_from_slice(&(value as u32).to_be_bytes());
    }
    assert_eq!(wire.len(), HEADER_BYTES_V1);
    wire.extend_from_slice(&encoded_point_v1(test_point_v1(0)));
    wire.extend_from_slice(&encoded_point_v1(test_point_v1(1)));
    wire.extend_from_slice(&encoded_point_v1(test_point_v1(2)));
    wire.extend_from_slice(existing_inverse_bytes_v1());
    wire.extend_from_slice(residual);
    let codec = codec_digest_v1(&wire);
    wire.extend_from_slice(&codec);
    assert_eq!(wire.len(), total);
    wire
}

fn refresh_codec_v1(wire: &mut [u8]) {
    let codec_offset = wire.len() - CODEC_DIGEST_BYTES_V1;
    let digest = codec_digest_v1(&wire[..codec_offset]);
    wire[codec_offset..].copy_from_slice(&digest);
}

fn role_root_with_v1(purpose: PhysicalPurposeV1, point: Point) -> [u8; DIGEST_BYTES_V1] {
    let mut builder = RoleRootBuilderV1::new_v1(purpose);
    for _ in 0..purpose.count_v1() {
        builder.absorb_v1(point).expect("role point");
    }
    builder.finish_v1().expect("complete role root")
}

fn baseline_pre_z_roots_v1() -> PreZRoleRootsV1 {
    static ROOTS: OnceLock<PreZRoleRootsV1> = OnceLock::new();
    *ROOTS.get_or_init(|| {
        let roots = core::array::from_fn(|index| {
            let purpose = PRE_Z_POINT_PURPOSES_V1[index];
            let point = match purpose {
                PhysicalPurposeV1::Multiplicity => test_point_v1(0),
                PhysicalPurposeV1::GlobalLookupSumcheckMask => test_point_v1(1),
                PhysicalPurposeV1::InverseProductMask => test_point_v1(2),
                _ => test_point_v1(2 + index),
            };
            role_root_with_v1(purpose, point)
        });
        let roots = PreZRoleRootsV1 { roots };
        roots.validate_v1().expect("distinct pre-z role roots");
        roots
    })
}

fn pre_z_roots_for_v1(
    multiplicity: Point,
    global_lookup_sumcheck_mask: Point,
    inverse_product_mask: Point,
) -> PreZRoleRootsV1 {
    let mut roots = baseline_pre_z_roots_v1();
    roots.roots[PRE_Z_POINT_PURPOSES_V1
        .iter()
        .position(|purpose| *purpose == PhysicalPurposeV1::Multiplicity)
        .expect("multiplicity role")] =
        role_root_with_v1(PhysicalPurposeV1::Multiplicity, multiplicity);
    roots.roots[PRE_Z_POINT_PURPOSES_V1
        .iter()
        .position(|purpose| *purpose == PhysicalPurposeV1::GlobalLookupSumcheckMask)
        .expect("global-lookup-sumcheck-mask role")] = role_root_with_v1(
        PhysicalPurposeV1::GlobalLookupSumcheckMask,
        global_lookup_sumcheck_mask,
    );
    roots.roots[PRE_Z_POINT_PURPOSES_V1
        .iter()
        .position(|purpose| *purpose == PhysicalPurposeV1::InverseProductMask)
        .expect("inverse-product-mask role")] =
        role_root_with_v1(PhysicalPurposeV1::InverseProductMask, inverse_product_mask);
    roots.validate_v1().expect("valid pre-z role roots");
    roots
}

fn safe_context_v1() -> PreZSafeContextV1 {
    let context = PreZSafeContextV1 {
        fixed_axes_digest: fixed_axes_digest_v1(),
        source_binding_digest: [0xa1; DIGEST_BYTES_V1],
        qpcs_binding_digest: [0xb2; DIGEST_BYTES_V1],
    };
    context.validate_v1().expect("distinct safe context");
    context
}

fn synthetic_added_at_v1(purpose: PhysicalPurposeV1, ordinal: usize) -> Option<Point> {
    let base = match purpose {
        PhysicalPurposeV1::ComparatorDifferenceInverse => 6,
        PhysicalPurposeV1::SmallPositiveInverse => 8,
        PhysicalPurposeV1::SmallNegativeInverse => 10,
        PhysicalPurposeV1::QMaskDigitInverse => 12,
        PhysicalPurposeV1::QMaskComplementInverse => 14,
        _ => return None,
    };
    Some(test_point_v1(base + ordinal % 2))
}

fn z_bytes_v1(live: &PreZChallengeLiveV1) -> [u8; DIGEST_BYTES_V1] {
    live.z.as_ref().to_le_bytes()
}

#[test]
fn sole_z_and_post_z_view_roundtrip_exact_geometry_and_cap() {
    let wire = canonical_wire_v1(b"inverse-products-and-global-lookup-follow");
    let view =
        PreZEnvelopeViewV1::from_canonical_prefix_v1(&wire).expect("canonical pre-z envelope");
    assert_eq!(view.existing_inverse_bytes.len(), EXISTING_INVERSE_BYTES_V1);
    assert_eq!(view.residual, b"inverse-products-and-global-lookup-follow");
    assert_eq!(view.multiplicity, test_point_v1(0));
    assert_eq!(view.sumcheck_mask, test_point_v1(1));
    assert_eq!(view.inverse_product_mask, test_point_v1(2));

    let roots = pre_z_roots_for_v1(
        view.multiplicity,
        view.sumcheck_mask,
        view.inverse_product_mask,
    );
    let live =
        derive_global_z_v1(pre_global_capability_v1(), safe_context_v1(), roots).expect("sole z");
    let z = z_bytes_v1(&live);
    assert!(challenge_outside_table_v1(*live.z.as_ref()));
    let pre_z_binding = live.pre_z_binding_digest;
    let (post_z_transcript, post_z_roots, post_z_live) =
        bind_post_z_v1(live, view, synthetic_added_at_v1).expect("post-z commitment view");
    assert_eq!(post_z_live._z.as_ref().to_le_bytes(), z);
    for digest in [
        pre_z_binding,
        post_z_transcript,
        post_z_roots.existing_root,
        post_z_roots.added_root,
        post_z_roots.alias_root,
        post_z_roots.global_root,
    ] {
        assert_ne!(digest, [0; DIGEST_BYTES_V1]);
    }
    assert_ne!(post_z_roots.existing_root, post_z_roots.added_root);
    assert_ne!(post_z_roots.alias_root, post_z_roots.global_root);
    assert_ne!(
        residual_digest_v1(
            pre_z_binding,
            post_z_transcript,
            post_z_roots,
            view.residual
        )
        .expect("post-z residual digest"),
        [0; DIGEST_BYTES_V1]
    );

    assert_eq!(HEADER_BYTES_V1, 56);
    assert_eq!(PRE_Z_SCALAR_COMMITMENTS_V1, 3);
    assert_eq!(PRE_Z_POINT_ROLE_COUNT_V1, 14);
    assert_eq!(PRE_Z_POINT_BYTES_V1, 99);
    assert_eq!(PRE_Z_PHYSICAL_COMMITMENTS_V1, 39_635);
    assert_eq!(EXISTING_INVERSE_BYTES_V1, 385_968);
    assert_eq!(MIN_WIRE_BYTES_V1, 386_156);
    assert_eq!(
        RNS_NATIVE_CENTERING_SUBTRACTION_RESIDUAL_MAX_BYTES_V1,
        500_639
    );
    assert_eq!(
        RNS_NATIVE_GLOBAL_LOOKUP_POST_Z_RESIDUAL_MAX_BYTES_V1,
        114_484
    );
    assert_eq!(wire.len(), MIN_WIRE_BYTES_V1 + view.residual.len() - 1);
    assert_eq!(
        PRE_Z_POINT_PURPOSES_V1
            .iter()
            .map(|purpose| purpose.count_v1())
            .sum::<usize>(),
        PRE_Z_PHYSICAL_COMMITMENTS_V1
    );
    assert_eq!(SOURCE_COMMITMENTS_V1, 344);
    assert_eq!(
        POST_Z_POINT_PURPOSES_V1
            .iter()
            .map(|purpose| purpose.count_v1())
            .sum::<usize>(),
        GLOBAL_INVERSE_POINTS_V1
    );
}

#[test]
fn z_binds_every_safe_pre_z_axis_but_not_post_z_inverse_bytes() {
    let wire = canonical_wire_v1(b"downstream");
    let view = PreZEnvelopeViewV1::from_canonical_prefix_v1(&wire).expect("baseline view");
    let context = safe_context_v1();
    let roots = pre_z_roots_for_v1(
        view.multiplicity,
        view.sumcheck_mask,
        view.inverse_product_mask,
    );
    let baseline_z = z_bytes_v1(
        &derive_global_z_v1(pre_global_capability_v1(), context, roots).expect("baseline z"),
    );

    let foreign_pre_global = ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1::test_fixture_v1(
        [0xa8; DIGEST_BYTES_V1],
        [0xb9; DIGEST_BYTES_V1],
    )
    .expect("foreign pre-global capability");
    assert_ne!(
        z_bytes_v1(
            &derive_global_z_v1(&foreign_pre_global, context, roots)
                .expect("claimed-pre-global-bound z"),
        ),
        baseline_z
    );

    let mut changed_context = context;
    changed_context.qpcs_binding_digest[0] ^= 1;
    assert_ne!(
        z_bytes_v1(
            &derive_global_z_v1(pre_global_capability_v1(), changed_context, roots)
                .expect("context-bound z"),
        ),
        baseline_z
    );
    let mut changed_roots = roots;
    changed_roots.roots[4][0] ^= 1;
    assert_ne!(
        z_bytes_v1(
            &derive_global_z_v1(pre_global_capability_v1(), context, changed_roots)
                .expect("role-bound z"),
        ),
        baseline_z
    );

    let mut changed_pre_z = wire.clone();
    changed_pre_z[HEADER_BYTES_V1..HEADER_BYTES_V1 + POINT_BYTES_V1]
        .copy_from_slice(&encoded_point_v1(test_point_v1(15)));
    refresh_codec_v1(&mut changed_pre_z);
    let changed_pre_z_view =
        PreZEnvelopeViewV1::from_canonical_prefix_v1(&changed_pre_z).expect("changed pre-z point");
    let changed_pre_z_roots = pre_z_roots_for_v1(
        changed_pre_z_view.multiplicity,
        changed_pre_z_view.sumcheck_mask,
        changed_pre_z_view.inverse_product_mask,
    );
    assert_ne!(
        z_bytes_v1(
            &derive_global_z_v1(pre_global_capability_v1(), context, changed_pre_z_roots)
                .expect("pre-z-point-bound z"),
        ),
        baseline_z
    );

    let mut changed_global_mask = wire.clone();
    let global_mask_offset = HEADER_BYTES_V1 + POINT_BYTES_V1;
    changed_global_mask[global_mask_offset..global_mask_offset + POINT_BYTES_V1]
        .copy_from_slice(&encoded_point_v1(test_point_v1(14)));
    refresh_codec_v1(&mut changed_global_mask);
    let changed_global_mask_view =
        PreZEnvelopeViewV1::from_canonical_prefix_v1(&changed_global_mask)
            .expect("changed global-lookup mask");
    let changed_global_mask_roots = pre_z_roots_for_v1(
        changed_global_mask_view.multiplicity,
        changed_global_mask_view.sumcheck_mask,
        changed_global_mask_view.inverse_product_mask,
    );
    assert_ne!(
        z_bytes_v1(
            &derive_global_z_v1(
                pre_global_capability_v1(),
                context,
                changed_global_mask_roots,
            )
            .expect("global-lookup-mask-bound z")
        ),
        baseline_z
    );

    let mut changed_inverse_product_mask = wire.clone();
    let inverse_product_mask_offset = HEADER_BYTES_V1 + 2 * POINT_BYTES_V1;
    changed_inverse_product_mask
        [inverse_product_mask_offset..inverse_product_mask_offset + POINT_BYTES_V1]
        .copy_from_slice(&encoded_point_v1(test_point_v1(15)));
    refresh_codec_v1(&mut changed_inverse_product_mask);
    let changed_inverse_product_mask_view =
        PreZEnvelopeViewV1::from_canonical_prefix_v1(&changed_inverse_product_mask)
            .expect("changed inverse-product mask");
    let changed_inverse_product_mask_roots = pre_z_roots_for_v1(
        changed_inverse_product_mask_view.multiplicity,
        changed_inverse_product_mask_view.sumcheck_mask,
        changed_inverse_product_mask_view.inverse_product_mask,
    );
    assert_ne!(
        z_bytes_v1(
            &derive_global_z_v1(
                pre_global_capability_v1(),
                context,
                changed_inverse_product_mask_roots,
            )
            .expect("inverse-product-mask-bound z")
        ),
        baseline_z
    );

    let inverse_offset = HEADER_BYTES_V1 + PRE_Z_POINT_BYTES_V1;
    let mut changed_post_z = wire;
    changed_post_z[inverse_offset..inverse_offset + POINT_BYTES_V1]
        .copy_from_slice(&encoded_point_v1(test_point_v1(15)));
    refresh_codec_v1(&mut changed_post_z);
    let changed_post_z_view =
        PreZEnvelopeViewV1::from_canonical_prefix_v1(&changed_post_z).expect("changed inverse");
    let changed_post_z_roots = pre_z_roots_for_v1(
        changed_post_z_view.multiplicity,
        changed_post_z_view.sumcheck_mask,
        changed_post_z_view.inverse_product_mask,
    );
    let changed_live =
        derive_global_z_v1(pre_global_capability_v1(), context, changed_post_z_roots)
            .expect("same pre-z transcript");
    assert_eq!(z_bytes_v1(&changed_live), baseline_z);

    let baseline_wire = canonical_wire_v1(b"downstream");
    let baseline_view =
        PreZEnvelopeViewV1::from_canonical_prefix_v1(&baseline_wire).expect("fresh baseline view");
    let baseline_live =
        derive_global_z_v1(pre_global_capability_v1(), context, roots).expect("fresh baseline z");
    let (baseline_post, baseline_roots, _) =
        bind_post_z_v1(baseline_live, baseline_view, synthetic_added_at_v1)
            .expect("baseline post-z");
    let (changed_post, changed_inverse_roots, _) =
        bind_post_z_v1(changed_live, changed_post_z_view, synthetic_added_at_v1)
            .expect("changed post-z");
    assert_ne!(baseline_post, changed_post);
    assert_ne!(
        baseline_roots.existing_root,
        changed_inverse_roots.existing_root
    );
    assert_ne!(
        baseline_roots.global_root,
        changed_inverse_roots.global_root
    );
}

#[test]
fn codec_is_canonical_capped_and_defers_inverse_decoding_until_after_z() {
    let wire = canonical_wire_v1(b"nonempty-residual");
    assert!(PreZEnvelopeViewV1::from_canonical_prefix_v1(&wire[..wire.len() - 1]).is_err());
    let mut trailing = wire.clone();
    trailing.push(0);
    assert!(PreZEnvelopeViewV1::from_canonical_prefix_v1(&trailing).is_err());

    let mut geometry = wire.clone();
    geometry[14] ^= 1;
    assert_eq!(
        PreZEnvelopeViewV1::from_canonical_prefix_v1(&geometry).map(|_| ()),
        Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidGeometry)
    );
    let mut invalid_pre_z = wire.clone();
    invalid_pre_z[HEADER_BYTES_V1..HEADER_BYTES_V1 + POINT_BYTES_V1].fill(0);
    assert_eq!(
        PreZEnvelopeViewV1::from_canonical_prefix_v1(&invalid_pre_z).map(|_| ()),
        Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidPoint)
    );
    let mut invalid_inverse_product_mask = wire.clone();
    let inverse_product_mask_offset = HEADER_BYTES_V1 + 2 * POINT_BYTES_V1;
    invalid_inverse_product_mask
        [inverse_product_mask_offset..inverse_product_mask_offset + POINT_BYTES_V1]
        .fill(0);
    assert_eq!(
        PreZEnvelopeViewV1::from_canonical_prefix_v1(&invalid_inverse_product_mask).map(|_| ()),
        Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidPoint)
    );

    let inverse_offset = HEADER_BYTES_V1 + PRE_Z_POINT_BYTES_V1;
    let mut invalid_inverse = wire.clone();
    invalid_inverse[inverse_offset..inverse_offset + POINT_BYTES_V1].fill(0);
    refresh_codec_v1(&mut invalid_inverse);
    let invalid_inverse_view = PreZEnvelopeViewV1::from_canonical_prefix_v1(&invalid_inverse)
        .expect("inverse bytes deliberately remain opaque before z");
    let invalid_inverse_roots = pre_z_roots_for_v1(
        invalid_inverse_view.multiplicity,
        invalid_inverse_view.sumcheck_mask,
        invalid_inverse_view.inverse_product_mask,
    );
    let invalid_inverse_live = derive_global_z_v1(
        pre_global_capability_v1(),
        safe_context_v1(),
        invalid_inverse_roots,
    )
    .expect("z precedes inverse decoding");
    assert_eq!(
        bind_post_z_v1(
            invalid_inverse_live,
            invalid_inverse_view,
            synthetic_added_at_v1,
        )
        .map(|_| ()),
        Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidPoint)
    );

    let mut bad_codec = wire;
    let last = bad_codec.len() - 1;
    bad_codec[last] ^= 1;
    let bad_codec_view =
        PreZEnvelopeViewV1::from_canonical_prefix_v1(&bad_codec).expect("prefix before codec bind");
    let bad_codec_roots = pre_z_roots_for_v1(
        bad_codec_view.multiplicity,
        bad_codec_view.sumcheck_mask,
        bad_codec_view.inverse_product_mask,
    );
    let bad_codec_live = derive_global_z_v1(
        pre_global_capability_v1(),
        safe_context_v1(),
        bad_codec_roots,
    )
    .expect("z before codec bind");
    assert_eq!(
        bind_post_z_v1(bad_codec_live, bad_codec_view, synthetic_added_at_v1).map(|_| ()),
        Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidIntegrity)
    );

    let at_cap = canonical_wire_v1(&vec![
        0x5a;
        RNS_NATIVE_GLOBAL_LOOKUP_POST_Z_RESIDUAL_MAX_BYTES_V1
    ]);
    assert_eq!(
        at_cap.len(),
        RNS_NATIVE_CENTERING_SUBTRACTION_RESIDUAL_MAX_BYTES_V1
    );
    assert!(PreZEnvelopeViewV1::from_canonical_prefix_v1(&at_cap).is_ok());
    let cap_plus_one = vec![0_u8; RNS_NATIVE_CENTERING_SUBTRACTION_RESIDUAL_MAX_BYTES_V1 + 1];
    assert_eq!(
        PreZEnvelopeViewV1::from_canonical_prefix_v1(&cap_plus_one).map(|_| ()),
        Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::ProofCapExceeded)
    );
}

#[test]
fn role_roots_are_ordered_counted_non_aliasing_and_fail_closed() {
    let purpose = PhysicalPurposeV1::SmallPositiveInverse;
    let positive = role_root_with_v1(purpose, test_point_v1(8));
    let negative_point_under_positive_role = role_root_with_v1(purpose, test_point_v1(10));
    let positive_point_under_negative_role =
        role_root_with_v1(PhysicalPurposeV1::SmallNegativeInverse, test_point_v1(8));
    assert_ne!(positive, negative_point_under_positive_role);
    assert_ne!(positive, positive_point_under_negative_role);

    let mut short = RoleRootBuilderV1::new_v1(PhysicalPurposeV1::Multiplicity);
    assert_eq!(
        short.finish_v1(),
        Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidGeometry)
    );
    short = RoleRootBuilderV1::new_v1(PhysicalPurposeV1::Multiplicity);
    short.absorb_v1(test_point_v1(0)).expect("one scalar point");
    assert_eq!(
        short.absorb_v1(test_point_v1(1)),
        Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidGeometry)
    );

    let mut zero_context = safe_context_v1();
    zero_context.source_binding_digest = [0; DIGEST_BYTES_V1];
    assert_eq!(
        zero_context.validate_v1(),
        Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext)
    );
    let mut duplicate_context = safe_context_v1();
    duplicate_context.qpcs_binding_digest = duplicate_context.source_binding_digest;
    assert_eq!(
        duplicate_context.validate_v1(),
        Err(RnsNativeGlobalLookupZCommitmentViewErrorV1::InvalidContext)
    );
}

#[test]
fn exact_inverse_product_inventory_is_cap_blocked_without_weakening() {
    assert_eq!(INVERSE_PRODUCT_COORDINATES_V1, 16_384);
    assert_eq!(INVERSE_PRODUCT_PLANES_PER_CORE_V1, 4);
    assert_eq!(INVERSE_PRODUCT_GATES_PER_CORE_V1, 65_536);
    assert_eq!(INVERSE_PRODUCT_COMMITMENTS_PER_CORE_V1, 8);
    assert_eq!(INVERSE_PRODUCT_CORE_POINTS_V1, 57);
    assert_eq!(INVERSE_PRODUCT_CORE_SCALARS_V1, 5);
    assert_eq!(INVERSE_PRODUCT_CORE_BYTES_V1, 2_041);
    assert_eq!(INVERSE_PRODUCT_RECORD_BYTES_V1, 2_045);

    assert_eq!(SMALLEST_COMPLETE_ROLE_PLANES_V1, 1_032);
    assert_eq!(SMALLEST_COMPLETE_ROLE_CORES_V1, 258);
    assert_eq!(SMALLEST_COMPLETE_ROLE_RECORD_BYTES_V1, 527_610);
    assert_eq!(SMALLEST_COMPLETE_ROLE_MIN_BYTES_V1, 527_643);
    assert_eq!(
        RNS_NATIVE_GLOBAL_LOOKUP_POST_Z_RESIDUAL_MAX_BYTES_V1,
        114_484
    );
    assert_eq!(SMALLEST_COMPLETE_ROLE_CAP_EXCESS_V1, 413_159);
    assert!(
        SMALLEST_COMPLETE_ROLE_MIN_BYTES_V1 > RNS_NATIVE_GLOBAL_LOOKUP_POST_Z_RESIDUAL_MAX_BYTES_V1
    );

    assert_eq!(MAX_FITTING_PARTIAL_CORES_V1, 55);
    assert_eq!(MAX_FITTING_PARTIAL_PLANES_V1, 220);
    assert!(MAX_FITTING_PARTIAL_PLANES_V1 < SMALLEST_COMPLETE_ROLE_PLANES_V1);
    assert!(
        MAX_FITTING_PARTIAL_CORES_V1 * INVERSE_PRODUCT_RECORD_BYTES_V1
            + CODEC_DIGEST_BYTES_V1
            + MIN_NONEMPTY_RESIDUAL_BYTES_V1
            <= RNS_NATIVE_GLOBAL_LOOKUP_POST_Z_RESIDUAL_MAX_BYTES_V1
    );
    assert!(
        (MAX_FITTING_PARTIAL_CORES_V1 + 1) * INVERSE_PRODUCT_RECORD_BYTES_V1
            + CODEC_DIGEST_BYTES_V1
            + MIN_NONEMPTY_RESIDUAL_BYTES_V1
            > RNS_NATIVE_GLOBAL_LOOKUP_POST_Z_RESIDUAL_MAX_BYTES_V1
    );

    assert_eq!(BOTH_SMALL_ROLES_MIN_BYTES_V1, 1_055_253);
    assert_eq!(ALL_INVERSE_PRODUCT_CORES_V1, 8_102);
    assert_eq!(ALL_INVERSE_PRODUCT_RECORD_BYTES_V1, 16_568_590);
    assert_eq!(ALL_INVERSE_PRODUCT_MIN_BYTES_V1, 16_568_623);
    assert_eq!(ALL_INVERSE_PRODUCT_CAP_EXCESS_V1, 16_454_139);

    assert!(!INVERSE_PRODUCT_RELATIONS_VERIFIED_V1);
    assert!(!GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1);
    assert!(!CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1);
    assert!(!RELEASE_READY_V1);
    let blocker = core::str::from_utf8(PRODUCT_CAP_BLOCKER_LANGUAGE_V1)
        .expect("cap-blocker language is canonical ASCII");
    assert!(blocker.contains("forbidden=partial-role-token"));
    assert!(blocker.contains("unaudited-rho-kappa-sumcheck-aggregation"));

    let source = include_str!("rns_native_global_lookup_z_commitment_view.rs");
    assert!(!source.contains("struct RnsNativeGlobalLookupInverseProductPrerequisiteV1"));
    assert!(!source.contains("verify_rns_native_global_lookup_inverse_product_v1"));
}

#[test]
fn boundary_is_private_move_only_non_authorizing_acyclic_and_fail_closed() {
    assert!(SOLE_GLOBAL_LOOKUP_Z_DERIVED_V1);
    assert!(POST_Z_INVERSE_COMMITMENT_VIEW_AUTHENTICATED_V1);
    assert!(LEGACY_GLOBAL_LOOKUP_SUMCHECK_MASK_RETIRED_V1);
    assert!(!INVERSE_PRODUCT_RELATIONS_VERIFIED_V1);
    assert!(!GLOBAL_LOOKUP_RELATIONS_VERIFIED_V1);
    assert!(!CROSS_FIELD_GLOBAL_LOOKUP_VERIFIED_V1);
    assert!(!RELEASE_READY_V1);

    let source = include_str!("rns_native_global_lookup_z_commitment_view.rs");
    for declaration in [
        "pub(super) struct RnsNativeGlobalLookupPreZPrerequisiteV1",
        "pub(super) struct RnsNativeGlobalLookupPostZPrerequisiteV1",
    ] {
        let offset = source.find(declaration).expect("private stage declaration");
        let attributes = source[..offset]
            .rsplit_once("\n\n")
            .map_or(&source[..offset], |(_, block)| block);
        assert!(!attributes.contains("derive(Clone"));
        assert!(!attributes.contains("derive(Copy"));
    }
    assert!(!source.contains("pub fn"));
    assert!(!source.contains("ReleaseAuthorization"));
    assert!(!source.contains("VerifiedReceipt"));
    assert!(source.contains("_z: ZeroizingT256ScalarCopyV1"));
    assert!(!source.contains("fn z(&self"));
    assert!(!source.contains("fn z_v1(&self"));
    assert_eq!(
        source.matches("pre_z.previous.residual_digest()").count(),
        1
    );
    assert_eq!(source.matches("pre_z.previous.binding_digest()").count(), 1);
    let pre_authentication = source
        .split("pub(super) fn authenticate_rns_native_global_lookup_post_z_v1")
        .next()
        .expect("pre-authentication source");
    assert!(!pre_authentication.contains("pre_z.previous.residual_digest()"));
    assert!(!pre_authentication.contains("pre_z.previous.binding_digest()"));
    assert!(source.contains("qpcs().parameter_digest()"));
    assert!(source.contains("qpcs_evaluation(limb, repetition)"));
    assert!(source.contains("POST_Z_TRANSCRIPT_DOMAIN_V1"));
    assert!(source.contains(
        "retired-global-lookup-sumcheck-mask[1;n=1024;702-scalars;retained-not-consumed-by-direct-membership]"
    ));
    assert!(source.contains("retained-not-consumed-by-direct-membership"));
    assert!(source.contains("source-snapshot-commitments=344"));
    assert!(source.contains("not-reencoded-as-pre-z-physical-point-roles"));
    assert!(!PRE_Z_ORDER_V1.starts_with(b"source[344],"));
    assert!(source.contains("_retired_sumcheck_mask: Point"));
    assert!(!source.contains("pub(super) const fn sumcheck_mask(&self) -> Point"));
    assert!(source.contains("inverse-product-mask[1;n=16384;87-scalars]"));
    assert!(source.contains("multiplicity: Point"));
    assert!(source.contains("pub(super) const fn multiplicity(&self) -> Point"));
    assert!(source.contains("multiplicity: pre_z.view.multiplicity"));
    assert!(source.contains("inverse_product_mask: Point"));
    assert!(source.contains("pub(super) const fn inverse_product_mask(&self) -> Point"));

    let sole_z = source
        .split_once("fn derive_global_z_v1(")
        .expect("sole-z derivation")
        .1
        .split_once("#[derive(Clone, Copy)]")
        .expect("sole-z derivation boundary")
        .0;
    let capability = sole_z
        .find("pre_global_capability: &ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1")
        .expect("opaque pre-global input");
    let opaque_digest = sole_z
        .find(".sole_z_binding_digest_v1()")
        .expect("opaque pre-global commitment");
    let claimed_frame = sole_z
        .find("CLAIMED_PRE_GLOBAL_LABEL_V1")
        .expect("claimed pre-global transcript frame");
    let local_axes = sole_z
        .find("b\"fixed-axes\"")
        .expect("first local safe axis");
    assert!(
        capability < opaque_digest && opaque_digest < claimed_frame && claimed_frame < local_axes
    );
    assert!(!sole_z.contains("post_cross_field_binding_digest"));
    assert!(!sole_z.contains("global_lookup_challenge_seed"));
    let production_pre_z = source
        .split_once("pub(super) fn derive_rns_native_global_lookup_pre_z_v1")
        .expect("production sole-z entry")
        .1
        .split_once("pub(super) fn authenticate_rns_native_global_lookup_post_z_v1")
        .expect("production sole-z entry boundary")
        .0;
    let production_signature = production_pre_z
        .split_once("where")
        .expect("production sole-z signature")
        .0;
    assert!(!production_signature.contains("pre_global_capability"));
    assert!(!production_signature.contains("ZkAmsMkheRnsNativePreGlobalLookupCapabilityV1"));
    assert!(
        production_signature
            .contains("previous: RnsNativeCenteringSubtractionPrerequisiteV1<'source, 'proof, S>")
    );
    let capability_borrow = production_pre_z
        .find("let pre_global_capability = previous.pre_global_lookup_capability_v1()")
        .expect("capability borrowed from exact recursively owned parent");
    let derive = production_pre_z
        .find("derive_global_z_v1(pre_global_capability, context, roots)?")
        .expect("sole z derived from owned-session capability");
    let borrow_end = production_pre_z[derive..]
        .find("\n    };")
        .map(|offset| derive + offset)
        .expect("capability borrow scope ends");
    let move_previous = production_pre_z
        .find("        previous,")
        .expect("recursive predecessor moved only after sole-z derivation");
    assert!(capability_borrow < derive && derive < borrow_end && borrow_end < move_previous);
    assert_eq!(
        production_pre_z
            .matches("previous.pre_global_lookup_capability_v1()")
            .count(),
        1
    );
    assert!(!production_pre_z.contains("test_fixture_v1"));

    let parent = include_str!("../mkhe.rs");
    assert_eq!(
        parent
            .matches("mod rns_native_global_lookup_z_commitment_view;")
            .count(),
        1
    );
    assert!(!parent.contains("pub use rns_native_global_lookup_z_commitment_view"));
    let composite = include_str!("rns_native_composite_verifier.rs");
    assert!(composite.contains("StageUnavailable"));
    assert!(composite.contains("CrossFieldGlobalLookup"));
}
