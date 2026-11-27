//! Retirement history shape checks cover the original current and rollback cuts.

use super::*;
use iroha_data_model::runtime::{definition::DefinitionId, retirement::RuntimeRetirementStateV1};

fn record() -> RuntimeRetirementStateV1 {
    RuntimeRetirementStateV1 {
        history_started: 2,
        last_required: Some(3),
        governed_excluded_at: None,
        receipt: None,
    }
}

#[test]
fn retirement_restore_rejects_invalid_current_and_undo_history() {
    let world = World::default();
    let budget = world.sccp_committee_generations.allocation_budget().clone();
    let definition = DefinitionId([0x79; 32]);
    {
        let mut block = world.block();
        block.runtime_retirements.insert(definition, record());
        block.commit();
    }
    validate_runtime_manifest_and_retirement_shapes(&world, &budget).unwrap();
    {
        let mut block = world.block();
        let mut invalid = record();
        invalid.history_started = 1;
        block.runtime_retirements.insert(definition, invalid);
        block.commit();
    }
    let error = validate_runtime_manifest_and_retirement_shapes(&world, &budget).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("invalid permanent runtime history")
    );
    {
        let mut block = world.block();
        block.runtime_retirements.insert(definition, record());
        block.commit();
    }
    let error = validate_runtime_manifest_and_retirement_shapes(&world, &budget).unwrap_err();
    assert!(
        error.to_string().contains("runtime_retirements.revert"),
        "{error}"
    );
    assert_eq!(
        world.runtime_retirements.view().get(&definition),
        Some(&record())
    );
}

#[test]
fn retirement_restore_preserves_original_undo_resource_refusal_and_retry() {
    let world = World::default();
    let budget = world.sccp_committee_generations.allocation_budget().clone();
    let limit = budget.limit_bytes();
    budget.set_limit_bytes(0);
    let error = validate_runtime_manifest_and_retirement_shapes(&world, &budget).unwrap_err();
    assert!(matches!(error, StateRestoreError::Admission(_)), "{error}");
    budget.set_limit_bytes(limit);
    validate_runtime_manifest_and_retirement_shapes(&world, &budget).unwrap();
}

#[test]
fn retirement_restore_requires_a_selected_history_owner_before_root_authentication() {
    let state = State::new_for_testing(
        World::default(),
        crate::kura::Kura::blank_kura_for_testing(),
        crate::query::store::LiveQueryStore::start_test(),
    );
    validate_runtime_retirement_roots(&state).unwrap();
    {
        let mut block = state.world.block();
        block
            .runtime_retirements
            .insert(DefinitionId([0x79; 32]), record());
        block.commit();
    }
    let error = validate_runtime_retirement_roots(&state).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unselected runtime retirement history"),
        "{error}"
    );
    {
        let mut block = state.world.block();
        block.runtime_retirements.remove(DefinitionId([0x79; 32]));
        block.commit();
    }
    let error = validate_runtime_retirement_roots(&state).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("unselected runtime retirement history"),
        "{error}"
    );
}

#[test]
fn c10_restore_authenticates_original_manifest_and_binding_at_both_cuts() {
    use crate::{
        manifest_signing_test_support::ManifestSigningFixture,
        smartcontracts::{
            code::registered_manifest::RegisteredManifest, ivm::profile::ResolvedIvmProfile,
        },
    };
    use iroha_data_model::{ivm_c10::ContractManifestC10V1, smart_contract::ContractArtifactId};
    use iroha_model_base::topology::DataSpaceId;

    let (bytes, embedded) = kotodama_lang::compiler::Compiler::new_c10()
        .compile_source_with_manifest(
            "seiyaku Stored { view fn amount() authorize(anyone) -> int { return 4; } }",
        )
        .unwrap();
    let key = ContractArtifactId::new(DataSpaceId::UNIVERSAL, embedded.code_hash.unwrap());
    let signing = ManifestSigningFixture::new();
    let owner = iroha_crypto::KeyPair::try_random().unwrap();
    let manifest = ContractManifestC10V1::try_from(embedded)
        .unwrap()
        .try_signed(signing.context(), signing.max_frame_bytes(), &owner)
        .unwrap();
    let host = ResolvedIvmProfile::of_id(ivm::IvmProfileId::C10_ABI_V1).definition_id();
    let world = World::default();
    assert!(!RegisteredManifest::captured_c10(&world.view(), &key).unwrap());
    crate::runtime::activate_additions_for_testing(
        &world,
        crate::runtime::catalog::additions::IVM_C10.module,
    )
    .unwrap();
    let selected = world.view().runtime_selection().clone();
    crate::runtime::activate_additions_for_testing(
        &world,
        crate::runtime::catalog::additions::IVM_C10.module,
    )
    .unwrap();
    assert_eq!(world.view().runtime_selection(), &selected);
    let budget = world.sccp_committee_generations.allocation_budget().clone();
    {
        let mut block = world.block();
        block.contract_code.insert(key, bytes);
        block.c10_contract_manifests.insert(key, manifest.clone());
        block.c10_contract_profiles.insert(key, host);
        block.commit();
    }
    {
        let view = world.view();
        assert!(RegisteredManifest::captured_c10(&view, &key).unwrap());
        let registered = RegisteredManifest::get(&view, &key).unwrap();
        assert_eq!(registered.permissions(), manifest.permissions.as_slice());
        let entry = manifest
            .entrypoints
            .as_ref()
            .unwrap()
            .iter()
            .find(|e| e.name == "amount")
            .unwrap();
        assert_eq!(
            registered.authorization("amount"),
            Some((entry.kind, &entry.authorization))
        );
        assert!(registered.authorization("missing").is_none());
    }
    validate_runtime_manifest_and_retirement_shapes(&world, &budget).unwrap();
    {
        let mut block = world.block();
        let mut substituted = manifest.clone();
        substituted.provenance.as_mut().unwrap().signer = iroha_crypto::KeyPair::try_random()
            .unwrap()
            .public_key()
            .clone();
        block.c10_contract_manifests.insert(key, substituted);
        block.commit();
    }
    let error = validate_runtime_manifest_and_retirement_shapes(&world, &budget).unwrap_err();
    assert!(
        error.to_string().contains("signature verification failed"),
        "{error}"
    );
    {
        let mut block = world.block();
        block.c10_contract_manifests.insert(key, manifest);
        block.commit();
    }
    let error = validate_runtime_manifest_and_retirement_shapes(&world, &budget).unwrap_err();
    assert!(
        error.to_string().contains("signature verification failed"),
        "{error}"
    );
    {
        let mut block = world.block();
        block.c10_contract_profiles.remove(key);
        block.commit();
    }
    let error = validate_runtime_manifest_and_retirement_shapes(&world, &budget).unwrap_err();
    assert!(error.to_string().contains("profile binding"), "{error}");
    assert!(RegisteredManifest::captured_c10(&world.view(), &key).is_err());
}

#[test]
fn c10_restore_refuses_a_profile_row_outside_its_installed_storage() {
    let world = World::default();
    let budget = world.sccp_committee_generations.allocation_budget().clone();
    let artifact = iroha_data_model::smart_contract::ContractArtifactId::new(
        iroha_model_base::topology::DataSpaceId::UNIVERSAL,
        iroha_crypto::Hash::new(b"unselected C10"),
    );
    let mut block = world.block();
    block
        .c10_contract_profiles
        .insert(artifact, DefinitionId([9; 32]));
    block.commit();
    let error = validate_runtime_manifest_and_retirement_shapes(&world, &budget).unwrap_err();
    assert!(
        error.to_string().contains("uninstalled C10 storage"),
        "{error}"
    );
}
