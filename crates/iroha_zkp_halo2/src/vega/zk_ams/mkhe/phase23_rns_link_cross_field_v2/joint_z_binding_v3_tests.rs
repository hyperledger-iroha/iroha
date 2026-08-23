use super::*;

fn axes_v3() -> CrossFieldPreZAxesV3 {
    CrossFieldPreZAxesV3 {
        fixed_axes_digest: [0x11; 32],
        source_manifest_digest: [0x22; 32],
        source_receipt_digest: [0x33; 32],
        source_formula_digest: [0x44; 32],
        source_mapping_digest: [0x55; 32],
        terminal_binding_digest: [0x66; 32],
        radix_pre_z_binding_digest: [0x77; 32],
        qpcs_parameter_digest: [0x88; 32],
    }
}

fn pre_z_v3() -> CrossFieldPreZBindingV3 {
    CrossFieldPreZBindingV3::bind_v3(
        CrossFieldPreZOwnerSealV3::TestOnly,
        axes_v3(),
        [0x99; 32],
        [0xaa; 32],
    )
    .expect("pre-z binding")
}

#[test]
fn split_inventory_counts_are_exact_and_disjoint() {
    assert_eq!(
        (
            SOURCE_POINTS_V3,
            EXISTING_D_POINTS_V3,
            EXISTING_S_POINTS_V3,
            PRE_Z_SCALAR_COMMITMENTS_V3
        ),
        (344, 6_192, 6_192, 2)
    );
    assert_eq!(
        (
            COMPARATOR_PRE_Z_POINTS_V3,
            SMALL_SOURCE_PRE_Z_POINTS_V3,
            Q_MASK_PRE_Z_POINTS_V3
        ),
        (12_384, 2_064, 12_160)
    );
    assert_eq!(ADDED_PRE_Z_POINTS_V3, 26_608);
    assert_eq!(CHALLENGE_INDEPENDENT_POINTS_V3, 39_338);
    assert_eq!(SHARED_EXISTING_INVERSE_POINTS_V3, 11_696);
    assert_eq!(
        (
            COMPARATOR_POST_Z_POINTS_V3,
            SMALL_SOURCE_POST_Z_POINTS_V3,
            Q_MASK_POST_Z_POINTS_V3
        ),
        (5_848, 2_064, 12_160)
    );
    assert_eq!(ADDED_POST_Z_POINTS_V3, 20_072);
    assert_eq!(GLOBAL_INVERSE_POINTS_V3, 31_768);
    assert_eq!(DENSE_PHYSICAL_INVENTORY_V3, 71_109);
}

#[test]
fn pre_and_post_bindings_have_literal_kats_and_bind_order() {
    let pre = pre_z_v3();
    assert_eq!(
        hex::encode(pre.binding_digest),
        "af95ceca72fc329d4cc8dd83f0b9977915c110a1eb8f526b2d76f9e866f3e699"
    );
    let post = CrossFieldPostZBindingV3::bind_v3(
        CrossFieldPostZOwnerSealV3::TestOnly,
        pre,
        [[0xbb; 32], [0xcc; 32], [0xdd; 32], [0xee; 32]],
    )
    .expect("post-z binding");
    assert_eq!(
        hex::encode(post.binding_digest),
        "e542baa2a238e354e46630a63688fd2961572c02545ad4626ce571bc2b6e7f72"
    );
    assert_eq!(post.validate_v3(), Ok(()));
}

#[test]
fn every_context_and_root_mutation_is_rejected_or_rebound() {
    let original = pre_z_v3().binding_digest;
    for field in 0..8 {
        let mut axes = axes_v3();
        let target = match field {
            0 => &mut axes.fixed_axes_digest,
            1 => &mut axes.source_manifest_digest,
            2 => &mut axes.source_receipt_digest,
            3 => &mut axes.source_formula_digest,
            4 => &mut axes.source_mapping_digest,
            5 => &mut axes.terminal_binding_digest,
            6 => &mut axes.radix_pre_z_binding_digest,
            _ => &mut axes.qpcs_parameter_digest,
        };
        target[0] ^= 1;
        let changed = CrossFieldPreZBindingV3::bind_v3(
            CrossFieldPreZOwnerSealV3::TestOnly,
            axes,
            [0x99; 32],
            [0xaa; 32],
        )
        .expect("changed binding");
        assert_ne!(changed.binding_digest, original);
    }
    let mut hostile = pre_z_v3();
    hostile.existing_d_root[0] ^= 1;
    assert_eq!(hostile.validate_v3(), Err(CrossFieldErrorV2::Context));
    assert!(
        CrossFieldPreZBindingV3::bind_v3(
            CrossFieldPreZOwnerSealV3::TestOnly,
            axes_v3(),
            [0; 32],
            [0xaa; 32]
        )
        .is_err()
    );

    for root in 0..4 {
        let mut post = CrossFieldPostZBindingV3::bind_v3(
            CrossFieldPostZOwnerSealV3::TestOnly,
            pre_z_v3(),
            [[0xbb; 32], [0xcc; 32], [0xdd; 32], [0xee; 32]],
        )
        .expect("post-z binding");
        match root {
            0 => post.shared_existing_inverse_root[0] ^= 1,
            1 => post.added_inverse_root[0] ^= 1,
            2 => post.alias_map_root[0] ^= 1,
            _ => post.global_inverse_root[0] ^= 1,
        }
        assert_eq!(post.validate_v3(), Err(CrossFieldErrorV2::Context));
    }
}

#[test]
fn gates_and_source_boundaries_remain_fail_closed() {
    for gate in [
        PRE_Z_BINDING_INHABITED_V3,
        POST_Z_BINDING_INHABITED_V3,
        CROSS_FIELD_PROOF_VERIFIED_V3,
        ZERO_KNOWLEDGE_ACCEPTED_V3,
        COMPLETE_ACCOUNTING_QUALIFIED_V3,
        AUTHORITY_MINTED_V3,
        RSS_QUALIFIED_V3,
        OPERATIONAL_RECEIPT_ACCEPTED_V3,
        RELEASE_READY_V3,
    ] {
        assert!(!gate);
    }
    let production = include_str!("joint_z_binding_v3.rs");
    let parent = include_str!("../phase23_rns_link_cross_field_v2.rs");
    assert!(production.lines().count() <= 350);
    assert!(production.len() <= 16_000);
    assert_eq!(parent.matches("mod joint_z_binding_v3;").count(), 1);
    for required in [
        "authenticated_source: Infallible",
        "shared_radix_inverses: Infallible",
        "DENSE_PHYSICAL_INVENTORY_V3: u32 = 71_109",
    ] {
        assert!(production.contains(required));
    }
    for forbidden in [
        "Vec<Point>",
        "impl Clone",
        "impl Deref",
        "callback",
        "Serialize",
        "Deserialize",
        "fn z_v3",
    ] {
        assert!(!production.contains(forbidden));
    }
}
