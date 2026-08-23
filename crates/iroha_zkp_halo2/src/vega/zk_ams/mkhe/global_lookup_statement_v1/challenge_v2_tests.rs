//! Tests for the additive sole-global-`z` V2 transcript contract.

use super::*;

#[rustfmt::skip]
fn context_v2() -> GlobalLookupPreZContextV2 {
    GlobalLookupPreZContextV2::new_v2(
        [0x11; 32],
        [0x22; 32],
        [0x33; 32],
        [0x44; 32],
        [0x55; 32],
        [0x66; 32],
    )
}

fn pre_z_v2() -> GlobalLookupPreZInventoryV2 {
    GlobalLookupPreZInventoryV2::new_v2([0x71; 32], PRE_Z_COMMITMENTS_V2)
}

fn post_z_v2() -> GlobalLookupPostZBindingsV2 {
    GlobalLookupPostZBindingsV2::new_v2(
        [0x72; 32],
        [0x73; 32],
        [0x74; 32],
        [0x75; 32],
        RADIX_EXISTING_INVERSES_V2,
        CROSS_FIELD_ADDED_INVERSES_V2,
        ALIASED_INVERSES_V2,
        GLOBAL_CUMULATIVE_INVERSES_V2,
    )
}

#[rustfmt::skip]
fn derived_z_v2(
    context: GlobalLookupPreZContextV2,
    pre_z: GlobalLookupPreZInventoryV2,
) -> GlobalLookupDerivedZV2 {
    GlobalLookupPreZTranscriptV2::begin_v2(
        context,
        pre_z,
        GlobalLookupPreZInputSealV2::TestOnly,
    )
    .unwrap()
    .derive_global_z_v2()
    .unwrap()
}

fn pre_z_observation_v2(
    context: GlobalLookupPreZContextV2,
    pre_z: GlobalLookupPreZInventoryV2,
) -> ([u8; 32], [u8; 32]) {
    let transcript = GlobalLookupPreZTranscriptV2::begin_v2(
        context,
        pre_z,
        GlobalLookupPreZInputSealV2::TestOnly,
    )
    .unwrap();
    let bound_context = transcript.bound_context_digest;
    let derived = transcript.derive_global_z_v2().unwrap();
    (bound_context, derived.test_only_z_bytes_v2())
}

fn rho_bound_v2(post_z: GlobalLookupPostZBindingsV2) -> GlobalLookupRhoBoundV2 {
    derived_z_v2(context_v2(), pre_z_v2())
        .bind_post_z_and_derive_rho_v2(post_z)
        .unwrap()
}

#[test]
fn manifest_topology_z_rho_and_transcript_have_independent_static_kats() {
    assert_eq!(
        hex::encode(challenge_manifest_digest_v2()),
        "e430ba88e410872413a79a8cf41f140d844f981b65916708d1c90e62fc947453"
    );
    assert_eq!(
        hex::encode(global_lookup_topology_digest_v2()),
        "7f5c120671318f34668b45c7eabb4022d934571840d8f0e60d294856a7d98f28"
    );
    let derived = derived_z_v2(context_v2(), pre_z_v2());
    assert_eq!(
        hex::encode(derived.test_only_z_bytes_v2()),
        "ad3746792bd567deed585ea4f2a417d8635d1ed75cb3a74ca2de459a63440b8f"
    );
    let bound = derived.bind_post_z_and_derive_rho_v2(post_z_v2()).unwrap();
    assert_eq!(
        hex::encode(bound.bound_context_digest),
        "2e6981f0f6e6458e497e2af4d9949a3fb6648dff8a8ca7e3baef33b0b0ec9e50"
    );
    assert_eq!(
        hex::encode(bound.rho[0].to_le_bytes()),
        "ae3bf24e663dc0942f9646ceed8092aedd63689624a04d23960951d79ed1a95e"
    );
    assert_eq!(
        hex::encode(bound.rho[28].to_le_bytes()),
        "59feb9c9f6eac029b9f773d62cf0cef798098a582d4b0fdc6ef53718266ba2a3"
    );
    assert_eq!(
        hex::encode(bound.transcript_digest),
        "18c17320c3f95d37ed997e60bc8b8736d17f5091d5bcf6cbd793edef09af813c"
    );
}

#[test]
#[rustfmt::skip]
fn every_pre_z_context_and_inventory_input_is_bound_before_z() {
    let baseline = pre_z_observation_v2(context_v2(), pre_z_v2());
    let mut mutations = [context_v2(); 6];
    mutations[0].fixed_axes_digest[0] ^= 1;
    mutations[1].source_binding_digest[0] ^= 1;
    mutations[2].radix_pre_z_digest[0] ^= 1;
    mutations[3].packing_digest[0] ^= 1;
    mutations[4].cross_field_pre_z_digest[0] ^= 1;
    mutations[5].qpcs_initial_root[0] ^= 1;
    for mutation in mutations {
        let observed = pre_z_observation_v2(mutation, pre_z_v2());
        assert_ne!(observed.0, baseline.0);
        assert_ne!(observed.1, baseline.1);
    }
    let mutated_inventory = GlobalLookupPreZInventoryV2::new_v2(
        [0x76; 32],
        PRE_Z_COMMITMENTS_V2,
    );
    let observed = pre_z_observation_v2(context_v2(), mutated_inventory);
    assert_eq!(observed.0, baseline.0);
    assert_ne!(observed.1, baseline.1);
    assert!(matches!(
        GlobalLookupPreZTranscriptV2::begin_v2(
            context_v2(),
            GlobalLookupPreZInventoryV2::new_v2([0x71; 32], PRE_Z_COMMITMENTS_V2 - 1),
            GlobalLookupPreZInputSealV2::TestOnly,
        ),
        Err(GlobalLookupChallengeErrorV2::Shape)
    ));
    let mut zero_context = context_v2();
    zero_context.cross_field_pre_z_digest = [0; 32];
    assert!(matches!(
        GlobalLookupPreZTranscriptV2::begin_v2(
            zero_context,
            pre_z_v2(),
            GlobalLookupPreZInputSealV2::TestOnly,
        ),
        Err(GlobalLookupChallengeErrorV2::Shape)
    ));
}

#[test]
#[rustfmt::skip]
fn all_post_z_bindings_are_absorbed_in_fixed_order_before_rho() {
    assert_eq!(
        TRANSCRIPT_FRAME_ORDER_V2,
        [
            b"challenge-manifest".as_slice(),
            b"global-lookup-topology".as_slice(),
            b"fixed-axes".as_slice(),
            b"source-binding".as_slice(),
            b"radix-pre-z".as_slice(),
            b"packing".as_slice(),
            b"cross-field-pre-z".as_slice(),
            b"qpcs-initial-root".as_slice(),
            b"pre-z-commitment-inventory".as_slice(),
            b"radix-post-z-existing-inverses".as_slice(),
            b"cross-field-post-z-added-inverses".as_slice(),
            b"radix-global-inverse-alias-map".as_slice(),
            b"global-post-z-inverse-inventory".as_slice(),
        ]
    );
    let baseline = rho_bound_v2(post_z_v2());
    let mut mutations = [post_z_v2(); 4];
    mutations[0].radix_existing_inverse_digest[0] ^= 1;
    mutations[1].cross_field_added_inverse_digest[0] ^= 1;
    mutations[2].alias_map_digest[0] ^= 1;
    mutations[3].global_cumulative_inverse_digest[0] ^= 1;
    for mutation in mutations {
        let observed = rho_bound_v2(mutation);
        assert_ne!(observed.rho[0], baseline.rho[0]);
        assert_ne!(observed.transcript_digest, baseline.transcript_digest);
    }

    let mut swapped = derived_z_v2(context_v2(), pre_z_v2());
    let mut live = swapped.live.take().unwrap();
    let post = post_z_v2();
    for (label, digest) in [
        (FRAME_CROSS_FIELD_POST_Z_V2, post.cross_field_added_inverse_digest),
        (FRAME_RADIX_POST_Z_V2, post.radix_existing_inverse_digest),
        (FRAME_ALIAS_MAP_V2, post.alias_map_digest),
        (FRAME_GLOBAL_POST_Z_V2, post.global_cumulative_inverse_digest),
    ] {
        absorb_frame_v2(&mut live.state, label, &digest).unwrap();
    }
    let swapped_rho = derive_challenge_v2(
        &mut live.state,
        &mut live.challenge_ordinal,
        ChallengePurposeV2::rho_v2(0),
        ChallengePredicateV2::Nonzero,
    )
    .unwrap();
    assert_ne!(swapped_rho, baseline.rho[0]);

    let invalid = GlobalLookupPostZBindingsV2::new_v2(
        [0x72; 32],
        [0x73; 32],
        [0x74; 32],
        [0x75; 32],
        RADIX_EXISTING_INVERSES_V2,
        CROSS_FIELD_ADDED_INVERSES_V2,
        ALIASED_INVERSES_V2 - 1,
        GLOBAL_CUMULATIVE_INVERSES_V2,
    );
    assert!(matches!(
        derived_z_v2(context_v2(), pre_z_v2()).bind_post_z_and_derive_rho_v2(invalid),
        Err(GlobalLookupChallengeErrorV2::Shape)
    ));
}

#[test]
#[rustfmt::skip]
fn z_policy_has_exact_table_boundary_and_rho_schedule() {
    assert!(!challenge_is_outside_table_v2(Scalar::from_u64(0)));
    assert!(!challenge_is_outside_table_v2(Scalar::from_u64(32_767)));
    assert!(challenge_is_outside_table_v2(Scalar::from_u64(32_768)));
    assert!(challenge_is_outside_table_v2(Scalar::from_u64(65_535)));
    assert_eq!((Z_ORDINAL_V2, RHO_FIRST_ORDINAL_V2, RHO_LAST_ORDINAL_V2, AFTER_RHO_ORDINAL_V2), (0, 1, 29, 30));
    assert_eq!((PRE_Z_COMMITMENTS_V2, RADIX_EXISTING_INVERSES_V2, CROSS_FIELD_ADDED_INVERSES_V2, ALIASED_INVERSES_V2, GLOBAL_CUMULATIVE_INVERSES_V2), (39_338, 11_696, 20_072, 11_696, 31_768));
}

#[test]
#[rustfmt::skip]
fn source_guards_preserve_v1_and_keep_v2_private_uninhabited_and_bounded() {
    let production = include_str!("challenge_v2.rs");
    let tests = include_str!("challenge_v2_tests.rs");
    let v1 = include_str!("challenge_v1.rs");
    let v1_tests = include_str!("challenge_v1_tests.rs");
    let parent = include_str!("../global_lookup_statement_v1.rs");
    assert!(production.lines().count() <= 500);
    assert!(tests.lines().count() <= 500);
    assert!(!production.contains("Vec<"));
    assert!(!production.contains("impl Clone for GlobalLookupDerivedZV2"));
    assert!(!production.contains("impl Deref for GlobalLookupDerivedZV2"));
    assert!(!production.contains("pub fn z"));
    assert!(production.contains("authenticated_pre_z_inventory: Infallible"));
    assert!(production.contains("z: ZeroizingT256ScalarCopyV1"));
    assert!(production.contains("live: Option<GlobalLookupDerivedZLiveV2>"));
    assert!(production.contains("global-radix-lookup-z"));
    assert!(production.contains("no-second-radix-z"));
    assert!(production.contains("const GLOBAL_LOOKUP_PROOF_VERIFIED_V2: bool = false;"));
    assert!(production.contains("const RELEASE_READY_V2: bool = false;"));
    assert!(parent.contains("iroha.zk-ams.v1.phase23.global-lookup.transcript\\0"));
    assert!(v1.contains("b\"radix-range\""));
    assert!(v1.contains("b\"cross-field\""));
    assert!(v1.contains("b\"lookup-z\""));
    assert!(v1.contains("b\"post-z-inverse-commitments\""));
    assert!(v1_tests.contains("e3730911785cb1e23332ee9a1361810c435f76b93becd54e3b0d189644b32d99"));
    assert!(v1_tests.contains("20aafcea0445adace67ff1a4677c2110d66278f3c9b1d2fd105f5e4ebefa47ad"));
    assert_eq!(parent.matches("global_lookup_statement_v1/challenge_v1.rs").count(), 1);
    assert_eq!(parent.matches("global_lookup_statement_v1/challenge_v2.rs").count(), 1);
}
