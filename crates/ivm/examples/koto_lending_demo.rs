//! Kotodama lending demo: a minimal borrow/mint flow on IVM.
use std::collections::HashMap;

use ivm::{
    AccountId, AssetDefinitionId, IVM, MockWorldStateView, PermissionToken, ScopedAccountId,
    kotodama::compiler::Compiler as KotodamaCompiler, mock_wsv::WsvHost,
};

fn fixture_account(domain: &str, hex_public_key: &str) -> ScopedAccountId {
    ScopedAccountId::new(
        domain.parse().expect("domain id"),
        hex_public_key.parse().expect("public key"),
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1) Compile the Kotodama sample to IVM bytecode
    let src = include_str!("../../kotodama_lang/src/samples/lending_simple.ko");
    let compiler = KotodamaCompiler::new();
    let bytecode = compiler
        .compile_source(src)
        .expect("compile lending_simple");

    // 2) Prepare a tiny world with a user, a vault account, and a debt asset
    let user = fixture_account(
        "wonderland",
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
    );
    let vault = fixture_account(
        "genesis",
        "ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4",
    );
    let debt_asset: AssetDefinitionId =
        iroha_data_model::asset::AssetDefinitionId::new("wonderland".parse()?, "stable".parse()?);

    // Initialize WSV: no balances yet; grant permissions so user can mint via host
    let mut wsv = MockWorldStateView::new();
    wsv.grant_permission(&user, PermissionToken::MintAsset(debt_asset.clone()));
    wsv.grant_permission(
        &user,
        PermissionToken::ReadAccountAssets(AccountId::from(&user)),
    );
    let user_subject = AccountId::from(&user);
    let vault_subject = AccountId::from(&vault);

    // 3) Map small integers to domainless account subjects used by syscalls
    let mut account_map = HashMap::new();
    account_map.insert(1u64, user_subject.clone());
    account_map.insert(2u64, vault_subject);
    let mut asset_map = HashMap::new();
    asset_map.insert(1u64, debt_asset.clone());

    let host = WsvHost::new_with_subject_map(wsv, user_subject, account_map, asset_map);

    // 4) Create VM, attach host, load program
    let mut vm = IVM::new(1_000_000);
    vm.set_host(host);
    vm.load_program(&bytecode).expect("load program");

    // 5) Execute borrow(user, vault, debt_asset, amount, collateral_value, current_debt_value, min_ratio_bps)
    // Pass args in r10..
    vm.set_register(10, 1); // user idx
    vm.set_register(11, 2); // vault idx
    vm.set_register(12, 1); // debt asset idx
    vm.set_register(13, 500); // amount to borrow
    vm.set_register(14, 10_000); // collateral value
    vm.set_register(15, 0); // current debt value
    vm.set_register(16, 1500); // min_ratio_bps = 150%

    vm.run().expect("run VM");
    println!("Borrow executed. User should have 500 of {debt_asset}.");
    println!("Done.");
    Ok(())
}
