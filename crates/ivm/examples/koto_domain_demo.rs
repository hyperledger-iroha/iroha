//! Kotodama domain demo: register, transfer, and unregister a domain on a mock WSV.
use std::collections::HashMap;

use ivm::{
    AccountId, IVM, MockWorldStateView, PermissionToken, ScopedAccountId,
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
    let src = include_str!("../../kotodama_lang/src/samples/domain_ops.ko");
    let compiler = KotodamaCompiler::new();
    let bytecode = compiler.compile_source(src).expect("compile domain_ops");

    // 2) Prepare a small world and grant domain permissions to the caller
    let alice = fixture_account(
        "wonderland",
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03",
    );
    let mut wsv = MockWorldStateView::new();
    wsv.grant_permission(&alice, PermissionToken::RegisterDomain);
    let alice_subject = AccountId::from(&alice);

    // No account index map needed for this sample (we pass pointers via TLVs)
    let host = WsvHost::new_with_subject(wsv, alice_subject, HashMap::new());

    // 3) Create VM, attach host, load program
    let mut vm = IVM::new(1_000_000);
    vm.set_host(host);
    vm.load_program(&bytecode).expect("load program");

    // 4) Run program; host enforces pointer-ABI TLV validation and permissions
    vm.run().expect("run VM");
    println!("Domain operations executed as {alice}");
    Ok(())
}
