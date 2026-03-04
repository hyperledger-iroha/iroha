//! Kotodama domain demo: register, transfer, and unregister a domain on a mock WSV.
use std::collections::HashMap;

use ivm::{
    AccountId, IVM, MockWorldStateView, PermissionToken,
    kotodama::compiler::Compiler as KotodamaCompiler, mock_wsv::WsvHost,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1) Compile the Kotodama sample to IVM bytecode
    let src = include_str!("../../kotodama_lang/src/samples/domain_ops.ko");
    let compiler = KotodamaCompiler::new();
    let bytecode = compiler.compile_source(src).expect("compile domain_ops");

    // 2) Prepare a small world and grant domain permissions to the caller
    let alice: AccountId = "alice@wonderland".parse()?;
    let mut wsv = MockWorldStateView::new();
    wsv.grant_permission(&alice, PermissionToken::RegisterDomain);

    // No index maps needed for this sample (we pass pointers via TLVs)
    let account_map: HashMap<u64, AccountId> = HashMap::new();
    let asset_map = HashMap::new();
    let host = WsvHost::new(wsv, alice.clone(), account_map, asset_map);

    // 3) Create VM, attach host, load program
    let mut vm = IVM::new(1_000_000);
    vm.set_host(host);
    vm.load_program(&bytecode).expect("load program");

    // 4) Run program; host enforces pointer-ABI TLV validation and permissions
    vm.run().expect("run VM");
    println!("Domain operations executed as {alice}");
    Ok(())
}
