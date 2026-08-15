const PRODUCTION_SOURCE_V1: &str = include_str!("state_owned_secret_adapter_v1.rs");
const TEST_SOURCE_V1: &str = include_str!("state_owned_secret_adapter_v1_tests.rs");
const CPK_PARENT_SOURCE_V1: &str = include_str!("../cpk_relation.rs");
const COLLECTIVE_SOURCE_V1: &str = include_str!("../collective.rs");
const ACTIVE_BINDING_SOURCE_V1: &str = include_str!("../active_exact_binding.rs");
const MKHE_FACADE_SOURCE_V1: &str = include_str!("../../mkhe.rs");

#[test]
fn adapter_is_private_public_only_and_authority_neutral() {
    assert!(PRODUCTION_SOURCE_V1.lines().count() <= 220);
    assert!(PRODUCTION_SOURCE_V1.len() <= 10_000);
    assert!(TEST_SOURCE_V1.lines().count() <= 180);
    assert!(TEST_SOURCE_V1.len() <= 8_000);
    for forbidden in [
        ["VerifiedPersistent", "WitnessBindingV1"].concat(),
        ["mint_", "collective_secret_binding"].concat(),
        ["from_verified_", "membership"].concat(),
        ["ZkAmsMkheCpkRelation", "ProofV1"].concat(),
        ["DirectRelation", "ProverSession"].concat(),
        ["pub struct StateOwned", "CpkSecretMembershipPrecursorV1"].concat(),
        [
            "impl Clone for StateOwned",
            "CpkSecretMembershipPrecursorV1",
        ]
        .concat(),
        ["impl Copy for StateOwned", "CpkSecretMembershipPrecursorV1"].concat(),
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
    assert_eq!(
        ACTIVE_BINDING_SOURCE_V1
            .matches("pub(super) fn mint_collective_secret_binding_from_verified_cpk_v1(")
            .count(),
        1
    );
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
