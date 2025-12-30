//! Kotodama structs: field access lowering via pointer-ABI to CoreHost.

use std::collections::HashMap;

use ivm::{
    IVM, KotodamaCompiler,
    mock_wsv::{AccountId, MockWorldStateView, WsvHost},
};

#[test]
fn struct_fields_lower_to_syscall_args() {
    if std::env::var("IROHA_RUN_IGNORED").ok().as_deref() != Some("1") {
        eprintln!(
            "Skipping: Kotodama struct field lowering test gated. Set IROHA_RUN_IGNORED=1 to run."
        );
        return;
    }
    // Define a struct with pointer-ABI fields and use it to call a builtin.
    let src = r#"
        seiyaku C {
            struct TransferArgs { domain: DomainId; to: AccountId; }
            fn main() {
                let args = TransferArgs(
                    domain("wonderland"),
                    account_id("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland")
                );
                transfer_domain(authority(), args.domain, args.to);
            }
        }
    "#;
    // Note: set IVM_COMPILER_DEBUG=1 in the environment to debug codegen.
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(src).expect("compile kotodama");
    let mut wsv = MockWorldStateView::new();
    // Ensure the target account exists in mock WSV to pass validation in host
    let bob: AccountId =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
            .parse()
            .unwrap();
    wsv.add_account_unchecked(bob.clone());
    let host = WsvHost::new(wsv, bob, HashMap::new(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&prog).expect("load");
    vm.run().expect("transfer_domain via struct fields");
}
