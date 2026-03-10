use std::collections::HashMap;

use iroha_crypto::PublicKey;
use ivm::{
    IVM, Memory, PointerType, VMError,
    instruction::wide,
    mock_wsv::{DomainId, MockWorldStateView, PermissionToken, ScopedAccountId, WsvHost},
    syscalls,
};
mod common;

fn json_value<T: norito::json::JsonSerialize + ?Sized>(value: &T) -> norito::json::Value {
    norito::json::to_value(value).expect("serialize json value")
}

fn json_object<const N: usize>(
    pairs: [(&'static str, norito::json::Value); N],
) -> norito::json::Value {
    norito::json::object(pairs).expect("serialize json object")
}

fn json_array<const N: usize>(items: [norito::json::Value; N]) -> norito::json::Value {
    norito::json::array(items).expect("serialize json array")
}

fn make_tlv(type_id: u16, payload: &[u8]) -> Vec<u8> {
    let payload = PointerType::from_u16(type_id)
        .map(|pty| common::payload_for_type(pty, payload))
        .unwrap_or_else(|| payload.to_vec());
    let mut out = Vec::with_capacity(7 + payload.len() + 32);
    out.extend_from_slice(&type_id.to_be_bytes());
    out.push(1);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload.as_ref());
    let h: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    out.extend_from_slice(&h);
    out
}

fn account(domain: &str, public_key: &str) -> ScopedAccountId {
    let domain: DomainId = domain.parse().unwrap();
    let public_key: PublicKey = public_key.parse().unwrap();
    ScopedAccountId::new(domain, public_key)
}

fn canonical_account(account: ScopedAccountId) -> ScopedAccountId {
    let value = norito::json::to_value(&account).expect("serialize account");
    let literal = value.as_str().expect("account literal");
    ScopedAccountId::parse_encoded(literal)
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
        .expect("canonical account id must parse")
}

fn run_env_result(vm: &mut IVM, env: norito::json::Value) -> Result<(), VMError> {
    let body = norito::json::to_vec(&env).expect("env json");
    let tlv = make_tlv(PointerType::Json as u16, &body);
    vm.memory.preload_input(0, &tlv).expect("preload input");
    vm.set_register(10, Memory::INPUT_START);
    // Build a tiny program: SCALL SMARTCONTRACT_EXECUTE_INSTRUCTION; HALT
    let mut code = Vec::new();
    code.extend_from_slice(
        &ivm::encoding::wide::encode_sys(
            wide::system::SCALL,
            syscalls::SYSCALL_SMARTCONTRACT_EXECUTE_INSTRUCTION as u8,
        )
        .to_le_bytes(),
    );
    code.extend_from_slice(&ivm::encoding::wide::encode_halt().to_le_bytes());
    let mut prog = ivm::ProgramMetadata::default().encode();
    prog.extend_from_slice(&code);
    vm.load_program(&prog).unwrap();
    vm.run()
}

fn run_env(vm: &mut IVM, env: norito::json::Value) {
    run_env_result(vm, env).expect("exec envelope");
}

#[test]
fn envelope_roles_permissions_triggers() {
    // Setup WSV and host
    let alice = canonical_account(account(
        "wonderland",
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
    ));
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(alice.clone());
    wsv.grant_permission(&alice, PermissionToken::ManageRoles);
    wsv.grant_permission(&alice, PermissionToken::ManagePermissions);
    wsv.grant_permission(&alice, PermissionToken::ManageTriggers);
    let host = WsvHost::new_with_subject(
        wsv,
        ivm::mock_wsv::AccountId::from(&alice.clone()),
        HashMap::new(),
    );
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    // 1) Create role with mixed permission forms and grant to alice
    let create_role_env = json_object([
        ("type", json_value("wsv.create_role")),
        (
            "payload",
            json_object([
                ("name", json_value("issuer")),
                (
                    "perms",
                    json_array([
                        json_value("register_domain"),
                        json_object([
                            ("type", json_value("read_assets")),
                            ("target", json_value(&alice)),
                        ]),
                    ]),
                ),
            ]),
        ),
    ]);
    run_env(&mut vm, create_role_env);

    let grant_role_env = json_object([
        ("type", json_value("wsv.grant_role")),
        (
            "payload",
            json_object([
                ("account_id", json_value(&alice)),
                ("role", json_value("issuer")),
            ]),
        ),
    ]);
    run_env(&mut vm, grant_role_env);

    // Verify role-derived permissions present
    let host_any = vm.host_mut_any().unwrap();
    let host = host_any.downcast_ref::<WsvHost>().unwrap();
    assert!(
        host.wsv
            .has_permission(&alice, &PermissionToken::RegisterDomain)
    );
    assert!(host.wsv.has_permission(
        &alice,
        &PermissionToken::ReadAccountAssets(ivm::mock_wsv::AccountId::from(&alice,)),
    ));

    // 2) Grant + revoke a direct permission
    let mint_perm_env = json_object([
        ("type", json_value("wsv.grant_permission")),
        (
            "payload",
            json_object([
                ("account_id", json_value(&alice)),
                ("permission", json_value("mint_asset:rose#domain")),
            ]),
        ),
    ]);
    run_env(&mut vm, mint_perm_env);
    let host_any = vm.host_mut_any().unwrap();
    let host = host_any.downcast_ref::<WsvHost>().unwrap();
    assert!(host.wsv.has_permission(
        &alice,
        &PermissionToken::MintAsset("rose#domain".parse().unwrap())
    ));

    let revoke_perm_env = json_object([
        ("type", json_value("wsv.revoke_permission")),
        (
            "payload",
            json_object([
                ("account_id", json_value(&alice)),
                (
                    "permission",
                    json_object([
                        ("type", json_value("mint_asset")),
                        ("target", json_value("rose#domain")),
                    ]),
                ),
            ]),
        ),
    ]);
    run_env(&mut vm, revoke_perm_env);
    let host_any = vm.host_mut_any().unwrap();
    let host = host_any.downcast_ref::<WsvHost>().unwrap();
    assert!(!host.wsv.has_permission(
        &alice,
        &PermissionToken::MintAsset("rose#domain".parse().unwrap())
    ));

    // 3) Trigger lifecycle: create -> disable -> remove
    let trig_name = "my_trigger";
    let create_trig_env = json_object([
        ("type", json_value("wsv.create_trigger")),
        ("payload", json_object([("name", json_value(trig_name))])),
    ]);
    run_env(&mut vm, create_trig_env);
    let host_any = vm.host_mut_any().unwrap();
    let host = host_any.downcast_ref::<WsvHost>().unwrap();
    assert_eq!(host.wsv.trigger_state(trig_name), Some(true));

    let disable_trig_env = json_object([
        ("type", json_value("wsv.set_trigger_enabled")),
        (
            "payload",
            json_object([
                ("name", json_value(trig_name)),
                ("enabled", json_value(&false)),
            ]),
        ),
    ]);
    run_env(&mut vm, disable_trig_env);
    let host_any = vm.host_mut_any().unwrap();
    let host = host_any.downcast_ref::<WsvHost>().unwrap();
    assert_eq!(host.wsv.trigger_state(trig_name), Some(false));

    let remove_trig_env = json_object([
        ("type", json_value("wsv.remove_trigger")),
        ("payload", json_object([("name", json_value(trig_name))])),
    ]);
    run_env(&mut vm, remove_trig_env);
    let host_any = vm.host_mut_any().unwrap();
    let host = host_any.downcast_ref::<WsvHost>().unwrap();
    assert_eq!(host.wsv.trigger_state(trig_name), None);

    // 4) Revoke the role and verify permissions are removed
    let revoke_role_env = json_object([
        ("type", json_value("wsv.revoke_role")),
        (
            "payload",
            json_object([
                ("account_id", json_value(&alice)),
                ("role", json_value("issuer")),
            ]),
        ),
    ]);
    run_env(&mut vm, revoke_role_env);
    let host_any = vm.host_mut_any().unwrap();
    let host = host_any.downcast_ref::<WsvHost>().unwrap();
    assert!(
        !host
            .wsv
            .has_permission(&alice, &PermissionToken::RegisterDomain)
    );
}

#[test]
fn envelope_missing_payload_is_rejected() {
    let alice = canonical_account(account(
        "wonderland",
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
    ));
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(alice.clone());
    let host =
        WsvHost::new_with_subject(wsv, ivm::mock_wsv::AccountId::from(&alice), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    let invalid_env = json_object([("type", json_value("wsv.create_role"))]);
    let err = run_env_result(&mut vm, invalid_env).expect_err("missing payload must fail");
    assert!(matches!(err, VMError::NoritoInvalid));
}

#[test]
fn envelope_payload_must_be_object() {
    let alice = canonical_account(account(
        "wonderland",
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
    ));
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(alice.clone());
    let host =
        WsvHost::new_with_subject(wsv, ivm::mock_wsv::AccountId::from(&alice), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    let invalid_env = json_object([
        ("type", json_value("wsv.create_role")),
        ("payload", json_value("not-an-object")),
    ]);
    let err = run_env_result(&mut vm, invalid_env).expect_err("payload must be an object");
    assert!(matches!(err, VMError::NoritoInvalid));
}

#[test]
fn envelope_admin_alias_rejects_without_manage_permissions() {
    let alice = canonical_account(account(
        "wonderland",
        "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774",
    ));
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(alice.clone());
    let host = WsvHost::new_with_subject(
        wsv,
        ivm::mock_wsv::AccountId::from(&alice.clone()),
        HashMap::new(),
    );
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    let create_role_env = json_object([
        ("type", json_value("wsv.create_role")),
        (
            "payload",
            json_object([
                ("name", json_value("issuer")),
                ("perms", json_array([json_value("register_domain")])),
            ]),
        ),
    ]);
    let err =
        run_env_result(&mut vm, create_role_env).expect_err("admin alias must require permission");
    assert!(matches!(err, VMError::PermissionDenied));
}
