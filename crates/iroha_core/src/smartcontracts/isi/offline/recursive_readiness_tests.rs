//! Kagemusha V4 recursive-readiness projection and release-selection tests.
use super::*;
use crate::state::World;
fn release_bound_record(
    parity: iroha_data_model::offline::KagemushaPastaCycleParityV1,
    manifest_sha256: [u8; 32],
) -> VerifyingKeyRecord {
    let binding = iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4 {
        version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        generation: "release-v4".to_owned(),
        manifest_sha256,
    };
    let owner = kagemusha_terminal_registry_v4::verifier_owner_manifest_id(&binding)
        .expect("test binding has a canonical owner id");
    let curve = match parity {
        iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq => "vesta",
        iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp => "pallas",
    };
    let mut record = VerifyingKeyRecord::new_with_owner(
        7,
        kagemusha_v4_circuit_id(parity),
        Some(owner),
        iroha_data_model::offline::KAGEMUSHA_VERIFIER_NAMESPACE,
        BackendTag::Halo2IpaPasta,
        curve,
        [0xA1; 32],
        [0xB2; 32],
    );
    record.max_proof_bytes = 1;
    record.activation_height = Some(5);
    record.status = ConfidentialStatus::Active;
    record
}
fn world_with_active_release_pairs(manifest_digests: &[[u8; 32]]) -> World {
    let mut world = World::default();
    for (index, manifest_sha256) in manifest_digests.iter().copied().enumerate() {
        let version = u32::try_from(index + 1).expect("test release version fits u32");
        for parity in [
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq,
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,
        ] {
            let mut record = release_bound_record(parity, manifest_sha256);
            record.version = version;
            let id = iroha_data_model::offline::kagemusha_recursive_spend_verifier_key_id_v4(
                parity,
                manifest_sha256,
            );
            world.verifying_keys.insert(id.clone(), record.clone());
            world
                .verifying_keys_by_circuit
                .insert((record.circuit_id.clone(), version), id);
        }
    }
    world
}
#[test]
fn release_qualified_verifier_id_rejects_generic_arbitrary_and_suffixed_substitutions() {
    let manifest_sha256 = [0xab; 32];
    let parity = iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq;
    let record = release_bound_record(parity, manifest_sha256);
    let exact = iroha_data_model::offline::kagemusha_recursive_spend_verifier_key_id_v4(
        parity,
        manifest_sha256,
    );
    ensure_release_qualified_kagemusha_v4_verifier_id(&exact, &record, parity, "Eq")
        .expect("the owner-derived release-qualified id must be accepted");
    assert_eq!(
        exact.backend.as_str(),
        iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
    );
    let arbitrary =
        iroha_data_model::offline::kagemusha_recursive_spend_verifier_key_id_v4(parity, [0x43; 32]);
    let suffixed = iroha_data_model::proof::VerifyingKeyId::new(
        iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4,
        format!("{}-substitute", exact.name),
    );
    let generic = iroha_data_model::proof::VerifyingKeyId::new(
        crate::zk::ZK_BACKEND_HALO2_IPA,
        exact.name.clone(),
    );
    for substituted in [&arbitrary, &suffixed, &generic] {
        assert!(
            ensure_release_qualified_kagemusha_v4_verifier_id(substituted, &record, parity, "Eq",)
                .is_err(),
            "a substituted registry identity must fail closed: {substituted:?}"
        );
    }
    let mut noncanonical_owner = record.clone();
    noncanonical_owner.owner_manifest_id = Some(format!(
        "kagemusha-v4-{}",
        hex::encode(manifest_sha256).to_uppercase()
    ));
    assert!(
        ensure_release_qualified_kagemusha_v4_verifier_id(
            &exact,
            &noncanonical_owner,
            parity,
            "Eq",
        )
        .is_err(),
        "the owner digest must use its canonical lowercase form"
    );
    let wrong_parity_record = release_bound_record(
        iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,
        manifest_sha256,
    );
    assert!(
        ensure_release_qualified_kagemusha_v4_verifier_id(
            &exact,
            &wrong_parity_record,
            parity,
            "Eq",
        )
        .is_err(),
        "the owner-derived id cannot substitute the opposite parity circuit"
    );
}
#[test]
fn readiness_projects_logical_roles_and_release_issuance_window() {
    let artifact_set = KagemushaAuthenticatedArtifactSetReadinessV4 {
        generation: "release-v4".to_owned(),
        manifest_sha256: [0x42; 32],
        release_policy_sha256: [0x43; 32],
        release_attestation_sha256: [0x44; 32],
        activation_height: 40,
        withdrawal_height: 80,
        max_proof_bytes: 65_536,
        asset_scale: 9,
    };
    for (parity, role, circuit) in [
        (
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq,
            iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_STEP_EQ_V4,
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_CIRCUIT_ID_V4,
        ),
        (
            iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEp,
            iroha_data_model::offline::KAGEMUSHA_VERIFIER_ROLE_STEP_EP_V4,
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_CIRCUIT_ID_V4,
        ),
    ] {
        let record = release_bound_record(parity, artifact_set.manifest_sha256);
        assert_eq!(record.withdraw_height, None);
        let projected = project_kagemusha_v4_verifier(parity, record, &artifact_set);
        assert_eq!(
            projected.id.backend.as_str(),
            crate::zk::ZK_BACKEND_HALO2_IPA
        );
        assert_eq!(projected.id.name, role);
        assert_eq!(projected.circuit_id, circuit);
        assert_eq!(projected.max_proof_bytes, artifact_set.max_proof_bytes);
        assert_eq!(projected.activation_height, artifact_set.activation_height);
        assert_eq!(
            projected.withdrawal_height,
            Some(artifact_set.withdrawal_height)
        );
    }
    assert!(!kagemusha_v4_issuance_active_at(40, 80, 39));
    assert!(kagemusha_v4_issuance_active_at(40, 80, 40));
    assert!(kagemusha_v4_issuance_active_at(40, 80, 79));
    assert!(!kagemusha_v4_issuance_active_at(40, 80, 80));
}
#[test]
fn startup_coverage_visits_historic_and_future_terminal_active_releases() {
    let world = world_with_active_release_pairs(&[[0x41; 32], [0x42; 32]]);
    let mut visited = Vec::new();
    let count = visit_active_kagemusha_v4_release_pairs(
        &world.view(),
        1,
        |step_eq_record, step_ep_record| {
            assert_eq!(step_eq_record.version, step_ep_record.version);
            visited.push(step_eq_record.version);
            Ok(())
        },
    )
    .expect("historic and not-yet-activated terminal releases must both be covered");
    assert_eq!(count, 2);
    assert_eq!(visited, [1, 2]);
}
#[test]
fn startup_coverage_fails_when_older_terminal_release_is_missing_locally() {
    let world = world_with_active_release_pairs(&[[0x41; 32], [0x42; 32]]);
    let error = visit_active_kagemusha_v4_release_pairs(&world.view(), 5, |step_eq_record, _| {
        if step_eq_record.version == 1 {
            return Err("older terminal release is absent from local catalog".to_owned());
        }
        Ok(())
    })
    .expect_err("missing historic redemption material must fail startup");
    assert!(error.contains("older terminal release"));
}
#[test]
fn startup_coverage_fails_before_future_release_activation_when_material_is_missing() {
    let world = world_with_active_release_pairs(&[[0x41; 32], [0x42; 32]]);
    let error = visit_active_kagemusha_v4_release_pairs(&world.view(), 1, |step_eq_record, _| {
        if step_eq_record.version == 2 {
            return Err("future terminal release is absent from local catalog".to_owned());
        }
        Ok(())
    })
    .expect_err("snapshot startup must preload future Active release material");
    assert!(error.contains("future terminal release"));
}
#[test]
fn issuance_window_overlap_is_half_open() {
    assert!(kagemusha_v4_issuance_windows_overlap(10, 20, 19, 30));
    assert!(kagemusha_v4_issuance_windows_overlap(10, 20, 10, 20));
    assert!(!kagemusha_v4_issuance_windows_overlap(10, 20, 20, 30));
    assert!(!kagemusha_v4_issuance_windows_overlap(20, 30, 10, 20));
}
#[test]
fn transaction_release_height_rejects_zero_future_and_pre_activation_snapshots() {
    let manifest_sha256 = [0x41; 32];
    let world = world_with_active_release_pairs(&[manifest_sha256]);
    let binding = iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4 {
        version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        generation: "release-v4".to_owned(),
        manifest_sha256,
    };
    let view = world.view();
    let parity = iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq;
    assert!(exact_kagemusha_v4_transaction_verifier_record(&view, &binding, parity, 0, 5).is_err());
    assert!(exact_kagemusha_v4_transaction_verifier_record(&view, &binding, parity, 6, 5).is_err());
    assert!(
        exact_kagemusha_v4_transaction_verifier_record(&view, &binding, parity, 4, 5).is_err(),
        "activation at the executing height must not validate a pre-activation request",
    );
    exact_kagemusha_v4_transaction_verifier_record(&view, &binding, parity, 5, 5)
        .expect("the exact activation-height snapshot is valid");
}
#[test]
fn transaction_release_height_rejects_release_withdrawn_by_execution() {
    let manifest_sha256 = [0x41; 32];
    let mut world = world_with_active_release_pairs(&[manifest_sha256]);
    let parity = iroha_data_model::offline::KagemushaPastaCycleParityV1::StepEq;
    let id = iroha_data_model::offline::kagemusha_recursive_spend_verifier_key_id_v4(
        parity,
        manifest_sha256,
    );
    let mut withdrawn = world
        .verifying_keys
        .view()
        .get(&id)
        .cloned()
        .expect("test Eq verifier");
    withdrawn.withdraw_height = Some(6);
    world.verifying_keys.insert(id, withdrawn);
    let binding = iroha_data_model::offline::KagemushaRecursiveSpendArtifactBindingV4 {
        version: iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_WIRE_VERSION_V4,
        generation: "release-v4".to_owned(),
        manifest_sha256,
    };
    assert!(
        exact_kagemusha_v4_transaction_verifier_record(&world.view(), &binding, parity, 5, 6,)
            .is_err(),
        "a release active at the signed snapshot but inactive now must fail closed",
    );
}
