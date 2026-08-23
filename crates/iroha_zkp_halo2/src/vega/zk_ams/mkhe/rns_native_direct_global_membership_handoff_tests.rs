use super::*;

fn digest_v1(tag: u8) -> [u8; DIGEST_BYTES_V1] {
    [tag; DIGEST_BYTES_V1]
}

#[test]
fn safe_core_projection_is_exactly_five_successor_independent_fields() {
    let safe = source_packing_safe_core_v1(RnsNativeCrossFieldRlweSafeCoreProjectionV1 {
        terminal_predecessor_context_binding_digest: digest_v1(1),
        candidate_pre_direct_inventory_context_digest: digest_v1(2),
        candidate_pre_direct_inventory_root: digest_v1(3),
        existing_radix_candidate_root: digest_v1(4),
        direct_core_safe_digest: digest_v1(5),
    });
    assert_eq!(
        safe.terminal_predecessor_context_binding_digest,
        digest_v1(1)
    );
    assert_eq!(
        safe.candidate_pre_direct_inventory_context_digest,
        digest_v1(2)
    );
    assert_eq!(safe.candidate_pre_direct_inventory_root, digest_v1(3));
    assert_eq!(safe.existing_radix_candidate_root, digest_v1(4));
    assert_eq!(safe.direct_core_safe_digest, digest_v1(5));
}

#[test]
fn handoff_is_zero_wire_and_all_activation_authorities_remain_false() {
    assert_eq!(DIRECT_GLOBAL_MEMBERSHIP_HANDOFF_OWNED_WIRE_BYTES_V1, 0);
    assert_eq!(
        DIRECT_GLOBAL_MEMBERSHIP_HANDOFF_SUCCESSOR_MAX_BYTES_V1,
        RNS_NATIVE_GLOBAL_MEMBERSHIP_RESIDUAL_MAX_BYTES_V1
    );
    assert_eq!(
        DIRECT_GLOBAL_MEMBERSHIP_HANDOFF_SUCCESSOR_MAX_BYTES_V1,
        108_464
    );
    const {
        assert!(VERIFIER_SIDE_DIRECT_GLOBAL_MEMBERSHIP_HANDOFF_IMPLEMENTED_V1);
        assert!(!PRODUCTION_AUTHORITATIVE_NUMERIC_SOURCE_AVAILABLE_V1);
        assert!(!PRODUCTION_AUTHENTICATED_REPLAY_OWNER_AVAILABLE_V1);
        assert!(!PRODUCTION_DERIVED_MASK_OWNER_AVAILABLE_V1);
        assert!(!PRODUCTION_DIRECT_STAGED_ADAPTER_AVAILABLE_V1);
        assert!(!COMPOSITE_ACCEPTANCE_AVAILABLE_V1);
        assert!(!READINESS_AVAILABLE_V1);
        assert!(!RECEIPT_AVAILABLE_V1);
        assert!(!RELEASE_READY_V1);
    }
}

#[test]
fn exact_carrier_recovery_verifies_direct_before_minting_combined_owner() {
    let source = include_str!("rns_native_direct_global_membership_handoff.rs");
    let handoff = source
        .split_once("pub(super) fn verify_rns_native_direct_global_membership_handoff_v2")
        .expect("handoff verifier")
        .1
        .split_once("#[cfg(test)]")
        .expect("handoff verifier boundary")
        .0;

    let membership_residual = handoff
        .find("let membership_residual = membership.residual();")
        .expect("membership residual");
    let clean_global_root = handoff
        .find("derive_rns_native_verified_global_lookup_core_root_v2(&membership)")
        .expect("clean global root before unwind");
    let membership_consume = handoff
        .find("let inverse = membership.into_previous_v1();")
        .expect("membership consume");
    let comparator_borrow = handoff
        .find("let comparator = range_carry.previous();")
        .expect("comparator borrow");
    let direct_verify = handoff
        .find("existing_radix.verify_claimed_direct_v2()")
        .expect("atomic four-core direct verifier");
    let safe_core = handoff
        .find("source_packing_safe_core_v1(atomic_direct.direct().safe_core_projection_v1())")
        .expect("safe-core projection");
    let terminal_discharge = handoff
        .find(".discharge_terminal_roots_v2(verified_global_lookup_root)")
        .expect("atomic terminal-root discharge");
    let outer = handoff
        .find("let mut outer_bindings = RnsNativeSourcePackingCombinedOuterBindingsV1")
        .expect("post-equation outer mapping");
    let combined = handoff
        .find("Ok(RnsNativeDirectGlobalMembershipHandoffV1")
        .expect("combined owner");
    assert!(
        clean_global_root < membership_residual
            && membership_residual < membership_consume
            && membership_consume < comparator_borrow
            && comparator_borrow < direct_verify
            && direct_verify < terminal_discharge
            && terminal_discharge < safe_core
            && safe_core < outer
            && outer < combined
    );

    let implementation = source
        .split_once(
            "impl<'source, 'proof, S: ZkAmsMkheRnsNativeSourceSnapshotV1>\n    RnsNativeSourcePackingCombinedDirectMembershipPredecessorV1<'proof>",
        )
        .expect("source-packing predecessor implementation")
        .1
        .split_once("/// Consume the completed membership chain")
        .expect("implementation boundary")
        .0;
    assert!(implementation.contains("self.membership_residual"));
    assert!(!implementation.contains("_direct.successor()"));
    assert!(!handoff.contains("to_vec("));
    assert!(!handoff.contains("clone("));
    assert!(!handoff.contains("source: P"));
    assert!(!handoff.contains("into_parts_v1"));

    let declaration = source
        .split_once("pub(super) struct RnsNativeDirectGlobalMembershipHandoffV1")
        .expect("combined owner declaration")
        .0;
    let declaration_prefix = &declaration[declaration.len().saturating_sub(400)..];
    assert!(!declaration_prefix.contains("derive(Clone"));
    assert!(!declaration_prefix.contains("derive(Copy"));
}

#[test]
fn safe_and_outer_mappings_preserve_the_audited_chronology() {
    let source = include_str!("rns_native_direct_global_membership_handoff.rs");
    let safe = source
        .split_once("fn source_packing_safe_core_v1(")
        .expect("safe-core mapper")
        .1
        .split_once("/// Move-only evidence")
        .expect("safe-core mapper boundary")
        .0;
    for required in [
        "terminal_predecessor_context_binding_digest",
        "candidate_pre_direct_inventory_context_digest",
        "candidate_pre_direct_inventory_root",
        "existing_radix_candidate_root",
        "direct_core_safe_digest",
    ] {
        assert!(safe.contains(required), "safe-core omission: {required}");
    }
    for forbidden in [
        "inventory_prior_context_digest",
        "inventory_binding_digest",
        "direct_binding_digest",
        "codec_digest",
        "membership_residual",
        "combined_outer_binding_digest",
    ] {
        assert!(
            !safe.contains(forbidden),
            "successor-dependent safe-core input: {forbidden}"
        );
    }

    let outer = source
        .split_once("let mut outer_bindings = RnsNativeSourcePackingCombinedOuterBindingsV1")
        .expect("outer mapping")
        .1
        .split_once("outer_bindings.combined_outer_binding_digest =")
        .expect("outer mapping boundary")
        .0;
    let fields = [
        "source_statement_anchor_digest:",
        "source_final_aggregation_schedule_digest:",
        "enclosing_packing_binding_digest:",
        "inventory_prior_context_digest:",
        "inventory_root:",
        "inventory_continuation_digest:",
        "inventory_binding_digest:",
        "direct_binding_digest:",
        "comparator_binding_digest,",
        "comparator_range_carry_binding_digest,",
        "small_sign_disjointness_binding_digest,",
        "q_mask_linear_relations_binding_digest,",
        "existing_radix_binding_digest,",
        "radix_complement_binding_digest,",
        "centering_subtraction_binding_digest,",
        "global_lookup_pre_z_binding_digest,",
        "global_lookup_post_z_binding_digest,",
        "global_inverse_product_binding_digest,",
        "global_membership_binding_digest,",
    ];
    let mut prior = 0;
    for (index, field) in fields.into_iter().enumerate() {
        let position = outer
            .find(field)
            .unwrap_or_else(|| panic!("outer omission: {field}"));
        assert!(index == 0 || prior < position, "outer order: {field}");
        prior = position;
    }
    assert!(outer.contains(
        "enclosing_packing_binding_digest: inventory.enclosing_packing_binding_digest_v1()"
    ));
    assert!(outer.contains(
        "source_final_aggregation_schedule_digest: linked_source.aggregation_schedule_digest()"
    ));
    assert!(outer.contains("global_lookup_post_z_binding_digest,"));
    assert!(!outer.contains("post_z_transcript_digest"));
    assert!(source.contains("outer_bindings.canonical_combined_outer_binding_digest_v1()"));

    let direct = include_str!("rns_native_cross_field_rlwe_direct.rs");
    let claimed = direct
        .split_once("pub(super) fn verify_rns_native_cross_field_rlwe_claimed_with_alias_v2")
        .expect("claimed-carrier verifier")
        .1
        .split_once("#[cfg(test)]")
        .expect("claimed-carrier verifier boundary")
        .0;
    let final_axes = claimed
        .find("validate_claimed_handoff_fixed_axes_v1(")
        .expect("final transcript/fixed axes");
    let identity = claimed
        .find("same_borrowed_slice_identity_v1(preflight.successor, exact_claimed_successor)")
        .expect("preflight slice identity");
    let four_core = claimed
        .find("verify_kernel_for_suite_v1::<ZkAmsT256BulletproofSuiteV1, _>(")
        .expect("existing four-core verifier");
    let cursor_complete = claimed
        .find("if !numeric_sidecar.is_complete_v2()")
        .expect("numeric cursor completion");
    let verified_identity = claimed
        .find("same_borrowed_slice_identity_v1(equality_pending.successor(), exact_claimed_successor)")
        .expect("verified slice identity");
    let equality = claimed
        .find("discharge_claimed_root_equality_v1()")
        .expect("claimed-root equality");
    assert!(
        final_axes < identity
            && identity < four_core
            && four_core < cursor_complete
            && cursor_complete < verified_identity
            && verified_identity < equality
    );

    let safe_digest = direct
        .split_once("fn direct_core_safe_digest_v1(")
        .expect("direct-core safe digest")
        .1
        .split_once("/// Non-authorizing projection")
        .expect("direct-core safe digest boundary")
        .0;
    for required in [
        "DIRECT_CORE_SAFE_DOMAIN_V1",
        "private_cross_field_core_root",
        "q_mask_s_root",
        "numeric_root",
        "commitment_root",
    ] {
        assert!(
            safe_digest.contains(required),
            "safe digest omission: {required}"
        );
    }
    for forbidden in [
        "successor",
        "codec",
        "binding_digest",
        "inventory",
        "continuation",
    ] {
        assert!(
            !safe_digest.contains(forbidden),
            "unsafe direct-core digest input: {forbidden}"
        );
    }
}
