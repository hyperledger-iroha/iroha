//! Kotodama domain demo: register, transfer, and unregister a domain on a mock WSV.
use ivm::{AccountId, IVM, MockWorldStateView, PermissionToken, mock_wsv::WsvHost};
use kotodama_lang::compiler::{Compiler, CompilerOptions};
fn fixture_account(hex_public_key: &str) -> AccountId {
    AccountId::new(hex_public_key.parse().expect("public key"))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1) Compile the Kotodama sample to IVM bytecode
    let src = include_str!("../../kotodama_lang/src/samples/domain_ops.ko");
    let options = CompilerOptions::default();
    let profile = options.target_profile()?;
    let compiler = Compiler::new_with_options(options);
    let bytecode = compiler.compile_source(src).expect("compile domain_ops");
    let contract = ivm::prepare_contract_for_profile(bytecode.into(), profile)?;
    // 2) Prepare a small world and grant domain permissions to the caller
    let alice =
        fixture_account("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03");
    let mut wsv = MockWorldStateView::new();
    wsv.grant_permission(&alice, PermissionToken::RegisterDomain);
    // No account index map needed for this sample (we pass pointers via TLVs)
    let host = WsvHost::new_with_subject(wsv, alice.clone());
    // 3) Create VM, attach host, load program
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_prepared(&contract)?;
    // 4) Select and run the public wrapper; the host enforces pointer-ABI validation.
    vm.select_entrypoint("run")?;
    vm.run().expect("run VM");
    println!("Domain operations executed as {alice}");
    Ok(())
}
