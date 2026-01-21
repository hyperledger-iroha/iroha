//! Kotodama lending demo: a minimal borrow/mint flow on IVM.
use std::collections::HashMap;

use ivm::{
    AccountId, AssetDefinitionId, IVM, MockWorldStateView, PermissionToken,
    kotodama::compiler::Compiler as KotodamaCompiler, mock_wsv::WsvHost,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1) Compile the Kotodama sample to IVM bytecode
    let src = include_str!("../../kotodama_lang/src/samples/lending_simple.ko");
    let compiler = KotodamaCompiler::new();
    let bytecode = compiler
        .compile_source(src)
        .expect("compile lending_simple");

    // 2) Prepare a tiny world with a user, a vault account, and a debt asset
    let user: AccountId =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
            .parse()?;
    let vault: AccountId =
        "ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4@genesis".parse()?;
    let debt_asset: AssetDefinitionId = "stable#wonderland".parse()?;

    // Initialize WSV: no balances yet; grant permissions so user can mint via host
    let mut wsv = MockWorldStateView::new();
    wsv.grant_permission(&user, PermissionToken::MintAsset(debt_asset.clone()));
    wsv.grant_permission(&user, PermissionToken::ReadAccountAssets(user.clone()));

    // 3) Map small integers to IDs used by syscalls
    let mut account_map = HashMap::new();
    account_map.insert(1u64, user.clone());
    account_map.insert(2u64, vault.clone());
    let mut asset_map = HashMap::new();
    asset_map.insert(1u64, debt_asset.clone());

    let host = WsvHost::new(wsv, user.clone(), account_map, asset_map);

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
