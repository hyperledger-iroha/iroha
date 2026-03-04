use std::collections::HashMap;

use ivm::{
    IVM,
    kotodama::compiler::Compiler as KotodamaCompiler,
    mock_wsv::{MockWorldStateView, PermissionToken, WsvHost},
};

#[test]
fn kotodama_create_and_grant_role_enables_mint() {
    let src = r#"
        fn main() {
          // Bootstrap domain/account/asset
          register_domain(domain("wonderland"));
          register_account(account_id("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"));
          register_asset("rose", "ROSE", 0, 1);
          // Create role with mint permission and grant to authority
          create_role(name("minter"), json("{\"perms\":[\"mint_asset:rose#wonderland\"]}"));
          grant_role(account_id("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"), name("minter"));
          // Mint using role permission
          mint_asset(account_id("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"), asset_definition("rose#wonderland"), 1);
        }
    "#;
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(src).expect("compile");
    let caller: ivm::AccountId =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
            .parse()
            .unwrap();
    let mut wsv = MockWorldStateView::new();
    // Permissions to bootstrap objects
    wsv.grant_permission(&caller, PermissionToken::RegisterDomain);
    wsv.grant_permission(&caller, PermissionToken::RegisterAccount);
    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
    let host = WsvHost::new(wsv, caller, HashMap::new(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&prog).expect("load");
    vm.run()
        .expect("program should execute with role-created permissions");
}
