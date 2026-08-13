//! Kotodama domain demo: register, transfer, and unregister a domain on a mock WSV.
use std::collections::HashMap;
use ivm::{
    AccountId, IVM, MockWorldStateView, PermissionToken, ProgramMetadata,
    kotodama::compiler::Compiler as KotodamaCompiler, mock_wsv::WsvHost,
};
fn fixture_account(hex_public_key: &str) -> AccountId {
    AccountId::new(hex_public_key.parse().expect("public key"))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1) Compile the Kotodama sample to IVM bytecode
    let src = include_str!("../../kotodama_lang/src/samples/domain_ops.ko");
    let compiler = KotodamaCompiler::new();
    let bytecode = compiler.compile_source(src).expect("compile domain_ops");
    let metadata = ProgramMetadata::parse(&bytecode).expect("parse domain_ops metadata");
    let run = metadata
        .contract_interface
        .as_ref()
        .expect("domain_ops contract interface")
        .entrypoints
        .iter()
        .find(|entrypoint| entrypoint.name == "run")
        .expect("run entrypoint descriptor");
    let entrypoint_pc =
        u64::try_from(metadata.prefix_len()).expect("program prefix fits u64") + run.entry_pc;
    // 2) Prepare a small world and grant domain permissions to the caller
    let alice =
        fixture_account("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03");
    let mut wsv = MockWorldStateView::new();
    wsv.grant_permission(&alice, PermissionToken::RegisterDomain);
    // No account index map needed for this sample (we pass pointers via TLVs)
    let host = WsvHost::new_with_subject(wsv, alice.clone(), HashMap::new());
    // 3) Create VM, attach host, load program
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&bytecode).expect("load program");
    // 4) Select and run the public wrapper; the host enforces pointer-ABI validation.
    vm.set_program_counter(entrypoint_pc)
        .expect("select run entrypoint");
    vm.run().expect("run VM");
    println!("Domain operations executed as {alice}");
    Ok(())
}
