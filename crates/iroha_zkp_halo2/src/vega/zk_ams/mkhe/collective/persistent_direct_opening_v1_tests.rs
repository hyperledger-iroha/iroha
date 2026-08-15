const PRODUCTION_SOURCE_V1: &str = include_str!("persistent_direct_opening_v1.rs");
const TEST_SOURCE_V1: &str = include_str!("persistent_direct_opening_v1_tests.rs");
const COLLECTIVE_SOURCE_V1: &str = include_str!("../collective.rs");
const MKHE_FACADE_SOURCE_V1: &str = include_str!("../../mkhe.rs");

#[test]
fn owner_is_move_only_private_and_contains_one_opening() {
    assert!(PRODUCTION_SOURCE_V1.lines().count() <= 180);
    assert!(PRODUCTION_SOURCE_V1.len() <= 8_000);
    assert!(TEST_SOURCE_V1.lines().count() <= 180);
    assert!(TEST_SOURCE_V1.len() <= 8_000);
    for forbidden in [
        ["derive(Clone", ", Copy)"].concat(),
        ["impl Clone for Persistent", "DirectOpeningOwnerV1"].concat(),
        ["impl Copy for Persistent", "DirectOpeningOwnerV1"].concat(),
        ["impl core::ops::", "Deref"].concat(),
        ["Norito", "Serialize"].concat(),
        ["Norito", "Deserialize"].concat(),
        ["pub ", "use persistent_direct_opening_v1"].concat(),
        ["fn ", "blindings("].concat(),
        ["fn ", "coefficients("].concat(),
        ["impl Fn", "Once"].concat(),
        "expected_".to_owned(),
    ] {
        assert!(
            !PRODUCTION_SOURCE_V1.contains(&forbidden),
            "forbidden owner surface: {forbidden}"
        );
    }
    let owner = PRODUCTION_SOURCE_V1
        .split("pub(super) struct PersistentDirectOpeningOwnerV1")
        .nth(1)
        .expect("opening owner")
        .split("impl PersistentDirectOpeningOwnerV1")
        .next()
        .expect("opening owner boundary");
    assert_eq!(owner.matches("secret: SecretPolynomial").count(), 1);
    assert_eq!(
        owner
            .matches("blindings: PersistentSecretCommitmentBlindingsV1")
            .count(),
        1
    );
    assert_eq!(
        owner
            .matches("verified_binding: Option<VerifiedPersistentWitnessBindingV1>")
            .count(),
        1
    );
    let guard = PRODUCTION_SOURCE_V1
        .split("pub(super) struct PostCpkPersistentDirectOpeningGuardV1")
        .nth(1)
        .expect("post-CPK guard")
        .split("impl<'a> PostCpkPersistentDirectOpeningGuardV1")
        .next()
        .expect("post-CPK guard boundary");
    assert_eq!(guard.matches(": ").count(), 4);
    let compact = PRODUCTION_SOURCE_V1
        .split("fn into_compacted_post_seal_v1")
        .nth(1)
        .expect("private compacting guard")
        .split("impl core::fmt::Debug")
        .next()
        .expect("private compacting guard boundary");
    for retained in [
        "self.owner",
        "self.public_error",
        "self.creation_mask_digit_burn",
    ] {
        assert_eq!(compact.matches(retained).count(), 1, "retained {retained}");
    }
    assert_eq!(compact.matches("drop(self.coefficients)").count(), 1);
    for forbidden in [
        "Result<",
        ".clone()",
        "validate(",
        "try_reserve",
        "consumer_mask",
    ] {
        assert!(
            !compact.contains(forbidden),
            "compaction operation: {forbidden}"
        );
    }
    assert!(!MKHE_FACADE_SOURCE_V1.contains("PersistentDirectOpeningOwnerV1"));
}

#[test]
fn constructor_commits_before_retaining_public_encodings() {
    let constructor = PRODUCTION_SOURCE_V1
        .split("pub(super) fn new_unverified")
        .nth(1)
        .expect("owner constructor")
        .split("impl core::fmt::Debug")
        .next()
        .expect("owner constructor boundary");
    let axes = constructor
        .find("axes.validate()?")
        .expect("axis validation");
    let narrowing = constructor
        .find("from_ternary_secret(&secret)?")
        .expect("erasing narrowing");
    let commitment = constructor
        .find("commit_persistent_secret_opening_v1(")
        .expect("eight commitment construction");
    let encoding = constructor
        .find("encode_persistent_opening_commitments_v1(&commitments)?")
        .expect("canonical point retention");
    let owner = constructor.find("Ok(Self {").expect("owner return");
    assert!(
        axes < narrowing && narrowing < commitment && commitment < encoding && encoding < owner
    );
    assert_eq!(
        super::super::ZK_AMS_MKHE_PERSISTENT_OPENING_RETAINED_POINT_BYTES_V1,
        264
    );
}

#[test]
fn collective_state_has_no_parallel_secret_or_blinding_owner() {
    let state = COLLECTIVE_SOURCE_V1
        .split("pub struct ZkAmsMkheCollectivePartyStateV1")
        .nth(1)
        .expect("party state")
        .split("impl core::fmt::Debug for ZkAmsMkheCollectivePartyStateV1")
        .next()
        .expect("party state boundary");
    assert_eq!(state.matches("PersistentDirectOpeningOwnerV1").count(), 1);
    for forbidden in [
        "persistent_secret_binding:",
        "persistent_secret_commitment_blindings:",
        "secret: SecretPolynomial",
    ] {
        assert!(
            !state.contains(forbidden),
            "parallel opening field: {forbidden}"
        );
    }
    let generator = COLLECTIVE_SOURCE_V1
        .split("pub fn generate_zk_ams_mkhe_collective_party_state_with_prepared_public_a_v1")
        .nth(1)
        .expect("party-state producer")
        .split("fn aggregate_zk_ams_mkhe_collective_public_key_v1")
        .next()
        .expect("party-state producer boundary");
    assert_eq!(
        generator
            .matches("PersistentDirectOpeningOwnerV1::new_unverified")
            .count(),
        1
    );
    assert!(!generator.contains("secret.clone()"));
    assert!(!generator.contains("persistent_secret_commitment_blindings.clone()"));
}

#[test]
fn sealed_lease_rechecks_all_eight_canonical_points() {
    let lease = COLLECTIVE_SOURCE_V1
        .split("impl PersistentDirectOpeningLeaseV1")
        .nth(1)
        .expect("exclusive lease implementation")
        .split("/// Opaque RLWE state")
        .next()
        .expect("exclusive lease boundary");
    let proof = lease
        .find("ZkAmsMkhePersistentMembershipEvidenceV1::prove(")
        .expect("membership proof");
    let commitments = lease
        .find("let commitments = evidence.commitments()")
        .expect("all evidence commitments");
    let encoding = lease
        .find("encode_persistent_opening_commitments_v1(&commitments)")
        .expect("canonical commitment encoding");
    let comparison = lease
        .find("encoded != self.owner.retained_commitment_wire")
        .expect("retained-point comparison");
    assert!(proof < commitments && commitments < encoding && encoding < comparison);
    assert!(!lease.contains("pub(super) const fn blindings"));
    assert!(!lease.contains("pub(super) const fn coefficients"));
}
