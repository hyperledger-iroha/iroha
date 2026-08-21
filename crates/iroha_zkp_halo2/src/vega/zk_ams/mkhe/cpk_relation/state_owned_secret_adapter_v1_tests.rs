const PRODUCTION_SOURCE_V1: &str = include_str!("state_owned_secret_adapter_v1.rs");
const TEST_SOURCE_V1: &str = include_str!("state_owned_secret_adapter_v1_tests.rs");
const CPK_PARENT_SOURCE_V1: &str = include_str!("../cpk_relation.rs");
const COLLECTIVE_SOURCE_V1: &str = include_str!("../collective.rs");
const BORROWED_PRODUCT_SOURCE_V1: &str = include_str!("../collective/borrowed_product.rs");
const ACTIVE_BINDING_SOURCE_V1: &str = include_str!("../active_exact_binding.rs");
const MKHE_FACADE_SOURCE_V1: &str = include_str!("../../mkhe.rs");

#[test]
fn adapter_is_private_public_only_and_authority_neutral() {
    assert!(PRODUCTION_SOURCE_V1.lines().count() <= 260);
    assert!(PRODUCTION_SOURCE_V1.len() <= 11_000);
    assert!(TEST_SOURCE_V1.lines().count() <= 180);
    assert!(TEST_SOURCE_V1.len() <= 8_000);
    for forbidden in [
        ["VerifiedPersistent", "WitnessBindingV1"].concat(),
        ["mint_", "collective_secret_binding"].concat(),
        ["from_verified_", "membership"].concat(),
        "fn response_coefficient_v1".to_owned(),
        "pub struct ReopenedStateOwnedCpkRelationPrecursorV1".to_owned(),
        ["pub struct StateOwned", "CpkSecretMembershipPrecursorV1"].concat(),
        "impl Clone for ReopenedStateOwnedCpkRelationPrecursorV1".to_owned(),
        "impl Copy for ReopenedStateOwnedCpkRelationPrecursorV1".to_owned(),
        "impl core::fmt::Debug for ReopenedStateOwnedCpkRelationPrecursorV1".to_owned(),
        ["Norito", "Serialize"].concat(),
        ["Norito", "Deserialize"].concat(),
        [".blindings", ".as_array()"].concat(),
        [".secret", ".coefficients"].concat(),
    ] {
        assert!(
            !PRODUCTION_SOURCE_V1.contains(&forbidden),
            "forbidden adapter surface: {forbidden}"
        );
    }
    assert!(CPK_PARENT_SOURCE_V1.contains(
        "#[path = \"cpk_relation/state_owned_secret_adapter_v1.rs\"]\npub(super) mod state_owned_secret_adapter_v1;"
    ));
    assert!(!MKHE_FACADE_SOURCE_V1.contains("StateOwnedCpkSecretMembershipPrecursorV1"));
    for terminal in [
        "fn into_proved_public_v1",
        ".consume_sealed_cpk_abort_session_v1(",
    ] {
        assert_eq!(PRODUCTION_SOURCE_V1.matches(terminal).count(), 1);
    }
    let mint = "pub(super) fn mint_collective_secret_binding_from_verified_cpk_v1(";
    assert_eq!(ACTIVE_BINDING_SOURCE_V1.matches(mint).count(), 1);
}

#[test]
fn pointer_axes_context_and_eight_points_precede_public_precursor() {
    let adapter = PRODUCTION_SOURCE_V1
        .split("fn prove_state_owned_cpk_secret_membership_v1")
        .nth(1)
        .expect("state-owned adapter")
        .split("fn map_relation_error_v1")
        .next()
        .expect("state-owned adapter boundary");
    let pointer = adapter
        .find("party_b_pointer.payload_blake3() != expected_party_b_payload_blake3")
        .expect("party-b BLAKE3 validation");
    let statement = adapter
        .find("ZkAmsMkheCpkShareStatementV1::from_governed_roster(")
        .expect("governed statement derivation");
    let axes = adapter
        .find("statement.profile_digest != lease.profile_digest()")
        .expect("lease/statement axis comparison");
    let context = adapter
        .find("ZkAmsMkhePersistentMembershipContextV1::from_relation_axes(")
        .expect("membership context derivation");
    let proof = adapter
        .find(".prove(context, random)")
        .expect("exclusive lease consumption");
    let return_value = adapter
        .find("Ok(StateOwnedCpkSecretMembershipPrecursorV1 {")
        .expect("opaque public precursor");
    assert!(pointer < statement && statement < axes && axes < context && context < proof);
    assert!(proof < return_value);
    assert!(adapter.contains("secret_membership.commitments().len() != 8"));
}

#[test]
fn collective_validates_share_and_pointer_before_narrowing_or_randomness() {
    let producer = COLLECTIVE_SOURCE_V1
        .split("pub(super) fn prove_state_owned_cpk_secret_membership_v1")
        .nth(1)
        .expect("collective precursor producer")
        .split("/// Admit the move-only party binding")
        .next()
        .expect("collective precursor boundary");
    let share = producer
        .find("validate_state_owned_cpk_source_v1(roster, share)?")
        .expect("share validation");
    let hash = producer
        .find("cpk_party_b_payload_blake3_v1(&share.party_public_b)?")
        .expect("party-b hash derivation");
    let pointer = producer
        .find("party_b_pointer.payload_blake3() != expected_party_b_payload_blake3")
        .expect("pointer comparison");
    let lease = producer
        .find("persistent_direct_opening_lease_v1(roster, share)?")
        .expect("exclusive lease");
    let proof = producer
        .find("state_owned_secret_adapter_v1::prove_state_owned_cpk_secret_membership_v1(")
        .expect("sealed adapter call");
    assert!(share < hash && hash < pointer && pointer < lease && lease < proof);
}

#[test]
fn state_owned_membership_inputs_keep_exact_reported_capacities() {
    let boxed = PRODUCTION_SOURCE_V1
        .split("fn into_exact_wire_box_v1")
        .nth(1)
        .expect("exact membership box")
        .split("/// Derive the exact CPK statement")
        .next()
        .expect("exact box boundary");
    for needle in [
        "bytes.capacity() != N",
        "let allocation = bytes.as_ptr();",
        "boxed.as_ptr() != allocation",
    ] {
        assert!(boxed.contains(needle));
    }
    let narrowing = COLLECTIVE_SOURCE_V1
        .split("impl ZeroizingT256MembershipCoefficientsV1")
        .nth(1)
        .expect("narrowing owner")
        .split("impl Drop for ZeroizingT256MembershipCoefficientsV1")
        .next()
        .expect("narrowing boundary");
    for needle in [
        "coefficients.0.capacity() != expected_coefficients",
        "coefficients.0.as_ptr() != allocation",
    ] {
        assert!(narrowing.contains(needle));
    }
    let commitments = COLLECTIVE_SOURCE_V1
        .split("fn commit_cpk_membership_opening_v1")
        .nth(1)
        .expect("commitment builder")
        .split("const _: ()")
        .next()
        .expect("commitment boundary");
    assert!(commitments.contains("commitments.capacity() != blindings.len()"));
    assert!(commitments.contains("commitments.as_ptr() != allocation"));
    let generator = COLLECTIVE_SOURCE_V1
        .split("pub fn generate_zk_ams_mkhe_collective_party_state_with_prepared_public_a_v1")
        .nth(1)
        .expect("party generator")
        .split("fn aggregate_zk_ams_mkhe_collective_public_key_v1")
        .next()
        .expect("party generator boundary");
    let capacity = generator
        .find("secret.coefficients.capacity() != profile.ring_degree")
        .expect("state witness capacity check");
    assert!(generator.contains("public_error.coefficients.capacity() != profile.ring_degree"));
    let multiplication = generator
        .find("borrowed_product::multiply_public_residues_by_secret_signed_v1(")
        .expect("party-b multiplication");
    assert!(capacity < multiplication);
    assert!(BORROWED_PRODUCT_SOURCE_V1.contains("values.capacity() != capacity"));
}
