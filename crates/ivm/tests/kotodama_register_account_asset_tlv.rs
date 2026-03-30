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
          register_account(account_id("sorauロ1PzEcクZkfGハ1レ9ミツRユDAuXヒyヤヰヰ3VgAク4ヌケWL6iXCEYDCW"));
          register_asset("rose", "ROSE", 0, 1);
          unregister_asset(asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"));
          unregister_account(account_id("sorauロ1PzEcクZkfGハ1レ9ミツRユDAuXヒyヤヰヰ3VgAク4ヌケWL6iXCEYDCW"));
        }
    "#;
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(src).expect("compile");

    // Prepare WSV host with permissions for the caller
    let _domain: ivm::mock_wsv::DomainId = "wonderland".parse().expect("domain id");
    let caller: ivm::mock_wsv::AccountId = ivm::mock_wsv::AccountId::new(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03"
            .parse()
            .expect("public key"),
    );
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(caller.clone());
    wsv.grant_permission(&caller, PermissionToken::RegisterDomain);
    wsv.grant_permission(&caller, PermissionToken::RegisterAccount);
    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);

    let account_map: HashMap<u64, ivm::mock_wsv::AccountId> = HashMap::new();
    let asset_map: HashMap<u64, ivm::AssetDefinitionId> = HashMap::new();
    let host = WsvHost::new_with_subject_map(wsv, caller.clone(), account_map, asset_map);

    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&prog).expect("load");
    vm.run()
        .expect("program should run with WsvHost TLV validation");
}
