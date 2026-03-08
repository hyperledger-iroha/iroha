use std::collections::HashMap;

use ivm::{
    IVM, MockWorldStateView, PermissionToken, kotodama::compiler::Compiler as KotodamaCompiler,
    mock_wsv::WsvHost,
};

#[test]
fn kotodama_register_account_and_unregister_asset() {
    // Program: register domain, then register an account, then register asset and unregister it
    let src = r#"
        fn main() {
          register_domain(domain("default"));
          register_account(account_id("6cmzPVPX8F5t35VB7wQQ68PAW8Wb1iAEr4PZHPLTQ3p69JAGG9oifzi"));
          register_asset("rose", "ROSE", 0, 1);
          unregister_asset(asset_definition("rose#wonderland"));
          unregister_account(account_id("6cmzPVPX8F5t35VB7wQQ68PAW8Wb1iAEr4PZHPLTQ3p69JAGG9oifzi"));
        }
    "#;
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(src).expect("compile");

    // Prepare WSV host with permissions for the caller
    let caller: ivm::mock_wsv::ScopedAccountId = ivm::mock_wsv::ScopedAccountId::new(
        "wonderland".parse().expect("domain id"),
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("public key"),
    );
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(caller.clone());
    wsv.grant_permission(&caller, PermissionToken::RegisterDomain);
    wsv.grant_permission(&caller, PermissionToken::RegisterAccount);
    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);

    let account_map: HashMap<u64, ivm::mock_wsv::AccountSubjectId> = HashMap::new();
    let asset_map: HashMap<u64, ivm::AssetDefinitionId> = HashMap::new();
    let host = WsvHost::new_with_subject_map(
        wsv,
        ivm::mock_wsv::AccountSubjectId::from(&caller),
        account_map,
        asset_map,
    );

    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&prog).expect("load");
    vm.run()
        .expect("program should run with WsvHost TLV validation");
}
