use std::collections::HashMap;

use ivm::{
    IVM,
    kotodama::compiler::Compiler as KotodamaCompiler,
    mock_wsv::{MockWorldStateView, PermissionToken, WsvHost},
};

fn compile(src: &str) -> Vec<u8> {
    let c = KotodamaCompiler::new();
    c.compile_source(src).expect("compile")
}

fn caller_account() -> ivm::mock_wsv::ScopedAccountId {
    ivm::mock_wsv::ScopedAccountId::new(
        "wonderland".parse().expect("domain id"),
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("public key"),
    )
}

#[test]
fn kotodama_revoke_role_denies_mint() {
    let caller: ivm::mock_wsv::ScopedAccountId = caller_account();

    // VM + host with bootstrap permissions
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(caller.clone());
    wsv.grant_permission(&caller, PermissionToken::RegisterDomain);
    wsv.grant_permission(&caller, PermissionToken::RegisterAccount);
    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
    let host = WsvHost::new_with_subject(
        wsv,
        ivm::mock_wsv::AccountId::from(&caller.clone()),
        HashMap::new(),
    );
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    // 1) Bootstrap + create+grant role + initial mint (should succeed)
    let prog_ok = compile(
        r#"
        fn main() {
          register_domain(domain("default"));
          register_account(account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"));
          register_asset("rose", "ROSE", 0, 1);
          create_role(name("minter"), json("{\"perms\":[\"mint_asset:62Fk4FPcMuLvW5QjDGNF2a4jAmjM\"]}"));
          grant_role(account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"), name("minter"));
          mint_asset(account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"), asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), 1);
        }
    "#,
    );
    vm.load_program(&prog_ok).expect("load ok");
    vm.run().expect("initial mint should succeed");

    // 2) Revoke role then attempt mint (should fail with PermissionDenied)
    let prog_revoke_then_mint = compile(
        r#"
        fn main() {
          revoke_role(account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"), name("minter"));
          mint_asset(account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"), asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), 1);
        }
    "#,
    );
    vm.load_program(&prog_revoke_then_mint)
        .expect("load revoke");
    let err = vm.run().unwrap_err();
    assert!(matches!(err, ivm::VMError::PermissionDenied));
}

#[test]
fn kotodama_delete_role_prevents_grant() {
    let caller: ivm::mock_wsv::ScopedAccountId = caller_account();
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(caller.clone());
    wsv.grant_permission(&caller, PermissionToken::RegisterDomain);
    wsv.grant_permission(&caller, PermissionToken::RegisterAccount);
    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
    let host = WsvHost::new_with_subject(
        wsv,
        ivm::mock_wsv::AccountId::from(&caller.clone()),
        HashMap::new(),
    );
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    // Bootstrap + create role (no grant)
    let prog_boot = compile(
        r#"
        fn main() {
          register_domain(domain("default"));
          register_account(account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"));
          register_asset("rose", "ROSE", 0, 1);
          create_role(name("minter"), json("{\"perms\":[\"mint_asset:62Fk4FPcMuLvW5QjDGNF2a4jAmjM\"]}"));
        }
    "#,
    );
    vm.load_program(&prog_boot).expect("load boot");
    vm.run().expect("boot ok");

    // Delete role then try to grant it (should fail)
    let prog_delete_then_grant = compile(
        r#"
        fn main() {
          delete_role(name("minter"));
          grant_role(account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"), name("minter"));
        }
    "#,
    );
    vm.load_program(&prog_delete_then_grant).expect("load del");
    let err = vm.run().unwrap_err();
    assert!(matches!(err, ivm::VMError::PermissionDenied));
}

#[test]
fn kotodama_delete_role_denied_while_assigned_then_succeeds_after_revoke() {
    let caller: ivm::mock_wsv::ScopedAccountId = caller_account();
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(caller.clone());
    wsv.grant_permission(&caller, PermissionToken::RegisterDomain);
    wsv.grant_permission(&caller, PermissionToken::RegisterAccount);
    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
    let host = WsvHost::new_with_subject(
        wsv,
        ivm::mock_wsv::AccountId::from(&caller.clone()),
        HashMap::new(),
    );
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    // Bootstrap: create role and grant it
    let boot = compile(
        r#"
        fn main() {
          register_domain(domain("default"));
          register_account(account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"));
          register_asset("rose", "ROSE", 0, 1);
          create_role(name("minter"), json("{\"perms\":[\"mint_asset:62Fk4FPcMuLvW5QjDGNF2a4jAmjM\"]}"));
          grant_role(account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"), name("minter"));
        }
    "#,
    );
    vm.load_program(&boot).expect("load boot");
    vm.run().expect("boot ok");

    // Attempt to delete role while still assigned -> should be denied
    let del = compile(
        r#"
        fn main() { delete_role(name("minter")); }
    "#,
    );
    vm.load_program(&del).expect("load del while assigned");
    let err = vm.run().unwrap_err();
    assert!(matches!(err, ivm::VMError::PermissionDenied));

    // Revoke then delete -> should succeed
    let revoke_delete = compile(
        r#"
        fn main() {
          revoke_role(account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"), name("minter"));
          delete_role(name("minter"));
        }
    "#,
    );
    vm.load_program(&revoke_delete).expect("load revoke+delete");
    vm.run().expect("revoke then delete ok");
}

#[test]
fn kotodama_combined_revoke_then_delete_blocks_grant_and_mint() {
    let caller: ivm::mock_wsv::ScopedAccountId = caller_account();
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(caller.clone());
    wsv.grant_permission(&caller, PermissionToken::RegisterDomain);
    wsv.grant_permission(&caller, PermissionToken::RegisterAccount);
    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
    let host = WsvHost::new_with_subject(
        wsv,
        ivm::mock_wsv::AccountId::from(&caller.clone()),
        HashMap::new(),
    );
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);

    // Bootstrap + create and grant role
    let boot = compile(
        r#"
        fn main() {
          register_domain(domain("default"));
          register_account(account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"));
          register_asset("rose", "ROSE", 0, 1);
          create_role(name("minter"), json("{\"perms\":[\"mint_asset:62Fk4FPcMuLvW5QjDGNF2a4jAmjM\"]}"));
          grant_role(account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"), name("minter"));
        }
    "#,
    );
    vm.load_program(&boot).expect("load boot");
    vm.run().expect("boot ok");

    // Revoke then delete role
    let revoke_delete = compile(
        r#"
        fn main() {
          revoke_role(account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"), name("minter"));
          delete_role(name("minter"));
        }
    "#,
    );
    vm.load_program(&revoke_delete).expect("load revoke+delete");
    vm.run().expect("revoke+delete ok");

    // Attempt to grant role now fails (role no longer exists)
    let grant_again = compile(
        r#"
        fn main() {
          grant_role(account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"), name("minter"));
        }
    "#,
    );
    vm.load_program(&grant_again).expect("load grant again");
    let err = vm.run().unwrap_err();
    assert!(matches!(err, ivm::VMError::PermissionDenied));

    // Mint is denied without the role
    let mint = compile(
        r#"
        fn main() {
          mint_asset(account_id("6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn"), asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), 1);
        }
    "#,
    );
    vm.load_program(&mint).expect("load mint");
    let err2 = vm.run().unwrap_err();
    assert!(matches!(err2, ivm::VMError::PermissionDenied));
}
