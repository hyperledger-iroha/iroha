//! Addressed contract permissions have local scope without creating delegation authority.

use super::*;
use iroha_data_model::{
    isi::{Grant, Revoke},
    permission::Permission,
    smart_contract::ContractArtifactId,
};
use iroha_executor_data_model::permission::smart_contract::{
    CanInvokeContractEntrypoint, CanUseContractPermission,
};
use iroha_primitives::json::Json;
use iroha_test_samples::BOB_ID;

fn permissions(contract: &ContractAddress) -> [Permission; 2] {
    [
        CanInvokeContractEntrypoint {
            contract: contract.clone(),
            entrypoint: "hajimari".into(),
        }
        .into(),
        CanUseContractPermission {
            contract: contract.clone(),
            permission: "write".parse().unwrap(),
        }
        .into(),
    ]
}

fn wrappers(permission: Permission) -> [InstructionBox; 4] {
    [
        Grant::account_permission(permission.clone(), BOB_ID.clone()).into(),
        Revoke::account_permission(permission.clone(), BOB_ID.clone()).into(),
        Grant::role_permission(permission.clone(), "service".parse().unwrap()).into(),
        Revoke::role_permission(permission, "service".parse().unwrap()).into(),
    ]
}

#[test]
fn private_contract_permission_scope_requires_exact_local_address_and_payload() {
    let (state, ds) = private_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    bind_scope(&mut tx, ds);
    for target in [ds, DataSpaceId::new(17), DataSpaceId::UNIVERSAL] {
        let contract = ContractAddress::derive(&tx.network_id, &ALICE_ID, 0, target).unwrap();
        for permission in permissions(&contract) {
            for instruction in wrappers(permission.clone()) {
                assert_eq!(
                    ensure_instruction_scope(&instruction, &mut tx).is_ok(),
                    target == ds
                );
            }
            if target != ds {
                continue;
            }
            let mut extra: norito::json::Value = permission.payload().try_into_any().unwrap();
            extra
                .as_object_mut()
                .unwrap()
                .insert("extra".into(), norito::json::Value::Bool(true));
            for malformed in [
                Permission::new(permission.name().into(), Json::try_new(extra).unwrap()),
                Permission::new(permission.name().into(), "null".parse::<Json>().unwrap()),
                Permission::new("UnreviewedPermission".into(), permission.payload().clone()),
                iroha_executor_data_model::permission::query::CanReadAllLedgerData.into(),
            ] {
                for instruction in wrappers(malformed) {
                    assert!(ensure_instruction_scope(&instruction, &mut tx).is_err());
                    assert!(!tx.is_execution_deferred());
                }
            }
        }
    }
}

#[test]
fn private_contract_permission_grant_and_revoke_keep_current_owner_authority() {
    let (state, ds) = private_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    bind_scope(&mut tx, ds);
    let contract = ContractAddress::derive(&tx.network_id, &ALICE_ID, 0, ds).unwrap();
    let code_hash = Hash::new(b"private contract permission fixture");
    let (_, mut manifest) = kotodama_lang::compiler::Compiler::new()
        .compile_source_with_manifest(
            "seiyaku PrivatePermissionFixture { hajimari() {} permission write; \
             kotoage fn write_value() authorize(write) {} }",
        )
        .unwrap();
    manifest.code_hash = Some(code_hash);
    tx.world.contract_manifests.insert(
        ContractArtifactId::for_address(&contract, code_hash).unwrap(),
        manifest,
    );
    for account in [BOB_ID.clone(), contract.subject_id()] {
        tx.world.accounts.insert(
            account,
            iroha_data_model::account::AccountValue::new(Default::default()),
        );
    }
    tx.world
        .contract_instances
        .insert(contract.clone(), code_hash);
    tx.world
        .contract_subject_addresses
        .insert(contract.subject_id(), contract.clone());
    tx.world.contract_subject_bindings.insert(
        contract.clone(),
        crate::smartcontracts::code::ContractSubjectBinding::new_direct(
            &contract,
            ALICE_ID.clone(),
        )
        .with_active_code_hash(code_hash),
    );
    for permission in permissions(&contract) {
        let grant: InstructionBox =
            Grant::account_permission(permission.clone(), BOB_ID.clone()).into();
        let revoke: InstructionBox =
            Revoke::account_permission(permission.clone(), BOB_ID.clone()).into();
        assert!(ensure_instruction_scope(&grant, &mut tx).is_ok());
        for instruction in [&grant, &revoke] {
            let error = Executor::Initial
                .execute_borrowed_overlay_instruction(&mut tx, &BOB_ID, instruction, None)
                .unwrap_err();
            assert!(
                matches!(error, ValidationFail::NotPermitted(ref reason)
                    if reason.contains("exact holder") && reason.contains("lifecycle owner")),
                "scope admission must retain delegation authorization: {error}"
            );
        }
        Executor::Initial
            .execute_instruction(&mut tx, &ALICE_ID, grant)
            .unwrap();
        assert!(
            crate::executor::authority_has_permission(&tx.world, &BOB_ID, &permission).unwrap()
        );
        Executor::Initial
            .execute_borrowed_overlay_instruction(&mut tx, &ALICE_ID, &revoke, None)
            .unwrap();
        assert!(
            !crate::executor::authority_has_permission(&tx.world, &BOB_ID, &permission).unwrap()
        );
    }
}

#[test]
fn private_contract_permission_decode_refusal_keeps_retry_owner() {
    let (state, ds) = private_state();
    let mut block = state.block(header(2));
    let mut tx = block.transaction();
    bind_scope(&mut tx, ds);
    let scope = execution_root_scope(&mut tx).unwrap();
    let contract = ContractAddress::derive(&tx.network_id, &ALICE_ID, 0, ds).unwrap();
    let [instruction, ..] = wrappers(permissions(&contract)[0].clone());
    let result = norito::core::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64),
        || ensure_private_instruction(&instruction, &mut tx, scope, ds, 0),
    );
    assert!(result.is_err());
    assert!(tx.is_execution_deferred());
    assert!(
        tx.borrow_execution_deferral()
            .unwrap()
            .decode_refusal()
            .is_some()
    );
}
