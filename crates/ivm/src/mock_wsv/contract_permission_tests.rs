//! Exact instance permission delegation in the local WSV host.

use super::*;

fn tlv(pointer_type: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(7 + payload.len() + CryptoHash::LENGTH);
    result.extend_from_slice(&(pointer_type as u16).to_be_bytes());
    result.push(1);
    result.extend_from_slice(&u32::try_from(payload.len()).unwrap().to_be_bytes());
    result.extend_from_slice(payload);
    result.extend_from_slice(CryptoHash::new(payload).as_ref());
    result
}

fn fixture() -> (WsvHost, IVM, ContractAddress, AccountId) {
    let actor = test_account_id(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
        "wonderland",
    );
    let contract = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .unwrap(),
        &actor,
        1,
        DataSpaceId::UNIVERSAL,
    )
    .unwrap();
    let program = kotodama_lang::compiler::Compiler::new()
        .compile_source(
            "seiyaku Grants {
                permission Member;
                import permission \"ChainRole\" as Shared;
                kotoage fn manage() authorize(anyone) {}
            }",
        )
        .unwrap();
    let mut vm = IVM::new(u64::MAX);
    vm.load_program(&program).unwrap();
    let host = WsvHost::new_with_subject(MockWorldStateView::new(), actor.clone());
    (host, vm, contract, actor)
}

fn call(
    host: &mut WsvHost,
    vm: &mut IVM,
    actor: &AccountId,
    permission: &str,
    syscall: u32,
) -> Result<u64, VMError> {
    let subject = vm
        .alloc_input_tlv(&tlv(
            PointerType::AccountId,
            &norito::to_bytes(actor).unwrap(),
        ))
        .unwrap();
    let name: Name = permission.parse().unwrap();
    let permission = vm
        .alloc_input_tlv(&tlv(PointerType::Name, &norito::to_bytes(&name).unwrap()))
        .unwrap();
    vm.set_register(10, subject);
    vm.set_register(11, permission);
    host.syscall(syscall, vm)
}

#[test]
fn contract_permission_delegation_requires_instance_and_exact_effect_authority() {
    let (mut host, mut vm, contract, actor) = fixture();
    let grant = syscalls::SYSCALL_GRANT_CONTRACT_PERMISSION;
    assert!(matches!(
        call(&mut host, &mut vm, &actor, "Member", grant),
        Err(VMError::PermissionDenied)
    ));
    host.bind_contract_runtime_context(actor.clone(), contract.clone(), "manage".into())
        .unwrap();
    assert!(matches!(
        call(&mut host, &mut vm, &actor, "Member", grant),
        Err(VMError::PermissionDenied)
    ));
    let token = PermissionToken::ContractPermission {
        contract: contract.clone(),
        permission: "Member".parse().unwrap(),
    };
    host.wsv.grant_permission(&actor, token.clone());
    assert!(matches!(
        call(&mut host, &mut vm, &actor, "Member", grant),
        Err(VMError::PermissionDenied)
    ));
    host.wsv.revoke_permission(&actor, &token);
    host.wsv
        .grant_permission(&contract.subject_id(), token.clone());
    call(&mut host, &mut vm, &actor, "Member", grant).unwrap();
    assert!(host.wsv.has_permission(&actor, &token));
    assert!(matches!(
        call(&mut host, &mut vm, &actor, "Member", grant),
        Err(VMError::PermissionDenied)
    ));
    call(
        &mut host,
        &mut vm,
        &actor,
        "Member",
        syscalls::SYSCALL_REVOKE_CONTRACT_PERMISSION,
    )
    .unwrap();
    assert!(!host.wsv.has_permission(&actor, &token));
    assert!(matches!(
        call(
            &mut host,
            &mut vm,
            &actor,
            "Member",
            syscalls::SYSCALL_REVOKE_CONTRACT_PERMISSION
        ),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn contract_permission_delegation_rejects_shared_and_undeclared_names() {
    let (mut host, mut vm, contract, actor) = fixture();
    host.bind_contract_runtime_context(actor.clone(), contract.clone(), "manage".into())
        .unwrap();
    for name in ["Shared", "Typo"] {
        host.wsv.grant_permission(
            &contract.subject_id(),
            PermissionToken::ContractPermission {
                contract: contract.clone(),
                permission: name.parse().unwrap(),
            },
        );
        assert!(matches!(
            call(
                &mut host,
                &mut vm,
                &actor,
                name,
                syscalls::SYSCALL_GRANT_CONTRACT_PERMISSION
            ),
            Err(VMError::PermissionDenied)
        ));
    }
    let wrong_type = vm
        .alloc_input_tlv(&tlv(PointerType::Blob, b"Member"))
        .unwrap();
    vm.set_register(11, wrong_type);
    assert!(matches!(
        host.syscall(syscalls::SYSCALL_GRANT_CONTRACT_PERMISSION, &mut vm),
        Err(VMError::NoritoInvalid)
    ));
}

fn transfer_permission_call(
    host: &mut WsvHost,
    vm: &mut IVM,
    recipient: &AccountId,
    permission: &Json,
    syscall: u32,
) -> Result<u64, VMError> {
    let recipient = vm
        .alloc_input_tlv(&tlv(
            PointerType::AccountId,
            &norito::to_bytes(recipient).unwrap(),
        ))
        .unwrap();
    let permission = vm
        .alloc_input_tlv(&tlv(
            PointerType::Json,
            &norito::to_bytes(permission).unwrap(),
        ))
        .unwrap();
    vm.set_register(10, recipient);
    vm.set_register(11, permission);
    host.syscall(syscall, vm)
}

#[test]
fn exact_transfer_json_delegation_uses_effect_subject_and_can_be_revoked() {
    let (mut host, mut vm, contract, actor) = fixture();
    host.bind_contract_runtime_context(actor.clone(), contract.clone(), "manage".into())
        .unwrap();
    let definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "rose".parse().unwrap(),
    );
    let bucket = AssetId::with_scope(definition, contract.subject_id(), AssetBalanceScope::Global);
    let permission = Json::from_str_norito(&format!(
        r#"{{"name":"CanTransferAsset","payload":{{"asset":"{bucket}"}}}}"#,
    ))
    .unwrap();
    let exact = PermissionToken::TransferAssetBucket(bucket);
    transfer_permission_call(
        &mut host,
        &mut vm,
        &actor,
        &permission,
        syscalls::SYSCALL_GRANT_PERMISSION,
    )
    .unwrap();
    assert!(host.wsv.has_permission(&actor, &exact));
    assert!(matches!(
        transfer_permission_call(
            &mut host,
            &mut vm,
            &actor,
            &permission,
            syscalls::SYSCALL_GRANT_PERMISSION,
        ),
        Err(VMError::PermissionDenied)
    ));
    transfer_permission_call(
        &mut host,
        &mut vm,
        &actor,
        &permission,
        syscalls::SYSCALL_REVOKE_PERMISSION,
    )
    .unwrap();
    assert!(!host.wsv.has_permission(&actor, &exact));
    assert!(matches!(
        transfer_permission_call(
            &mut host,
            &mut vm,
            &actor,
            &permission,
            syscalls::SYSCALL_REVOKE_PERMISSION,
        ),
        Err(VMError::PermissionDenied)
    ));
}

#[test]
fn exact_transfer_json_delegation_rejects_foreign_source_and_broad_tokens() {
    let (mut host, mut vm, contract, actor) = fixture();
    host.bind_contract_runtime_context(actor.clone(), contract.clone(), "manage".into())
        .unwrap();
    host.wsv
        .grant_permission(&contract.subject_id(), PermissionToken::ManagePermissions);
    let definition = AssetDefinitionId::derive_from_components(
        DomainId::try_new("wonderland", "universal").unwrap(),
        "rose".parse().unwrap(),
    );
    let foreign = AssetId::with_scope(definition.clone(), actor.clone(), AssetBalanceScope::Global);
    for source in [
        format!(r#"{{"name":"CanTransferAsset","payload":{{"asset":"{foreign}"}}}}"#),
        format!(
            r#"{{"name":"CanTransferAssetWithDefinition","payload":{{"asset_definition":"{definition}"}}}}"#
        ),
    ] {
        let permission = Json::from_str_norito(&source).unwrap();
        for syscall in [
            syscalls::SYSCALL_GRANT_PERMISSION,
            syscalls::SYSCALL_REVOKE_PERMISSION,
        ] {
            assert!(matches!(
                transfer_permission_call(&mut host, &mut vm, &actor, &permission, syscall),
                Err(VMError::PermissionDenied)
            ));
        }
    }
    assert!(
        !host
            .wsv
            .has_permission(&actor, &PermissionToken::TransferAssetBucket(foreign))
    );
}

#[test]
fn exact_trigger_json_delegation_requires_owner_and_can_be_revoked() {
    for name in ["CanExecuteTrigger", "CanModifyTriggerMetadata"] {
        let (mut host, mut vm, contract, actor) = fixture();
        host.bind_contract_runtime_context(actor.clone(), contract.clone(), "manage".into())
            .unwrap();
        let trigger: Name = "owned_trigger".parse().unwrap();
        host.wsv.triggers.insert(trigger.to_string(), true);
        host.wsv
            .trigger_authorities
            .insert(trigger.to_string(), contract.subject_id());
        let permission = Json::from_str_norito(&format!(
            r#"{{"name":"{name}","payload":{{"trigger":"{trigger}"}}}}"#
        ))
        .unwrap();
        let exact = if name == "CanExecuteTrigger" {
            PermissionToken::ExecuteTrigger(trigger)
        } else {
            PermissionToken::ModifyTriggerMetadata(trigger)
        };
        transfer_permission_call(
            &mut host,
            &mut vm,
            &actor,
            &permission,
            syscalls::SYSCALL_GRANT_PERMISSION,
        )
        .unwrap();
        assert!(host.wsv.has_permission(&actor, &exact));
        transfer_permission_call(
            &mut host,
            &mut vm,
            &actor,
            &permission,
            syscalls::SYSCALL_REVOKE_PERMISSION,
        )
        .unwrap();
        assert!(!host.wsv.has_permission(&actor, &exact));
        host.wsv
            .trigger_authorities
            .insert("owned_trigger".into(), actor.clone());
        assert!(matches!(
            transfer_permission_call(
                &mut host,
                &mut vm,
                &actor,
                &permission,
                syscalls::SYSCALL_GRANT_PERMISSION
            ),
            Err(VMError::PermissionDenied)
        ));
    }
}

#[test]
fn exact_trigger_json_delegation_rejects_missing_malformed_and_broad_tokens() {
    let (mut host, mut vm, contract, actor) = fixture();
    host.bind_contract_runtime_context(actor.clone(), contract, "manage".into())
        .unwrap();
    for source in [
        r#"{"name":"CanExecuteTrigger","payload":{"trigger":"missing"}}"#,
        r#"{"name":"CanModifyTriggerMetadata","payload":{"trigger":"missing"}}"#,
        r#"{"name":"CanExecuteTrigger","payload":{}}"#,
        r#"{"name":"CanModifyTriggerMetadata","payload":{"trigger":12}}"#,
        r#"{"name":"CanModifyTrigger","payload":{"trigger":"missing"}}"#,
        r#"{"name":"CanRegisterTrigger","payload":{}}"#,
    ] {
        let permission = Json::from_str_norito(source).unwrap();
        for syscall in [
            syscalls::SYSCALL_GRANT_PERMISSION,
            syscalls::SYSCALL_REVOKE_PERMISSION,
        ] {
            assert!(matches!(
                transfer_permission_call(&mut host, &mut vm, &actor, &permission, syscall),
                Err(VMError::PermissionDenied | VMError::NoritoInvalid)
            ));
        }
    }
}
