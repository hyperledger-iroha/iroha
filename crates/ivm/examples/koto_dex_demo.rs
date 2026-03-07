//! Kotodama DEX demo: compile and run a simple XYK pool on IVM.
use std::collections::HashMap;

use iroha_primitives::numeric::Numeric;
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
    let src = include_str!("../../kotodama_lang/src/samples/dex_simple.ko");
    let compiler = KotodamaCompiler::new();
    let bytecode = compiler.compile_source(src).expect("compile dex_simple");

    // 2) Prepare a tiny world with Alice (trader), Pool account, and two assets
    let alice = fixture_account(
        "wonderland",
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
    );
    let pool = fixture_account(
        "genesis",
        "ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4",
    );
    let asset_a: AssetDefinitionId = "usdc#wonderland".parse()?;
    let asset_b: AssetDefinitionId = "eth#wonderland".parse()?;

    // Initial balances: Alice has 1_000 USDC, pool has 10_000 USDC and 100 ETH
    let wsv = MockWorldStateView::with_balances(&[
        ((alice.clone(), asset_a.clone()), Numeric::from(1_000_u64)),
        ((pool.clone(), asset_a.clone()), Numeric::from(10_000_u64)),
        ((pool.clone(), asset_b.clone()), Numeric::from(100_u64)),
    ]);
    let mut wsv = wsv;
    // Grant permissions for caller (Alice) to transfer these assets
    wsv.grant_permission(&alice, PermissionToken::TransferAsset(asset_a.clone()));
    wsv.grant_permission(&alice, PermissionToken::TransferAsset(asset_b.clone()));
    wsv.grant_permission(
        &alice,
        PermissionToken::ReadAccountAssets(AccountId::from_account_id(&alice)),
    );
    wsv.grant_permission(
        &alice,
        PermissionToken::ReadAccountAssets(AccountId::from_account_id(&pool)),
    );
    let alice_subject = AccountId::from_account_id(&alice);

    // 3) Map small integers used by the program to account subjects in the host
    let mut account_map = HashMap::new();
    account_map.insert(1u64, alice_subject.clone());
    account_map.insert(2u64, AccountId::from_account_id(&pool));
    let mut asset_map = HashMap::new();
    asset_map.insert(1u64, asset_a.clone()); // input
    asset_map.insert(2u64, asset_b.clone()); // output

    let host = WsvHost::new_with_subject_map(wsv, alice_subject, account_map, asset_map);

    // 4) Create the VM, attach host, load program
    let mut vm = IVM::new(1_000_000);
    vm.set_host(host);
    vm.load_program(&bytecode).expect("load program");

    // 5) Set up arguments in r10.. (trader, pool, asset_in, asset_out, amount_in, reserve_in, reserve_out)
    // trader=1 (alice), pool=2, asset_in=1 (usdc), asset_out=2 (eth)
    vm.set_register(10, 1);
    vm.set_register(11, 2);
    vm.set_register(12, 1);
    vm.set_register(13, 2);
    let amount_in = 500u64; // Alice sells 500 USDC
    vm.set_register(14, amount_in);
    // current pool reserves (usdc, eth)
    vm.set_register(15, 10_000);
    vm.set_register(16, 100);

    // 6) Run the program and read return value from r10
    vm.run().expect("run VM");
    let amount_out = vm.register(10);

    // 7) Inspect balances (via host's WSV)
    // Pull the host back out to access WSV (unsafe: we clone the VM and recover host),
    // but for simplicity in this demo, re-create expected balances via calculation.
    println!("Swap result: sold {amount_in} USDC for {amount_out} ETH (quoted)");
    // Note: moving host out of vm is not exposed; for demo, we print expected keys and remind to check logs.
    println!("Accounts mapped: trader=1 -> {alice} ; pool=2 -> {pool}");
    println!("Assets mapped: 1 -> {asset_a} ; 2 -> {asset_b}");
    println!("Done.");
    Ok(())
}
