//! Smoke test: Kotodama role ops compile to correct pointer-ABI syscalls
//! and execute against the mock WSV host.

use std::collections::HashMap;

use ivm::{
    AssetDefinitionId, IVM, PermissionToken,
    kotodama::compiler::Compiler as KotodamaCompiler,
    mock_wsv::{AccountId, MockWorldStateView, WsvHost},
};

fn make_vm_with_wsv() -> (IVM, AccountId) {
    let _domain: ivm::mock_wsv::DomainId = "wonderland".parse().expect("domain id");
    let alice: AccountId = AccountId::new(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("public key"),
    );
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(alice.clone());
    let host = WsvHost::new_with_subject(wsv, alice.clone(), HashMap::new());
    let mut vm = IVM::new(1_000_000);
    vm.set_host(host);
    (vm, alice)
}

#[test]
fn kotodama_roles_roundtrip_on_wsvhost() {
    let (mut vm, alice) = make_vm_with_wsv();
    let compiler = KotodamaCompiler::new();

    // 1) Create role `minter` with permission mint_asset:rose#wonder
    let src_create = r#"
        fn main() {
          create_role(name("minter"), json("{\"perms\":[\"mint_asset:rose#wonder\"]}"));
        }
    "#;
    let prog = compiler
        .compile_source(src_create)
        .expect("compile create_role");
    vm.load_program(&prog).unwrap();
    vm.run().expect("create_role should succeed");
    // Inspect host state
    {
        let host_any = vm.host_mut_any().unwrap();
        let host = host_any
            .downcast_mut::<WsvHost>()
            .expect("downcast WsvHost");
        assert!(
            !host
                .wsv
                .create_role("minter", std::collections::HashSet::new()),
            "role should exist"
        );
    }

    // 2) Grant role to alice and check derived permission
    let src_grant = r#"
        fn main() {
          grant_role(account_id("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"), name("minter"));
        }
    "#;
    let prog = compiler
        .compile_source(src_grant)
        .expect("compile grant_role");
    vm.load_program(&prog).unwrap();
    vm.run().expect("grant_role should succeed");
    {
        let host_any = vm.host_mut_any().unwrap();
        let host = host_any
            .downcast_mut::<WsvHost>()
            .expect("downcast WsvHost");
        let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonder".parse().unwrap(),
            "rose".parse().unwrap(),
        );
        let tok = PermissionToken::MintAsset(asset);
        assert!(
            host.wsv.has_permission(&alice, &tok),
            "alice should inherit mint permission via role"
        );
    }

    // 3) Revoke role and delete; verify permissions removed and role absent
    let src_cleanup = r#"
        fn main() {
          revoke_role(account_id("sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB"), name("minter"));
          delete_role(name("minter"));
        }
    "#;
    let prog = compiler
        .compile_source(src_cleanup)
        .expect("compile revoke/delete");
    vm.load_program(&prog).unwrap();
    vm.run().expect("revoke+delete should succeed");
    {
        let host_any = vm.host_mut_any().unwrap();
        let host = host_any
            .downcast_mut::<WsvHost>()
            .expect("downcast WsvHost");
        let asset: AssetDefinitionId = iroha_data_model::asset::AssetDefinitionId::new(
            "wonder".parse().unwrap(),
            "rose".parse().unwrap(),
        );
        let tok = PermissionToken::MintAsset(asset);
        assert!(
            !host.wsv.has_permission(&alice, &tok),
            "permission should be removed after revoke"
        );
        // delete_role returns false if role still exists; we can probe by trying to delete again (should be false because already removed)
        assert!(!host.wsv.delete_role("minter"), "role should be deleted");
    }
}
